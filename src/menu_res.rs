//! Native decoder for the original Konung **menu image RES format**.
//!
//! This is deliberately *not* a universal Konung `.RES` decoder. `.RES` is used
//! by several unrelated internal formats (see `res.rs` for the indexed GRAPH /
//! OBJECTS grammar). This module handles only the menu-image family used by
//! `MENU{width}R.RES`, `ME{width}R.RES`, `NEW{width}R.RES`, `NEWH{width}R.RES`,
//! `PLAY{width}R.RES` and `INTO{width}R.RES`.
//!
//! Recovered layout (`KONUNG_SOURCE_OF_TRUTH.md` §0 evidence label:
//! **BINARY-LAYOUT VERIFIED** against `MENU640R.RES`, `MENU800R.RES`,
//! `NEWH640R.RES`, `NEWH800R.RES`):
//!
//! ```text
//! repeat until EOF:
//!     u8  magic[4] = 00 00 00 05
//!     u32 payload_size
//!     u16 width
//!     u16 height
//!     u16 row_size[height]
//!     u8  row_data[sum(row_size)]
//!
//! payload_size = 4 + height*2 + sum(row_size)
//! next_image   = image_offset + 8 + payload_size
//! ```
//!
//! Each row is independently RLE encoded (see `decode_row`). Pixels are
//! RGB555 little-endian; skipped (transparent) pixels keep `alpha = 0`.
//!
//! The decoder knows only about images. Menu semantics (which index is a
//! background, a button, a portrait, …) live in the `frontend` layer.

/// One decoded 32-bit RGBA image. Every byte is `R,G,B,A` in that order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuImage {
    pub width: u16,
    pub height: u16,
    /// `width * height * 4` bytes, row-major, top-left origin.
    pub rgba: Vec<u8>,
}

/// A parsed menu resource: a sequence of concatenated images.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuRes {
    pub images: Vec<MenuImage>,
}

/// A parse failure. Carries the source name, the image record index and the
/// byte offset, so callers can produce a §34-style diagnostic such as
/// `MENU640R.RES: image 7 at 0x123456: literal run exceeds image width`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuResError {
    pub source: String,
    pub image: usize,
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for MenuResError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: image {} at 0x{:X}: {}",
            self.source, self.image, self.offset, self.message
        )
    }
}

impl std::error::Error for MenuResError {}

impl MenuResError {
    fn new(source: &str, image: usize, offset: usize, message: impl Into<String>) -> Self {
        MenuResError {
            source: source.to_string(),
            image,
            offset,
            message: message.into(),
        }
    }
}

const MAGIC: [u8; 4] = [0x00, 0x00, 0x00, 0x05];
/// Upper bound guarding against absurd allocations from a corrupt header.
const MAX_DIM: u16 = 8192;

fn rd_u16(b: &[u8], o: usize) -> u16 {
    (b[o] as u16) | ((b[o + 1] as u16) << 8)
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    (b[o] as u32) | ((b[o + 1] as u32) << 8) | ((b[o + 2] as u32) << 16) | ((b[o + 3] as u32) << 24)
}

/// Exact RGB555 → 8-bit expansion (`bits 10..14` R, `5..9` G, `0..4` B).
#[inline]
pub fn rgb555_to_rgba(pixel: u16) -> [u8; 4] {
    let r5 = ((pixel >> 10) & 0x1f) as u8;
    let g5 = ((pixel >> 5) & 0x1f) as u8;
    let b5 = (pixel & 0x1f) as u8;
    let r = (r5 << 3) | (r5 >> 2);
    let g = (g5 << 3) | (g5 >> 2);
    let b = (b5 << 3) | (b5 >> 2);
    [r, g, b, 255]
}

/// Decode one row of tokens into `rgba` at row `y` (which has already been
/// initialized to transparent).
///
/// ```text
/// 00       end of row
/// 01..7F   literal run: `token` × u16 RGB555
/// 81..FF   transparent run: `token & 0x7F` pixels skipped
/// ```
fn decode_row(
    source: &str,
    image: usize,
    row_off: usize,
    row: &[u8],
    y: usize,
    width: usize,
    rgba: &mut [u8],
) -> Result<(), MenuResError> {
    let mut x = 0usize;
    let mut q = 0usize;
    let mut saw_eol = false;
    while q < row.len() {
        let token = row[q];
        q += 1;
        if token == 0 {
            saw_eol = true;
            break;
        }
        let count = (token & 0x7f) as usize;
        if token & 0x80 != 0 {
            // Transparent run: advance X only.
            if x + count > width {
                return Err(MenuResError::new(
                    source,
                    image,
                    row_off + q - 1,
                    format!("transparent run of {count} at x={x} exceeds image width {width}"),
                ));
            }
            x += count;
        } else {
            // Literal run: `count` RGB555 pixels.
            if x + count > width {
                return Err(MenuResError::new(
                    source,
                    image,
                    row_off + q - 1,
                    format!("literal run of {count} at x={x} exceeds image width {width}"),
                ));
            }
            if q + count * 2 > row.len() {
                return Err(MenuResError::new(
                    source,
                    image,
                    row_off + q,
                    format!(
                        "literal run of {count} needs {} RGB555 bytes but only {} remain",
                        count * 2,
                        row.len() - q
                    ),
                ));
            }
            for k in 0..count {
                let p = rd_u16(row, q);
                q += 2;
                let di = (y * width + x + k) * 4;
                rgba[di..di + 4].copy_from_slice(&rgb555_to_rgba(p));
            }
            x += count;
        }
    }
    if !saw_eol {
        return Err(MenuResError::new(
            source,
            image,
            row_off,
            format!("row {y} is missing its 0x00 end-of-row token"),
        ));
    }
    if q != row.len() {
        return Err(MenuResError::new(
            source,
            image,
            row_off + q,
            format!(
                "row {y} has {} trailing byte(s) after end-of-row",
                row.len() - q
            ),
        ));
    }
    Ok(())
}

impl MenuRes {
    /// Parse a byte buffer, using `source` (typically the filename) in errors.
    pub fn parse_named(source: &str, bytes: &[u8]) -> Result<MenuRes, MenuResError> {
        let mut images = Vec::new();
        let mut off = 0usize;
        let mut index = 0usize;
        while off < bytes.len() {
            if off + 8 > bytes.len() {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    "truncated record header (need 8 bytes)",
                ));
            }
            if bytes[off..off + 4] != MAGIC {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!(
                        "bad magic {:02X} {:02X} {:02X} {:02X}, expected 00 00 00 05",
                        bytes[off],
                        bytes[off + 1],
                        bytes[off + 2],
                        bytes[off + 3]
                    ),
                ));
            }
            let payload_size = rd_u32(bytes, off + 4) as usize;
            let payload_start = off + 8;
            let record_end = payload_start
                .checked_add(payload_size)
                .ok_or_else(|| MenuResError::new(source, index, off, "payload_size overflow"))?;
            if record_end > bytes.len() {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!(
                        "record claims {payload_size} payload bytes but only {} remain",
                        bytes.len() - payload_start.min(bytes.len())
                    ),
                ));
            }
            if payload_size < 4 {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    "payload too small for width/height header",
                ));
            }
            let width = rd_u16(bytes, payload_start) as usize;
            let height = rd_u16(bytes, payload_start + 2) as usize;
            if width == 0 || height == 0 {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!("zero image dimension {width}x{height}"),
                ));
            }
            if width > MAX_DIM as usize || height > MAX_DIM as usize {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!("implausible image dimension {width}x{height}"),
                ));
            }
            let table_bytes = height
                .checked_mul(2)
                .ok_or_else(|| MenuResError::new(source, index, off, "row table overflow"))?;
            let header_bytes = 4usize
                .checked_add(table_bytes)
                .ok_or_else(|| MenuResError::new(source, index, off, "row table overflow"))?;
            if header_bytes > payload_size {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!(
                        "row_size table of {table_bytes} bytes does not fit payload of {payload_size}"
                    ),
                ));
            }
            let sizes_at = payload_start + 4;
            let mut row_p = payload_start + header_bytes;
            let mut sum_rows = 0usize;
            for y in 0..height {
                let row_size = rd_u16(bytes, sizes_at + y * 2) as usize;
                sum_rows += row_size;
            }
            if 4 + table_bytes + sum_rows != payload_size {
                return Err(MenuResError::new(
                    source,
                    index,
                    off,
                    format!(
                        "payload_size {} != 4 + {} + {} row bytes",
                        payload_size, table_bytes, sum_rows
                    ),
                ));
            }

            let mut rgba = vec![0u8; width * height * 4];
            for y in 0..height {
                let row_size = rd_u16(bytes, sizes_at + y * 2) as usize;
                let row_end = row_p + row_size;
                if row_end > record_end {
                    return Err(MenuResError::new(
                        source,
                        index,
                        row_p,
                        format!(
                            "row {y} of {row_size} bytes extends past the record ({} left)",
                            record_end - row_p
                        ),
                    ));
                }
                decode_row(
                    source,
                    index,
                    row_p,
                    &bytes[row_p..row_end],
                    y,
                    width,
                    &mut rgba,
                )?;
                row_p = row_end;
            }

            images.push(MenuImage {
                width: width as u16,
                height: height as u16,
                rgba,
            });
            off = record_end;
            index += 1;
        }
        Ok(MenuRes { images })
    }

    /// Parse a buffer without a source name (used only for synthetic tests).
    pub fn parse(bytes: &[u8]) -> Result<MenuRes, MenuResError> {
        MenuRes::parse_named("<memory>", bytes)
    }
}

#[cfg(test)]
mod tests;
