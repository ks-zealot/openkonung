//! World-map screen (interface mode 5): open/close, click hit-test and travel.
//!
//! Faithful to `FUN_0042CBD0(4)` + `FUN_00428390` + `FUN_004381C8` mode 5. A
//! click resolves the run value (0-based), then `FUN_0042260C` code 3 requires
//! and consumes one class-11 subtype-20 «Шар магического прыжка» from the
//! leader's inventory (`FUN_00433A8C(0x15)` / `FUN_00421AD0`) and matches the
//! transition `dst (+0x04)` against `run + 1` (the compiler's `>>8`).
//! Rendering lives in [`crate::world_map`]; this module owns the state and the
//! travel side effect.

use super::*;

/// Class-11 subtype-20 «Шар магического прыжка»: the world-map travel cost
/// (`FUN_00433A8C(0x15)` finds it in the leader's inventory, `FUN_00421AD0`
/// removes the slot).
const TRAVEL_ORB: (u8, u8) = (0x0B, 0x14);

impl PlayableApp {
    /// Toggle interface mode 5 (`FUN_0042CBD0(4)`, or the close branch).
    pub fn toggle_map(&mut self) {
        if self.dialog.is_some() || self.trade.is_some() || self.character_screen {
            return;
        }
        if self.map_open {
            self.map_open = false;
            self.map_overlay = None;
            self.queue_sound(4);
        } else {
            self.quest_log_open = false;
            self.map_open = true;
            self.map_overlay = None;
            self.hovered_slot = None;
            self.queue_sound(4);
        }
    }

    /// Close the map without a sound (used by travel/ESC paths).
    pub fn close_map(&mut self) {
        if self.map_open {
            self.map_open = false;
            self.map_overlay = None;
            self.hovered_slot = None;
        }
    }

    /// Handle a left click while the map is open. Returns `true` when the click
    /// was inside the map hit rectangle (including empty/undiscovered cells),
    /// so the caller does not issue a world move.
    pub fn map_click(&mut self, vx: i32, vy: i32) -> bool {
        let l = crate::hud::layout(VIRTUAL_W);
        let (dx, dy) = crate::world_map::overlay_offset(VIRTUAL_W);
        let mx = vx - l.viewport.x - dx;
        let my = vy - l.viewport.y - dy;
        if mx <= 0
            || mx >= crate::world_map::MAP_W as i32
            || my <= 0
            || my >= crate::world_map::MAP_H as i32
        {
            return false;
        }
        let Some(loc) = crate::world_map::location_at(mx, my) else {
            return true;
        };
        if (loc as usize) >= self.discovered_locations.len()
            || self.discovered_locations[loc as usize] == 0
        {
            // `FUN_004381C8` returns -1 for an undiscovered location.
            return true;
        }
        // The original aborts when the clicked run is the current location
        // (`_DAT_0081D6C4 - 1 == run`).
        if loc as u32 == self.current_map.saturating_sub(1) {
            return true;
        }
        // `FUN_00433A8C(0x15)`: the leader's **inventory** must hold a class-11
        // subtype-20 «Шар магического прыжка»; without one the click does
        // nothing at all.
        let Some(orb) = self
            .player
            .inventory
            .iter()
            .copied()
            .find(|&id| id != 0 && self.world.res.item_class_subtype(id) == Some(TRAVEL_ORB))
        else {
            return true;
        };
        // `FUN_00421AD0`: consume the orb *before* the record scan, so an
        // unmatched click still spends it (original behaviour).
        crate::inventory::remove_inventory_id(&mut self.player.inventory, orb);
        // `FUN_0042260C` shifts the compiler's `run*0x100|0x103` by 8, i.e. it
        // compares the transition `dst (+0x04)` with **`run + 1`** (the 1-based
        // location id). The run table itself is 0-based.
        let edge = self.edges.iter().find(|e| e.to == loc as u32 + 1).cloned();
        if let Some(e) = edge {
            let [ax, ay] = e.arrival();
            let from = self.current_map;
            self.close_map();
            self.travel_to_map(from, e.to, ax, ay, (e.meta & 7) as i32, e.rec);
        }
        true
    }

    /// Composite the map over the already-drawn HUD (the original leaves the
    /// left party panel and bottom bar visible).
    pub(super) fn draw_map_screen(&mut self) {
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        let PlayableApp {
            frame,
            world,
            discovered_locations,
            map_overlay,
            ..
        } = self;
        crate::world_map::draw(
            frame,
            fw,
            fh,
            &mut world.res,
            VIRTUAL_W,
            discovered_locations,
            map_overlay,
        );
    }
}
