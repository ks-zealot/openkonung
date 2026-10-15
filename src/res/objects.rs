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
    /// 6-row table (actions `6..=11` alias rows `0..=5`), `anim_frame_index`
    /// wraps at the row's frame count.
    pub fn monster_frame_state(
        &mut self,
        object_id: usize,
        palette_id: usize,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, usize) {
        let (frame_id, count) = self.monster_mapped_frame(object_id, action, direction, anim_frame_index);
        let key = (object_id, palette_id as u32, frame_id);
        if let Some(v) = self.monster_frame_cache.get(&key) {
            return v.clone();
        }
        let value = self.decode_monster_frame(object_id, palette_id, frame_id, count);
        self.monster_frame_cache.insert(key, value.clone());
        value
    }


    pub(super) fn decode_monster_frame(&self, object_id: usize, pid: usize, frame_id: usize, seq_len: usize) -> (Img, i32, i32, usize) {
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


}
