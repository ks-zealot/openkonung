//! `camera` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    pub(super) fn desired_player_pan(&self) -> [f64; 2] {
        let (px, py) = entity_to_local_px_f(self.player_x, self.player_y);
        [VIRTUAL_W as f64 * 0.5 - px, VIRTUAL_H as f64 * 0.5 - py]
    }

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

    pub(super) fn fit_detail(&mut self) {
        self.center_camera_on_player(true, 0.0);
    }

    // --- portals ---

    pub fn window_game_rect(&self) -> (i32, i32, i32, i32, f64) {
        let sw = self.host_w.max(1);
        let sh = self.host_h.max(1);
        let scale = (sw as f64 / VIRTUAL_W as f64).min(sh as f64 / VIRTUAL_H as f64);
        let w = (VIRTUAL_W as f64 * scale).round().max(1.0) as i32;
        let h = (VIRTUAL_H as f64 * scale).round().max(1.0) as i32;
        ((sw - w) / 2, (sh - h) / 2, w, h, scale)
    }

    pub(super) fn window_to_virtual(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (rx, ry, w, h, scale) = self.window_game_rect();
        if x < rx as f64 || x >= (rx + w) as f64 || y < ry as f64 || y >= (ry + h) as f64 {
            return None;
        }
        Some(((x - rx as f64) / scale, (y - ry as f64) / scale))
    }

    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.host_w = w.max(1);
        self.host_h = h.max(1);
        if !self.mode_graph && self.camera_follow {
            self.center_camera_on_player(true, 0.0);
        }
    }
}
