//! `world_draw` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
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
        self.layer_counts =
            self.world
                .draw_background(&mut self.frame, fw, fh, self.current_map, iox, ioy);

        // Dynamic depth pass: props + NPC/mob actors + protagonist, ordered by
        // projected anchor. The exact original depth key is unresolved (§25);
        // this is a documented provisional anchor sort (never raw Entity x/y).
        let mut sprites: Vec<WorldSprite> = self.world.collect_props(self.current_map);
        self.props_drawn = sprites.len();
        let mut rejected = self.collect_actor_sprites(&mut sprites);
        // Recruited companions render in the same dynamic depth pass and follow
        // the leader (`update_party`).
        rejected.extend(self.collect_party_sprites(&mut sprites));

        // Protagonist through the canonical Entity visual path (animated). The
        // sprite is drawn from the pre-step (render) cell; the per-frame anchor
        // sweeps it onto the committed (`player_x/player_y`) cell over the cycle.
        let moving = self.player_active;
        let (plx, ply) = entity_to_local_px_f(self.player_render_x, self.player_render_y);
        match self.player_frame(moving) {
            Ok((im, gx, gy)) => sprites.push(WorldSprite {
                img: im,
                gx,
                gy,
                lx: plx,
                ly: ply,
            }),
            Err(reason) => rejected.push(format!(
                "player hero={} visual_index={} palette_offset=0x{:X} facing={} reason={}",
                self.player.hero.name,
                self.player.hero.visual_index,
                self.player.hero.palette_offset,
                self.player_dir,
                reason
            )),
        }
        self.population_rejected = rejected;
        self.actors_drawn = sprites.len().saturating_sub(self.props_drawn);

        sprites.sort_by(|a, b| {
            a.ly.partial_cmp(&b.ly)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.lx.partial_cmp(&b.lx).unwrap_or(std::cmp::Ordering::Equal))
        });
        for s in &sprites {
            let ax = iox + s.lx.round() as i32 + s.gx;
            let ay = ioy + s.ly.round() as i32 + s.gy;
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
        }

        // Ranged projectile pool (`0x80B0D0`) is drawn after the entity/prop
        // depth pass (original sort key `y + 0x5E`).
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

        // Day/night ambient + LIGHTS maps are applied to the finished world
        // (terrain/entities/props/projectiles) before the interface overlays.
        self.apply_world_lighting(&sprites, iox, ioy);

        // Live portals: drawn at the CANONICAL source trigger (center of the
        // transition trigger rectangle) through the normal Entity/runtime-grid
        // render transform, so the visible ring is exactly the cell gameplay
        // tests. The associated 244/245 gate center is debug-only (T overlay).
        for b in self.outgoing(self.current_map) {
            let (lx, ly) = entity_to_local_px_f(b.trigger[0], b.trigger[1]);
            let sx = iox + lx.round() as i32;
            let sy = ioy + ly.round() as i32;
            if -30 <= sx && sx <= VIRTUAL_W + 30 && -30 <= sy && sy <= VIRTUAL_H + 30 {
                fill_ellipse(
                    &mut self.frame,
                    fw,
                    fh,
                    sx as f32,
                    sy as f32,
                    18.0,
                    7.0,
                    [58, 61, 58, 255],
                );
                ellipse_outline(
                    &mut self.frame,
                    fw,
                    fh,
                    sx as f32,
                    sy as f32,
                    18.0,
                    7.0,
                    2,
                    [150, 155, 145, 255],
                );
                ellipse_outline(
                    &mut self.frame,
                    fw,
                    fh,
                    sx as f32,
                    sy as f32,
                    12.0,
                    4.0,
                    1,
                    [94, 99, 92, 255],
                );
                let to = self.edges[b.edge].to;
                let dest = self.label(to);
                self.text_clamped(sx + 20, sy - 8, 0xAF_F0C3, 1.0, format!("->{}", dest));
            }
        }

        if let Some((tx, ty)) = self.target {
            let (x, y) = entity_to_local_px_f(tx, ty);
            let sx = iox + x.round() as i32;
            let sy = ioy + y.round() as i32;
            ellipse_outline(
                &mut self.frame,
                fw,
                fh,
                sx as f32,
                sy as f32,
                6.0,
                6.0,
                1,
                [255, 220, 80, 255],
            );
        }

        // protagonist position marker (overlay; sprite already drawn in depth pass)
        let pax = iox + plx.round() as i32;
        let pay = ioy + ply.round() as i32;
        ellipse_outline(
            &mut self.frame,
            fw,
            fh,
            pax as f32,
            pay as f32,
            3.0,
            3.0,
            1,
            [255, 245, 120, 255],
        );

        // Game.N WorldObject pass (`FUN_0042532C` marker pass). The original
        // draws this after terrain/entities/party rings and outside the depth
        // sort, in x-then-y scan order; sprite anchor is the Entity projection
        // (`FUN_00438A78`) minus half the sprite size.
        {
            let PlayableApp {
                frame,
                world,
                active_world_objects,
                torch_reveal,
                ..
            } = self;
            // `_DAT_0081D628` (torch): the original also draws the 64x43
            // LIGHTS.RES mask behind each nearby ground marker and blits the
            // marker with its bright palette. We blend the mask and keep the
            // marker's unshaded colors (markers are already drawn after the
            // lighting pass).
            let torch_mask = if *torch_reveal {
                world
                    .res
                    .lights
                    .as_ref()
                    .map(|l| (l.large.w, l.large.h, l.large.data.clone()))
            } else {
                None
            };
            for obj in active_world_objects.iter() {
                // `FUN_0042532C`: `-1 < (i16)marker[+0x10]` — buried records
                // (negative money) are part of the active pool but invisible
                // until the copper mirror flips their money.
                if obj.money < 0 {
                    continue;
                }
                let Some(img) = world.res.intr_sprite(obj.sprite as usize) else {
                    continue;
                };
                let (lx, ly) = entity_to_local_px(obj.x, obj.y);
                if let Some((mw, mh, mask)) = torch_mask.as_ref() {
                    let hx = iox + lx - 0x20 + img.w as i32 / 2;
                    let hy = ioy + ly - 0x15 + img.h as i32 / 2;
                    blend_light_mask(frame, fw, fh, *mw, *mh, mask, hx, hy);
                }
                let ax = iox + lx - img.w as i32 / 2;
                let ay = ioy + ly - img.h as i32 / 2;
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
        {
            let PlayableApp {
                frame,
                world,
                player,
                hovered_slot,
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
                inventory_scroll,
                *hovered_slot,
                held_id,
                cursor_px,
                &portraits,
                player.auto_enabled(),
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
            let trade_name = self
                .trade
                .as_ref()
                .and_then(|t| self.tables.as_ref()?.entities.get(t.npc_entity))
                .and_then(|e| self.world.res.character_name(e.name_id))
                .unwrap_or_else(|| "????????".to_string());
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
                ..
            } = self;
            if let Some(state) = dialog.as_ref() {
                crate::dialog::draw(frame, fw, fh, &mut world.res, state);
            }
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
                self.text_wrapped(12, 34, 0xFF_E0A0, 1.0, msg);
            }
        }

        let portals = self.outgoing(self.current_map).len();
        let cur = self.label(self.current_map);
        let hdr = format!(
            "OpenKonung v15 | {} | hero:{} Game.{} | 800x600 native camera | pal:{} | portals:{} | V follow | C char | H graph | D ids | P pop | L layers | T portals",
            cur, self.player.hero.name, self.profile, self.player.palette_id(), portals
        );
        let hdr = if self.teleport_debug {
            format!(
                "{} | cell=({},{})",
                hdr,
                self.player_x.round() as i32,
                self.player_y.round() as i32
            )
        } else {
            hdr
        };
        let mut hy = self.text_wrapped(12, 10, 0xEB_F5EB, 1.0, hdr);
        hy = self.text_wrapped(
            12,
            hy + 2,
            0xC8_D8C8,
            1.0,
            "SPACE стойка | A атака | F стоп | 1/2/3 оружие | M карта | Q журнал | C/I герой | V камера | Home центр"
                .to_string(),
        );
        if let Some((a, b, r)) = self.last_teleport {
            let an = self.label(a);
            let bn = self.label(b);
            hy = self.text_wrapped(
                12,
                hy,
                0xBE_DCC3,
                1.0,
                format!("last teleport {}->{} r{}", an, bn, r),
            );
        }
        if let Some(err) = self.teleport_error.clone() {
            hy = self.text_wrapped(12, hy, 0xFF_6B6B, 1.0, err);
        }

        if self.teleport_debug {
            self.draw_portal_debug(hy + 4);
        }
        if self.population_debug {
            self.draw_population_debug();
        }
        if self.layer_debug {
            self.draw_layer_debug();
        }
    }
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
            let lit = crate::lighting::light_pixel(dst, a);
            frame[o..o + 4].copy_from_slice(&lit);
        }
    }
}
