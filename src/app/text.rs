//! `text` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    // --- drawing ---

    /// Queue one text run for the overlay pass at virtual `(x, y)`.
    ///
    /// The overlay list is cleared and re-emitted every `draw_detail`, so these
    /// coordinates are virtual-screen pixels, not world cells.
    pub(super) fn text(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) {
        self.texts.push(TextItem {
            x,
            y,
            rgb,
            scale,
            s,
        });
    }

    /// Emit `s` word-wrapped so no line exceeds the right screen edge. Returns
    /// the y coordinate just below the last emitted line.
    pub(super) fn text_wrapped(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) -> i32 {
        let max_px = (VIRTUAL_W - x - 4).max(48);
        let line_h = (14.0 * scale).ceil() as i32 + 2;
        let mut cy = y;
        for line in wrap_to_px(&s, scale, max_px) {
            self.text(x, cy, rgb, scale, line);
            cy += line_h;
        }
        cy
    }
}
