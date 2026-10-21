//! `human` subsystem of [`crate::res`] (split from `res.rs`).

use super::*;

/// Peaceful-stance overlay draw order, indexed by facing.
///
/// `konung.res` DGROUP VA `0x4631FC` (`FUN_00427e9c` peace branch). The battle
/// stance uses separate tables below because the active weapon is drawn in the
/// hand rather than sheathed.
pub const HUMAN_DRAW_ORDER_PEACE: [[usize; 6]; 8] = [
    [3, 2, 4, 1, 0, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 5, 1],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 4, 5, 1, 0],
];

/// Battle-stance overlay order A (`konung.res` DGROUP VA `0x462BFC`):
/// `[action_bucket][facing][draw_index] = equipment_slot`.
///
/// Used when `Entity+0x12 != 1` or the action bucket is `2` (action `5`).
pub const HUMAN_DRAW_ORDER_BATTLE_A: [[[usize; 6]; 8]; 4] = [
    [
        [3, 2, 0, 4, 5, 1],
        [3, 5, 2, 4, 0, 1],
        [3, 5, 4, 0, 2, 1],
        [3, 5, 4, 1, 0, 2],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 1, 4, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 0, 1, 2, 5],
        [3, 2, 5, 0, 4, 1],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
    [
        [3, 2, 0, 4, 1, 5],
        [3, 2, 0, 5, 4, 1],
        [3, 5, 4, 2, 0, 1],
        [3, 5, 4, 0, 1, 2],
        [3, 2, 5, 4, 0, 1],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 0, 4, 5, 1],
        [3, 5, 4, 2, 1, 0],
        [3, 5, 4, 2, 1, 0],
        [3, 5, 4, 1, 0, 2],
        [3, 4, 2, 1, 0, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 1, 0, 2, 4, 5],
        [3, 0, 2, 1, 4, 5],
    ],
];

/// Battle-stance overlay order B (`konung.res` DGROUP VA `0x462EFC`), used when
/// `Entity+0x12 == 1` and the action bucket is not `2`.
pub const HUMAN_DRAW_ORDER_BATTLE_B: [[[usize; 6]; 8]; 4] = [
    [
        [3, 2, 0, 1, 4, 5],
        [3, 2, 0, 1, 4, 5],
        [3, 4, 0, 2, 1, 5],
        [3, 4, 5, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 0, 1, 2, 5],
        [3, 2, 5, 0, 4, 1],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
    [
        [3, 2, 0, 4, 1, 5],
        [3, 2, 0, 5, 4, 1],
        [3, 5, 4, 2, 0, 1],
        [3, 5, 4, 0, 1, 2],
        [3, 2, 5, 4, 0, 1],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 2, 1, 0, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 2, 5, 4, 1, 0],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
];

/// `FUN_00427e9c` action bucket: walk (`1`) → 3, engage (`5`) → 2, use (`4`)
/// → 1, everything else (`0`/`2`/`3`) → 0.
pub fn battle_overlay_bucket(action: usize) -> usize {
    match action {
        1 => 3,
        5 => 2,
        4 => 1,
        _ => 0,
    }
}

/// Overlay order and the slots whose `equipped_visual` selector is advanced by
/// one (the "in hand"/alternate GRAPH block) for the current stance.
///
/// Mirrors `FUN_00427e9c`:
/// - peace: fixed direction order, no variants;
/// - battle, `Entity+0x12 == 1`, bucket != 2: order B, slot 1 variant;
/// - else: order A, active slot (unless it is 1) and slot 5 variants.
/// Offscreen paper-doll surface size (`FUN_0042ea6c`: clip `0x9f × 0x13f`,
/// final visible blit `0xA0 × 0xA0`). The remake only needs the visible square.
pub const PAPER_DOLL_W: usize = 160;
pub const PAPER_DOLL_H: usize = 160;

struct HumanLayer {
    im: Image,
    x: i32,
    y: i32,
}

pub fn overlay_plan(
    battle: bool,
    weapon_slot: u8,
    action: usize,
    direction: usize,
) -> (&'static [usize; 6], [bool; 6]) {
    let dir = direction & 7;
    if !battle {
        return (&HUMAN_DRAW_ORDER_PEACE[dir], [false; 6]);
    }
    let bucket = battle_overlay_bucket(action);
    let mut variant = [false; 6];
    if weapon_slot == 1 && bucket != 2 {
        variant[1] = true;
        (&HUMAN_DRAW_ORDER_BATTLE_B[bucket][dir], variant)
    } else {
        if weapon_slot != 1 {
            if let Some(v) = variant.get_mut(weapon_slot as usize) {
                *v = true;
            }
        }
        variant[5] = true;
        (&HUMAN_DRAW_ORDER_BATTLE_A[bucket][dir], variant)
    }
}


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

// --- resource container ---


impl Res {
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
        let direction = direction & 7;
        let palette_id = match palette_override_id {
            Some(p) => p,
            None => (u32(&vr, 0x41) / 512) as i32,
        };
        let base_pos = self.human_base_positions[base_id];

        let (_, count) =
            human_mapped_frame(&self.graph, base_pos, action, direction, anim_frame_index);
        let fi = anim_frame_index % count;

        let mut equipment = [0u16; 6];
        for slot in 0..6 {
            let o = 0x45 + slot * 2;
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
    /// (D8, / 512 = palette id). `equipment` is the entity's `Entity+0x66`
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
    /// Humanoid family (`archetype & 0x40 == 0`): `base_id = visual_index`,
    /// `palette_id = palette_offset / 512`, resolved through
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
            let (im, gx, gy, _) =
                self.monster_frame_state(object_id, palette_id as usize, action, facing as usize, anim_frame_index);
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
    /// - humanoid (`archetype & 0x40 == 0`): GRAPH row `action` for `direction`.
    /// - object/monster: 6-row table (`action 6..=11` alias `0..=5`).
    pub fn action_cycle_frames(
        &self,
        archetype: u8,
        visual_index: u8,
        direction: usize,
        action: usize,
    ) -> Option<usize> {
        if archetype & 0x40 != 0 {
            let object_id = visual_index as usize;
            if self.object_entries.get(object_id).and_then(|x| *x).is_none() {
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
        self.action_cycle_frames(archetype, visual_index, direction, crate::anim::walk_action(battle))
    }


    /// GRAPH human-overlay selector (`equipped_visual`, prototype `+0x3E`) for
    /// an equipped item-instance id. The instance is decoded from `instances`
    /// (the loaded save for Game.N actors, or `Game.res` for the variant path)
    /// and resolved against the `konung.res` templates. `None` when the item has
    /// no body overlay (`equipped_visual` outside `0..=23`).
    pub(super) fn overlay_selector(&self, instances: &[u8], id: u16) -> Option<usize> {
        let db = self.item_db.as_ref()?;
        let inst = crate::item::ItemDb::decode_instance(instances, id)?;
        let ov = db.info(inst.class, inst.subtype)?.equipped_visual;
        if (0..24).contains(&ov) {
            Some(ov as usize)
        } else {
            None
        }
    }


    /// Build the base body plus every equipped overlay for one frame.
    ///
    /// Order and the `equipped_visual + 1` "in hand" variants are selected by
    /// the original `FUN_00427e9c` rules (`overlay_plan`).
    #[allow(clippy::too_many_arguments)]
    fn human_layers(
        &self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        instances: &[u8],
        base_pos: usize,
        action: usize,
        direction: usize,
        fi: usize,
        palette_id: i32,
    ) -> (Vec<HumanLayer>, i32, i32) {
        let pid = palette_id.clamp(0, 255) as usize;
        let base_fid = human_mapped_frame(&self.graph, base_pos, action, direction, fi).0;
        let base_im = render_graph_4d28_frame(&self.graph, base_pos, &self.palettes[pid], base_fid);

        let goff = base_pos + 0x2F28 + action * 0x300 + direction * 0x60 + fi * 8;
        let body_x = if goff + 8 <= self.graph.len() { i16(&self.graph, goff + 4) } else { 0 };
        let body_y = if goff + 8 <= self.graph.len() { i16(&self.graph, goff + 6) } else { 0 };

        let aoff = base_pos + 0x828 + action * 0x180 + direction * 0x30 + fi * 4;
        let anchor_x = if aoff + 4 <= self.graph.len() { i16(&self.graph, aoff) } else { 0 };
        let anchor_y = if aoff + 4 <= self.graph.len() { i16(&self.graph, aoff + 2) } else { 0 };

        let (order, variant) = overlay_plan(battle, weapon_slot, action, direction);
        let mut layers: Vec<HumanLayer> = vec![HumanLayer {
            im: base_im,
            x: body_x,
            y: body_y,
        }];

        for &slot in order.iter() {
            let handle = equipment[slot] as u32;
            if handle == 0 {
                continue;
            }
            let mut selector = match self.overlay_selector(instances, handle as u16) {
                Some(s) => s,
                None => continue,
            };
            if variant.get(slot).copied().unwrap_or(false)
                && selector + 1 < self.human_overlay_positions.len()
            {
                selector += 1;
            }
            let opos = self.human_overlay_positions[selector];
            let ofid = human_mapped_frame(&self.graph, opos, action, direction, fi).0;
            let opid = (self.overlay_palette_offsets[selector] / 512) as usize;
            let oim = render_graph_4d28_frame(&self.graph, opos, &self.palettes[opid.min(255)], ofid);
            let ooff = opos + 0x2F28 + action * 0x300 + direction * 0x60 + fi * 8;
            let ox = if ooff + 8 <= self.graph.len() { anchor_x + i16(&self.graph, ooff + 4) } else { anchor_x };
            let oy = if ooff + 8 <= self.graph.len() { anchor_y + i16(&self.graph, ooff + 6) } else { anchor_y };
            layers.push(HumanLayer { im: oim, x: ox, y: oy });
        }

        (layers, body_x, body_y)
    }


    #[allow(clippy::too_many_arguments)]
    pub(super) fn compose_human(
        &self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        instances: &[u8],
        base_pos: usize,
        action: usize,
        direction: usize,
        fi: usize,
        palette_id: i32,
        count: usize,
    ) -> (Img, i32, i32, i32) {
        let (layers, _, _) = self.human_layers(
            equipment, weapon_slot, battle, instances, base_pos, action, direction, fi, palette_id,
        );

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
        let shift_x = 0x3C - body_x;
        let shift_y = 0x14 - body_y;
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
