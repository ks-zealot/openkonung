//! `cursor` subsystem of [`crate::app`]: the original game-drawn mouse cursor.
//!
//! The original hides the Win32 cursor (`ShowCursor(0)`; WndProc `SetCursor(0)`)
//! and each frame blits one of 11 GRAPH cursor bitmaps at the pointer
//! (`FUN_0042A470` -> `FUN_0043D2FE`). The frame is chosen by `FUN_00428740`
//! (hover override) over the base id `_DAT_0081D660`, which `FUN_00424E0C` sets
//! from the active item/action class. This module ports the selection; the
//! bitmaps are decoded by [`crate::res::cursor`].

use super::*;
use crate::coords::local_px_to_entity;

/// Base cursor id for an item/action class byte, porting the `FUN_00424E0C`
/// jump table (`0x424E78`, cases `0..=0xC`, default `9`):
///
/// | class | 0,2 | 1,12 | 3,4,5 | 6,7,8,10,11,>12 | 9 |
/// |-------|-----|------|-------|-----------------|---|
/// | id    | 10  | 2    | 0     | 9               | 8 |
///
/// (`10`=sword, `2`=bow, `0`=chest/object, `8`=green/location, `9`=creature.)
pub fn cursor_for_class(class: u8) -> usize {
    match class {
        0 | 2 => 10,
        1 | 12 => 2,
        3 | 4 | 5 => 0,
        9 => 8,
        _ => 9,
    }
}

impl PlayableApp {
    /// Base cursor id `_DAT_0081D660` (`FUN_00424E0C`): the class of the item
    /// under the HUD pointer, when resolvable; otherwise the plain pointer `4`.
    fn cursor_base(&self) -> usize {
        if let Some(slot) = self.hovered_slot {
            if let Some(&id) = self.player.inventory().get(slot) {
                if let Some(db) = self.world.res.item_db.as_ref() {
                    if let Some(info) = db.resolve(&self.world.res.save_game, id) {
                        return cursor_for_class(info.class);
                    }
                }
            }
        }
        4
    }

    /// The original cursor frame index (`0..11`) for the current pointer, porting
    /// `FUN_00428740`: hovering a talkable human -> `5`, a monster -> `1`, a
    /// non-talkable human or blocked ground -> `3`, passable ground -> `7`;
    /// otherwise the base id (`0/2/4/8/9/10`).
    pub(super) fn cursor_index(&mut self) -> usize {
        let base = self.cursor_base();
        if self.mode_graph {
            return base;
        }
        let (cx, cy) = self.cursor_virtual;
        if cx < 0.0 || cy < 0.0 || cx >= VIRTUAL_W as f64 || cy >= VIRTUAL_H as f64 {
            return base;
        }
        // Modal screens keep the base cursor.
        if self.dialog.is_some() || self.character_screen {
            return base;
        }
        let vx = cx.round() as i32;
        let vy = cy.round() as i32;
        // Actor under the pointer (same projection as the click hit test).
        if let Some((_id, archetype, dialogue_id)) = self.actor_at(vx, vy) {
            if archetype & 0x40 == 0 {
                // Human: talkable (`dialogue_id != 0xFF`) -> talk, else "no".
                return if dialogue_id != 0xFF { 5 } else { 3 };
            }
            // Monster / object family -> attack.
            return 1;
        }
        // Ground: passable -> move, blocked -> "no".
        let lx = vx - self.detail_pan[0].round() as i32;
        let ly = vy - self.detail_pan[1].round() as i32;
        let (tx, ty) = local_px_to_entity(lx, ly);
        let free = self
            .world
            .loc(self.current_map)
            .map(|l| l.collision_free(tx, ty))
            .unwrap_or(true);
        if free {
            7
        } else {
            3
        }
    }
}
