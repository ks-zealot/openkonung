//! `camera` subsystem of [`crate::app`]: the world-view pan and the
//! host-canvas -> virtual-screen mapping.
//!
//! The original has no separate camera matrix: `FUN_0041435C` adds the global
//! screen origin to the actor projection and the renderer subtracts nothing
//! else, so panning is just an offset applied to every world draw. Here that
//! offset is [`PlayableApp`]'s `detail_pan` (virtual pixels), and the host
//! canvas is letterboxed to a 800x600 (or 640x480) virtual surface by
//! [`PlayableApp::window_game_rect`].

use super::*;

impl PlayableApp {
    /// Pan that pins the protagonist's sprite anchor at the screen centre, in
    /// virtual pixels.
    ///
    /// Exact port of the original camera target: the world loop pushes
    /// `[leader+0xEC]`/`[leader+0xE4]` into `FUN_00428C94` every frame
    /// (`0x424AF2`), and those fields are
    /// `viewport_origin + Entity+0xE8/+0xF0 + frame_(dx,dy)` (`FUN_0041435C`).
    /// The remake's `(px, py)` is the same actor projection
    /// (`crate::coords::entity_to_local_px_f` on the **pre-step** render cell,
    /// the cell the sprite is drawn from) and `(gx, gy)` is the same frame
    /// anchor used by the sprite blit, so the pan is
    /// `[screen/2 - px - gx, screen/2 - py - gy]`.
    pub(super) fn desired_player_pan(&mut self) -> [f64; 2] {
        let (px, py) = entity_to_local_px_f(self.player_render_x, self.player_render_y);
        // Original `Entity+0xE4/+0xEC` uses the **body** anchor record, not the
        // composed sprite's bounding box (equipped overlays extend past it).
        let (action, fi, _) = self.player_pose(self.player_active);
        let dir = (self.player_dir.max(0) as usize) & 7;
        let (gx, gy) = self.world.res.human_body_anchor(
            self.player.hero.visual_index as usize,
            action,
            dir,
            fi,
        );
        [
            VIRTUAL_W as f64 * 0.5 - px - gx as f64,
            VIRTUAL_H as f64 * 0.5 - py - gy as f64,
        ]
    }

    /// Snap `detail_pan` onto the original camera position.
    ///
    /// `FUN_00428C94` assigns `camera = target - (400,300)` directly, clamped to
    /// `[0x3A, 0x81D73C-0x81D720]` / `[0x20, 0x81D758-0x81D71C]`, where
    /// `0x81D73C`/`0x81D758` are the per-map terrain extents
    /// ([`crate::loc::Location::camera_bounds`]). There is no easing in the
    /// original, so this is called every frame while camera follow is on; the
    /// remake's `V` toggle is the manual-pan escape hatch.
    pub fn center_camera_on_player(&mut self) {
        let viewport = crate::hud::layout(VIRTUAL_W as i32).viewport;
        let desired = self.desired_player_pan();
        // Original camera variables (`0x81D72C`/`0x81D748`): screen = viewport - camera.
        let mut cam_x = viewport.x - desired[0].round() as i32;
        let mut cam_y = viewport.y - desired[1].round() as i32;
        let (right, bottom) = self
            .world
            .loc(self.current_map)
            .map(|l| l.camera_bounds())
            .unwrap_or((i32::MAX, i32::MAX));
        if cam_x < 0x3A {
            cam_x = 0x3A;
        } else if cam_x + viewport.w > right {
            cam_x = right - viewport.w;
        }
        if cam_y < 0x20 {
            cam_y = 0x20;
        } else if cam_y + viewport.h > bottom {
            cam_y = bottom - viewport.h;
        }
        self.detail_pan = [
            (viewport.x - cam_x) as f64,
            (viewport.y - cam_y) as f64,
        ];
    }

    /// Re-centre the world view (map entry, `Home`, graph->game).
    pub(super) fn fit_detail(&mut self) {
        self.center_camera_on_player();
    }

    // --- portals ---

    /// Letterboxed game rectangle inside the host canvas.
    ///
    /// Returns `(x, y, w, h, scale)`: the virtual surface is scaled by the
    /// smaller of the width/height ratios (no distortion) and centred, so the
    /// shell can blur-fill the remaining bands. `w`/`h` are at least 1.
    pub fn window_game_rect(&self) -> (i32, i32, i32, i32, f64) {
        let sw = self.host_w.max(1);
        let sh = self.host_h.max(1);
        let scale = (sw as f64 / VIRTUAL_W as f64).min(sh as f64 / VIRTUAL_H as f64);
        let w = (VIRTUAL_W as f64 * scale).round().max(1.0) as i32;
        let h = (VIRTUAL_H as f64 * scale).round().max(1.0) as i32;
        ((sw - w) / 2, (sh - h) / 2, w, h, scale)
    }

    /// Map a host-canvas point into virtual pixels, or `None` when it falls in
    /// the letterbox bands. Pointer input outside the game rect is ignored so a
    /// click on the black border cannot move the party.
    pub(super) fn window_to_virtual(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (rx, ry, w, h, scale) = self.window_game_rect();
        if x < rx as f64 || x >= (rx + w) as f64 || y < ry as f64 || y >= (ry + h) as f64 {
            return None;
        }
        Some(((x - rx as f64) / scale, (y - ry as f64) / scale))
    }

    /// Record a new host canvas size (`Event` resize / initial layout).
    ///
    /// Snaps the camera back onto the player because the letterbox scale
    /// changed.
    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.host_w = w.max(1);
        self.host_h = h.max(1);
        if self.camera_follow {
            self.center_camera_on_player();
        }
    }
}
