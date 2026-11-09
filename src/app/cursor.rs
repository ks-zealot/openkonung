//! `cursor` subsystem of [`crate::app`]: the original game-drawn mouse cursor.
//!
//! The original hides the Win32 cursor (`ShowCursor(0)`; WndProc `SetCursor(0)`)
//! and each frame blits one of 11 GRAPH cursor bitmaps at the pointer
//! (`FUN_0042A470` -> `FUN_0043D2FE`). The frame is chosen by `FUN_00428740`
//! (hover override) over the base id `_DAT_0081D660`, which `FUN_00424E0C` sets
//! from the item class currently in hand. This module ports that selection; the
//! bitmaps are decoded by [`crate::res::cursor`].
//!
//! Frames (`GRAPH.RES` blocks 0..=10):
//! `0` chest/armor, `1` attack, `2` bow, `3` "no", `4` neutral, `5` talk,
//! `6` archer-ready prop, `7` ground-object hand, `8` potion/herb,
//! `9` creature/generic, `10` sword.

use super::*;
use crate::coords::local_px_to_entity;

/// Base cursor id for an item/action class byte, porting the `FUN_00424E0C`
/// jump table (`0x424E78`, cases `0..=0xC`, default `9`):
///
/// | class | 0,2 | 1,12 | 3,4,5 | 6,7,8,10,11,>12 | 9 |
/// |-------|-----|------|-------|-----------------|---|
/// | id    | 10  | 2    | 0     | 9               | 8 |
///
/// (`10`=sword, `2`=bow, `0`=chest/object, `8`=green/location, `9`=creature.)
pub fn cursor_for_class(class: u8) -> usize {
    match class {
        0 | 2 => 10,
        1 | 12 => 2,
        3 | 4 | 5 => 0,
        9 => 8,
        _ => 9,
    }
}

/// The actor under the pointer (`_DAT_0081D578`), reduced to the fields the
/// original cursor selector reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CursorActor {
    /// `Entity+0x05` — owning group id. The protagonist and recruited
    /// companions live in the reserved party group `0`.
    pub group_id: u8,
    /// `Entity+0xD0` — `0xFF` = no dialogue.
    pub dialogue_id: u8,
}

/// Pointer context assembled by [`PlayableApp::cursor_index`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CursorHover {
    /// Interactive prop under the pointer (`_DAT_0081D57C`, `+0x22 & 1`).
    pub prop: bool,
    pub actor: Option<CursorActor>,
    /// Runtime collision flag `0x2000` (ground WorldObject / portal prop).
    pub flag_2000: bool,
    /// Cell occupied (static low 12 bits or a live entity).
    pub occupied: bool,
}

/// Unit/party state read by the original selector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CursorState {
    /// `_DAT_0081D5E4` (Ctrl).
    pub ctrl: bool,
    /// `_DAT_0081D624` (Shift).
    pub shift: bool,
    /// Any **selected** living unit in battle stance (`Entity+0x03 & 4`).
    pub party_battle: bool,
    /// Leader in battle stance (`_DAT_0081D56C + 0x03 & 4`).
    pub leader_battle: bool,
    /// Current location owns a settlement (`_DAT_0081D580 != 0`).
    pub settlement: bool,
    /// A selected living unit is ranged-ready (`0x81A814` loop).
    pub ranged_ready: bool,
}

/// Pure port of `FUN_00428740` (`0x428740`). Returns the GRAPH cursor frame.
///
/// The original returns the base id immediately when it is not `4`, so holding
/// an item disables every hover override (including over menus).
pub(super) fn select_cursor(base: usize, hover: &CursorHover, state: &CursorState) -> usize {
    if base != 4 {
        return base;
    }
    // Archer-ready prop (`0x4287e3..0x4288c4`): only inside a settlement and
    // only for a selected unit whose bow, plain ammo and battle stance are set.
    if hover.prop && state.settlement && state.ranged_ready {
        return 6;
    }
    if let Some(actor) = hover.actor {
        // Same group as the leader (party): Ctrl forces the talk frame.
        if actor.group_id == 0 {
            return if state.ctrl { 5 } else { 4 };
        }
        // Any selected unit in battle stance -> attack frame.
        if state.party_battle {
            return 1;
        }
        // Peace stance: non-talkable, Shift (force attack) or a leader already
        // in battle stance suppress the talk frame.
        if actor.dialogue_id == 0xFF || state.shift || state.leader_battle {
            return 3;
        }
        return 5;
    }
    // A non-interactive prop never reaches here (`_81D57C` is only set for
    // interactive props); an interactive prop without a ready archer blocks the
    // ground branch and returns the base id.
    if hover.prop {
        return 4;
    }
    // Ground WorldObject hand (`0x428a2b..0x428a85`): `0x2000` + free cell.
    if hover.flag_2000 && !hover.occupied {
        return 7;
    }
    if hover.occupied {
        return 3;
    }
    4
}

impl PlayableApp {
    /// Base cursor id `_DAT_0081D660`. The original sets it from the class of
    /// the item currently in hand (`FUN_00424E0C` called by `FUN_0041E450` /
    /// `FUN_0041E6BC` / `FUN_0041E8A8`); it is reset to `4` when the item is
    /// placed, dropped or cancelled (`FUN_00428E98`).
    fn cursor_base(&self) -> usize {
        if let Some(held) = self.held_item {
            if let Some(class) = self.world.res.item_class(held.id) {
                return cursor_for_class(class);
            }
        }
        4
    }

    /// The original cursor frame index (`0..11`) for the current pointer.
    ///
    /// Mirrors `FUN_00428740`: base id first, then the gameplay/viewport gate,
    /// then interactive prop -> actor -> ground precedence.
    pub(super) fn cursor_index(&mut self) -> usize {
        // `_DAT_0081D660 = 7`: the shovel arms the ground/dig cursor for one
        // world click (`FUN_00414DE4` case 0x18; `FUN_0042260C` resets it).
        if self.dig_armed {
            return 7;
        }
        let base = self.cursor_base();
        if base != 4 {
            return base;
        }
        if self.mode_graph || crate::video::is_pending() {
            return base;
        }
        // Modal screens (`_DAT_0081D610 != 0`) keep the base cursor.
        if self.dialog.is_some()
            || self.character_screen
            || self.map_open
            || self.quest_log_open
            || self.trade.is_some()
        {
            return base;
        }
        let (cx, cy) = self.cursor_virtual;
        if cx < 0.0 || cy < 0.0 || cx >= VIRTUAL_W as f64 || cy >= VIRTUAL_H as f64 {
            return base;
        }
        let vx = cx.round() as i32;
        let vy = cy.round() as i32;
        let layout = crate::hud::layout(VIRTUAL_W);
        // `_81D738/_81D744/_81D754/_81D760` — the world viewport only.
        if !layout.viewport.contains(vx, vy) {
            return base;
        }
        // `_81D600` / `_81D750`: the inventory strip row is interface, not
        // world. The remake always shows a member's strip, so this applies
        // whenever the pointer is over the slot row.
        if layout.inventory_region.contains(vx, vy) {
            return base;
        }

        let lx = vx - self.detail_pan[0].round() as i32;
        let ly = vy - self.detail_pan[1].round() as i32;
        let (tx, ty) = local_px_to_entity(lx, ly);

        let cell = self
            .world
            .loc(self.current_map)
            .map(|l| l.collision_cell(tx, ty))
            .unwrap_or(0);
        let marker = self
            .active_world_objects
            .iter()
            .any(|o| o.cell() == (tx, ty) && o.money >= 0);
        let hover = CursorHover {
            prop: self
                .world
                .interactive_prop_at_local(self.current_map, lx, ly)
                .is_some(),
            actor: self.cursor_actor_at(vx, vy),
            flag_2000: cell & 0x2000 != 0 || marker,
            occupied: cell & 0x0FFF != 0 || self.live_actor_cell(tx, ty),
        };
        let state = CursorState {
            ctrl: self.ctrl_down,
            shift: self.shift_down,
            party_battle: self.selected_party_in_battle(),
            leader_battle: crate::anim::battle_from_flags(self.player.flags_b),
            settlement: self
                .settlements
                .iter()
                .any(|s| s.loc_id as u32 == self.current_map),
            ranged_ready: self.cursor_ranged_ready(),
        };
        select_cursor(base, &hover, &state)
    }

    /// `_DAT_0081D578` for the selector: non-player actors plus the protagonist
    /// (the leader is an ordinary rendered Entity in the original). Clicks use
    /// [`Self::actor_at`] instead so the leader keeps its move/selection path.
    fn cursor_actor_at(&mut self, vx: i32, vy: i32) -> Option<CursorActor> {
        let mut hit = self
            .actor_at(vx, vy)
            .map(|(_, _, dialogue_id, group_id)| CursorActor {
                group_id,
                dialogue_id,
            });
        if self.leader_sprite_covers(vx, vy) {
            hit = Some(CursorActor {
                group_id: 0,
                dialogue_id: self.player.dialogue_id,
            });
        }
        hit
    }

    /// Pixel-accurate leader sprite hit test using the same frame as the draw
    /// pass (`player_render_x/y`, `player_frame`).
    fn leader_sprite_covers(&mut self, mx: i32, my: i32) -> bool {
        let iox = self.detail_pan[0].round() as i32;
        let ioy = self.detail_pan[1].round() as i32;
        let moving = self.player_active;
        let (plx, ply) = entity_to_local_px_f(self.player_render_x, self.player_render_y);
        match self.player_frame(moving) {
            Ok((im, gx, gy)) => {
                let x = iox + plx.round() as i32 + gx;
                let y = ioy + ply.round() as i32 + gy;
                super::population::sprite_opaque_at(&im, x, y, mx, my)
            }
            Err(_) => false,
        }
    }

    /// True when a live visible actor (including the leader) occupies `(tx, ty)`
    /// on the runtime grid. The map loader ORs the entity index into the
    /// collision cell (`FUN_0043ADF0` `0x43B…`) for every entity with
    /// `archetype & 0x80 == 0` and action row not 3/9, so the original reports
    /// these cells as blocked even when the pointer misses the sprite.
    pub(super) fn live_actor_cell(&self, tx: i32, ty: i32) -> bool {
        if let Some(pop) = self.population_cache.get(&self.current_map) {
            for a in &pop.actors {
                let (cell, dead, action) = pop
                    .motion
                    .get(&a.source_entity_id())
                    .map(|m| (m.cell, m.dead, m.action))
                    .unwrap_or(((a.entity.x as i32, a.entity.y as i32), false, 0));
                if dead || a.entity.archetype & 0x80 != 0 || action == 3 || action == 9 {
                    continue;
                }
                if cell == (tx, ty) {
                    return true;
                }
            }
        }
        for m in &self.party {
            if m.motion.dead
                || m.entity.archetype & 0x80 != 0
                || m.motion.action == 3
                || m.motion.action == 9
            {
                continue;
            }
            if m.motion.cell == (tx, ty) {
                return true;
            }
        }
        !self.player.dead
            && (self.player_x.round() as i32, self.player_y.round() as i32) == (tx, ty)
    }

    /// `0x428807..0x4288c4` loop: any **selected**, living unit in battle
    /// stance (`Entity+0x03 & 0x04`). Dead rows 3/9 and the graphics-rebuild
    /// bit `0x80` are skipped.
    pub(super) fn selected_party_in_battle(&self) -> bool {
        if self.party_selected.first().copied().unwrap_or(false)
            && unit_can_act(
                self.player.entity_class(),
                self.player.dead,
                0,
            )
            && self.player.flags_b & crate::anim::FLAG_BATTLE != 0
        {
            return true;
        }
        for (i, m) in self.party.iter().enumerate() {
            if !self.party_selected.get(i + 1).copied().unwrap_or(false) {
                continue;
            }
            if unit_can_act(m.entity.archetype, m.motion.dead, m.motion.action)
                && m.entity.flags_b & crate::anim::FLAG_BATTLE != 0
            {
                return true;
            }
        }
        false
    }

    /// Archer-ready predicate used only by the prop cursor frame `6`
    /// (`0x428878..0x4288bd`): battle stance, active weapon slot `1`, a bow in
    /// `Equipment[1]`, ammo in `+0xCC`, and a plain ammo instance (`+0x02 == 0`).
    /// Deliberately looser than the firing gate (no quantity/class/range test).
    pub(super) fn cursor_ranged_ready(&self) -> bool {
        if self.party_selected.first().copied().unwrap_or(false)
            && unit_can_act(
                self.player.entity_class(),
                self.player.dead,
                0,
            )
            && self.player.flags_b & crate::anim::FLAG_BATTLE != 0
            && self.player.raw_0x12 == 1
            && self.player.equipment[1] != 0
            && self.player.special_equipment != 0
            && self
                .world
                .res
                .item_instance(self.player.special_equipment)
                .map(|i| i.modifier_index == 0)
                .unwrap_or(false)
        {
            return true;
        }
        for (i, m) in self.party.iter().enumerate() {
            if !self.party_selected.get(i + 1).copied().unwrap_or(false) {
                continue;
            }
            if unit_can_act(m.entity.archetype, m.motion.dead, m.motion.action)
                && m.entity.flags_b & crate::anim::FLAG_BATTLE != 0
                && m.entity.raw_0x12 == 1
                && m.entity.equipment[1] != 0
                && m.entity.special_equipment != 0
                && self
                    .world
                    .res
                    .item_instance(m.entity.special_equipment)
                    .map(|i| i.modifier_index == 0)
                    .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Diagnostic wrapper for the selected cursor frame (smoke tooling).
    pub fn debug_cursor_frame(&mut self) -> usize {
        self.cursor_index()
    }

    /// Diagnostic: virtual screen point of runtime cell `(x, y)` under the
    /// current interface pan (`entity_to_local_px` + `detail_pan`).
    pub fn debug_cell_screen_point(&self, x: i32, y: i32) -> (i32, i32) {
        let (lx, ly) = entity_to_local_px_f(x as f64, y as f64);
        (
            (self.detail_pan[0] + lx).round() as i32,
            (self.detail_pan[1] + ly).round() as i32,
        )
    }

    /// Drawn AUTO (`INTR[169/170]`) rectangle at the current resolution.
    pub(super) fn auto_rect(&self) -> (i32, i32, i32, i32) {
        let l = crate::hud::layout(VIRTUAL_W);
        let on = self.player.auto_enabled();
        let (w, h) = self
            .world
            .res
            .intr
            .as_ref()
            .and_then(|a| a.dimensions(if on { crate::intr::AUTO_ON } else { crate::intr::AUTO_OFF }))
            .unwrap_or((60, 24));
        (l.auto_pos.0, l.auto_pos.1, w as i32, h as i32)
    }
}

/// Original liveness test used by the selected-unit loops: not dead/corpse
/// (`archetype & 0x80`, action rows `9`/`3`).
fn unit_can_act(archetype: u8, dead: bool, action: u8) -> bool {
    !dead && archetype & 0x80 == 0 && action != 3 && action != 9
}
