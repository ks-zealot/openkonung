//! GRAPH human frame decoding and animation-cycle lookup.

use super::super::*;
use crate::constants::combat::OBJECT_ARCHETYPE_BIT;

pub fn human_frame_count(graph: &[u8], block_pos: usize, action: usize, direction: usize) -> usize {
    let o = block_pos
        + action * HUMAN_ACTION_STRIDE
        + direction * HUMAN_DIRECTION_STRIDE
        + HUMAN_FRAME_COUNT_OFFSET;
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
    let o = block_pos
        + action * HUMAN_ACTION_STRIDE
        + direction * HUMAN_DIRECTION_STRIDE
        + fi * HUMAN_FRAME_ID_SIZE
        + HUMAN_FRAME_ID_OFFSET;
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
    let start = block_pos + HUMAN_BLOCK_DATA_OFFSET;
    let end = (block_pos + total).min(graph.len());
    let payload = if start < end {
        &graph[start..end]
    } else {
        &[][..]
    };
    let go = block_pos + HUMAN_ANCHOR_TABLE_OFFSET + frame_id * HUMAN_ANCHOR_RECORD_SIZE;
    let sprite_off = if go + 8 <= graph.len() {
        u32(graph, go + 4) as usize
    } else {
        0
    };
    let (idx, alpha, w, h) = decode_indexed_rle(payload, sprite_off);
    indexed_to_rgba(&idx, &alpha, w, h, palette)
}

// --- resource container ---

impl Res {
    pub fn human_frame(
        &mut self,
        variant_id: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, i32) {
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
        let base_id = vr[HUMAN_VARIANT_BASE_OFFSET] as usize;
        if base_id >= self.human_base_positions.len() {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, 1);
        }
        let direction = direction & 7;
        let palette_id = match palette_override_id {
            Some(p) => p,
            None => (u32(&vr, HUMAN_VARIANT_PALETTE_OFFSET) / PALETTE_OFFSET_SCALE) as i32,
        };
        let base_pos = self.human_base_positions[base_id];

        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;

        let mut equipment = [0u16; 6];
        for slot in 0..6 {
            let o = HUMAN_VARIANT_EQUIPMENT_OFFSET + slot * HUMAN_FRAME_ID_SIZE;
            if o + 2 <= vr.len() {
                equipment[slot] = u16(&vr, o) as u16;
            }
        }

        let key = (
            variant_id as i32,
            action as i32,
            direction as i32,
            fi as i32,
            palette_id,
            false,
            0,
            equipment,
        );
        if let Some(v) = self.npc_frame_cache.get(&key) {
            return v.clone();
        }

        let value = self.compose_human(
            &equipment, 0, false, &self.game, base_pos, action, direction, fi, palette_id, count,
        );
        self.npc_frame_cache.insert(key, value.clone());
        value
    }

    /// Render a human animation frame directly from a Game.N entity's
    /// `visual_index` (human base-graphics index) and its `palette_offset`
    /// (D8, / PALETTE_OFFSET_SCALE = palette id). `equipment` is the entity's `Entity+0x66`
    /// six-slot array; equipped items composite their GRAPH overlay blocks. No
    /// Game.res variant lookup is involved.
    ///
    /// `weapon_slot` is `Entity+0x12` (active weapon/equipment slot) and
    /// `battle` is `Entity+0x03 & 0x04`; together they select the battle
    /// overlay order and the `equipped_visual + 1` "in hand" variant
    /// (`FUN_00427e9c`).
    pub fn human_frame_from_base(
        &mut self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        base_id: usize,
        palette_id: i32,
        action: usize,
        direction: usize,
        anim_frame_index: usize,
    ) -> (Img, i32, i32, i32) {
        if base_id >= self.human_base_positions.len() {
            return (Rc::new(Image::new(1, 1, [0, 0, 0, 0])), 0, 0, 1);
        }
        let direction = direction & 7;
        let base_pos = self.human_base_positions[base_id];
        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;
        // Negative key space keeps these entries distinct from variant-based ones.
        let key = (
            -(base_id as i32) - 1,
            action as i32,
            direction as i32,
            fi as i32,
            palette_id,
            battle,
            weapon_slot,
            *equipment,
        );
        if let Some(v) = self.npc_frame_cache.get(&key) {
            return v.clone();
        }
        let value = self.compose_human(
            equipment,
            weapon_slot,
            battle,
            &self.save_game,
            base_pos,
            action,
            direction,
            fi,
            palette_id,
            count,
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
    /// Graphics family is chosen by `archetype & OBJECT_ARCHETYPE_BIT` (never `kind`):
    /// - clear → humanoid family, `visual_index` indexes `human_base_positions`;
    /// - set   → object/monster family, `visual_index` indexes `OBJECTS.RES`.
    ///
    /// `palette_offset` (+0xD8) is used as `palette_id = palette_offset / PALETTE_OFFSET_SCALE`.
    ///
    /// Returns an explicit error (no fallback sprite selection) when the family
    /// index or palette cannot be resolved, or when the decoder yields no frame.
    pub fn actor_frame(
        &mut self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        archetype: u8,
        visual_index: u8,
        palette_offset: u32,
        facing: u8,
    ) -> Result<(Img, i32, i32), String> {
        self.actor_frame_state(
            equipment,
            weapon_slot,
            battle,
            archetype,
            visual_index,
            palette_offset,
            facing,
            0,
            0,
        )
    }

    /// Stateful variant of [`actor_frame`] for animated humanoids.
    ///
    /// Humanoid family (`archetype & OBJECT_ARCHETYPE_BIT == 0`): `base_id = visual_index`,
    /// `palette_id = palette_offset / PALETTE_OFFSET_SCALE`, resolved through
    /// [`human_frame_from_base`] with the supplied `action` / `anim_frame_index`
    /// (frame wrapping/count comes from the GRAPH animation tables) and the
    /// entity's `equipment` (`Entity+0x66`) composited as GRAPH overlay blocks.
    ///
    /// Object/monster family: animation state is not yet supported, so this
    /// preserves the existing frame-0 behavior of [`actor_frame`]; monsters have
    /// no equipment overlays.
    pub fn actor_frame_state(
        &mut self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        archetype: u8,
        visual_index: u8,
        palette_offset: u32,
        facing: u8,
        action: usize,
        anim_frame_index: usize,
    ) -> Result<(Img, i32, i32), String> {
        let palette_id = (palette_offset / PALETTE_OFFSET_SCALE) as i32;
        if !(0..PALETTE_COUNT).contains(&palette_id) {
            return Err(format!(
                "palette_offset 0x{palette_offset:X} -> palette id {palette_id} out of range"
            ));
        }

        let (im, gx, gy) = if archetype & OBJECT_ARCHETYPE_BIT != 0 {
            let object_id = visual_index as usize;
            if self
                .object_entries
                .get(object_id)
                .and_then(|x| *x)
                .is_none()
            {
                return Err(format!(
                    "object family visual_index {object_id} has no OBJECTS.RES entry"
                ));
            }
            if self.object_block(object_id).len() < OBJECT_FAMILY_MIN_BLOCK_LEN {
                return Err(format!(
                    "object family visual_index {object_id} block too small for 0x3228 family"
                ));
            }
            let (im, gx, gy, _) = self.monster_frame_state(
                object_id,
                palette_id as usize,
                action,
                facing as usize,
                anim_frame_index,
            );
            (im, gx, gy)
        } else {
            let base_id = visual_index as usize;
            if base_id >= self.human_base_positions.len() {
                return Err(format!(
                    "human family visual_index {base_id} out of range (bases={})",
                    self.human_base_positions.len()
                ));
            }
            let (im, gx, gy, _) = self.human_frame_from_base(
                equipment,
                weapon_slot,
                battle,
                base_id,
                palette_id,
                action,
                facing as usize,
                anim_frame_index,
            );
            (im, gx, gy)
        };

        if im.w == 0 || im.h == 0 {
            return Err(format!(
                "decoder produced an empty frame (archetype 0x{archetype:02X}, visual_index {visual_index}, palette {palette_id})"
            ));
        }
        Ok((im, gx, gy))
    }

    /// Animation-cycle length in frames for one family/direction/action, the
    /// original `Entity+0xD7` written by `FUN_0041435c`. `FUN_00413de4` wraps the
    /// frame counter at this value, and the walk action steps one collision cell
    /// per completed cycle.
    ///
    /// `None` when the family/index cannot be resolved.
    /// - humanoid (`archetype & OBJECT_ARCHETYPE_BIT == 0`): GRAPH row `action` for `direction`.
    /// - object/monster: 6-row table (`action 6..=11` alias `0..=5`).
    pub fn action_cycle_frames(
        &self,
        archetype: u8,
        visual_index: u8,
        direction: usize,
        action: usize,
    ) -> Option<usize> {
        if archetype & OBJECT_ARCHETYPE_BIT != 0 {
            let object_id = visual_index as usize;
            if self
                .object_entries
                .get(object_id)
                .and_then(|x| *x)
                .is_none()
            {
                return None;
            }
            let n = self.monster_frame_count(object_id, action, direction & 7);
            if n == 0 {
                None
            } else {
                Some(n)
            }
        } else {
            let base_id = visual_index as usize;
            let base_pos = *self.human_base_positions.get(base_id)?;
            let n = human_frame_count(&self.graph, base_pos, action, direction & 7);
            if n == 0 {
                None
            } else {
                Some(n)
            }
        }
    }

    /// Walk-cycle length for the current stance: human row `1` (battle) / `7`
    /// (peaceful); object rows alias to `1`.
    pub fn walk_cycle_frames(
        &self,
        archetype: u8,
        visual_index: u8,
        direction: usize,
        battle: bool,
    ) -> Option<usize> {
        self.action_cycle_frames(
            archetype,
            visual_index,
            direction,
            crate::anim::walk_action(battle),
        )
    }
}
