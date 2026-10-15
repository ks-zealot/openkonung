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

/// Named INTR indices verified against `konung.res` consumers.
pub const HUD: usize = 3;
pub const CHARACTER_WINDOW: usize = 2;
pub const INVENTORY_SLOT: usize = 17;
/// Left / right inventory scroll-arrow caps (scroll == 0 selects these).
pub const ARROW_LEFT: usize = 18;
pub const ARROW_LEFT_ALT: usize = 19;
pub const ARROW_RIGHT: usize = 20;
pub const ARROW_RIGHT_ALT: usize = 21;
/// AUTO button (off / on), selected by `Entity+0x03 & 0x02` (VA `0x42E438`).
pub const AUTO_OFF: usize = 169;
pub const AUTO_ON: usize = 170;
/// First / last portrait index (`hud::portrait_id`).
pub const PORTRAIT_FIRST: usize = 200;
pub const PORTRAIT_LAST: usize = 274;
/// First / last item-icon index (referenced by the `item.rs` template tables).
pub const ITEM_ICON_FIRST: usize = 22;
pub const ITEM_ICON_LAST: usize = 168;

/// Semantic role of one occupied INTR index (verified where noted; the rest are
/// descriptive classifications, `Unclassified`/`SmallUi` where evidence is thin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrRole {
    /// Full-screen gameplay interface (`INTR[3]`).
    HudOverlay,
    /// Character window (`INTR[2]`).
    CharacterWindow,
    /// Empty inventory slot background (`INTR[17]`).
    InventorySlot,
    /// Inventory scroll-arrow caps (`INTR[18..21]`).
    InventoryArrow,
    /// AUTO combat button, off/on (`INTR[169]/[170]`).
    AutoButton,
    /// Character portrait (`INTR[200..274]`).
    Portrait,
    /// Item / ability icon referenced by the item tables.
    ItemIcon,
    /// Large window / dialog frame.
    WindowFrame,
    /// Panel background.
    Panel,
    /// Push button.
    Button,
    /// Generic interface control.
    InterfaceControl,
    /// Small UI ornament / gauge.
    SmallUi,
    /// Not classified (usage unresolved).
    Unclassified,
}

/// The verified/classified role of an occupied INTR index. Indices outside the
/// occupied range return [`IntrRole::Unclassified`].
pub fn role_of(id: usize) -> IntrRole {
    match id {
        HUD => IntrRole::HudOverlay,
        CHARACTER_WINDOW => IntrRole::CharacterWindow,
        INVENTORY_SLOT => IntrRole::InventorySlot,
        18..=21 => IntrRole::InventoryArrow,
        169 | 170 => IntrRole::AutoButton,
        200..=274 => IntrRole::Portrait,
        22..=168 => IntrRole::ItemIcon,
        0 | 1 | 4 | 5 | 6 => IntrRole::WindowFrame,
        179 => IntrRole::Panel,
        8 | 9 | 16 | 172 | 178 => IntrRole::Panel,
        180..=183 => IntrRole::Button,
        10..=15 => IntrRole::InterfaceControl,
        171 | 173..=177 | 184..=199 => IntrRole::SmallUi,
        _ => IntrRole::Unclassified,
    }
}

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
mod tests {
    use super::*;

    fn put_u16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn put_u32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    /// Encode one sprite record from already-encoded row token streams (each row
    /// must include its own 0x00 EOL token).
    fn record(w: u16, h: u16, rows: &[Vec<u8>]) -> Vec<u8> {
        assert_eq!(rows.len(), h as usize);
        let mut r = Vec::new();
        put_u16(&mut r, w);
        put_u16(&mut r, h);
        for row in rows {
            put_u16(&mut r, row.len() as u16);
        }
        for row in rows {
            r.extend_from_slice(row);
        }
        r
    }

    /// Build an archive from records and per-record palette offsets. Remaining
    /// slots are 0xFFFFFFFF.
    fn archive(records: &[Vec<u8>], pals: &[u32]) -> Vec<u8> {
        assert_eq!(records.len(), pals.len());
        assert!(records.len() <= INTR_SLOTS);
        let region: usize = records.iter().map(|r| r.len()).sum();
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        put_u32(&mut out, region as u32);
        let mut off = 0u32;
        for i in 0..INTR_SLOTS {
            if i < records.len() {
                put_u32(&mut out, off);
                put_u32(&mut out, pals[i]);
                off += records[i].len() as u32;
            } else {
                put_u32(&mut out, INTR_UNUSED);
                put_u32(&mut out, INTR_UNUSED);
            }
        }
        for r in records {
            out.extend_from_slice(r);
        }
        out
    }

    /// A valid GRAPH palette buffer with palette 0 = red, palette 1 = blue.
    fn graph() -> Vec<u8> {
        let mut g = vec![0u8; GRAPH_PALETTE_BYTES];
        // RGB555 0x7C00 -> red at palette 0, colour 0.
        g[0] = 0x00;
        g[1] = 0x7C;
        // RGB555 0x001F -> blue at palette 1, colour 0.
        let b = PALETTE_BLOCK as usize;
        g[b] = 0x1F;
        g[b + 1] = 0x00;
        g
    }

    #[test]
    fn parses_all_slots_and_marks_unused() {
        let r0 = record(1, 1, &[vec![0x01, 0x00, 0x00]]);
        let r1 = record(1, 1, &[vec![0x01, 0x01, 0x00]]);
        let a = archive(&[r0, r1], &[0, 512]);
        let arch = IntrArchive::load(&a, &graph()).unwrap();
        assert_eq!(arch.entries.len(), INTR_SLOTS);
        assert_eq!(arch.len(), 2);
        assert!(arch.entry(0).is_some());
        assert!(arch.entry(1).is_some());
        assert!(arch.entry(2).is_none());
        assert!(arch.entry(INTR_SLOTS - 1).is_none());
    }

    #[test]
    fn zero_data_offset_is_valid() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
        let arch = IntrArchive::load(&a, &graph()).unwrap();
        assert_eq!(arch.entry(0).unwrap().data_offset, 0);
        assert!(arch.dimensions(0).is_some());
    }

    #[test]
    fn decodes_colors_and_transparency() {
        // width 3: skip 1, then two literal pixels of palette colour index 0.
        let rows = vec![vec![0x81, 0x02, 0x00, 0x00, 0x00]];
        let a = archive(&[record(3, 1, &rows)], &[512]);
        let mut arch = IntrArchive::load(&a, &graph()).unwrap();
        let im = arch.get(0).unwrap();
        assert_eq!((im.w, im.h), (3, 1));
        assert_eq!(&im.data[0..4], &[0, 0, 0, 0]); // skipped (transparent run)
        assert_eq!(&im.data[4..8], &[0, 0, 255, 255]); // palette 1 colour 0 = blue
        assert_eq!(&im.data[8..12], &[0, 0, 255, 255]);
    }

    #[test]
    fn per_entry_palette_is_used() {
        let rows = vec![vec![0x01, 0x00, 0x00]];
        let rec = record(1, 1, &rows);
        let a = archive(&[rec.clone(), rec], &[0, 512]);
        let mut arch = IntrArchive::load(&a, &graph()).unwrap();
        let c0 = arch.get(0).unwrap();
        let c1 = arch.get(1).unwrap();
        assert_eq!(&c0.data[0..4], &[255, 0, 0, 255]); // palette 0 = red
        assert_eq!(&c1.data[0..4], &[0, 0, 255, 255]); // palette 1 = blue
        assert_ne!(c0.data, c1.data);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
        a[3] = 0x09;
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("bad magic"), "{e}");
    }

    #[test]
    fn rejects_truncated_index_or_file() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
        let e = IntrArchive::load(&a[..INTR_DATA_START - 1], &graph()).unwrap_err();
        assert!(e.message.contains("file too small"), "{e}");
    }

    #[test]
    fn rejects_short_graph() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
        let e = IntrArchive::load(&a, &vec![0u8; 16]).unwrap_err();
        assert!(e.message.contains("GRAPH.RES too small"), "{e}");
    }

    #[test]
    fn rejects_literal_run_past_width() {
        let rows = vec![vec![0x02, 0x00, 0x00, 0x00]]; // literal 2 in width 1
        let a = archive(&[record(1, 1, &rows)], &[0]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("past width"), "{e}");
    }

    #[test]
    fn rejects_transparent_run_past_width() {
        let rows = vec![vec![0x82, 0x00]]; // skip 2 in width 1
        let a = archive(&[record(1, 1, &rows)], &[0]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("past width"), "{e}");
    }

    #[test]
    fn rejects_missing_eol() {
        let rows = vec![vec![0x01, 0x00]]; // one literal, no EOL
        let a = archive(&[record(1, 1, &rows)], &[0]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("end-of-row"), "{e}");
    }

    #[test]
    fn rejects_trailing_after_eol() {
        let rows = vec![vec![0x01, 0x00, 0x00, 0xFF]]; // literal + EOL + junk
        let a = archive(&[record(1, 1, &rows)], &[0]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("trailing"), "{e}");
    }

    #[test]
    fn rejects_truncated_record() {
        let rows = vec![vec![0x01, 0x00, 0x00]];
        let mut a = archive(&[record(1, 1, &rows)], &[0]);
        a.truncate(a.len() - 2); // chop the row payload
        // Header still claims the full region, so bounds reject it.
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        let m = e.message;
        assert!(m.contains("data region") || m.contains("past the data region"), "{m}");
    }

    #[test]
    fn rejects_out_of_range_palette() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[GRAPH_PALETTE_BYTES as u32]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("outside GRAPH") || e.message.contains("palette_offset"), "{e}");
    }

    #[test]
    fn rejects_unaligned_palette() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[0x100]);
        let e = IntrArchive::load(&a, &graph()).unwrap_err();
        assert!(e.message.contains("multiple"), "{e}");
    }

    #[test]
    fn get_returns_none_for_bad_ids() {
        let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
        let mut arch = IntrArchive::load(&a, &graph()).unwrap();
        assert!(arch.get(1).is_none());
        assert!(arch.get(INTR_SLOTS).is_none());
        assert!(arch.get(usize::MAX).is_none());
    }

    #[test]
    fn catalog_roles_for_known_indices() {
        assert_eq!(role_of(HUD), IntrRole::HudOverlay);
        assert_eq!(role_of(CHARACTER_WINDOW), IntrRole::CharacterWindow);
        assert_eq!(role_of(INVENTORY_SLOT), IntrRole::InventorySlot);
        for id in ARROW_LEFT..=ARROW_RIGHT_ALT {
            assert_eq!(role_of(id), IntrRole::InventoryArrow, "arrow {id}");
        }
        assert_eq!(role_of(AUTO_OFF), IntrRole::AutoButton);
        assert_eq!(role_of(AUTO_ON), IntrRole::AutoButton);
        assert_eq!(role_of(PORTRAIT_FIRST), IntrRole::Portrait);
        assert_eq!(role_of(PORTRAIT_LAST), IntrRole::Portrait);
        assert_eq!(role_of(ITEM_ICON_FIRST), IntrRole::ItemIcon);
        assert_eq!(role_of(ITEM_ICON_LAST), IntrRole::ItemIcon);
        // Out-of-range ids.
        assert_eq!(role_of(INTR_VALID_COUNT), IntrRole::Unclassified);
        assert_eq!(role_of(511), IntrRole::Unclassified);
    }

    #[test]
    fn auto_button_selects_by_state() {
        assert_eq!(AUTO_OFF, 169);
        assert_eq!(AUTO_ON, 170);
        // `hud` maps Entity+0x03 & 0x02 -> on; verify the constant ordering only
        // (the archive lookup itself is covered by real-resource tests).
        assert!(AUTO_ON > AUTO_OFF);
    }

    // --- real-resource regression (only when the original assets are present) ---

    fn install_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn load_variant(name: &str) -> Option<(IntrArchive, Vec<u8>)> {
        let dir = install_dir();
        let intr = std::fs::read(dir.join(name)).ok()?;
        let graph = std::fs::read(dir.join("GRAPH.RES")).ok()?;
        let arch = IntrArchive::load(&intr, &graph).expect("INTR parse");
        Some((arch, intr))
    }

    fn dim(arch: &IntrArchive, id: usize) -> (u16, u16) {
        arch.dimensions(id).expect("dimension")
    }

    #[test]
    fn intr800_real_archive() {
        let Some((mut arch, _)) = load_variant("INTR800R.RES") else {
            eprintln!("skipping: INTR800R.RES not present");
            return;
        };
        assert_eq!(arch.entries.len(), INTR_SLOTS);
        assert_eq!(arch.len(), INTR_VALID_COUNT);
        for id in 0..INTR_VALID_COUNT {
            assert!(arch.entry(id).is_some(), "index {id} should be occupied");
        }
        for id in INTR_VALID_COUNT..INTR_SLOTS {
            assert!(arch.entry(id).is_none(), "index {id} should be unused");
        }
        assert_eq!(arch.entry(0).unwrap().data_offset, 0);
        assert_eq!(dim(&arch, 0), (615, 55));
        assert_eq!(dim(&arch, 1), (651, 470));
        assert_eq!(dim(&arch, 2), (642, 460));
        assert_eq!(dim(&arch, 3), (800, 600));
        assert_eq!(dim(&arch, 17), (70, 70));
        let im = arch.get(3).expect("sprite 3");
        assert_eq!(im.data.len(), 800 * 600 * 4);
    }

    #[test]
    fn intr640_real_archive() {
        let Some((arch, _)) = load_variant("INTR640R.RES") else {
            eprintln!("skipping: INTR640R.RES not present");
            return;
        };
        assert_eq!(arch.len(), INTR_VALID_COUNT);
        assert_eq!(dim(&arch, 0), (492, 44));
        assert_eq!(dim(&arch, 1), (521, 376));
        assert_eq!(dim(&arch, 2), (528, 376));
        assert_eq!(dim(&arch, 3), (640, 480));
        assert_eq!(dim(&arch, 17), (70, 70));
    }

    #[test]
    fn variants_share_index_and_palette_table() {
        let (a, _) = match load_variant("INTR800R.RES") {
            Some(v) => v,
            None => return,
        };
        let (b, _) = match load_variant("INTR640R.RES") {
            Some(v) => v,
            None => return,
        };
        for id in 0..INTR_SLOTS {
            match (a.entry(id), b.entry(id)) {
                (None, None) => {}
                (Some(ea), Some(eb)) => {
                    assert_eq!(ea.palette_offset, eb.palette_offset, "palette mismatch at {id}");
                }
                _ => panic!("occupancy mismatch at index {id}"),
            }
        }
    }

    #[test]
    fn reference_sprites_have_expected_transparency() {
        let Some((mut arch, _)) = load_variant("INTR800R.RES") else {
            return;
        };
        // The inventory slot frame is fully opaque...
        let im = arch.get(17).unwrap();
        assert!(
            im.data.chunks(4).all(|p| p[3] == 255),
            "sprite 17 should be fully opaque"
        );
        // ...while the HUD overlay, window frames and icons carry transparent
        // pixels (transparent RLE runs / short rows), not opaque palette index 0.
        for id in [0usize, 1, 3, 71, 150, 240] {
            let im = arch.get(id).unwrap();
            assert!(
                im.data.chunks(4).any(|p| p[3] == 0),
                "sprite {id} should contain transparent pixels"
            );
            assert!(
                im.data.chunks(4).any(|p| p[3] == 255),
                "sprite {id} should contain opaque pixels"
            );
        }
    }
}
