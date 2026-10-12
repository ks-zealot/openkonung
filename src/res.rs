//! Original Konung 1.03 resource decoders.
//!
//! Faithful port of `konung_world_viewer_legacy_tk.py::KonungResources`.
//! Keeps the exact recovered binary layout and image grammar.

use std::collections::HashMap;
use std::rc::Rc;

pub const MAP_COUNT: usize = 20;
pub const MAP_SIZE: usize = 0x2F120;
pub const MAP_W: usize = 160;
pub const MAP_H: usize = 80;

pub const GRID_X_STEP: i32 = 58;
pub const GRID_Y_STEP: i32 = 16;
pub const GRID_ODD_SHIFT: i32 = 29;
pub const TERRAIN_TILE_W: i32 = 114;
pub const TERRAIN_TILE_H: i32 = 64;
pub const MAP_ORIGIN_X: i32 = 29;
pub const MAP_PIXEL_W: i32 = MAP_ORIGIN_X + (MAP_W as i32 - 1) * GRID_X_STEP + TERRAIN_TILE_W;
pub const MAP_PIXEL_H: i32 = (MAP_H as i32 - 1) * GRID_Y_STEP + TERRAIN_TILE_H;

pub const GAME_ITEMS_OFF: usize = 0x00000;
pub const GAME_VARIANTS_OFF: usize = 0x4279A;
pub const GAME_VARIANT_SIZE: usize = 256;
pub const GAME_VARIANT_COUNT: usize = 320;

pub const HUMAN_DRAW_ORDER: [[usize; 6]; 8] = [
    [3, 2, 4, 1, 0, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 5, 1],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 4, 5, 1, 0],
];

pub fn u16(b: &[u8], o: usize) -> u32 {
    (b[o] as u32) | ((b[o + 1] as u32) << 8)
}
pub fn i16(b: &[u8], o: usize) -> i32 {
    u16(b, o) as u16 as i16 as i32
}
pub fn u32(b: &[u8], o: usize) -> u32 {
    (b[o] as u32)
        | ((b[o + 1] as u32) << 8)
        | ((b[o + 2] as u32) << 16)
        | ((b[o + 3] as u32) << 24)
}
pub fn i32v(b: &[u8], o: usize) -> i32 {
    u32(b, o) as i32
}

pub fn cell_to_screen(cx: i32, cy: i32) -> (i32, i32) {
    (
        MAP_ORIGIN_X + cx * GRID_X_STEP - (cy & 1) * GRID_ODD_SHIFT,
        cy * GRID_Y_STEP,
    )
}

#[derive(Clone)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>, // RGBA
}

impl Image {
    pub fn new(w: usize, h: usize, fill: [u8; 4]) -> Image {
        let mut data = vec![0u8; w * h * 4];
        if fill[3] != 0 || fill[0] != 0 || fill[1] != 0 || fill[2] != 0 {
            let mut i = 0;
            while i < data.len() {
                data[i..i + 4].copy_from_slice(&fill);
                i += 4;
            }
        }
        Image { w, h, data }
    }
}

pub type Img = Rc<Image>;

pub fn palette_rgb(graph: &[u8], palette_id: usize) -> [[u8; 3]; 256] {
    let mut out = [[0u8; 3]; 256];
    let base = palette_id * 512;
    if base + 512 > graph.len() {
        return out;
    }
    for i in 0..256 {
        let v = u16(graph, base + i * 2);
        let b = (v & 31) * 255 / 31;
        let g = ((v >> 5) & 31) * 255 / 31;
        let r = ((v >> 10) & 31) * 255 / 31;
        out[i] = [r as u8, g as u8, b as u8];
    }
    out
}

/// Decode the Konung indexed-image grammar into index + alpha arrays.
pub fn decode_indexed_rle(payload: &[u8], off: usize) -> (Vec<u8>, Vec<u8>, usize, usize) {
    if off + 4 > payload.len() {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let w = u16(payload, off) as usize;
    let h = u16(payload, off + 2) as usize;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let mut idx = vec![0u8; w * h];
    let mut alpha = vec![0u8; w * h];
    // Row sizes live in a table immediately after the header; the row payloads
    // themselves are concatenated with no inline size prefix.
    let sizes_at = off + 4;
    if sizes_at + 2 * h > payload.len() {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let mut p = off + 4 + 2 * h;
    for y in 0..h {
        let row_size = u16(payload, sizes_at + y * 2) as usize;
        if p + row_size > payload.len() {
            break;
        }
        let row = &payload[p..p + row_size];
        p += row_size;
        let mut x = 0usize;
        let mut q = 0usize;
        while q < row.len() {
            let cmd = row[q];
            q += 1;
            if cmd == 0 {
                break;
            }
            if cmd & 0x80 != 0 {
                x += (cmd & 0x7F) as usize;
            } else {
                let n = cmd as usize;
                if q + n > row.len() {
                    break;
                }
                let mut take = n;
                if x + take > w {
                    take = w.saturating_sub(x);
                }
                for k in 0..take {
                    let di = y * w + x + k;
                    idx[di] = row[q + k];
                    alpha[di] = 255;
                }
                q += n;
                x += n;
            }
        }
    }
    (idx, alpha, w, h)
}

pub fn indexed_to_rgba(idx: &[u8], alpha: &[u8], w: usize, h: usize, pal: &[[u8; 3]; 256]) -> Image {
    let mut data = vec![0u8; w * h * 4];
    for i in 0..w * h {
        if alpha[i] > 0 {
            let c = pal[idx[i] as usize];
            data[i * 4] = c[0];
            data[i * 4 + 1] = c[1];
            data[i * 4 + 2] = c[2];
            data[i * 4 + 3] = 255;
        }
    }
    Image { w, h, data }
}

#[derive(Clone, Copy)]
pub struct ExeProfile {
    pub name: &'static str,
    pub dgroup_va: u32,
    pub dgroup_raw: u32,
    pub item_template_va: u32,
    pub overlay_palette_va: u32,
}

pub fn exe_profile(len: usize) -> Option<ExeProfile> {
    match len {
        3796992 => Some(ExeProfile {
            name: "LAN_KONG.EXE 2003",
            dgroup_va: 0x470000,
            dgroup_raw: 0x54600,
            item_template_va: 0x48089C,
            overlay_palette_va: 0x478DD4,
        }),
        939008 => Some(ExeProfile {
            name: "CLIENT.EXE 1999 v1.03",
            dgroup_va: 0x440000,
            dgroup_raw: 0x29C00,
            item_template_va: 0x44FEC0,
            overlay_palette_va: 0x44885C,
        }),
        _ => None,
    }
}

pub fn exe_va_to_off(va: u32, p: &ExeProfile) -> usize {
    (p.dgroup_raw + (va - p.dgroup_va)) as usize
}

// --- human animation helpers (free functions over raw graph bytes) ---

pub fn human_frame_count(graph: &[u8], block_pos: usize, action: usize, direction: usize) -> usize {
    let o = block_pos + action * 0xD0 + direction * 0x1A + 0x08;
    if o + 2 > graph.len() {
        return 0;
    }
    u16(graph, o) as usize
}

pub fn human_mapped_frame(
    graph: &[u8],
    block_pos: usize,
    action: usize,
    direction: usize,
    frame_index: usize,
) -> (usize, usize) {
    let mut count = human_frame_count(graph, block_pos, action, direction);
    if count == 0 {
        count = 1;
    }
    let fi = frame_index % count;
    let o = block_pos + action * 0xD0 + direction * 0x1A + fi * 2 + 0x0A;
    if o + 2 > graph.len() {
        return (0, count);
    }
    (u16(graph, o) as usize, count)
}

pub fn render_graph_4d28_frame(
    graph: &[u8],
    block_pos: usize,
    palette: &[[u8; 3]; 256],
    frame_id: usize,
) -> Image {
    if block_pos + 4 > graph.len() {
        return Image::new(1, 1, [0, 0, 0, 0]);
    }
    let total = u32(graph, block_pos + 4) as usize;
    let start = block_pos + 0x4D28;
    let end = (block_pos + total).min(graph.len());
    let payload = if start < end { &graph[start..end] } else { &[][..] };
    let go = block_pos + 0x1728 + frame_id * 8;
    let sprite_off = if go + 8 <= graph.len() {
        u32(graph, go + 4) as usize
    } else {
        0
    };
    let (idx, alpha, w, h) = decode_indexed_rle(payload, sprite_off);
    indexed_to_rgba(&idx, &alpha, w, h, palette)
}

pub fn equipment_overlay_selector(
    game: &[u8],
    exe: &[u8],
    profile: &ExeProfile,
    handle: u32,
) -> Option<i32> {
    if handle == 0 {
        return None;
    }
    let o = GAME_ITEMS_OFF + handle as usize * 16;
    if o + 16 > game.len() {
        return None;
    }
    let ir = &game[o..o + 16];
    let runtime_type = ir[0];
    let subtype = ir[3];
    if runtime_type > 5 {
        return None;
    }
    let template_id = 14 + subtype as usize;
    let off = exe_va_to_off(profile.item_template_va, profile) + template_id * 64;
    if off + 0x40 > exe.len() {
        return None;
    }
    Some(i16(exe, off + 0x3E))
}

// --- resource container ---

pub struct Res {
    pub game: Vec<u8>,
    pub graph: Vec<u8>,
    pub objects: Vec<u8>,
    pub exe: Vec<u8>,
    pub profile: ExeProfile,
    pub palettes: Vec<[[u8; 3]; 256]>,

    pub terrain_table_pos: usize,
    pub terrain_payload_pos: usize,
    pub terrain_payload_size: usize,

    pub human_base_positions: Vec<usize>,
    pub human_overlay_positions: Vec<usize>,
    pub object_entries: Vec<Option<(usize, usize)>>,
    pub overlay_palette_offsets: Vec<u32>,

    terrain_tile_cache: HashMap<usize, Option<Img>>,
    prop_sprite_cache: HashMap<(usize, u32), (Img, i32, i32)>,
    monster_frame_cache: HashMap<(usize, u32, usize), (Img, i32, i32, usize)>,
    npc_frame_cache: HashMap<(i32, i32, i32, i32, i32), (Img, i32, i32, i32)>,
}

impl Res {
    pub fn new(game: Vec<u8>, graph: Vec<u8>, objects: Vec<u8>, exe: Vec<u8>) -> Res {
        let profile = exe_profile(exe.len())
            .unwrap_or_else(|| panic!("unsupported executable size {}", exe.len()));
        let palettes: Vec<[[u8; 3]; 256]> = (0..256).map(|i| palette_rgb(&graph, i)).collect();

        let (terrain_table_pos, terrain_payload_pos, terrain_payload_size, human_base_positions, human_overlay_positions) =
            parse_graph_layout(&graph);

        let object_entries = parse_object_index(&objects);

        let pal_off = exe_va_to_off(profile.overlay_palette_va, &profile);
        let mut overlay_palette_offsets = Vec::with_capacity(24);
        for i in 0..24 {
            let o = pal_off + i * 4;
            overlay_palette_offsets.push(if o + 4 <= exe.len() { u32(&exe, o) } else { 0 });
        }

        Res {
            game,
            graph,
            objects,
            exe,
            profile,
            palettes,
            terrain_table_pos,
            terrain_payload_pos,
            terrain_payload_size,
            human_base_positions,
            human_overlay_positions,
            object_entries,
            overlay_palette_offsets,
            terrain_tile_cache: HashMap::new(),
            prop_sprite_cache: HashMap::new(),
            monster_frame_cache: HashMap::new(),
            npc_frame_cache: HashMap::new(),
        }
    }

    pub fn variant(&self, variant_id: usize) -> &[u8] {
        let o = GAME_VARIANTS_OFF + variant_id * GAME_VARIANT_SIZE;
        &self.game[o..o + GAME_VARIANT_SIZE]
    }

    pub fn object_block(&self, object_id: usize) -> &[u8] {
        match self.object_entries.get(object_id).and_then(|x| *x) {
            Some((off, size)) if off + size <= self.objects.len() => {
                &self.objects[off..off + size]
            }
            _ => &[],
        }
    }

    // --- terrain ---

    pub fn terrain_tile(&mut self, tile_id: usize) -> Option<Img> {
        let tile_id = tile_id & 0x1FF;
        if let Some(v) = self.terrain_tile_cache.get(&tile_id) {
            return v.clone();
        }
        let value = self.decode_terrain_tile(tile_id);
        self.terrain_tile_cache.insert(tile_id, value.clone());
        value
    }

    fn decode_terrain_tile(&self, tile_id: usize) -> Option<Img> {
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
        let payload = &self.graph
            [self.terrain_payload_pos..(self.terrain_payload_pos + self.terrain_payload_size).min(self.graph.len())];
        let (idx, alpha, w, h) = decode_indexed_rle(payload, off);
        if w == 0 || h == 0 {
            return None;
        }
        let pid = (palette_off / 512) as usize;
        let pal = &self.palettes[pid.min(255)];
        Some(Rc::new(indexed_to_rgba(&idx, &alpha, w, h, pal)))
    }

    // --- props ---

    pub fn prop_sprite(&mut self, object_id: usize, frame: usize, palette_override: u32) -> (Img, i32, i32) {
        let key = (object_id, palette_override);
        if let Some(v) = self.prop_sprite_cache.get(&key) {
            return v.clone();
        }
        let value = self.decode_prop_sprite(object_id, frame, palette_override);
        self.prop_sprite_cache.insert(key, value.clone());
        value
    }

    fn decode_prop_sprite(&self, object_id: usize, frame: usize, palette_override: u32) -> (Img, i32, i32) {
        let blk = self.object_block(object_id);
        if blk.len() < 0xC8 {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0);
        }
        let frame_count = (u32(blk, 0x0C) as usize).max(1);
        let frame = frame % frame_count;
        let sprite_off = if 0x80 + frame * 8 + 8 <= blk.len() {
            u32(blk, 0x80 + frame * 8 + 4) as usize
        } else {
            0
        };
        let palette_off = if palette_override != 0 {
            palette_override
        } else {
            u32(blk, 0x14)
        };
        let pid = (palette_off / 512) as usize;
        let payload = &blk[0x100..];
        let (idx, alpha, w, h) = decode_indexed_rle(payload, sprite_off);
        let pal = &self.palettes[pid.min(255)];
        let im = indexed_to_rgba(&idx, &alpha, w, h, pal);
        let go = 0xC0 + frame * 8;
        let (gx, gy) = if go + 8 <= blk.len() {
            (i16(blk, go + 4), i16(blk, go + 6))
        } else {
            (0, 0)
        };
        (Rc::new(im), gx, gy)
    }

    // --- monsters ---

    pub fn monster_animation0_sequence(&self, object_id: usize) -> Vec<u16> {
        let blk = self.object_block(object_id);
        if blk.len() < 0x2E {
            return vec![0];
        }
        let count = u16(blk, 0x08) as usize;
        if count == 0 || count > 18 {
            return vec![0];
        }
        (0..count).map(|i| u16(blk, 0x0A + i * 2) as u16).collect()
    }

    pub fn monster_frame(&mut self, variant_id: usize, anim_frame_index: usize) -> (Img, i32, i32, usize) {
        let vr = self.variant(variant_id).to_vec();
        let object_id = vr[0x3F] as usize;
        let palette_off = u32(&vr, 0x41);
        let pid = (palette_off / 512) as usize;
        self.monster_frame_from_object(object_id, pid, anim_frame_index)
    }

    /// Render a monster animation-0 frame directly from a Game.N entity's
    /// `visual_index`/object id and its `palette_offset` (D8, / 512 = palette
    /// id). No Game.res variant lookup is involved. Retained for the canonical
    /// population/visual phase (MD §10, §12).
    pub fn monster_frame_from_object(
        &mut self,
        object_id: usize,
        palette_id: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, usize) {
        let seq = self.monster_animation0_sequence(object_id);
        let frame_id = seq[anim_frame_index % seq.len()] as usize;
        let key = (object_id, palette_id as u32, frame_id);
        if let Some(v) = self.monster_frame_cache.get(&key) {
            return v.clone();
        }
        let value = self.decode_monster_frame(object_id, palette_id, frame_id, seq.len());
        self.monster_frame_cache.insert(key, value.clone());
        value
    }

    fn decode_monster_frame(&self, object_id: usize, pid: usize, frame_id: usize, seq_len: usize) -> (Img, i32, i32, usize) {
        let blk = self.object_block(object_id);
        let value = if blk.len() < 0x3228 + 8 {
            (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, seq_len)
        } else {
            let sprite_off = if 0x728 + frame_id * 8 + 8 <= blk.len() {
                u32(blk, 0x728 + frame_id * 8 + 4) as usize
            } else {
                0
            };
            let payload = &blk[0x3228..];
            let (idx, alpha, w, h) = decode_indexed_rle(payload, sprite_off);
            let pal = &self.palettes[pid.min(255)];
            let im = indexed_to_rgba(&idx, &alpha, w, h, pal);
            let go = 0x1728 + frame_id * 8;
            let (gx, gy) = if go + 8 <= blk.len() {
                (i16(blk, go + 4), i16(blk, go + 6))
            } else {
                (0, 0)
            };
            (Rc::new(im), gx, gy, seq_len)
        };
        value
    }

    // --- humans ---

    pub fn human_frame(&mut self, variant_id: usize, anim_frame_index: usize) -> (Img, i32, i32, i32) {
        let direction = (self.variant(variant_id)[1] & 7) as usize;
        self.human_frame_state(variant_id, 0, direction, anim_frame_index, None)
    }

    pub fn human_frame_state(
        &mut self,
        variant_id: usize,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
        palette_override_id: Option<i32>,
    ) -> (Img, i32, i32, i32) {
        let vr = self.variant(variant_id).to_vec();
        let base_id = vr[0x3F] as usize;
        if base_id >= self.human_base_positions.len() {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, 1);
        }
        let action = action % 6;
        let direction = direction & 7;
        let palette_id = match palette_override_id {
            Some(p) => p,
            None => (u32(&vr, 0x41) / 512) as i32,
        };
        let base_pos = self.human_base_positions[base_id];

        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;

        let key = (variant_id as i32, action as i32, direction as i32, fi as i32, palette_id);
        if let Some(v) = self.npc_frame_cache.get(&key) {
            return v.clone();
        }

        let mut equipment = [0u16; 6];
        for slot in 0..6 {
            let o = 0x45 + slot * 2;
            if o + 2 <= vr.len() {
                equipment[slot] = u16(&vr, o) as u16;
            }
        }
        let value = self.compose_human(
            &equipment, base_pos, action, direction, fi, palette_id, count,
        );
        self.npc_frame_cache.insert(key, value.clone());
        value
    }

    /// Render a human animation frame directly from a Game.N entity's
    /// `visual_index` (human base-graphics index) and its `palette_offset`
    /// (D8, / 512 = palette id). No equipment is applied (equipment rendering
    /// is a later task) and no Game.res variant lookup is involved. Retained
    /// for the canonical population/visual phase (MD §10, §12).
    pub fn human_frame_from_base(
        &mut self,
        base_id: usize,
        palette_id: i32,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, i32) {
        if base_id >= self.human_base_positions.len() {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, 1);
        }
        let action = action % 6;
        let direction = direction & 7;
        let base_pos = self.human_base_positions[base_id];
        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;
        // Negative key space keeps these entries distinct from variant-based ones.
        let key = (-(base_id as i32) - 1, action as i32, direction as i32, fi as i32, palette_id);
        if let Some(v) = self.npc_frame_cache.get(&key) {
            return v.clone();
        }
        let value = self.compose_human(
            &[0u16; 6], base_pos, action, direction, fi, palette_id, count,
        );
        self.npc_frame_cache.insert(key, value.clone());
        value
    }

    /// Resolve and render frame 0 for a Game.N entity from its original visual
    /// fields (`KONUNG_SOURCE_OF_TRUTH.md` §5.1/§10).
    ///
    /// Convenience wrapper equivalent to [`actor_frame_state`] with
    /// `action = 0, anim_frame_index = 0` (used by NPC/mob rendering).
    ///
    /// Graphics family is chosen by `archetype & 0x40` (never `kind`):
    /// - clear → humanoid family, `visual_index` indexes `human_base_positions`;
    /// - set   → object/monster family, `visual_index` indexes `OBJECTS.RES`.
    ///
    /// `palette_offset` (+0xD8) is used as `palette_id = palette_offset / 512`.
    ///
    /// Returns an explicit error (no fallback sprite selection) when the family
    /// index or palette cannot be resolved, or when the decoder yields no frame.
    pub fn actor_frame(
        &mut self,
        archetype: u8,
        visual_index: u8,
        palette_offset: u32,
        facing: u8,
    ) -> Result<(Img, i32, i32), String> {
        self.actor_frame_state(archetype, visual_index, palette_offset, facing, 0, 0)
    }

    /// Stateful variant of [`actor_frame`] for animated humanoids.
    ///
    /// Humanoid family (`archetype & 0x40 == 0`): `base_id = visual_index`,
    /// `palette_id = palette_offset / 512`, resolved through
    /// [`human_frame_from_base`] with the supplied `action` / `anim_frame_index`
    /// (frame wrapping/count comes from the GRAPH animation tables). No Game.res
    /// variant and no equipment overlays are involved.
    ///
    /// Object/monster family: animation state is not yet supported, so this
    /// preserves the existing frame-0 behavior of [`actor_frame`].
    pub fn actor_frame_state(
        &mut self,
        archetype: u8,
        visual_index: u8,
        palette_offset: u32,
        facing: u8,
        action: usize,
        anim_frame_index: usize,
    ) -> Result<(Img, i32, i32), String> {
        let palette_id = (palette_offset / 512) as i32;
        if !(0..256).contains(&palette_id) {
            return Err(format!(
                "palette_offset 0x{palette_offset:X} -> palette id {palette_id} out of range"
            ));
        }

        let (im, gx, gy) = if archetype & 0x40 != 0 {
            let object_id = visual_index as usize;
            if self.object_entries.get(object_id).and_then(|x| *x).is_none() {
                return Err(format!(
                    "object family visual_index {object_id} has no OBJECTS.RES entry"
                ));
            }
            if self.object_block(object_id).len() < 0x3228 + 8 {
                return Err(format!(
                    "object family visual_index {object_id} block too small for 0x3228 family"
                ));
            }
            let (im, gx, gy, _) = self.monster_frame_from_object(object_id, palette_id as usize, 0);
            (im, gx, gy)
        } else {
            let base_id = visual_index as usize;
            if base_id >= self.human_base_positions.len() {
                return Err(format!(
                    "human family visual_index {base_id} out of range (bases={})",
                    self.human_base_positions.len()
                ));
            }
            let (im, gx, gy, _) =
                self.human_frame_from_base(base_id, palette_id, action, facing as usize, anim_frame_index);
            (im, gx, gy)
        };

        if im.w == 0 || im.h == 0 {
            return Err(format!(
                "decoder produced an empty frame (archetype 0x{archetype:02X}, visual_index {visual_index}, palette {palette_id})"
            ));
        }
        Ok((im, gx, gy))
    }

    #[allow(clippy::too_many_arguments)]
    fn compose_human(
        &self,
        equipment: &[u16; 6],
        base_pos: usize,
        action: usize,
        direction: usize,
        fi: usize,
        palette_id: i32,
        count: usize,
    ) -> (Img, i32, i32, i32) {
        let pid = palette_id.clamp(0, 255) as usize;
        let base_fid = human_mapped_frame(&self.graph, base_pos, action, direction, fi).0;
        let base_im = render_graph_4d28_frame(&self.graph, base_pos, &self.palettes[pid], base_fid);

        let goff = base_pos + 0x2F28 + action * 0x300 + direction * 0x60 + fi * 8;
        let body_x = if goff + 8 <= self.graph.len() { i16(&self.graph, goff + 4) } else { 0 };
        let body_y = if goff + 8 <= self.graph.len() { i16(&self.graph, goff + 6) } else { 0 };

        let aoff = base_pos + 0x828 + action * 0x180 + direction * 0x30 + fi * 4;
        let anchor_x = if aoff + 4 <= self.graph.len() { i16(&self.graph, aoff) } else { 0 };
        let anchor_y = if aoff + 4 <= self.graph.len() { i16(&self.graph, aoff + 2) } else { 0 };

        struct Layer {
            im: Image,
            x: i32,
            y: i32,
        }
        let mut layers: Vec<Layer> = vec![Layer { im: base_im, x: body_x, y: body_y }];

        for &slot in HUMAN_DRAW_ORDER[direction].iter() {
            let handle = equipment[slot] as u32;
            if handle == 0 {
                continue;
            }
            let selector = match equipment_overlay_selector(&self.game, &self.exe, &self.profile, handle) {
                Some(s) => s,
                None => continue,
            };
            if selector < 0 || selector >= 24 {
                continue;
            }
            let opos = self.human_overlay_positions[selector as usize];
            let ofid = human_mapped_frame(&self.graph, opos, action, direction, fi).0;
            let opid = (self.overlay_palette_offsets[selector as usize] / 512) as usize;
            let oim = render_graph_4d28_frame(&self.graph, opos, &self.palettes[opid.min(255)], ofid);
            let ooff = opos + 0x2F28 + action * 0x300 + direction * 0x60 + fi * 8;
            let ox = if ooff + 8 <= self.graph.len() { anchor_x + i16(&self.graph, ooff + 4) } else { anchor_x };
            let oy = if ooff + 8 <= self.graph.len() { anchor_y + i16(&self.graph, ooff + 6) } else { anchor_y };
            layers.push(Layer { im: oim, x: ox, y: oy });
        }

        let mut minx = i32::MAX;
        let mut miny = i32::MAX;
        let mut maxx = i32::MIN;
        let mut maxy = i32::MIN;
        for l in &layers {
            minx = minx.min(l.x);
            miny = miny.min(l.y);
            maxx = maxx.max(l.x + l.im.w as i32);
            maxy = maxy.max(l.y + l.im.h as i32);
        }
        if minx == i32::MAX {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, count as i32);
        }
        let cw = (maxx - minx).max(1) as usize;
        let ch = (maxy - miny).max(1) as usize;
        let mut canvas = Image::new(cw, ch, [0, 0, 0, 0]);
        for l in &layers {
            blit(&mut canvas.data, cw, ch, &l.im.data, l.im.w, l.im.h, l.x - minx, l.y - miny);
        }
        (Rc::new(canvas), minx, miny, count as i32)
    }

    pub fn find_fallback_human(&self, requested: Option<usize>) -> Option<usize> {
        if let Some(vid) = requested {
            if vid < GAME_VARIANT_COUNT {
                let vr = self.variant(vid);
                if (vr[4] & 0x40) == 0 && (vr[0x3F] as usize) < self.human_base_positions.len() {
                    return Some(vid);
                }
            }
            return None;
        }
        for vid in 0..GAME_VARIANT_COUNT {
            let vr = self.variant(vid);
            if vr.len() < 0x91 {
                continue;
            }
            if vr[4] & 0x40 != 0 {
                continue;
            }
            if vr[0x3F] as usize >= self.human_base_positions.len() {
                continue;
            }
            let mut has_equip = false;
            for s in 0..6 {
                if u16(vr, 0x45 + s * 2) != 0 {
                    has_equip = true;
                    break;
                }
            }
            if !has_equip {
                continue;
            }
            let pal_off = u32(vr, 0x41);
            if pal_off % 512 != 0 || (pal_off / 512) >= 256 {
                continue;
            }
            return Some(vid);
        }
        None
    }
}

fn parse_graph_layout(graph: &[u8]) -> (usize, usize, usize, Vec<usize>, Vec<usize>) {
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
    let terrain_payload_size = if pos + 8 <= graph.len() { u32(graph, pos + 4) as usize } else { 0 };
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
    (terrain_table_pos, terrain_payload_pos, terrain_payload_size, human_base_positions, human_overlay_positions)
}

fn parse_object_index(objects: &[u8]) -> Vec<Option<(usize, usize)>> {
    let mut entries = Vec::with_capacity(512);
    for i in 0..512 {
        let o = 8 + i * 8;
        if o + 8 > objects.len() {
            entries.push(None);
            continue;
        }
        let off = u32(objects, o);
        let size = u32(objects, o + 4);
        if off == 0xFFFFFFFF || size == 0xFFFFFFFF {
            entries.push(None);
        } else {
            entries.push(Some((0x1008 + off as usize, size as usize)));
        }
    }
    entries
}

/// Alpha-composite `src` onto `dst` at (ox, oy). Straight "over" operator.
pub fn blit(dst: &mut [u8], dw: usize, dh: usize, src: &[u8], sw: usize, sh: usize, ox: i32, oy: i32) {
    if sw == 0 || sh == 0 {
        return;
    }
    for sy in 0..sh {
        let dy = oy + sy as i32;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        let srow = sy * sw * 4;
        let drow = dy as usize * dw * 4;
        for sx in 0..sw {
            let dx = ox + sx as i32;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let si = srow + sx * 4;
            let a = src[si + 3];
            if a == 0 {
                continue;
            }
            let di = drow + dx as usize * 4;
            if a == 255 {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
            } else {
                let sa = a as u32;
                let inv = 255 - sa;
                dst[di] = ((src[si] as u32 * sa + dst[di] as u32 * inv) / 255) as u8;
                dst[di + 1] = ((src[si + 1] as u32 * sa + dst[di + 1] as u32 * inv) / 255) as u8;
                dst[di + 2] = ((src[si + 2] as u32 * sa + dst[di + 2] as u32 * inv) / 255) as u8;
                dst[di + 3] = 255;
            }
        }
    }
}

/// Fill a rectangle (opaque) clipping to the buffer.
pub fn fill_rect(dst: &mut [u8], dw: usize, dh: usize, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(dw as i32);
    let y1 = (y + h).min(dh as i32);
    for yy in y0..y1 {
        let row = yy as usize * dw * 4;
        for xx in x0..x1 {
            let di = row + xx as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
    }
}

/// Draw a filled ellipse using a simple scanline test.
pub fn fill_ellipse(dst: &mut [u8], dw: usize, dh: usize, cx: f32, cy: f32, rx: f32, ry: f32, c: [u8; 4]) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let y0 = (cy - ry).floor().max(0.0) as i32;
    let y1 = (cy + ry).ceil().min(dh as f32 - 1.0) as i32;
    for yy in y0..=y1 {
        let dy = (yy as f32 + 0.5 - cy) / ry;
        if dy.abs() > 1.0 {
            continue;
        }
        let half = rx * (1.0 - dy * dy).max(0.0).sqrt();
        let sx = (cx - half).floor() as i32;
        let ex = (cx + half).ceil() as i32;
        for xx in sx.max(0)..=ex.min(dw as i32 - 1) {
            let di = yy as usize * dw * 4 + xx as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
    }
}

/// Bresenham line.
pub fn line(dst: &mut [u8], dw: usize, dh: usize, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && x0 < dw as i32 && y0 >= 0 && y0 < dh as i32 {
            let di = y0 as usize * dw * 4 + x0 as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{human_frame_count, human_mapped_frame};

    #[test]
    fn human_frames_wrap_by_graph_sequence_count() {
        // Synthetic GRAPH block at position 0: action 0 / direction 0 has a
        // 3-frame sequence with frame ids 7, 9, 11 (table-driven, not hardcoded).
        let mut g = vec![0u8; 0x200];
        g[0x08] = 3; // count (low byte)
        g[0x0A] = 7;
        g[0x0C] = 9;
        g[0x0E] = 11;

        assert_eq!(human_frame_count(&g, 0, 0, 0), 3);
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 0), (7, 3));
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 1), (9, 3));
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 2), (11, 3));
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 3), (7, 3)); // wraps
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 7), (9, 3)); // 7 % 3 == 1
    }

    #[test]
    fn zero_count_sequence_is_single_frame() {
        let g = vec![0u8; 0x200];
        assert_eq!(human_frame_count(&g, 0, 0, 0), 0);
        assert_eq!(human_mapped_frame(&g, 0, 0, 0, 5), (0, 1));
    }
}
