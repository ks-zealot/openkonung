//! `graph` subsystem of [`crate::res`] (split from `res.rs`).
//!
//! Walks the `GRAPH.RES` block sequence and decodes the terrain tile bank:
//!
//! ```text
//! 0x20000            11 cursor blocks: [u32 tag][u32 size][payload]
//! terrain block      [u32 tag][u32 size]
//!                      table   512 x {u32 offset, u32 palette_offset} (0x1000)
//!                      payload tile data at block + 0x1008
//! 3 human base blocks, then 24 human overlay blocks
//! ```
//!
//! Terrain tiles are indexed `0..=511`; a table entry's offset is relative to
//! the payload start and `0xFFFFFFFF` marks an absent tile.

use super::*;

/// Parse the `GRAPH.RES` block layout used by the rest of the decoder.
///
/// Returns `(terrain_table_pos, terrain_payload_pos, terrain_payload_size,
/// human_base_positions, human_overlay_positions)`. A truncated archive yields
/// whatever prefixes were readable instead of an error, so callers must treat
/// empty position vectors as "no such block".
pub(crate) fn parse_graph_layout(graph: &[u8]) -> (usize, usize, usize, Vec<usize>, Vec<usize>) {
    let mut pos = 0x20000usize;
    // 11 small blocks: [type, u32 size] + payload.
    for _ in 0..11 {
        if pos + 8 > graph.len() {
            break;
        }
        let size = u32(graph, pos + 4) as usize;
        pos += 8 + size;
    }
    let terrain_block_pos = pos;
    let terrain_payload_size = if pos + 8 <= graph.len() {
        u32(graph, pos + 4) as usize
    } else {
        0
    };
    let terrain_table_pos = pos + 8;
    let terrain_payload_pos = pos + 8 + 0x1000;
    pos = terrain_payload_pos + terrain_payload_size;

    let mut human_base_positions = Vec::new();
    for _ in 0..3 {
        if pos + 4 > graph.len() {
            break;
        }
        human_base_positions.push(pos);
        pos += u32(graph, pos + 4) as usize;
    }
    let mut human_overlay_positions = Vec::new();
    for _ in 0..24 {
        if pos + 4 > graph.len() {
            break;
        }
        human_overlay_positions.push(pos);
        pos += u32(graph, pos + 4) as usize;
    }
    let _ = terrain_block_pos;
    (
        terrain_table_pos,
        terrain_payload_pos,
        terrain_payload_size,
        human_base_positions,
        human_overlay_positions,
    )
}

impl Res {
    /// Decoded terrain tile by bank index, cached.
    ///
    /// The id is masked to 9 bits (512 tiles) first, so callers may pass an
    /// unmasked terrain byte; an absent tile yields `None`.
    pub fn terrain_tile(&mut self, tile_id: usize) -> Option<Img> {
        let tile_id = tile_id & 0x1FF;
        if let Some(v) = self.terrain_tile_cache.get(&tile_id) {
            return v.clone();
        }
        let value = self.decode_terrain_tile(tile_id);
        self.terrain_tile_cache.insert(tile_id, value.clone());
        value
    }

    /// Decode one terrain tile straight from `GRAPH.RES` without caching it.
    ///
    /// The 8-byte table entry at `terrain_table_pos + tile_id*8` holds
    /// `{u32 payload offset, u32 palette_offset}`; `0xFFFFFFFF` or an
    /// out-of-range entry is an absent tile. The palette is
    /// `palette_offset / 512`, clamped to the last palette.
    pub(super) fn decode_terrain_tile(&self, tile_id: usize) -> Option<Img> {
        if tile_id >= 512 {
            return None;
        }
        let to = self.terrain_table_pos + tile_id * 8;
        if to + 8 > self.graph.len() {
            return None;
        }
        let off = u32(&self.graph, to) as usize;
        let palette_off = u32(&self.graph, to + 4);
        if off == 0xFFFFFFFF {
            return None;
        }
        let payload = &self.graph[self.terrain_payload_pos
            ..(self.terrain_payload_pos + self.terrain_payload_size).min(self.graph.len())];
        let (idx, alpha, w, h) = decode_indexed_rle(payload, off);
        if w == 0 || h == 0 {
            return None;
        }
        let pid = (palette_off / 512) as usize;
        let pal = &self.palettes[pid.min(255)];
        Some(Rc::new(indexed_to_rgba(&idx, &alpha, w, h, pal)))
    }

    // --- props ---
}
