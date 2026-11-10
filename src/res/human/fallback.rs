//! Fallback human variant selection.
//!
//! A Game.res variant record is human when its archetype byte at `+0x04` lacks
//! the object bit, its base-graphics index at `+0x3F` addresses a parsed human
//! block, and its palette offset at `+0x41` selects one of the 256 palettes.

use super::super::*;
use crate::constants::combat::OBJECT_ARCHETYPE_BIT;

impl Res {
    /// Pick a human variant id for the frontend hero preview.
    ///
    /// With `Some(vid)` the request is validated and either returned unchanged
    /// or rejected with `None` - it is never silently substituted. With `None`
    /// the first variant is chosen that is human, has a valid base block, at
    /// least one non-empty equipment slot (`+0x45`, six `u16` ids) and an
    /// aligned in-range palette offset.
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
