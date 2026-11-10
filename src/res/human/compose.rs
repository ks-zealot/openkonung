//! Human body + equipment overlay compositing.

use super::super::*;

/// One composited GRAPH layer: a decoded frame plus its anchor offsets relative
/// to the actor's draw origin.
pub(super) struct HumanLayer {
    /// Decoded RGBA frame.
    pub(super) im: Image,
    /// X anchor offset in screen pixels.
    pub(super) x: i32,
    /// Y anchor offset in screen pixels.
    pub(super) y: i32,
}

impl Res {
    /// GRAPH human-overlay selector (`equipped_visual`, prototype `+0x3E`) for
    /// an equipped item-instance id. The instance is decoded from `instances`
    /// (the loaded save for Game.N actors, or `Game.res` for the variant path)
    /// and resolved against the `konung.res` templates. `None` when the item has
    /// no body overlay (`equipped_visual` outside `0..=23`).
    pub(crate) fn overlay_selector(&self, instances: &[u8], id: u16) -> Option<usize> {
        let db = self.item_db.as_ref()?;
        let inst = crate::item::ItemDb::decode_instance(instances, id)?;
        let ov = db.info(inst.class, inst.subtype)?.equipped_visual;
        if (0..HUMAN_OVERLAY_SELECTOR_COUNT).contains(&ov) {
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
    pub(super) fn human_layers(
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
        let pid = palette_id.clamp(0, PALETTE_MAX_INDEX) as usize;
        let base_fid = human_mapped_frame(&self.graph, base_pos, action, direction, fi).0;
        let base_im = render_graph_4d28_frame(&self.graph, base_pos, &self.palettes[pid], base_fid);

        let goff = base_pos
            + HUMAN_BODY_ANCHOR_TABLE_OFFSET
            + action * HUMAN_BODY_ACTION_STRIDE
            + direction * HUMAN_BODY_DIRECTION_STRIDE
            + fi * HUMAN_ANCHOR_RECORD_SIZE;
        let body_x = if goff + 8 <= self.graph.len() {
            i16(&self.graph, goff + 4)
        } else {
            0
        };
        let body_y = if goff + 8 <= self.graph.len() {
            i16(&self.graph, goff + 6)
        } else {
            0
        };

        let aoff = base_pos
            + HUMAN_OVERLAY_ANCHOR_TABLE_OFFSET
            + action * HUMAN_OVERLAY_ACTION_STRIDE
            + direction * HUMAN_OVERLAY_DIRECTION_STRIDE
            + fi * HUMAN_OVERLAY_ANCHOR_RECORD_SIZE;
        let anchor_x = if aoff + 4 <= self.graph.len() {
            i16(&self.graph, aoff)
        } else {
            0
        };
        let anchor_y = if aoff + 4 <= self.graph.len() {
            i16(&self.graph, aoff + 2)
        } else {
            0
        };

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
            let opid = (self.overlay_palette_offsets[selector] / PALETTE_OFFSET_SCALE) as usize;
            let oim = render_graph_4d28_frame(
                &self.graph,
                opos,
                &self.palettes[opid.min(PALETTE_MAX_INDEX as usize)],
                ofid,
            );
            let ooff = opos
                + HUMAN_BODY_ANCHOR_TABLE_OFFSET
                + action * HUMAN_BODY_ACTION_STRIDE
                + direction * HUMAN_BODY_DIRECTION_STRIDE
                + fi * HUMAN_ANCHOR_RECORD_SIZE;
            let ox = if ooff + 8 <= self.graph.len() {
                anchor_x + i16(&self.graph, ooff + 4)
            } else {
                anchor_x
            };
            let oy = if ooff + 8 <= self.graph.len() {
                anchor_y + i16(&self.graph, ooff + 6)
            } else {
                anchor_y
            };
            layers.push(HumanLayer {
                im: oim,
                x: ox,
                y: oy,
            });
        }

        (layers, body_x, body_y)
    }

    /// Flatten the body and equipped-overlay layers into one sprite.
    ///
    /// The canvas is the union bounding box of every layer, drawing each in
    /// plan order with the straight "over" operator. Returns
    /// `(image, min_x, min_y, frame_count)` where the offsets are the canvas
    /// origin relative to the actor anchor.
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
            equipment,
            weapon_slot,
            battle,
            instances,
            base_pos,
            action,
            direction,
            fi,
            palette_id,
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
            blit(
                &mut canvas.data,
                cw,
                ch,
                &l.im.data,
                l.im.w,
                l.im.h,
                l.x - minx,
                l.y - miny,
            );
        }
        (Rc::new(canvas), minx, miny, count as i32)
    }
}
