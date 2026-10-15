//! Native decoder for the original Konung **INTR** interface-image archives.
//!
//! Handles `INTR{width}R.RES` (`INTR640R.RES`, `INTR800R.RES`). This is the
//! archive family used by the original in-game graphical interface (party
//! panel, character window, inventory slots, equipment/portrait icons). It is
//! *not* the indexed GRAPH/OBJECTS grammar in `res.rs`, although each sprite
//! payload uses the exact same row-based indexed RLE grammar and the same
//! GRAPH palettes.
//!
//! Recovered layout (**BINARY-LAYOUT VERIFIED** against `INTR640R.RES` and
//! `INTR800R.RES`):
//!
//! ```text
//! u8  magic[4]        = 00 00 00 01
//! u32 data_region_size
//! Entry index_table[512]        // 8 bytes each, little-endian
//!     u32 data_offset           // relative to INTR_DATA_START
//!     u32 palette_offset        // byte offset into GRAPH.RES palette data
//! data_region[...]              // 275 sprite records (0..=274)
//! ```
//!
//! Each occupied index (0..=274) points at a sprite record:
//!
//! ```text
//! u16 width
//! u16 height
//! u16 row_size[height]
//! u8  row_data[sum(row_size)]
//! ```
//!
//! Row data uses the same token grammar as [`crate::res::decode_indexed_rle`]:
//! `0x00` ends the row, `0x80 | n` skips `n` transparent pixels, and a literal
//! token `n` is followed by `n` palette-index bytes. `palette_offset` selects a
//! 256-colour block in `GRAPH.RES` (`palette id = palette_offset / 512`); the
//! palette is **per entry**, never global.
//!
//! Unused index slots are `0xFFFFFFFF`. Sprite data is decoded lazily on first
//! [`IntrArchive::get`] and cached as [`Img`] (`Rc<Image>`), matching the rest of
//! the resource layer. `load` performs a full structural validation pass over
//! every occupied record (bounds, end-of-row, width) without allocating pixels.

use std::collections::HashMap;
use std::rc::Rc;

use crate::res::{indexed_to_rgba, palette_rgb, Img};

mod roles;

pub use self::roles::*;

#[inline]
fn rd_u16(b: &[u8], o: usize) -> u16 {
    (b[o] as u16) | ((b[o + 1] as u16) << 8)
}

/// 8-byte archive header.
pub const INTR_HEADER_SIZE: usize = 8;
/// Index table size (512 entries x 8 bytes).
pub const INTR_INDEX_SIZE: usize = 0x1000;
/// First byte of the graphical data region.
pub const INTR_DATA_START: usize = INTR_HEADER_SIZE + INTR_INDEX_SIZE;
/// Number of index slots.
pub const INTR_SLOTS: usize = 512;
/// Number of occupied sprite indices in both original variants.
pub const INTR_VALID_COUNT: usize = 275;
/// Index table marker for an unused slot.
pub const INTR_UNUSED: u32 = 0xFFFF_FFFF;
/// Bytes per palette block (256 colours x 2 bytes).
pub const PALETTE_BLOCK: u32 = 512;
/// Bytes of palette data at the start of `GRAPH.RES`.
pub const GRAPH_PALETTE_BYTES: usize = 0x2_0000;

const MAGIC: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
/// Upper bound guarding against absurd allocations from a corrupt header.
const MAX_DIM: u16 = 8192;

// ---------------------------------------------------------------------------
// Verified INTR index catalog (source of truth: konung.res consumers + RES dims)
// ---------------------------------------------------------------------------

/// One raw index-table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrEntry {
    /// Offset relative to [`INTR_DATA_START`].
    pub data_offset: u32,
    /// Byte offset into the `GRAPH.RES` palette data.
    pub palette_offset: u32,
}

impl IntrEntry {
    /// Palette id (`palette_offset / 512`).
    pub fn palette_id(&self) -> usize {
        (self.palette_offset / PALETTE_BLOCK) as usize
    }
}

/// A malformed archive or sprite record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrError {
    pub message: String,
}

impl std::fmt::Display for IntrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "INTR: {}", self.message)
    }
}

impl std::error::Error for IntrError {}

impl IntrError {
    fn new(message: impl Into<String>) -> Self {
        IntrError { message: message.into() }
    }
}

/// A decoded, palette-resolved interface archive. Sprite buffers are `Rc`-shared
/// and cached lazily; callers clone the [`Img`] handle cheaply.
pub struct IntrArchive {
    entries: Vec<Option<IntrEntry>>,
    data: Vec<u8>,
    palettes: Vec<[[u8; 3]; 256]>,
    cache: HashMap<usize, Img>,
}

impl std::fmt::Debug for IntrArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "IntrArchive {{ occupied: {}, cached: {} }}",
            self.len(),
            self.cache.len()
        )
    }
}

impl IntrArchive {
    /// Parse and validate an `INTR{width}R.RES` archive against its `GRAPH.RES`.
    ///
    /// Every occupied record is bounds-checked and its RLE token stream walked
    /// (without allocating pixels), so a malformed stream is rejected here.
    pub fn load(intr_data: &[u8], graph_data: &[u8]) -> Result<Self, IntrError> {
        if intr_data.len() < INTR_DATA_START {
            return Err(IntrError::new(format!(
                "file too small ({} bytes, need at least {})",
                intr_data.len(),
                INTR_DATA_START
            )));
        }
        if intr_data[0..4] != MAGIC {
            return Err(IntrError::new(format!(
                "bad magic {:02X} {:02X} {:02X} {:02X}, expected 00 00 00 01",
                intr_data[0], intr_data[1], intr_data[2], intr_data[3]
            )));
        }
        if graph_data.len() < GRAPH_PALETTE_BYTES {
            return Err(IntrError::new(format!(
                "GRAPH.RES too small for palette data ({} bytes, need {})",
                graph_data.len(),
                GRAPH_PALETTE_BYTES
            )));
        }
        let data_region_size = crate::res::u32(intr_data, 4) as usize;
        let region_end = INTR_DATA_START
            .checked_add(data_region_size)
            .ok_or_else(|| IntrError::new("data region size overflow"))?;
        if region_end > intr_data.len() {
            return Err(IntrError::new(format!(
                "data region ends at 0x{region_end:X} but file is {} bytes",
                intr_data.len()
            )));
        }

        let mut entries: Vec<Option<IntrEntry>> = Vec::with_capacity(INTR_SLOTS);
        for i in 0..INTR_SLOTS {
            let o = INTR_HEADER_SIZE + i * 8;
            let data_offset = crate::res::u32(intr_data, o);
            let palette_offset = crate::res::u32(intr_data, o + 4);
            let entry = if data_offset == INTR_UNUSED || palette_offset == INTR_UNUSED {
                None
            } else {
                if palette_offset % PALETTE_BLOCK != 0 {
                    return Err(IntrError::new(format!(
                        "index {i}: palette_offset 0x{palette_offset:X} is not a multiple of {PALETTE_BLOCK}"
                    )));
                }
                if palette_offset as usize >= GRAPH_PALETTE_BYTES {
                    return Err(IntrError::new(format!(
                        "index {i}: palette_offset 0x{palette_offset:X} selects palette {} outside GRAPH (max {})",
                        palette_offset / PALETTE_BLOCK,
                        GRAPH_PALETTE_BYTES / PALETTE_BLOCK as usize - 1
                    )));
                }
                let abs = INTR_DATA_START + data_offset as usize;
                if abs >= region_end {
                    return Err(IntrError::new(format!(
                        "index {i}: data_offset 0x{data_offset:X} at 0x{abs:X} outside data region (end 0x{region_end:X})"
                    )));
                }
                Some(IntrEntry { data_offset, palette_offset })
            };
            entries.push(entry);
        }

        for (id, e) in entries.iter().enumerate() {
            if let Some(e) = e {
                validate_record(id, intr_data, region_end, e.data_offset as usize)?;
            }
        }

        let palettes: Vec<[[u8; 3]; 256]> = (0..256).map(|i| palette_rgb(graph_data, i)).collect();

        Ok(IntrArchive {
            entries,
            data: intr_data.to_vec(),
            palettes,
            cache: HashMap::new(),
        })
    }

    /// Number of occupied sprite indices.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.is_none())
    }

    /// Raw index-table entry for `id`, or `None` if the slot is unused.
    pub fn entry(&self, id: usize) -> Option<IntrEntry> {
        self.entries.get(id).copied().flatten()
    }

    /// Decoded dimensions of `id` without allocating pixel data.
    pub fn dimensions(&self, id: usize) -> Option<(u16, u16)> {
        let e = self.entry(id)?;
        let abs = INTR_DATA_START + e.data_offset as usize;
        if abs + 4 > self.data.len() {
            return None;
        }
        Some((rd_u16(&self.data, abs), rd_u16(&self.data, abs + 2)))
    }

    /// Decode (or return the cached) sprite `id` as an `Rc<Image>`.
    ///
    /// Returns `None` for unused/out-of-range indices or a record that cannot be
    /// decoded (validated at load, so this should not happen for real data).
    pub fn get(&mut self, id: usize) -> Option<Img> {
        if let Some(v) = self.cache.get(&id) {
            return Some(v.clone());
        }
        let e = self.entry(id)?;
        let payload = &self.data[INTR_DATA_START..];
        let (idx, alpha, w, h) = crate::res::decode_indexed_rle(payload, e.data_offset as usize);
        if w == 0 || h == 0 {
            return None;
        }
        let pal = &self.palettes[e.palette_id().min(255)];
        let img = Rc::new(indexed_to_rgba(&idx, &alpha, w, h, pal));
        self.cache.insert(id, img.clone());
        Some(img)
    }

    /// Semantic role of `id` (see [`role_of`]).
    pub fn role(&self, id: usize) -> IntrRole {
        role_of(id)
    }

    /// The AUTO combat button sprite for the given state
    /// (`INTR[170]` when `on`, else `INTR[169]`).
    pub fn auto_button(&mut self, on: bool) -> Option<Img> {
        self.get(if on { AUTO_ON } else { AUTO_OFF })
    }
}

/// Walk one record's token stream, validating bounds without decoding pixels.
fn validate_record(
    id: usize,
    data: &[u8],
    region_end: usize,
    data_offset: usize,
) -> Result<(), IntrError> {
    let abs = INTR_DATA_START + data_offset;
    if abs + 4 > region_end {
        return Err(IntrError::new(format!(
            "index {id}: record header at 0x{abs:X} outside data region"
        )));
    }
    let w = rd_u16(data, abs);
    let h = rd_u16(data, abs + 2);
    if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
        return Err(IntrError::new(format!(
            "index {id}: implausible dimensions {w}x{h}"
        )));
    }
    let sizes_at = abs + 4;
    let table_end = sizes_at
        .checked_add(2 * h as usize)
        .ok_or_else(|| IntrError::new(format!("index {id}: row table overflow")))?;
    if table_end > region_end {
        return Err(IntrError::new(format!(
            "index {id}: row table of {h} rows does not fit the data region"
        )));
    }
    let mut p = table_end;
    for y in 0..h as usize {
        let row_size = rd_u16(data, sizes_at + y * 2) as usize;
        let row_end = p
            .checked_add(row_size)
            .ok_or_else(|| IntrError::new(format!("index {id}: row {y} size overflow")))?;
        if row_end > region_end {
            return Err(IntrError::new(format!(
                "index {id}: row {y} of {row_size} bytes extends past the data region"
            )));
        }
        validate_row(id, y, w as usize, &data[p..row_end])?;
        p = row_end;
    }
    Ok(())
}

/// Validate one row's RLE tokens: EOL present, no trailing bytes, never past width.
fn validate_row(id: usize, y: usize, width: usize, row: &[u8]) -> Result<(), IntrError> {
    let mut x = 0usize;
    let mut q = 0usize;
    let mut saw_eol = false;
    while q < row.len() {
        let cmd = row[q];
        q += 1;
        if cmd == 0 {
            saw_eol = true;
            break;
        }
        if cmd & 0x80 != 0 {
            x += (cmd & 0x7F) as usize;
            if x > width {
                return Err(IntrError::new(format!(
                    "index {id}: row {y} transparent run past width {width}"
                )));
            }
        } else {
            let n = cmd as usize;
            if q + n > row.len() {
                return Err(IntrError::new(format!(
                    "index {id}: row {y} literal run of {n} needs {n} bytes but only {} remain",
                    row.len() - q
                )));
            }
            x += n;
            if x > width {
                return Err(IntrError::new(format!(
                    "index {id}: row {y} literal run past width {width}"
                )));
            }
            q += n;
        }
    }
    if !saw_eol {
        return Err(IntrError::new(format!("index {id}: row {y} is missing its 0x00 end-of-row token")));
    }
    if q != row.len() {
        return Err(IntrError::new(format!(
            "index {id}: row {y} has {} trailing byte(s) after end-of-row",
            row.len() - q
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
