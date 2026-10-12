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
//! Each row is independently RLE encoded (see [`decode_row`]). Pixels are
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
    (b[o] as u32)
        | ((b[o + 1] as u32) << 8)
        | ((b[o + 2] as u32) << 16)
        | ((b[o + 3] as u32) << 24)
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
                    format!(
                        "transparent run of {count} at x={x} exceeds image width {width}"
                    ),
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
            format!("row {y} has {} trailing byte(s) after end-of-row", row.len() - q),
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
mod tests {
    use super::*;

    /// Build a single record. `rows` is a list of already-encoded row bytes
    /// (each must include its own EOL token).
    fn record(w: u16, h: u16, rows: &[Vec<u8>]) -> Vec<u8> {
        assert_eq!(rows.len(), h as usize);
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        let sum: usize = rows.iter().map(|r| r.len()).sum();
        let payload = 4 + rows.len() * 2 + sum;
        out.extend_from_slice(&(payload as u32).to_le_bytes());
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        for r in rows {
            out.extend_from_slice(&(r.len() as u16).to_le_bytes());
        }
        for r in rows {
            out.extend_from_slice(r);
        }
        out
    }

    #[test]
    fn header_and_single_pixel() {
        // 1x1 opaque red (RGB555 0x7C00).
        let data = record(1, 1, &[vec![0x01, 0x00, 0x7C, 0x00]]);
        let r = MenuRes::parse(&data).unwrap();
        assert_eq!(r.images.len(), 1);
        let im = &r.images[0];
        assert_eq!((im.width, im.height), (1, 1));
        assert_eq!(&im.rgba, &[255, 0, 0, 255]);
    }

    #[test]
    fn transparent_run_advances_without_pixels() {
        // 4x1: two transparent then two literal.
        let row = vec![0x82, 0x02, 0x00, 0x7C, 0xE0, 0x03, 0x00];
        let data = record(4, 1, &[row]);
        let im = &MenuRes::parse(&data).unwrap().images[0];
        assert_eq!(&im.rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(&im.rgba[4..8], &[0, 0, 0, 0]);
        assert_eq!(&im.rgba[8..12], &[255, 0, 0, 255]);
        // 0x03E0 = green
        assert_eq!(&im.rgba[12..16], &[0, 255, 0, 255]);
    }

    #[test]
    fn eol_ends_row() {
        // 2x2, each row one literal then EOL.
        let data = record(
            2,
            2,
            &[
                vec![0x01, 0x00, 0x7C, 0x00],
                vec![0x81, 0x01, 0x00, 0x7C, 0x00],
            ],
        );
        let im = &MenuRes::parse(&data).unwrap().images[0];
        assert_eq!(&im.rgba[0..4], &[255, 0, 0, 255]);
        // second row starts with one transparent, then red.
        let row2 = (1 * 2) * 4;
        assert_eq!(&im.rgba[row2..row2 + 4], &[0, 0, 0, 0]);
        assert_eq!(&im.rgba[row2 + 4..row2 + 8], &[255, 0, 0, 255]);
    }

    #[test]
    fn multiple_records_reach_eof() {
        let a = record(1, 1, &[vec![0x00]]);
        let b = record(2, 1, &[vec![0x01, 0x00, 0x7C, 0x00]]);
        let mut data = a;
        data.extend_from_slice(&b);
        let r = MenuRes::parse(&data).unwrap();
        assert_eq!(r.images.len(), 2);
        assert_eq!((r.images[0].width, r.images[1].width), (1, 2));
    }

    #[test]
    fn rejects_literal_run_beyond_width() {
        // width 1, literal run of 2.
        let data = record(1, 1, &[vec![0x02, 0x00, 0x7C, 0x00, 0x00, 0x7C, 0x00]]);
        let e = MenuRes::parse(&data).unwrap_err();
        assert!(e.message.contains("exceeds image width"), "{e}");
    }

    #[test]
    fn rejects_transparent_run_beyond_width() {
        let data = record(1, 1, &[vec![0x82, 0x00]]);
        let e = MenuRes::parse(&data).unwrap_err();
        assert!(e.message.contains("transparent run"), "{e}");
    }

    #[test]
    fn rejects_missing_rgb555_bytes() {
        // token claims 2 pixels but only one is present.
        let data = record(2, 1, &[vec![0x02, 0x00, 0x7C, 0x00]]);
        let e = MenuRes::parse(&data).unwrap_err();
        assert!(e.message.contains("RGB555 bytes"), "{e}");
    }

    #[test]
    fn rejects_missing_eol() {
        // One white pixel (0x7FFF -> bytes FF 7F, no 0x00) and no EOL token.
        let e = MenuRes::parse(&record(1, 1, &[vec![0x01, 0xFF, 0x7F]])).unwrap_err();
        assert!(e.message.contains("end-of-row"), "{e}");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = record(1, 1, &[vec![0x00]]);
        data[3] = 0x09;
        let e = MenuRes::parse(&data).unwrap_err();
        assert!(e.message.contains("bad magic"), "{e}");
    }

    #[test]
    fn rejects_truncated_payload() {
        let mut data = record(4, 1, &[vec![0x81, 0x01, 0x00, 0x7C, 0x00]]);
        data.truncate(data.len() - 2);
        let e = MenuRes::parse(&data).unwrap_err();
        assert!(e.message.contains("payload") || e.message.contains("row"), "{e}");
    }

    fn install_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// Real-resource regression: only runs when the original assets are present.
    #[test]
    fn menu640_real_resource() {
        let path = install_dir().join("MENU640R.RES");
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: {} not present", path.display());
            return;
        };
        let res = MenuRes::parse_named("MENU640R.RES", &bytes).expect("MENU640R.RES parse");
        assert_eq!(res.images.len(), 15, "MENU640R.RES image count");
        assert_eq!((res.images[0].width, res.images[0].height), (640, 480));
        assert_eq!((res.images[2].width, res.images[2].height), (13, 13));
        // First full row is exactly 1287 bytes (640*2 + 6 control + 1 EOL).
        assert_eq!(res.images[0].rgba.len(), 640 * 480 * 4);
    }

    #[test]
    fn newh640_real_resource() {
        let path = install_dir().join("NEWH640R.RES");
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: {} not present", path.display());
            return;
        };
        let res = MenuRes::parse_named("NEWH640R.RES", &bytes).expect("NEWH640R.RES parse");
        assert_eq!(res.images.len(), 13, "NEWH640R.RES image count");
        assert_eq!((res.images[0].width, res.images[0].height), (640, 480));
        assert_eq!((res.images[1].width, res.images[1].height), (100, 100));
        assert_eq!((res.images[3].width, res.images[3].height), (56, 65));
    }

    #[test]
    fn menu800_and_newh800_real_resource() {
        for (name, count) in [("MENU800R.RES", 15usize), ("NEWH800R.RES", 13usize)] {
            let path = install_dir().join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                eprintln!("skipping: {} not present", path.display());
                continue;
            };
            let res = MenuRes::parse_named(name, &bytes).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(res.images.len(), count, "{name} image count");
            assert_eq!(res.images[0].width, 800);
            assert_eq!(res.images[0].height, 600);
        }
    }

    #[test]
    fn rgb555_expansion_matches_expected() {
        assert_eq!(rgb555_to_rgba(0x7C00), [255, 0, 0, 255]);
        assert_eq!(rgb555_to_rgba(0x03E0), [0, 255, 0, 255]);
        assert_eq!(rgb555_to_rgba(0x001F), [0, 0, 255, 255]);
        assert_eq!(rgb555_to_rgba(0x7FFF), [255, 255, 255, 255]);
        assert_eq!(rgb555_to_rgba(0x0000), [0, 0, 0, 255]);
    }
}
