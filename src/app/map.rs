//! World-map screen (interface mode 5): open/close, click hit-test and travel.
//!
//! Faithful to `FUN_0042CBD0(4)` + `FUN_00428390` + `FUN_004381C8` mode 5.
//! Rendering lives in [`crate::world_map`]; this module owns the state and the
//! travel side effect.

use super::*;

impl PlayableApp {
    /// Toggle interface mode 5 (`FUN_0042CBD0(4)`, or the close branch).
    pub fn toggle_map(&mut self) {
        if self.dialog.is_some() || self.trade.is_some() || self.character_screen {
            return;
        }
        if self.map_open {
            self.map_open = false;
            self.map_overlay = None;
            self.sounds.push(4);
        } else {
            self.quest_log_open = false;
            self.map_open = true;
            self.map_overlay = None;
            self.hovered_slot = None;
            self.sounds.push(4);
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
        let edge = self.edges.iter().find(|e| e.to == loc as u32).cloned();
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
