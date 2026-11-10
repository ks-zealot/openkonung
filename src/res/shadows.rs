//! Actor and equipped-item **ground shadows** (`FUN_0043C040`, `FUN_0043D247`).
//!
//! GRAPH animation frames carry a dedicated shadow mask: the 8-byte frame record
//! `{u32 shadow_offset, u32 sprite_offset}` lives at
//!
//! ```text
//! human  block + 0x1728 + frame_id*8      (data base block + 0x4D28)
//! object block + 0x0728 + frame_id*8      (data base block + 0x3228)
//! ```
//!
//! and the per-frame anchors are a separate 8-byte record
//!
//! ```text
//! human  block + 0x2F28 + action*0x300 + direction*0x60 + fi*8
//! object block + 0x1728 + row*0x480    + direction*0x90 + fi*8
//! ```
//!
//! whose **first** `u16` pair is the shadow anchor and whose second pair is the
//! sprite anchor (`FUN_00414078` writes `+0xDC`/`+0xE0` from exactly these).
//!
//! The world render pass (`FUN_00426384`) collects the body mask of every
//! visible actor, the mask of every equipped-item overlay (`FUN_00424ED0`) and
//! every projectile mask, then calls `FUN_0043D247` to halve those pixels.
//!
//! Verified sample data (shipped resources): the first human base frame 0 has
//! `w=43 h=85` at offset 0 and its shadow `w=107 h=22` at offset 2167; object 0
//! frame 0 has a 237×43 mask.

use super::*;
use crate::constants::combat::OBJECT_ARCHETYPE_BIT;

/// Shadow data for one human block frame (body or equipped-item overlay).
struct HumanShadow {
    block_pos: usize,
    off: usize,
    gx: i32,
    gy: i32,
}

impl Res {
    fn human_shadow(
        &self,
        block_pos: usize,
        action: usize,
        direction: usize,
        fi: usize,
    ) -> Option<HumanShadow> {
        let (frame_id, _) = human_mapped_frame(&self.graph, block_pos, action, direction, fi);
        let rec = block_pos + HUMAN_ANCHOR_TABLE_OFFSET + frame_id * HUMAN_ANCHOR_RECORD_SIZE;
        if rec + 8 > self.graph.len() {
            return None;
        }
        let off = u32(&self.graph, rec) as usize;
        let data_base = block_pos as u64 + HUMAN_BLOCK_DATA_OFFSET as u64;
        if off == 0 || data_base + off as u64 + 4 > self.graph.len() as u64 {
            return None;
        }
        let ao = block_pos as u64
            + HUMAN_BODY_ANCHOR_TABLE_OFFSET as u64
            + action as u64 * HUMAN_BODY_ACTION_STRIDE as u64
            + (direction & 7) as u64 * HUMAN_BODY_DIRECTION_STRIDE as u64
            + fi as u64 * HUMAN_ANCHOR_RECORD_SIZE as u64;
        if ao + 4 > self.graph.len() as u64 {
            return None;
        }
        let ao = ao as usize;
        Some(HumanShadow {
            block_pos,
            off,
            gx: i16(&self.graph, ao) as i32,
            gy: i16(&self.graph, ao + 2) as i32,
        })
    }

    fn object_shadow(
        &self,
        object_id: usize,
        action: usize,
        direction: usize,
        fi: usize,
    ) -> Option<(usize, i32, i32)> {
        let blk = self.object_block(object_id);
        if blk.len() < 0x3228 + 8 {
            return None;
        }
        let (frame_id, count) = self.monster_mapped_frame(object_id, action, direction, fi);
        let rec = 0x728 + frame_id * 8;
        if rec + 8 > blk.len() {
            return None;
        }
        let off = u32(blk, rec) as usize;
        if off == 0 || 0x3228u64 + off as u64 + 4 > blk.len() as u64 {
            return None;
        }
        let row = Self::object_row(action);
        let fi = if count == 0 { 0 } else { fi % count };
        let ao = super::objects::monster_anchor_offset(row, direction, fi);
        if ao + 4 > blk.len() {
            return None;
        }
        Some((off, i16(blk, ao) as i32, i16(blk, ao + 2) as i32))
    }

    /// Append the ground-shadow spans of one actor frame.
    ///
    /// `origin` is the actor cell's projected **screen** position (camera pan
    /// already applied); the mask anchors are added on top. `clip` is the
    /// inclusive viewport rectangle. Returns `true` when at least one mask was
    /// added.
    ///
    /// Human actors also contribute one mask per equipped item overlay, in the
    /// same draw order `FUN_00424ED0` uses. Object/monster actors have no
    /// overlays.
    #[allow(clippy::too_many_arguments)]
    pub fn add_actor_shadow(
        &self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        archetype: u8,
        visual_index: u8,
        action: usize,
        facing: u8,
        anim_frame_index: usize,
        origin: (i32, i32),
        clip: crate::render::spans::Clip,
        out: &mut Vec<crate::render::spans::Span>,
    ) -> bool {
        if archetype & OBJECT_ARCHETYPE_BIT != 0 {
            let Some((off, gx, gy)) = self.object_shadow(
                visual_index as usize,
                action,
                facing as usize,
                anim_frame_index,
            ) else {
                return false;
            };
            let blk = self.object_block(visual_index as usize);
            let payload = &blk[0x3228..];
            return crate::render::spans::add_mask(
                payload,
                off,
                origin.0 + gx,
                origin.1 + gy,
                clip,
                out,
            );
        }

        let base_id = visual_index as usize;
        let Some(&base_pos) = self.human_base_positions.get(base_id) else {
            return false;
        };
        let direction = (facing as usize) & 7;
        let mut added = false;

        if let Some(s) = self.human_shadow(base_pos, action, direction, anim_frame_index) {
            let payload = &self.graph[s.block_pos + HUMAN_BLOCK_DATA_OFFSET..];
            added |= crate::render::spans::add_mask(
                payload,
                s.off,
                origin.0 + s.gx,
                origin.1 + s.gy,
                clip,
                out,
            );
        }

        // `FUN_00424ED0`: every equipped overlay contributes its own mask, in
        // the same battle/peace draw order as the sprite compositor. The overlay
        // block's shadow anchor is relative to the body block's overlay anchor
        // (`HUMAN_OVERLAY_ANCHOR_TABLE_OFFSET`), so both offsets are applied.
        for (opos, ofi, ax, ay) in self.overlay_shadow_blocks(
            equipment,
            weapon_slot,
            battle,
            base_pos,
            action,
            direction,
            anim_frame_index,
        ) {
            if let Some(s) = self.human_shadow(opos, action, direction, ofi) {
                let payload = &self.graph[s.block_pos + HUMAN_BLOCK_DATA_OFFSET..];
                added |= crate::render::spans::add_mask(
                    payload,
                    s.off,
                    origin.0 + s.gx + ax,
                    origin.1 + s.gy + ay,
                    clip,
                    out,
                );
            }
        }
        added
    }

    /// Overlay blocks active for this pose: `(block_pos, fi, anchor_x, anchor_y)`.
    fn overlay_shadow_blocks(
        &self,
        equipment: &[u16; 6],
        weapon_slot: u8,
        battle: bool,
        base_pos: usize,
        action: usize,
        direction: usize,
        fi: usize,
    ) -> Vec<(usize, usize, i32, i32)> {
        let (order, variant) = super::human::overlay_plan(battle, weapon_slot, action, direction);
        let mut out = Vec::new();
        for &slot in order.iter() {
            let handle = equipment.get(slot).copied().unwrap_or(0);
            if handle == 0 {
                continue;
            }
            let Some(mut selector) = self.overlay_selector(&self.save_game, handle) else {
                continue;
            };
            if variant.get(slot).copied().unwrap_or(false)
                && selector + 1 < self.human_overlay_positions.len()
            {
                selector += 1;
            }
            let Some(&opos) = self.human_overlay_positions.get(selector) else {
                continue;
            };
            let (_, count) = human_mapped_frame(&self.graph, opos, action, direction, fi);
            let ofi = if count == 0 { 0 } else { fi % count };
            let aoff = base_pos as u64
                + HUMAN_OVERLAY_ANCHOR_TABLE_OFFSET as u64
                + action as u64 * HUMAN_OVERLAY_ACTION_STRIDE as u64
                + direction as u64 * HUMAN_OVERLAY_DIRECTION_STRIDE as u64
                + ofi as u64 * HUMAN_OVERLAY_ANCHOR_RECORD_SIZE as u64;
            let (ax, ay) = if aoff + 4 <= self.graph.len() as u64 {
                (
                    i16(&self.graph, aoff as usize) as i32,
                    i16(&self.graph, aoff as usize + 2) as i32,
                )
            } else {
                (0, 0)
            };
            out.push((opos, ofi, ax, ay));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load() -> Option<Res> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let read = |n: &str| std::fs::read(dir.join(n)).ok();
        let (game, graph, objects, exe, konung, save) = (
            read("Game.res")?,
            read("GRAPH.RES")?,
            read("Objects.res")?,
            read("CLIENT.EXE")?,
            read("konung.res")?,
            read("Game.1")?,
        );
        let mut res = Res::new(game, graph, objects, exe);
        res.load_items_and_fonts(&konung);
        res.set_save_game(&save);
        Some(res)
    }

    const CLIP: crate::render::spans::Clip = (0, 0, 799, 599);

    #[test]
    fn a_human_body_frame_contributes_a_ground_shadow() {
        let Some(res) = load() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let mut out = Vec::new();
        assert!(res.add_actor_shadow(
            &[0u16; 6],
            0,
            false,
            0,
            1,
            crate::anim::idle_action(false),
            0,
            0,
            (400, 300),
            CLIP,
            &mut out,
        ));
        assert!(!out.is_empty(), "the body frame has a shadow mask");
        // The shipped shadow mask is a wide, flat ground ellipse anchored near
        // the feet: it must stay inside a small box below/around the origin.
        for s in &out {
            assert!(s.row >= 300 - 120 && s.row <= 300 + 120, "row {}", s.row);
            assert!(s.x1 >= s.x0);
            assert!(s.x0 >= 0 && s.x1 <= 799);
        }
    }

    #[test]
    fn equipped_overlays_add_their_own_masks() {
        let Some(res) = load() else {
            return;
        };
        let mut bare = Vec::new();
        res.add_actor_shadow(
            &[0u16; 6],
            0,
            false,
            0,
            1,
            crate::anim::idle_action(false),
            0,
            0,
            (400, 300),
            CLIP,
            &mut bare,
        );
        // ????? (Game.1 entity 277): equipment [0,0,288,289,0,0].
        let mut dressed = Vec::new();
        res.add_actor_shadow(
            &[0, 0, 288, 289, 0, 0],
            2,
            false,
            0,
            1,
            crate::anim::idle_action(false),
            0,
            0,
            (400, 300),
            CLIP,
            &mut dressed,
        );
        assert!(
            dressed.len() > bare.len(),
            "armour/knife overlays add shadow spans ({} vs {})",
            dressed.len(),
            bare.len()
        );
    }

    #[test]
    fn an_object_frame_contributes_a_ground_shadow() {
        let Some(res) = load() else {
            return;
        };
        // Object 0 is a shipped actor block with a 237x43 ground mask.
        let mut out = Vec::new();
        assert!(res.add_actor_shadow(
            &[0u16; 6],
            0,
            false,
            0x41,
            0,
            0,
            0,
            0,
            (400, 300),
            CLIP,
            &mut out,
        ));
        assert!(!out.is_empty());
        let min_x = out.iter().map(|s| s.x0).min().unwrap();
        let max_x = out.iter().map(|s| s.x1).max().unwrap();
        let min_y = out.iter().map(|s| s.row).min().unwrap();
        let max_y = out.iter().map(|s| s.row).max().unwrap();
        assert!(
            max_x - min_x > 50,
            "wide ground mask: x {min_x}..{max_x}, y {min_y}..{max_y}, spans {}",
            out.len()
        );
        assert!(max_y - min_y >= 5, "multi-row mask: y {min_y}..{max_y}");
    }

    #[test]
    fn the_config_gate_is_a_plain_flag() {
        // `0x81D620` is `GameConfig::shadows`; the draw pass is skipped when 0.
        assert_eq!(crate::config::GameConfig::DEFAULT.shadows, 1);
    }
}

#[cfg(test)]
mod sweep {
    use super::*;

    fn load() -> Option<Res> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let read = |n: &str| std::fs::read(dir.join(n)).ok();
        let (game, graph, objects, exe, konung, save) = (
            read("Game.res")?,
            read("GRAPH.RES")?,
            read("Objects.res")?,
            read("CLIENT.EXE")?,
            read("konung.res")?,
            read("Game.1")?,
        );
        let mut res = Res::new(game, graph, objects, exe);
        res.load_items_and_fonts(&konung);
        res.set_save_game(&save);
        Some(res)
    }

    #[test]
    fn every_shipped_frame_decodes_without_panicking() {
        let Some(res) = load() else { return };
        let clip = (0, 0, 799, 599);
        let mut out = Vec::new();
        let loadouts: [[u16; 6]; 3] = [[0; 6], [0, 0, 288, 289, 0, 0], [1, 2, 3, 4, 5, 6]];
        for equipment in loadouts {
            for battle in [false, true] {
                for slot in 0..3u8 {
                    for base in 0..res.human_base_positions.len().min(3) as u8 {
                        for action in 0..10 {
                            for dir in 0..8u8 {
                                for fi in 0..18 {
                                    out.clear();
                                    res.add_actor_shadow(
                                        &equipment,
                                        slot,
                                        battle,
                                        0,
                                        base,
                                        action,
                                        dir,
                                        fi,
                                        (400, 300),
                                        clip,
                                        &mut out,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        for object in 0..30u8 {
            for action in 0..12 {
                for dir in 0..8u8 {
                    for fi in 0..18 {
                        out.clear();
                        res.add_actor_shadow(
                            &[0u16; 6],
                            0,
                            false,
                            0x41,
                            object,
                            action,
                            dir,
                            fi,
                            (400, 300),
                            clip,
                            &mut out,
                        );
                    }
                }
            }
        }
    }
}
