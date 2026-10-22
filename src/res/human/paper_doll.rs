//! Character-sheet paper-doll rendering (FUN_0042ea6c).

use super::super::*;

impl Res {
    /// Render the character-sheet paper doll (`FUN_0042ea6c`).
    ///
    /// The original renders the live selected entity into a `160x160` region
    /// with the base sprite's top-left at `(0x3C, 0x14)` and then blits that
    /// region to the per-resolution position table (`0x46035C`/`0x460360`,
    /// read by the caller). The same equipment overlay plan as the world sprite
    /// is used, so all equipped items (including the battle "in hand" variant)
    /// appear.
    #[allow(clippy::too_many_arguments)]
    pub fn paper_doll_frame(
        &mut self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        base_id: usize,
        palette_id: i32,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
    ) -> Option<Img> {
        if base_id >= self.human_base_positions.len() {
            return None;
        }
        let direction = direction & 7;
        let base_pos = self.human_base_positions[base_id];
        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;
        let key = (
            base_id,
            palette_id,
            battle,
            weapon_slot,
            action,
            direction,
            fi,
            *equipment,
        );
        if let Some(v) = self.paper_doll_cache.get(&key) {
            return Some(v.clone());
        }
        let (layers, body_x, body_y) = self.human_layers(
            equipment,
            weapon_slot,
            battle,
            &self.save_game,
            base_pos,
            action,
            direction,
            fi,
            palette_id,
        );
        // Original offscreen origin: base body top-left at (0x3C, 0x14).
        let shift_x = PAPER_DOLL_BODY_ORIGIN.0 - body_x;
        let shift_y = PAPER_DOLL_BODY_ORIGIN.1 - body_y;
        let mut canvas = Image::new(PAPER_DOLL_W, PAPER_DOLL_H, [0, 0, 0, 0]);
        for l in &layers {
            blit(
                &mut canvas.data,
                PAPER_DOLL_W,
                PAPER_DOLL_H,
                &l.im.data,
                l.im.w,
                l.im.h,
                l.x + shift_x,
                l.y + shift_y,
            );
        }
        let value = Rc::new(canvas);
        self.paper_doll_cache.insert(key, value.clone());
        Some(value)
    }
}
