//! `graph` subsystem of [`crate::res`] (split from `res.rs`).

use super::*;

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
    pub fn terrain_tile(&mut self, tile_id: usize) -> Option<Img> {
        let tile_id = tile_id & 0x1FF;
        if let Some(v) = self.terrain_tile_cache.get(&tile_id) {
            return v.clone();
        }
        let value = self.decode_terrain_tile(tile_id);
        self.terrain_tile_cache.insert(tile_id, value.clone());
        value
    }

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
