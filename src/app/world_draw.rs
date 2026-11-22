//! `world_draw` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    /// Compose one gameplay framebuffer: world, overlays, interface and cursor.
    ///
    /// The order follows the original draw tree:
    ///
    /// 1. base terrain plus GRAPH placements, then the ambient/`LIGHTS.N`
    ///    background pass (palette versions are per element in the original);
    /// 2. the `FUN_0042532C` selection rings and ground-object pass (unshaded
    ///    rings; markers ambient-shaded unless the torch is out), both before the
    ///    darken and under the actors;
    /// 3. ground shadows (`FUN_0043C040`/`FUN_0043D247`) while the `shadows`
    ///    config word is set - the original halves the terrain/rings/markers
    ///    pixels before the depth-sorted sprites are drawn;
    /// 4. the dynamic depth pass of props, NPCs, companions and the protagonist,
    ///    sorted by projected anchor, each blitted with its own palette version
    ///    (unshaded on lit high-map cells);
    /// 5. the projectile pool (base palette in the original);
    /// 6. the HUD, then the interface modes (map, quest log, exchange, character
    ///    screen, dialogue), the hover text and finally the game-drawn cursor.
    pub(super) fn draw_detail(&mut self) {
        self.texts.clear();
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        for px in self.frame.iter_mut() {
            *px = 0;
        }
        fill_rect(
            &mut self.frame,
            fw,
            fh,
            0,
            0,
            VIRTUAL_W,
            VIRTUAL_H,
            [7, 10, 9, 255],
        );

        let ox = self.detail_pan[0];
        let oy = self.detail_pan[1];
        let iox = ox.round() as i32;
        let ioy = oy.round() as i32;

        self.ensure_population(self.current_map);

        // Static background: base terrain (both byte layers) + GRAPH placements.
        self.world
            .draw_background(&mut self.frame, fw, fh, self.current_map, iox, ioy);

        // Ambient + `LIGHTS.N` terrain pass. In the original every element picks
        // its palette version when it is drawn, so the background is shaded
        // here, the rings/markers decide individually, and sprites are shaded at
        // blit time from `sprite_ambient`.
        self.apply_background_lighting(iox, ioy);

        // `FUN_0042532C`: the selection ellipses are drawn right after the
        // terrain and before the ground markers and the actors, with the base
        // (unshaded) palette.
        self.draw_selection_rings(iox, ioy);

        // `FUN_0042532C` ground-object pass: before the shadow darken and under
        // the actors; ambient palette unless the torch is out.
        self.draw_world_markers(iox, ioy);

        // Dynamic depth pass data: props + NPC/mob actors + protagonist, ordered
        // by projected anchor. The exact original depth key is unresolved (§25);
        // this is a documented provisional anchor sort (never raw Entity x/y).
        let mut sprites: Vec<WorldSprite> = self.world.collect_props(self.current_map);
        self.collect_actor_sprites(&mut sprites);
        // Recruited companions render in the same dynamic depth pass and follow
        // the leader (`update_party`).
        self.collect_party_sprites(&mut sprites);

        // Protagonist through the canonical Entity visual path (animated). The
        // sprite is drawn from the pre-step (render) cell; the per-frame anchor
        // sweeps it onto the committed (`player_x/player_y`) cell over the cycle.
        let moving = self.player_active;
        let (plx, ply) = entity_to_local_px_f(self.player_render_x, self.player_render_y);
        let (pose_action, pose_frame, pose_battle) = self.player_pose(moving);
        match self.player_frame(moving) {
            Ok((im, gx, gy)) => sprites.push(WorldSprite {
                img: im,
                gx,
                gy,
                lx: plx,
                ly: ply,
                shadow: Some(crate::render::ShadowKey {
                    equipment: self.player.equipment,
                    weapon_slot: self.player.raw_0x12,
                    battle: pose_battle,
                    archetype: self.player.entity_class(),
                    visual_index: self.player.hero.visual_index,
                    action: pose_action,
                    facing: self.player_dir.max(0) as u8,
                    frame: pose_frame,
                }),
            }),
            Err(_) => {}
        }

        // `FUN_00426384`: shadow masks for the current location's actors,
        // equipment overlays and props, then `FUN_0043D247` halves those pixels.
        // The original runs this after the terrain/rings/markers and **before**
        // the depth-sorted props/actors, so the ground darkens but the actors do
        // not. Gated by the `shadows` config word (`0x81D620`).
        if crate::config::config().shadows != 0 {
            // Screen-space clip: `origin` below already includes the camera pan.
            let clip = (0, 0, fw as i32 - 1, fh as i32 - 1);
            let loc = self.current_map;
            let mut spans: Vec<crate::render::spans::Span> = Vec::new();
            {
                let PlayableApp { world, frame, .. } = self;
                for s in sprites.iter() {
                    let Some(key) = s.shadow else {
                        continue;
                    };
                    world.res.add_actor_shadow(
                        &key.equipment,
                        key.weapon_slot,
                        key.battle,
                        key.archetype,
                        key.visual_index,
                        key.action,
                        key.facing,
                        key.frame,
                        (iox + s.lx.round() as i32, ioy + s.ly.round() as i32),
                        clip,
                        &mut spans,
                    );
                }
                // `FUN_00426384` prop loop: every prop on the map adds its
                // frame's ground mask (burning props with >= 2 flame parts
                // skip it). Same run budget as the actor masks.
                if let Some(loc_data) = world.loc(loc) {
                    for p in &loc_data.props {
                        let (lx, ly) = crate::loc::prop_to_local_px(p.grid_x, p.grid_y);
                        world.res.add_prop_shadow(
                            p.object_id,
                            p.frame as usize,
                            (iox + lx, ioy + ly),
                            clip,
                            &mut spans,
                        );
                    }
                }
                crate::render::spans::darken(frame, fw as usize, fh as usize, &spans);
            }
            self.shadow_spans = spans.len();
            self.shadow_debug = if spans.len() <= 64 {
                spans.clone()
            } else {
                Vec::new()
            };
        } else {
            self.shadow_spans = 0;
            self.shadow_debug.clear();
        }

        sprites.sort_by(|a, b| {
            a.ly.partial_cmp(&b.ly)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.lx.partial_cmp(&b.lx).unwrap_or(std::cmp::Ordering::Equal))
        });
        for s in &sprites {
            let ax = iox + s.lx.round() as i32 + s.gx;
            let ay = ioy + s.ly.round() as i32 + s.gy;
            // The original chooses the palette version per sprite: the ambient
            // delta (`FUN_0043D971`) unless the sprite's cell is a lit
            // `LIGHTS.N` high-map cell, which uses the unshaded palette.
            let delta = self.sprite_ambient(s.lx, s.ly);
            if delta.is_zero() {
                blit(
                    &mut self.frame,
                    fw,
                    fh,
                    &s.img.data,
                    s.img.w,
                    s.img.h,
                    ax,
                    ay,
                );
            } else {
                blit_shaded(
                    &mut self.frame,
                    fw,
                    fh,
                    &s.img.data,
                    s.img.w,
                    s.img.h,
                    ax,
                    ay,
                    delta,
                );
            }
        }

        // Ranged projectile pool (`0x80B0D0`) is drawn after the entity/prop
        // depth pass (original sort key `y + 0x5E`). The original passes the
        // base palette (`FUN_0043CEAA(..., DAT_005076f4, ...)`), so arrows are
        // not ambient-shaded.
        {
            let PlayableApp {
                frame,
                world,
                projectiles,
                world_frame,
                ..
            } = self;
            let parity = *world_frame as usize & 1;
            for p in projectiles.iter() {
                let Some(img) = world.res.intr_sprite(p.sprite_base + parity) else {
                    continue;
                };
                let ax = iox + p.x.round() as i32 + p.draw_dx as i32;
                let ay = ioy + p.y.round() as i32 + p.draw_dy as i32;
                blit(frame, fw, fh, &img.data, img.w, img.h, ax, ay);
            }
        }

        // Original INTR HUD: whole-screen overlay, empty inventory slot strip and
        // (once bound) party portraits. Composited after the world and its
        // overlays; transparent regions keep the world visible. Debug overlays
        // below draw on top. Split borrows keep `frame` and `world.res` disjoint.
        let owner = self.inventory_owner;
        let owner_inventory = self.owner_inventory(owner);
        let inventory_scroll = self.inventory_scroll;
        let held_id = self.held_item.map(|h| h.id);
        let cursor_px = (
            self.cursor_virtual.0.round() as i32,
            self.cursor_virtual.1.round() as i32,
        );
        let hud_press = self.hud_press.map(|(code, _)| code);
        let inventory_visible = self.inventory_strip_visible;
        {
            let PlayableApp {
                frame,
                world,
                player,
                party_selected,
                party,
                ..
            } = self;
            // Protagonist (party slot 0) uses the original selector with the
            // actual character fields; recruited companions (slots 1..) use the
            // non-main-hero portrait branch with their `Entity` fields
            // (`FUN_0042C730`). The marker and HP bar are composed per member.
            let mut portraits: Vec<crate::hud::PartyPortrait> = Vec::with_capacity(1 + party.len());
            portraits.push(crate::hud::PartyPortrait {
                cell: 0,
                intr_id: crate::hud::portrait_id(
                    true,
                    player.portrait_group(),
                    player.portrait_status(),
                    player.hp_internal,
                ),
                selected: party_selected[0],
                health: player.hp_internal,
                poisoned: player.periodic_damage != 0,
            });
            for (i, m) in party.iter().enumerate() {
                let cell = i + 1;
                if cell >= crate::hud::PARTY_CELLS {
                    break;
                }
                portraits.push(crate::hud::PartyPortrait {
                    cell,
                    intr_id: crate::hud::portrait_id(
                        false,
                        m.entity.visual_index,
                        m.entity.archetype,
                        m.entity.stats.hp_internal,
                    ),
                    selected: party_selected[cell],
                    health: m.entity.stats.hp_internal,
                    poisoned: m.entity.stats.periodic_damage != 0,
                });
            }
            crate::hud::draw_game_hud(
                frame,
                fw,
                fh,
                &mut world.res,
                &owner_inventory,
                inventory_visible,
                inventory_scroll,
                held_id,
                cursor_px,
                &portraits,
                player.raw_0x12,
                crate::anim::battle_from_flags(player.flags_b),
                hud_press,
            );
        }

        // World-map (interface mode 5) and quest-log (mode 2) cover the
        // viewport; the left party panel and bottom bar stay visible, exactly
        // like the original `FUN_0042CBD0` cases 5/2.
        if self.map_open {
            self.draw_map_screen();
        } else if self.quest_log_open {
            self.draw_quest_log_screen();
        }

        // Exchange (interface mode 7, `FUN_00430D6C`). Drawn over the HUD
        // after the world, with the original stone fill/window/rows/buttons.
        if self.trade.is_some() {
            let trade_name = if self.ground_pile.is_some() {
                String::new()
            } else {
                self.trade
                    .as_ref()
                    .and_then(|t| self.tables.as_ref()?.entities.get(t.npc_entity))
                    .and_then(|e| self.world.res.character_name(e.name_id))
                    .unwrap_or_else(|| "????????".to_string())
            };
            let message = self.trade_message.clone();
            let cursor = (
                self.cursor_virtual.0.round() as i32,
                self.cursor_virtual.1.round() as i32,
            );
            let PlayableApp {
                frame,
                world,
                trade,
                ..
            } = self;
            if let Some(state) = trade.as_ref() {
                crate::trade_screen::draw(
                    frame,
                    fw,
                    fh,
                    &mut world.res,
                    state,
                    &trade_name,
                    message.as_deref(),
                    cursor,
                );
            }
        }

        // Original character screen (INTR[2]) over the world area; its window
        // geometry keeps the party panel and bottom inventory strip visible.
        if self.character_screen {
            let owner = self.inventory_owner;
            let doll_pose = match owner {
                crate::inventory::InventoryOwner::Protagonist => {
                    let doll_battle = crate::anim::battle_from_flags(self.player.flags_b);
                    let doll_action = if moving {
                        crate::anim::walk_action(doll_battle)
                    } else {
                        crate::anim::idle_action(doll_battle)
                    };
                    crate::character_screen::PaperDollPose {
                        action: doll_action,
                        direction: self.player_dir.max(0) as u8,
                        frame: self.player_frame_index(moving, doll_action),
                    }
                }
                crate::inventory::InventoryOwner::Party(i) => self
                    .party
                    .get(i)
                    .map(|m| crate::character_screen::PaperDollPose {
                        action: m.motion.action as usize,
                        direction: m.motion.facing,
                        frame: m.motion.frame as usize,
                    })
                    .unwrap_or_default(),
            };
            let (visual_index, palette_id) = match owner {
                crate::inventory::InventoryOwner::Protagonist => {
                    (self.player.hero.visual_index, self.player.hero.palette_id())
                }
                crate::inventory::InventoryOwner::Party(i) => self
                    .party
                    .get(i)
                    .map(|m| {
                        (
                            m.entity.visual_index,
                            (m.entity.palette_offset / 512) as i32,
                        )
                    })
                    .unwrap_or((0, 0)),
            };
            let sheet = crate::character_screen::CharacterSheet {
                name_id: match owner {
                    crate::inventory::InventoryOwner::Protagonist => self.player.hero.name_id,
                    crate::inventory::InventoryOwner::Party(i) => {
                        self.party.get(i).map(|m| m.entity.name_id).unwrap_or(0)
                    }
                },
                class: self.owner_class(owner),
                equipment: self.owner_equipment(owner),
                additional: self.slots(owner).map(|s| *s.additional).unwrap_or([0; 5]),
                special: self.slots(owner).map(|s| s.special).unwrap_or(0),
                inventory: self.owner_inventory(owner),
                raw_0x12: self.owner_raw_0x12(owner),
                flags_b: self.owner_flags_b(owner),
                visual_index,
                palette_id,
                snapshot: self.snapshot(owner).unwrap_or_default(),
                pose: doll_pose,
            };
            let held = self.held_item.map(|h| h.id);
            let PlayableApp { frame, world, .. } = self;
            crate::character_screen::draw(frame, fw, fh, &mut world.res, &sheet, held, cursor_px);
            if let Some(id) = self.character_hover_item(cursor_px) {
                let PlayableApp { frame, world, .. } = self;
                crate::app::tooltip::draw_item_tooltip(
                    frame,
                    fw,
                    fh,
                    &world.res,
                    id,
                    cursor_px,
                    (0, 0, fw as i32, fh as i32),
                );
            }
        }

        // NPC dialogue (original `FUN_0042CBD0` mode 1): INTR[1] window + INTR[0]
        // band, NPC statement and selectable options.
        {
            let PlayableApp {
                frame,
                world,
                dialog,
                dialog_background_tiled,
                ..
            } = self;
            if let Some(state) = dialog.as_ref() {
                crate::dialog::draw(
                    frame,
                    fw,
                    fh,
                    &mut world.res,
                    state,
                    *dialog_background_tiled,
                );
            }
        }

        // Original lower-panel hover text (`FUN_00430958` actor panel,
        // `FUN_0042F5F8` item detail, `FUN_0042E9CC` fixed strings). Drawn after
        // the interface overlays and before the game cursor, left aligned at
        // viewport.left+10 / viewport.bottom+5 with 15 px wrapped lines.
        // The original never blanks the lower panel: when nothing is hovered it
        // keeps the last description (a fresh game starts with the quest-log
        // hint). The character sheet leaves the HUD text visible.
        let panel_text = self.hover_panel_text().or_else(|| {
            if self.dialog.is_none()
                && !self.map_open
                && !self.quest_log_open
                && self.trade.is_none()
            {
                Some(crate::app::hover::DEFAULT_PANEL_TEXT.to_string())
            } else {
                None
            }
        });
        if let Some(text) = panel_text {
            let PlayableApp { frame, world, .. } = self;
            crate::hud::draw_hover_text(frame, fw, fh, VIRTUAL_W, &world.res, &text);
        }

        // Original game-drawn mouse cursor (`FUN_0042A470` -> `FUN_0043D2FE`): the
        // Win32 cursor is hidden and one of the 11 GRAPH cursor frames is blitted
        // at the pointer (`_DAT_0081D5C0/_D5C4`). Drawn last so it sits over the
        // world, HUD and dialogue.
        let cursor = self.cursor_index();
        if let Some(img) = self.world.res.cursor_sprite(cursor) {
            let (cx, cy) = self.cursor_virtual;
            if cx >= 0.0 && cy >= 0.0 && cx < VIRTUAL_W as f64 && cy < VIRTUAL_H as f64 {
                blit(
                    &mut self.frame,
                    fw,
                    fh,
                    &img.data,
                    img.w,
                    img.h,
                    cx.round() as i32,
                    cy.round() as i32,
                );
            }
        }

        if let Some(msg) = self.pickup_message.clone() {
            if self.pickup_message_t > 0.0 {
                self.text_wrapped(12, 10, 0xFF_E0A0, 1.0, msg);
            }
        }
    }

    /// Selection ellipses of the selected party units (`FUN_0042532C`).
    ///
    /// The original loops the selection array `0x81A814` (up to 10, `0`
    /// terminated) and draws one 56x32 INTR ellipse per unit at
    /// `(viewport + Entity+0xE8 - cam_x - 0x1D, viewport + Entity+0xF0 - cam_y -
    /// 0x10)`, with the base (unshaded) palette. The sprite is chosen by current
    /// health (`Entity+0x5C`):
    ///
    /// ```text
    /// hp < 401   -> INTR[172] (blue)
    /// hp < 801   -> INTR[126] (cyan)
    /// otherwise  -> INTR[178] (green)
    /// ```
    ///
    /// While the unit is in a walk row (`1`/`7`) the ellipse is nudged by the
    /// current frame byte against the facing (the original 8-case switch), so it
    /// stays under the feet during the step bob.
    pub fn selection_rings(&self) -> Vec<(usize, i32, i32)> {
        let iox = self.detail_pan[0].round() as i32;
        let ioy = self.detail_pan[1].round() as i32;
        let mut out = Vec::new();
        let mut push = |hp: u16, lx: f64, ly: f64, action: u8, frame: u8, facing: u8| {
            let sprite = selection_ring_sprite(hp);
            let (x, y) = selection_ring_position(iox, ioy, lx, ly, action, frame, facing);
            out.push((sprite, x, y));
        };
        if self.party_selected.first().copied().unwrap_or(false) {
            let (action, frame, _) = self.player_pose(self.player_active);
            let (lx, ly) = entity_to_local_px_f(self.player_render_x, self.player_render_y);
            push(
                self.player.hp_internal,
                lx,
                ly,
                action as u8,
                frame as u8,
                self.player_dir.max(0) as u8,
            );
        }
        for (i, m) in self.party.iter().enumerate() {
            if !self.party_selected.get(i + 1).copied().unwrap_or(false) {
                continue;
            }
            let (lx, ly) = entity_to_local_px_f(
                m.motion.render_cell.0 as f64,
                m.motion.render_cell.1 as f64,
            );
            push(
                m.entity.stats.hp_internal,
                lx,
                ly,
                m.motion.action,
                m.motion.frame,
                m.motion.facing,
            );
        }
        out
    }

    /// Draw the selection ellipses (base palette, never ambient-shaded).
    fn draw_selection_rings(&mut self, _iox: i32, _ioy: i32) {
        let rings = self.selection_rings();
        if rings.is_empty() {
            return;
        }
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        for (sprite, x, y) in rings {
            let Some(img) = self.world.res.intr_sprite(sprite) else {
                continue;
            };
            blit(&mut self.frame, fw, fh, &img.data, img.w, img.h, x, y);
        }
    }

    /// `FUN_0042532C` ground WorldObject pass.
    ///
    /// Drawn after the selection rings, **before** the shadow darken and under
    /// the depth-sorted actors, in x-then-y scan order; the sprite anchor is the
    /// Entity projection (`FUN_00438A78`) minus half the sprite size. With the
    /// torch out (`_DAT_0081D628`) the original blends the 64x43 `LIGHTS.RES`
    /// mask and uses the base palette; otherwise the markers use the ambient
    /// palette version (`FUN_0043D971`).
    fn draw_world_markers(&mut self, iox: i32, ioy: i32) {
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        let torch = self.torch_reveal;
        let ambient = if torch {
            crate::lighting::Ambient::default()
        } else {
            self.ambient_delta()
        };
        let torch_mask = if torch {
            self.world
                .res
                .lights
                .as_ref()
                .map(|l| (l.large.w, l.large.h, l.large.data.clone()))
        } else {
            None
        };
        for obj in self.active_world_objects.iter() {
            // `FUN_0042532C`: `-1 < (i16)marker[+0x10]` — buried records
            // (negative money) are part of the active pool but invisible until
            // the copper mirror flips their money.
            if obj.money < 0 {
                continue;
            }
            let Some(img) = self.world.res.intr_sprite(obj.sprite as usize) else {
                continue;
            };
            let (lx, ly) = entity_to_local_px(obj.x, obj.y);
            if let Some((mw, mh, mask)) = torch_mask.as_ref() {
                let hx = iox + lx - 0x20 + img.w as i32 / 2;
                let hy = ioy + ly - 0x15 + img.h as i32 / 2;
                blend_light_mask(&mut self.frame, fw, fh, *mw, *mh, mask, hx, hy);
            }
            let ax = iox + lx - img.w as i32 / 2;
            let ay = ioy + ly - img.h as i32 / 2;
            if ambient.is_zero() {
                blit(&mut self.frame, fw, fh, &img.data, img.w, img.h, ax, ay);
            } else {
                blit_shaded(
                    &mut self.frame,
                    fw,
                    fh,
                    &img.data,
                    img.w,
                    img.h,
                    ax,
                    ay,
                    ambient,
                );
            }
        }
    }
}

/// Selection-ellipse INTR index for a unit's current internal health.
///
/// `FUN_0042532C` tests `Entity+0x5C` against `0x191` (401) and `0x321` (801);
/// the shipped ellipses are 56x32 in both INTR archives and use palette 0.
pub fn selection_ring_sprite(hp: u16) -> usize {
    if hp < 0x191 {
        172
    } else if hp < 0x321 {
        126
    } else {
        178
    }
}

/// Screen top-left of one selection ellipse.
///
/// `(iox, ioy)` is the world pan (`viewport - camera`) and `(lx, ly)` the
/// unit's stored projection (`Entity+0xE8/+0xF0` = the drawn render cell). The
/// walk rows (`1` battle / `7` peace) nudge the ellipse by the current frame
/// byte, one of the eight original facing cases; all arithmetic is the
/// original's integer (`f / 2 + f * 2`, `f / 2 + f`).
pub fn selection_ring_position(
    iox: i32,
    ioy: i32,
    lx: f64,
    ly: f64,
    action: u8,
    frame: u8,
    facing: u8,
) -> (i32, i32) {
    let mut x = iox + lx.round() as i32 - 0x1D;
    let mut y = ioy + ly.round() as i32 - 0x10;
    let row = action as usize;
    if row == crate::anim::ACT_BATTLE_WALK || row == crate::anim::ACT_PEACE_WALK {
        let f = frame as i32;
        let side = f / 2 + f * 2;
        let diag = f / 2 + f;
        match facing & 7 {
            0 => x -= f + f * 4,
            1 => {
                x -= side;
                y -= diag;
            }
            2 => y -= side,
            3 => {
                x += side;
                y -= diag;
            }
            4 => x += f + f * 4,
            5 => {
                x += side;
                y += diag;
            }
            6 => y += side,
            _ => {
                x -= side;
                y += diag;
            }
        }
    }
    (x, y)
}


/// Blend the 64x43 `LIGHTS.RES` mask over `frame` at `(ax, ay)` (torch
/// highlight, `_DAT_0081D628`). The mask is an 8-bit intensity; the lighting
/// blend expects the 0..31 range used by the `LIGHTS.N` maps.
fn blend_light_mask(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    mw: usize,
    mh: usize,
    mask: &[u8],
    ax: i32,
    ay: i32,
) {
    for y in 0..mh {
        let py = ay + y as i32;
        if py < 0 || py >= fh as i32 {
            continue;
        }
        for x in 0..mw {
            let px = ax + x as i32;
            if px < 0 || px >= fw as i32 {
                continue;
            }
            let v = mask[y * mw + x];
            if v == 0 {
                continue;
            }
            let a = ((v as u32 * 31) / 255) as u8;
            let o = (py as usize * fw + px as usize) * 4;
            let dst = [frame[o], frame[o + 1], frame[o + 2], frame[o + 3]];
            let lit = crate::lighting::torch_pixel(dst, a);
            frame[o..o + 4].copy_from_slice(&lit);
        }
    }
}
