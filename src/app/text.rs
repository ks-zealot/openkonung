//! `text` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    /// User-facing location label: the original name for Konung.N, falling back
    /// to `Konung.N` when the table entry is missing/empty. In debug mode the
    /// numeric id is appended: `Родники [Konung.1]`.
    pub(super) fn label(&self, id: u32) -> String {
        let name = if id >= 1 {
            self.location_names.get((id - 1) as usize)
        } else {
            None
        };
        match name {
            Some(n) if !n.is_empty() => {
                if self.debug_ids {
                    format!("{} [Konung.{}]", n, id)
                } else {
                    n.clone()
                }
            }
            _ => format!("Konung.{}", id),
        }
    }

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

    /// Emit `s` shifted left/shrunk so its right edge stays on screen (keeps
    /// marker labels attached to their marker when they sit near the right edge).
    pub(super) fn text_clamped(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) {
        let w = text_px(&s, scale);
        let x = x.min((VIRTUAL_W - 4 - w).max(2)).max(2);
        let y = y.min(VIRTUAL_H - 16).max(0);
        self.text(x, y, rgb, scale, s);
    }
}
