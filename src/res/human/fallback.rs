//! Fallback human variant selection.

use super::super::*;
use crate::constants::combat::OBJECT_ARCHETYPE_BIT;

impl Res {
    pub fn find_fallback_human(&self, requested: Option<usize>) -> Option<usize> {
        if let Some(vid) = requested {
            if vid < GAME_VARIANT_COUNT {
                let vr = self.variant(vid);
                if (vr[HUMAN_VARIANT_FLAGS_OFFSET] & OBJECT_ARCHETYPE_BIT) == 0
                    && (vr[HUMAN_VARIANT_BASE_OFFSET] as usize) < self.human_base_positions.len()
                {
                    return Some(vid);
                }
            }
            return None;
        }
        for vid in 0..GAME_VARIANT_COUNT {
            let vr = self.variant(vid);
            if vr.len() < HUMAN_MIN_VARIANT_LEN {
                continue;
            }
            if vr[HUMAN_VARIANT_FLAGS_OFFSET] & OBJECT_ARCHETYPE_BIT != 0 {
                continue;
            }
            if vr[HUMAN_VARIANT_BASE_OFFSET] as usize >= self.human_base_positions.len() {
                continue;
            }
            let mut has_equip = false;
            for s in 0..HUMAN_EQUIPMENT_SLOTS {
                if u16(vr, HUMAN_VARIANT_EQUIPMENT_OFFSET + s * HUMAN_FRAME_ID_SIZE) != 0 {
                    has_equip = true;
                    break;
                }
            }
            if !has_equip {
                continue;
            }
            let pal_off = u32(vr, HUMAN_VARIANT_PALETTE_OFFSET);
            if pal_off % PALETTE_OFFSET_SCALE != 0
                || (pal_off / PALETTE_OFFSET_SCALE) >= PALETTE_COUNT as u32
            {
                continue;
            }
            return Some(vid);
        }
        None
    }
}
