//! Per-scanline span accumulator and the shadow pass.
//!
//! The original does **not** have a dirty-rectangle system. It keeps a
//! 600-entry array of per-scanline run lists (`DAT_004C6B50`); a producer,
//! `FUN_0043C040`, clips an RLE-shaped sprite mask into those lists, and a
//! consumer sweeps them. The one consumer that affects the shipped look is
//! `FUN_0043D247`, which halves the colour of every pixel in the accumulated
//! spans — the actor/projectile **ground shadow** pass.
//!
//! ```text
//! FUN_0043C040(mask, x, y)          // add one sprite-shaped mask
//! FUN_0043D247(surface)             // pixel = (pixel & 0xF7DF) >> 1
//! ```
//!
//! Mask layout (identical to the indexed-RLE sprite payload):
//!
//! ```text
//! [u16 w][u16 h][h * u16 row_size][row payloads]
//! row: 0x00 = end of row, byte & 0x80 = skip (byte & 0x7F) px, else run of `byte` px
//! ```

/// One horizontal run of pixels, `x1` inclusive.
///
/// The accumulator stores runs in the order the masks were added, never merged:
/// overlapping body + equipment masks therefore darken a shared pixel once per
/// mask, exactly like the original run lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Screen scanline (`y`), already offset by the mask's row.
    pub row: i32,
    /// First covered pixel column, inclusive.
    pub x0: i32,
    /// Last covered pixel column, inclusive.
    pub x1: i32,
}

/// Inclusive viewport clip rectangle `(x0, y0, x1, y1)`.
pub type Clip = (i32, i32, i32, i32);

/// `FUN_0043C040`: append one sprite-shaped mask's opaque runs to `out`.
///
/// `(x, y)` is the screen origin of the mask (its own anchor already applied).
/// Runs and rows are clipped to `clip`. Returns `false` for a malformed header
/// or when every run fell outside the clip.
pub fn add_mask(
    payload: &[u8],
    off: usize,
    x: i32,
    y: i32,
    clip: Clip,
    out: &mut Vec<Span>,
) -> bool {
    let len = payload.len() as u64;
    let base = off as u64;
    if base + 4 > len {
        return false;
    }
    let w = u16::from_le_bytes([payload[off], payload[off + 1]]) as i32;
    let h = u16::from_le_bytes([payload[off + 2], payload[off + 3]]) as i32;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return false;
    }
    let sizes_at = base + 4;
    if sizes_at + 2 * h as u64 > len {
        return false;
    }
    let before = out.len();
    let mut row_data = (sizes_at + 2 * h as u64) as usize;
    for row in 0..h {
        let size_at = (sizes_at + row as u64 * 2) as usize;
        let size = u16::from_le_bytes([payload[size_at], payload[size_at + 1]]) as usize;
        if row_data as u64 + size as u64 > len {
            break;
        }
        let runs = &payload[row_data..row_data + size];
        row_data += size;
        let screen_row = y + row;
        if screen_row < clip.1 || screen_row > clip.3 {
            continue;
        }
        let mut cursor = 0i32;
        let mut q = 0usize;
        while q < runs.len() {
            let cmd = runs[q];
            q += 1;
            if cmd == 0 {
                break;
            }
            if cmd & 0x80 != 0 {
                cursor += (cmd & 0x7F) as i32;
                continue;
            }
            let count = cmd as i32;
            let x0 = (x + cursor).max(clip.0);
            let x1 = (x + cursor + count - 1).min(clip.2);
            cursor += count;
            if x0 <= x1 {
                out.push(Span {
                    row: screen_row,
                    x0,
                    x1,
                });
            }
        }
    }
    out.len() > before
}

/// `FUN_0043D247`: halve every pixel of every span.
///
/// Deviation (documented, deliberate): the original halves a 16-bit RGB555 word
/// with `(c & mask) >> 1`; the remake's framebuffer is RGBA8, so each of the
/// three colour channels is halved with integer truncation. Alpha is untouched.
pub fn darken(frame: &mut [u8], stride: usize, height: usize, spans: &[Span]) {
    for s in spans {
        let row = s.row;
        if row < 0 || row as usize >= height {
            continue;
        }
        let x0 = s.x0.max(0) as usize;
        let x1 = (s.x1 as usize).min(stride.saturating_sub(1));
        if x0 > x1 {
            continue;
        }
        let base = row as usize * stride * 4;
        for x in x0..=x1 {
            let i = base + x * 4;
            if i + 3 >= frame.len() {
                break;
            }
            frame[i] >>= 1;
            frame[i + 1] >>= 1;
            frame[i + 2] >>= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a mask payload: `[u16 w][u16 h][h*u16 sizes][rows]`.
    fn mask(w: u16, rows: &[&[u8]]) -> (Vec<u8>, usize) {
        let mut p = Vec::new();
        p.extend_from_slice(&w.to_le_bytes());
        p.extend_from_slice(&(rows.len() as u16).to_le_bytes());
        for r in rows {
            p.extend_from_slice(&(r.len() as u16).to_le_bytes());
        }
        for r in rows {
            p.extend_from_slice(r);
        }
        (p, 0)
    }

    /// A skip byte then a short literal run becomes one clipped span on the
    /// mask's row.
    #[test]
    fn a_single_row_run_becomes_one_span() {
        let (payload, off) = mask(4, &[&[0x82, 0x02, 0x00]]);
        let mut out = Vec::new();
        assert!(add_mask(&payload, off, 10, 5, (0, 0, 799, 599), &mut out));
        assert_eq!(
            out,
            vec![Span {
                row: 5,
                x0: 12,
                x1: 13
            }]
        );
    }

    /// A run straddling the right clip edge is truncated, and a mask entirely
    /// outside the clip reports failure and emits nothing.
    #[test]
    fn runs_are_clipped_to_the_viewport() {
        let (payload, off) = mask(8, &[&[0x04, 0x00]]);
        let mut out = Vec::new();
        assert!(add_mask(&payload, off, 796, 0, (0, 0, 799, 599), &mut out));
        assert_eq!(
            out,
            vec![Span {
                row: 0,
                x0: 796,
                x1: 799
            }]
        );
        // Entirely outside the clip.
        let mut out2 = Vec::new();
        assert!(!add_mask(
            &payload,
            off,
            900,
            0,
            (0, 0, 799, 599),
            &mut out2
        ));
        assert!(out2.is_empty());
    }

    /// Only rows inside the clip's y range contribute spans.
    #[test]
    fn rows_outside_the_clip_are_skipped() {
        let (payload, off) = mask(2, &[&[0x02, 0x00], &[0x02, 0x00]]);
        let mut out = Vec::new();
        assert!(add_mask(&payload, off, 0, 10, (0, 11, 799, 599), &mut out));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].row, 11);
    }

    /// A zero width or height in the mask header is rejected instead of
    /// producing an empty successful pass.
    #[test]
    fn a_zero_size_mask_is_rejected() {
        let payload = [0u8; 8];
        let mut out = Vec::new();
        assert!(!add_mask(&payload, 0, 0, 0, (0, 0, 799, 599), &mut out));
    }

    /// `darken` halves RGB inside the spans, leaves alpha and every pixel
    /// outside them untouched, and never touches other scanlines.
    #[test]
    fn darken_halves_every_channel_of_the_spans_only() {
        let mut frame = vec![200u8; 8 * 4 * 2];
        let spans = [Span {
            row: 0,
            x0: 1,
            x1: 2,
        }];
        darken(&mut frame, 8, 2, &spans);
        assert_eq!(&frame[0..4], &[200, 200, 200, 200]); // untouched
        assert_eq!(&frame[4..8], &[100, 100, 100, 200]); // halved
        assert_eq!(&frame[8..12], &[100, 100, 100, 200]);
        assert_eq!(&frame[12..16], &[200, 200, 200, 200]);
        // The second row is untouched.
        assert_eq!(&frame[32..36], &[200, 200, 200, 200]);
    }
}
