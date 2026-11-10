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
    /// Pan that would put the protagonist cell at the centre of the virtual
    /// screen, in virtual pixels.
    ///
    /// Uses the verified actor projection `FUN_00438A78`
    /// ([`crate::coords::entity_to_local_px_f`]) on the committed cell
    /// (`Entity+0x08/+0x0A`), not the draw-only pre-step cell.
    pub(super) fn desired_player_pan(&self) -> [f64; 2] {
        let (px, py) = entity_to_local_px_f(self.player_x, self.player_y);
        [VIRTUAL_W as f64 * 0.5 - px, VIRTUAL_H as f64 * 0.5 - py]
    }

    /// Move `detail_pan` toward (or snap onto) the player-centred pan.
    ///
    /// `immediate` snaps; otherwise the offset is eased with the per-frame
    /// factor `1 - exp(-camera_follow_strength * dt)`, which is frame-rate
    /// independent and converges to the target without overshoot. Only the
    /// camera-follow toggle (`V`) calls the eased form, so a manual middle-drag
    /// pan is never fought by the follow.
    pub fn center_camera_on_player(&mut self, immediate: bool, dt: f64) {
        let desired = self.desired_player_pan();
        if immediate {
            self.detail_pan = desired;
            return;
        }
        let a = 1.0 - (-self.camera_follow_strength * dt.max(0.0)).exp();
        self.detail_pan[0] += (desired[0] - self.detail_pan[0]) * a;
        self.detail_pan[1] += (desired[1] - self.detail_pan[1]) * a;
    }

    /// Re-centre the world view instantly (map entry, `Home`, graph->game).
    ///
    /// Kept as a named entry point because the original has a distinct
    /// "fit view" path (`Home`) that must not start an eased catch-up.
    pub(super) fn fit_detail(&mut self) {
        self.center_camera_on_player(true, 0.0);
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
    /// changed; the graph view keeps its own zoom/pan untouched.
    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.host_w = w.max(1);
        self.host_h = h.max(1);
        if !self.mode_graph && self.camera_follow {
            self.center_camera_on_player(true, 0.0);
        }
    }
}
