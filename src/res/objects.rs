//! `objects` subsystem of [`crate::res`] (split from `res.rs`).

use super::*;

pub(crate) fn parse_object_index(objects: &[u8]) -> Vec<Option<(usize, usize)>> {
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


/// Block-relative byte offset of an object/monster per-frame gx/gy entry.
///
/// Original `FUN_00414078` object branch (`0x41410a`-`0x414143`) computes
/// `block + 0x1728 + row*0x480 + direction*0x90 + fi*8`; the table therefore has
/// 18 frame slots per direction (`0x90/8`) and 144 per row (`0x480/8`). It is
/// **not** indexed by the animation frame id.
pub(crate) fn monster_anchor_offset(row: usize, direction: usize, fi: usize) -> usize {
    0x1728 + row * 0x480 + (direction & 7) * 0x90 + fi * 8
}


impl Res {
    pub fn prop_sprite(&mut self, object_id: usize, frame: usize, palette_override: u32) -> (Img, i32, i32) {
        let key = (object_id, palette_override);
        if let Some(v) = self.prop_sprite_cache.get(&key) {
            return v.clone();
        }
        let value = self.decode_prop_sprite(object_id, frame, palette_override);
        self.prop_sprite_cache.insert(key, value.clone());
        value
    }


    pub(super) fn decode_prop_sprite(&self, object_id: usize, frame: usize, palette_override: u32) -> (Img, i32, i32) {
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


    /// Original object block row for an action value. The object/monster family
    /// has 6 rows of `0x130` bytes; actions `6..=11` alias rows `0..=5`
    /// (`FUN_0041435C` / `FUN_00414078`: `if (5 < action) action -= 6`).
    pub(super) fn object_row(action: usize) -> usize {
        if action > 5 {
            action - 6
        } else {
            action
        }
    }


    /// Animation-0 (row 0, direction 0) frame-id sequence. Retained for
    /// callers that only need the base sequence.
    pub fn monster_animation0_sequence(&self, object_id: usize) -> Vec<u16> {
        let blk = self.object_block(object_id);
        if blk.len() < 0x0C {
            return vec![0];
        }
        let (_, count) = self.monster_mapped_frame(object_id, 0, 0, 0);
        if count == 0 {
            return vec![0];
        }
        (0..count)
            .map(|i| u16(blk, 0x0A + i * 2) as u16)
            .collect()
    }


    /// Frame count for one object-family action/direction. `0` when unresolved.
    /// Table layout: `block + row*0x130 + direction*0x26 + 8` (count `u16`).
    pub fn monster_frame_count(&self, object_id: usize, action: usize, direction: usize) -> usize {
        let blk = self.object_block(object_id);
        let row = Self::object_row(action);
        let o = row * 0x130 + (direction & 7) * 0x26 + 8;
        if o + 2 > blk.len() {
            return 0;
        }
        u16(blk, o) as usize
    }


    /// Frame id and count for one object-family action/direction/frame index.
    pub fn monster_mapped_frame(
        &self,
        object_id: usize,
        action: usize,
        direction: usize,
        frame_index: usize,
    ) -> (usize, usize) {
        let mut count = self.monster_frame_count(object_id, action, direction);
        if count == 0 {
            count = 1;
        }
        let blk = self.object_block(object_id);
        let row = Self::object_row(action);
        let fi = frame_index % count;
        let o = row * 0x130 + (direction & 7) * 0x26 + fi * 2 + 0x0A;
        if o + 2 > blk.len() {
            return (0, count);
        }
        (u16(blk, o) as usize, count)
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
        self.monster_frame_state(object_id, palette_id, 0, 0, anim_frame_index)
    }


    /// Stateful object/monster frame: `action`/`direction` select the original
    /// 6-row animation table (actions `6..=11` alias rows `0..=5`), and
    /// `anim_frame_index` wraps at the row's frame count.
    ///
    /// The **sprite id** comes from that row/direction animation table
    /// (`block + row*0x130 + dir*0x26 + fi*2 + 0x0A`), but the screen **anchor**
    /// is a separate per-`(row, direction, frame_index)` table (`FUN_00414078`
    /// object branch `0x41410a`-`0x414143`):
    /// `block + 0x1728 + row*0x480 + direction*0x90 + fi*8`.
    pub fn monster_frame_state(
        &mut self,
        object_id: usize,
        palette_id: usize,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, usize) {
        let (frame_id, count) = self.monster_mapped_frame(object_id, action, direction, anim_frame_index);
        let row = Self::object_row(action);
        let dir = direction & 7;
        let fi = if count == 0 { 0 } else { anim_frame_index % count };
        let im = self.monster_sprite(object_id, palette_id, frame_id);
        let (gx, gy) = self.monster_anchor(object_id, row, dir, fi);
        (im, gx, gy, count)
    }


    /// Decoded (cached) monster sprite image for an animation frame id. The
    /// anchor is resolved separately because a frame id is shared across rows
    /// and directions (`monster_anchor`).
    fn monster_sprite(&mut self, object_id: usize, palette_id: usize, frame_id: usize) -> Img {
        let key = (object_id, palette_id as u32, frame_id);
        if let Some(v) = self.monster_frame_cache.get(&key) {
            return v.clone();
        }
        let value = self.decode_monster_sprite(object_id, palette_id, frame_id);
        self.monster_frame_cache.insert(key, value.clone());
        value
    }


    /// Per-frame screen anchor from the original object anchor table
    /// (`FUN_00414078` object branch, `0x41410a`-`0x414143`): the entry lives at
    /// `block + 0x1728 + row*0x480 + direction*0x90 + fi*8`, with the `u16` gx
    /// at `+4` and gy at `+6`. `(0, 0)` when the entry is out of range.
    ///
    /// This is **not** indexed by animation frame id; doing so reads unrelated
    /// slots and makes any object animation visibly jump.
    pub(super) fn monster_anchor(
        &self,
        object_id: usize,
        row: usize,
        direction: usize,
        fi: usize,
    ) -> (i32, i32) {
        let blk = self.object_block(object_id);
        let go = monster_anchor_offset(row, direction, fi);
        if go + 8 <= blk.len() {
            (i16(blk, go + 4), i16(blk, go + 6))
        } else {
            (0, 0)
        }
    }


    /// Decode one object/monster sprite by animation frame id. The sprite
    /// offset table is indexed by frame id (`block + 0x728 + frame_id*8 + 4`).
    pub(super) fn decode_monster_sprite(&self, object_id: usize, pid: usize, frame_id: usize) -> Img {
        let blk = self.object_block(object_id);
        if blk.len() < 0x3228 + 8 {
            return Rc::new(Image::new(1, 1, [0, 0, 0, 0]));
        }
        let sprite_off = if 0x728 + frame_id * 8 + 8 <= blk.len() {
            u32(blk, 0x728 + frame_id * 8 + 4) as usize
        } else {
            0
        };
        let payload = &blk[0x3228..];
        let (idx, alpha, w, h) = decode_indexed_rle(payload, sprite_off);
        let pal = &self.palettes[pid.min(255)];
        Rc::new(indexed_to_rgba(&idx, &alpha, w, h, pal))
    }

    // --- humans ---


}
