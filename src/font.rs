//! Original Konung proportional bitmap fonts.
//!
//! Two fonts are embedded in `konung.res` (`KONUNG_SOURCE_OF_TRUTH.md` §0 label:
//! **BINARY-LAYOUT VERIFIED** against the supplied executable):
//!
//! ```text
//! small  table file 0x171E34  (VA 0x58BC34)
//! large  table file 0x16193C  (VA 0x57B73C)
//! ```
//!
//! Table header:
//!
//! ```text
//! +0x00 u8   first_char   (0x20)
//! +0x01 u8   count        (0xD1 = 209, chars 0x20..0xF0)
//! +0x02 u16  fallback_width (4)
//! +0x04 u32  glyph_ptr[count]   // VA, not file offset
//! ```
//!
//! Glyph record (`+0x00 u16 width`, `+0x02 u16 height`,
//! `+0x04 u16 encoded_row_size[height]`, then scanline data). Characters are
//! **byte-indexed** through CP866. Rows use signed-byte RLE:
//!
//! ```text
//! cmd == 0   end of scanline
//! cmd  < 0   transparent skip ((cmd as u8) & 0x7F) pixels
//! cmd  > 0   copy `cmd` literal u16 (RGB555) pixels
//! ```
//!
//! Both measurement and drawing use the same per-glyph advance (glyph width, or
//! `fallback_width`). Out-of-range/null glyphs advance by `fallback_width` and
//! draw nothing — never a substitute modern font.

use crate::menu_res::rgb555_to_rgba;
use crate::names::char_to_cp866;
use crate::res::u16 as rd_u16;

pub const SMALL_FONT_OFF: usize = 0x171E34;
pub const LARGE_FONT_OFF: usize = 0x16193C;

/// `konung.res` DGROUP mapping: VA 0x450000 == file offset 0x36200.
/// Glyph pointers in the font tables are VAs into this segment.
const FONT_DGROUP_VA: u32 = 0x450000;
const FONT_DGROUP_FILE: usize = 0x36200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalFont {
    Small,
    Large,
}

/// A decoded glyph: `width * height * 4` RGBA, transparent (alpha 0) where the
/// original scanline skipped.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

pub struct FontTable {
    first_char: u8,
    fallback: i32,
    glyphs: Vec<Option<Glyph>>,
}

impl FontTable {
    /// Parse a font table from the raw `konung.res` bytes. Returns `None` if the
    /// table header is out of bounds.
    pub fn parse(data: &[u8], table_off: usize) -> Option<FontTable> {
        if table_off + 4 > data.len() {
            return None;
        }
        let first_char = data[table_off];
        let count = data[table_off + 1] as usize;
        let fallback = rd_u16(data, table_off + 2) as i32;
        let ptrs_at = table_off + 4;
        if ptrs_at + count * 4 > data.len() {
            return None;
        }
        let mut glyphs = Vec::with_capacity(count);
        for i in 0..count {
            let va = crate::res::u32(data, ptrs_at + i * 4);
            if va == 0 {
                glyphs.push(None);
            } else {
                glyphs.push(decode_glyph(data, va));
            }
        }
        Some(FontTable {
            first_char,
            fallback,
            glyphs,
        })
    }

    pub fn first_char(&self) -> u8 {
        self.first_char
    }

    pub fn fallback_width(&self) -> i32 {
        self.fallback
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        let b = char_to_cp866(ch)?;
        let idx = b as i32 - self.first_char as i32;
        if idx < 0 || idx as usize >= self.glyphs.len() {
            return None;
        }
        self.glyphs[idx as usize].as_ref()
    }

    fn advance(&self, ch: char) -> i32 {
        match self.glyph(ch) {
            Some(g) => g.width as i32,
            None => self.fallback,
        }
    }

    /// Canonical text width: the sum of the same per-glyph advances used by
    /// [`FontTable::draw`].
    pub fn measure(&self, text: &str) -> i32 {
        text.chars().map(|c| self.advance(c)).sum()
    }

    /// Draw `text` at `(x, y)` (top-left of the line box) into an RGBA buffer.
    /// Returns the advance width. `rgb_override` recolours opaque pixels when
    /// `Some`.
    pub fn draw(
        &self,
        dst: &mut [u8],
        dw: usize,
        dh: usize,
        text: &str,
        x: i32,
        y: i32,
        rgb_override: Option<[u8; 3]>,
    ) -> i32 {
        let mut pen = x;
        for ch in text.chars() {
            match self.glyph(ch) {
                Some(g) => {
                    blit_glyph(dst, dw, dh, g, pen, y, rgb_override);
                    pen += g.width as i32;
                }
                None => pen += self.fallback,
            }
        }
        pen - x
    }
}

fn va_to_file(va: u32) -> usize {
    (FONT_DGROUP_FILE as i64 + (va as i64 - FONT_DGROUP_VA as i64)) as usize
}

fn decode_glyph(data: &[u8], va: u32) -> Option<Glyph> {
    let fo = va_to_file(va);
    if fo + 4 > data.len() {
        return None;
    }
    let w = rd_u16(data, fo) as usize;
    let h = rd_u16(data, fo + 2) as usize;
    if w == 0 || h == 0 || w > 512 || h > 512 {
        return None;
    }
    let sizes_at = fo + 4;
    if sizes_at + h * 2 > data.len() {
        return None;
    }
    let mut rgba = vec![0u8; w * h * 4];
    let mut p = sizes_at + h * 2;
    for y in 0..h {
        let row_size = rd_u16(data, sizes_at + y * 2) as usize;
        if p + row_size > data.len() {
            return None;
        }
        let row = &data[p..p + row_size];
        p += row_size;
        let mut x = 0usize;
        let mut i = 0usize;
        while i < row.len() {
            let cmd = row[i] as i8;
            i += 1;
            if cmd == 0 {
                break;
            }
            if cmd < 0 {
                x += ((cmd as u8) & 0x7f) as usize;
            } else {
                let n = cmd as usize;
                if i + n * 2 > row.len() {
                    break;
                }
                for _ in 0..n {
                    let px = (row[i] as u16) | ((row[i + 1] as u16) << 8);
                    i += 2;
                    if x < w {
                        let c = rgb555_to_rgba(px);
                        let di = (y * w + x) * 4;
                        rgba[di..di + 4].copy_from_slice(&c);
                    }
                    x += 1;
                }
            }
        }
    }
    Some(Glyph {
        width: w as u16,
        height: h as u16,
        rgba,
    })
}

fn blit_glyph(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    g: &Glyph,
    ox: i32,
    oy: i32,
    rgb_override: Option<[u8; 3]>,
) {
    for gy in 0..g.height as i32 {
        let dy = oy + gy;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        for gx in 0..g.width as i32 {
            let dx = ox + gx;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let si = (gy as usize * g.width as usize + gx as usize) * 4;
            if g.rgba[si + 3] == 0 {
                continue;
            }
            let di = dy as usize * dw * 4 + dx as usize * 4;
            match rgb_override {
                Some(c) => {
                    dst[di] = c[0];
                    dst[di + 1] = c[1];
                    dst[di + 2] = c[2];
                    dst[di + 3] = 255;
                }
                None => dst[di..di + 4].copy_from_slice(&g.rgba[si..si + 4]),
            }
        }
    }
}

/// Both original fonts, parsed once from `konung.res`.
pub struct Fonts {
    pub small: Option<FontTable>,
    pub large: Option<FontTable>,
}

impl Fonts {
    pub fn parse(konung: &[u8]) -> Fonts {
        Fonts {
            small: FontTable::parse(konung, SMALL_FONT_OFF),
            large: FontTable::parse(konung, LARGE_FONT_OFF),
        }
    }

    pub fn get(&self, font: OriginalFont) -> Option<&FontTable> {
        match font {
            OriginalFont::Small => self.small.as_ref(),
            OriginalFont::Large => self.large.as_ref(),
        }
    }

    pub fn measure(&self, font: OriginalFont, text: &str) -> i32 {
        self.get(font).map(|t| t.measure(text)).unwrap_or(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        dst: &mut [u8],
        dw: usize,
        dh: usize,
        font: OriginalFont,
        text: &str,
        x: i32,
        y: i32,
        rgb_override: Option<[u8; 3]>,
    ) -> i32 {
        match self.get(font) {
            Some(t) => t.draw(dst, dw, dh, text, x, y, rgb_override),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn konung() -> Option<Vec<u8>> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../konung.res");
        std::fs::read(&p).ok()
    }

    #[test]
    fn table_landmarks() {
        let Some(d) = konung() else {
            eprintln!("skipping: konung.res not present");
            return;
        };
        let small = FontTable::parse(&d, SMALL_FONT_OFF).expect("small font");
        let large = FontTable::parse(&d, LARGE_FONT_OFF).expect("large font");
        assert_eq!(small.first_char(), 0x20);
        assert_eq!(large.first_char(), 0x20);
        assert_eq!(small.fallback_width(), 4);
        assert_eq!(large.fallback_width(), 4);
        assert_eq!(small.glyph_count(), 0xD1);
        assert_eq!(large.glyph_count(), 0xD1);
    }

    #[test]
    fn latin_and_cyrillic_decode_with_expected_sizes() {
        let Some(d) = konung() else { return };
        let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
        let large = FontTable::parse(&d, LARGE_FONT_OFF).unwrap();
        // Typical 'A' sizes documented by the original-code trace.
        let sa = small.glyph('A').unwrap();
        assert_eq!((sa.width, sa.height), (8, 12));
        let la = large.glyph('A').unwrap();
        assert_eq!((la.width, la.height), (11, 16));
        // CP866 Cyrillic must resolve (А = 0x80).
        assert!(small.glyph('\u{0410}').is_some(), "Cyrillic А decodes");
        // Space is a null glyph -> fallback advance.
        assert!(small.glyph(' ').is_none());
        assert_eq!(small.measure(" "), 4);
    }

    #[test]
    fn widths_are_variable_and_measure_matches_draw() {
        let Some(d) = konung() else { return };
        let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
        let iw = small.glyph('i').map(|g| g.width);
        let mw = small.glyph('M').map(|g| g.width);
        if let (Some(i), Some(m)) = (iw, mw) {
            assert_ne!(i, m, "font must be proportional");
        }
        for s in ["1", "25", "100", "12345"] {
            let measured = small.measure(s);
            let mut buf = vec![0u8; 64 * 32 * 4];
            let drawn = small.draw(&mut buf, 64, 32, s, 0, 0, None);
            assert_eq!(measured, drawn, "measure/draw advance mismatch for {s}");
        }
    }

    #[test]
    fn transparent_skip_does_not_overwrite_destination() {
        let Some(d) = konung() else { return };
        let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
        let mut buf = vec![0xABu8; 64 * 32 * 4];
        small.draw(&mut buf, 64, 32, "1", 0, 0, None);
        // A pixel well to the right of the glyph must be untouched.
        let di = (0 * 64 + 40) * 4;
        assert_eq!(buf[di], 0xAB);
        // At least one glyph pixel must have been written.
        assert!(buf.chunks_exact(4).any(|p| p[3] == 255));
    }
}
