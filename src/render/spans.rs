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

/// `FUN_0043E4B7`: one frame's run capacity (`0x5076D8 = 0x2EDE`). When the
/// budget is exhausted `FUN_0043C040` emits no further runs for that frame.
pub const RUN_BUDGET: usize = 0x2EDE;

/// Expand a 5-bit channel to 8 bits (`(v << 3) | (v >> 2)`).
#[inline]
pub fn expand5(v: u16) -> u8 {
    (((v & 31) << 3) | ((v & 31) >> 2)) as u8
}

/// Expand a 6-bit channel to 8 bits (`(v << 2) | (v >> 4)`).
#[inline]
pub fn expand6(v: u16) -> u8 {
    (((v & 63) << 2) | ((v & 63) >> 4)) as u8
}

/// Recover the 5-bit palette channel from the bit-replicated 8-bit expansion.
#[inline]
fn collapse5(c: u8) -> u16 {
    ((c as u32 * 31 + 127) / 255) as u16
}

/// `FUN_00427A3C` (the 16bpp palette conversion, `0x43A31C`): a 555 palette
/// word becomes a 565 screen word with `(c & 0x7FE0) << 1 | (c & 0x1F)`.
#[inline]
pub fn palette555_to_565(p: u16) -> u16 {
    ((p & 0x7FE0) << 1) | (p & 0x1F)
}

/// `FUN_0043D247`: halve one screen pixel, in 565.
///
/// The framebuffer holds the 565-expanded 555 palette, so the
/// palette word is recovered, converted to 565 exactly like the original's
/// 16bpp path, halved with `(c & 0xF7DF) >> 1`, and expanded back.
#[inline]
pub fn darken_pixel(rgb: [u8; 3]) -> [u8; 3] {
    let p = (collapse5(rgb[0]) << 10) | (collapse5(rgb[1]) << 5) | collapse5(rgb[2]);
    let d = (palette555_to_565(p) & 0xF7DF) >> 1;
    [
        expand5(d >> 11),
        expand6((d >> 5) & 63),
        expand5(d & 31),
    ]
}

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
                // `0x5076D8`: the frame's run budget; once exhausted the
                // original stores nothing more.
                if out.len() >= RUN_BUDGET {
                    return out.len() > before;
                }
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
/// Exact 16-bit arithmetic: the palette word is recovered, converted to 565
/// like the original's 16bpp path, halved with `(c & 0xF7DF) >> 1`, expanded
/// back. Alpha is untouched.
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
            let out = darken_pixel([frame[i], frame[i + 1], frame[i + 2]]);
            frame[i] = out[0];
            frame[i + 1] = out[1];
            frame[i + 2] = out[2];
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

    /// `darken` halves RGB inside the spans in the original 565 form, leaves
    /// alpha and every pixel outside them untouched, and never touches other
    /// scanlines.
    #[test]
    fn darken_halves_every_channel_of_the_spans_only() {
        // White (a 555 palette entry expanded through the 565 path).
        let mut frame = vec![255u8; 8 * 4 * 2];
        let spans = [Span {
            row: 0,
            x0: 1,
            x1: 2,
        }];
        darken(&mut frame, 8, 2, &spans);
        // `0x7FFF` -> 565 `0xFFDF`, `(c & 0xF7DF) >> 1` = `0x7BEF`:
        // r5 15, g6 31, b5 15.
        assert_eq!(&frame[0..4], &[255, 255, 255, 255]); // untouched
        assert_eq!(&frame[4..8], &[123, 125, 123, 255]); // exact 565 halving
        assert_eq!(&frame[8..12], &[123, 125, 123, 255]);
        assert_eq!(&frame[12..16], &[255, 255, 255, 255]);
        // The second row is untouched.
        assert_eq!(&frame[32..36], &[255, 255, 255, 255]);
    }

    /// Every 555 palette colour halves to `(r5 >> 1, g5, b5 >> 1)` in the
    /// expanded 565 form: the green field keeps the 5-bit value (its 6-bit
    /// screen word was `g5 << 1`).
    #[test]
    fn darken_matches_the_original_palette_arithmetic() {
        for r5 in 0u16..32 {
            for g5 in 0u16..32 {
                for b5 in 0u16..32 {
                    let pre = [expand5(r5), expand5(g5), expand5(b5)];
                    let post = darken_pixel(pre);
                    assert_eq!(
                        post,
                        [expand5(r5 >> 1), expand6(g5), expand5(b5 >> 1)],
                        "r5 {r5} g5 {g5} b5 {b5}"
                    );
                }
            }
        }
    }

    /// `0x5076D8`: at most `RUN_BUDGET` runs are accumulated per frame.
    #[test]
    fn the_run_budget_caps_the_accumulator() {
        let (payload, off) = mask(4, &[&[0x04, 0x00]]);
        let mut out = vec![
            Span {
                row: 0,
                x0: 0,
                x1: 0,
            };
            RUN_BUDGET
        ];
        let before = out.len();
        assert!(!add_mask(&payload, off, 0, 0, (0, 0, 799, 599), &mut out));
        assert_eq!(out.len(), before, "the budget must stop new runs");
    }
}
