//! `FLAMES.RES` decoder: the original 74-frame prop fire animation.
//!
//! Format (verified from `FUN_0043A75C` + `FUN_00427D94` and the shipped file):
//!
//! ```text
//! 74 x [u32 magic = 0x05000000][u32 payload_size]
//! payload:
//!   u16 width
//!   u16 height
//!   u16 row_byte_len[height]
//!   row data (row_byte_len bytes each)
//! ```
//!
//! Per row:
//! - `0x00` -> row end;
//! - `b & 0x80` -> skip `b & 0x7F` pixels (transparent);
//! - otherwise -> `b` literal 16-bit pixels.
//!
//! Pixels are RGB555 (max channels in the shipped frames: R 31, G 22, B 4).
//! The game converts 555 -> 565 for its DirectDraw surface; the remake expands
//! 555 directly to RGBA.

use super::*;

/// Number of blocks in `FLAMES.RES` (`FUN_0043A75C` loop count `0x4A`).
pub const FLAME_BLOCK_COUNT: usize = 74;
/// Block tag written by the shipped archive.
pub const FLAME_MAGIC: u32 = 0x0500_0000;

/// One prop flame emitter from the object record (`+0x1C + i*5`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlameEmitter {
    /// Frame-range selector (`+0`: 0..=6).
    pub selector: u8,
    /// Screen-space x offset from the prop anchor (`+1`, `i16`).
    pub x: i16,
    /// Screen-space y offset from the prop anchor (`+3`, `i16`).
    pub y: i16,
}

/// `0x45AE68` (min) / `0x45AE6C` (max) frame ranges, stride `8`.
/// The original loop is inclusive (`min..=max`), stored here as half-open
/// `[min, end)` so the 74 blocks are covered exactly.
pub const FLAME_RANGES: [(u8, u8); 7] = [
    (0, 11),
    (11, 22),
    (22, 33),
    (33, 44),
    (44, 55),
    (55, 60),
    (60, 74),
];

/// Half-open frame range `[min, end)` for a flame selector.
pub fn flame_range(selector: u8) -> Option<(u8, u8)> {
    FLAME_RANGES.get(selector as usize).copied()
}

impl Res {
    /// Decode and attach `FLAMES.RES`. Returns the number of frames.
    pub fn load_flames(&mut self, data: &[u8]) -> Result<usize, String> {
        let frames = parse_flames(data)?;
        let count = frames.len();
        self.flames = frames;
        Ok(count)
    }

    /// Decoded frame, if `FLAMES.RES` was loaded.
    pub fn flame_frame(&self, index: usize) -> Option<Img> {
        self.flames.get(index).cloned()
    }

    /// Number of decoded `FLAMES.RES` frames (`0` until loaded).
    pub fn flame_count(&self) -> usize {
        self.flames.len()
    }
}

/// Parse all 74 `FLAMES.RES` blocks.
pub fn parse_flames(data: &[u8]) -> Result<Vec<Img>, String> {
    let mut frames = Vec::with_capacity(FLAME_BLOCK_COUNT);
    let mut pos = 0usize;
    for index in 0..FLAME_BLOCK_COUNT {
        if pos + 8 > data.len() {
            return Err(format!(
                "FLAMES.RES: frame {index} header out of bounds at 0x{pos:X}"
            ));
        }
        let magic = u32(data, pos);
        let size = u32(data, pos + 4) as usize;
        if magic != FLAME_MAGIC {
            return Err(format!(
                "FLAMES.RES: frame {index} bad magic 0x{magic:08X} at 0x{pos:X}"
            ));
        }
        let start = pos + 8;
        let end = start
            .checked_add(size)
            .ok_or_else(|| format!("FLAMES.RES: frame {index} size overflow"))?;
        if end > data.len() {
            return Err(format!(
                "FLAMES.RES: frame {index} payload out of bounds (0x{start:X}..0x{end:X})"
            ));
        }
        frames.push(decode_flame_frame(&data[start..end], index)?);
        pos = end;
    }
    if pos != data.len() {
        return Err(format!(
            "FLAMES.RES: {} trailing bytes after {FLAME_BLOCK_COUNT} frames",
            data.len() - pos
        ));
    }
    Ok(frames)
}

/// Decode one `[u16 w][u16 h][u16 row_len[h]][rows]` frame payload.
///
/// Row tokens are `0x00` = end of row, `0x80 | n` = skip `n` pixels
/// (transparent) and `n` = `n` literal RGB555 pixels. Literal pixels past the
/// declared width are consumed but not stored; a row that overruns the payload
/// is an error, so a corrupt frame never decodes partially.
fn decode_flame_frame(payload: &[u8], index: usize) -> Result<Img, String> {
    if payload.len() < 4 {
        return Err(format!("FLAMES.RES: frame {index} payload too short"));
    }
    let width = u16(payload, 0) as usize;
    let height = u16(payload, 2) as usize;
    if width == 0 || height == 0 || width > 1024 || height > 1024 {
        return Err(format!(
            "FLAMES.RES: frame {index} bad dimensions {width}x{height}"
        ));
    }
    let row_table = 4usize;
    let data_start = row_table + height * 2;
    if data_start > payload.len() {
        return Err(format!("FLAMES.RES: frame {index} row table truncated"));
    }

    let mut image = Image::new(width, height, [0, 0, 0, 0]);
    let mut row_start = data_start;
    for row in 0..height {
        let row_len = u16(payload, row_table + row * 2) as usize;
        let row_end = row_start
            .checked_add(row_len)
            .ok_or_else(|| format!("FLAMES.RES: frame {index} row {row} overflow"))?;
        if row_end > payload.len() {
            return Err(format!("FLAMES.RES: frame {index} row {row} out of bounds"));
        }
        let mut q = row_start;
        let mut x = 0usize;
        while q < row_end {
            let control = payload[q];
            q += 1;
            if control == 0 {
                break;
            }
            if control & 0x80 != 0 {
                x += (control & 0x7F) as usize;
                continue;
            }
            let count = control as usize;
            if q + count * 2 > row_end {
                return Err(format!(
                    "FLAMES.RES: frame {index} row {row} literal overruns row"
                ));
            }
            for _ in 0..count {
                let value = u16(payload, q);
                q += 2;
                if x >= width {
                    continue;
                }
                let r5 = ((value >> 10) & 0x1F) as u32;
                let g5 = ((value >> 5) & 0x1F) as u32;
                let b5 = (value & 0x1F) as u32;
                let o = (row * width + x) * 4;
                image.data[o] = (r5 * 255 / 31) as u8;
                image.data[o + 1] = (g5 * 255 / 31) as u8;
                image.data[o + 2] = (b5 * 255 / 31) as u8;
                image.data[o + 3] = 255;
                x += 1;
            }
        }
        row_start = row_end;
    }
    Ok(Rc::new(image))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 4x2 frame whose rows mix skip and literal tokens.
    fn synthetic_frame() -> Vec<u8> {
        // 4x2: row 0 = skip 1 + 2 pixels + end; row 1 = 1 skip + 1 pixel + end.
        let mut payload = Vec::new();
        payload.extend_from_slice(&4u16.to_le_bytes());
        payload.extend_from_slice(&2u16.to_le_bytes());
        let row0: Vec<u8> = {
            let mut r = vec![0x81]; // skip 1
            r.push(0x02); // 2 literal pixels
            r.extend_from_slice(&0x7C00u16.to_le_bytes()); // red
            r.extend_from_slice(&0x03E0u16.to_le_bytes()); // green
            r.push(0x00);
            r
        };
        let row1: Vec<u8> = {
            let mut r = vec![0x81, 0x01];
            r.extend_from_slice(&0x001Fu16.to_le_bytes()); // blue
            r.push(0x00);
            r
        };
        payload.extend_from_slice(&(row0.len() as u16).to_le_bytes());
        payload.extend_from_slice(&(row1.len() as u16).to_le_bytes());
        payload.extend_from_slice(&row0);
        payload.extend_from_slice(&row1);
        payload
    }

    /// Skipped pixels stay transparent and literal RGB555 values expand to full
    /// red/green/blue channels.
    #[test]
    fn decodes_555_literals_and_skips() {
        let img = decode_flame_frame(&synthetic_frame(), 0).expect("decodes");
        assert_eq!((img.w, img.h), (4, 2));
        assert_eq!(img.data[0], 0, "skipped pixel transparent");
        let red = (1 * 4) as usize;
        assert_eq!(&img.data[red..red + 4], &[255, 0, 0, 255]);
        let green = (2 * 4) as usize;
        assert_eq!(&img.data[green..green + 4], &[0, 255, 0, 255]);
        let blue_row = (4 + 1) * 4;
        assert_eq!(&img.data[blue_row..blue_row + 4], &[0, 0, 255, 255]);
    }

    /// The shipped archive holds 74 256x195 frames and their brightest pixels
    /// are warm (R >= G >= B).
    #[test]
    fn real_flames_has_74_frames_and_fire_colors() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("FLAMES.RES");
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let frames = parse_flames(&bytes).expect("shipped FLAMES.RES decodes");
        assert_eq!(frames.len(), FLAME_BLOCK_COUNT);
        let frame = &frames[0];
        assert_eq!((frame.w, frame.h), (256, 195));
        let mut max_r = 0u8;
        let mut max_g = 0u8;
        let mut max_b = 0u8;
        for px in frame.data.chunks_exact(4) {
            max_r = max_r.max(px[0]);
            max_g = max_g.max(px[1]);
            max_b = max_b.max(px[2]);
        }
        assert!(
            max_r >= max_g && max_g >= max_b,
            "fire should be red/yellow"
        );
        assert!(max_r > 200, "frame must contain bright pixels");
    }

    /// The seven `FLAME_RANGES` entries are contiguous and cover exactly the 74
    /// shipped blocks.
    #[test]
    fn flame_ranges_cover_all_74_blocks() {
        let mut next = 0u8;
        let mut total = 0usize;
        for selector in 0..7u8 {
            let (min, end) = flame_range(selector).expect("selector");
            assert_eq!(min, next, "ranges must be contiguous");
            assert!(min < end);
            total += (end - min) as usize;
            next = end;
        }
        assert_eq!(next, 74);
        assert_eq!(total, FLAME_BLOCK_COUNT);
        assert!(flame_range(7).is_none());
    }
}
