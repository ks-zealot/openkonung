//! New-Hero character preview images.
//!
//! The original renderer (`FUN_0042EA6C`) composes the live creation entity into
//! a 160x160 surface (`FUN_00427E9C`, body origin `(0x3C,0x14)`) and blits it at
//! the zero-size NEWH draw point 22: `(470,127)` at 640 and `(570,160)` at 800.
//! The pose is the battle idle (`Entity+0x03 & 4`, action 0) at facing 5, with
//! the per-hero preview loadout set by the hero handlers
//! (`Entity+0x66..+0x70`).
//!
//! `Res::paper_doll_frame` is the same compositor used by the character sheet,
//! so this module only prepares the per-hero equipment and the three possible
//! active-weapon-slot (`Entity+0x12`) variants.

use crate::hero::{HeroTemplate, HEROES};
use crate::res::{Image, Img, Res};
use std::rc::Rc;

/// Number of pre-rendered active-weapon-slot variants per hero.
pub const PREVIEW_SLOTS: usize = 3;
/// Size of the offscreen preview surface (`FUN_0042EA6C`). 
pub const PREVIEW_SIZE: usize = 160;

/// Cosmetic preview loadout stored in `HeroTemplate.preset` (`Entity+0x66..`).
///
/// This is **not** the gameplay starting loadout: the original reloads `Game.N`
/// and keeps its `Entity[0]` equipment/inventory (knife + amulet part + balms).
pub fn preview_equipment(hero: &HeroTemplate) -> [u16; 6] {
    let p = &hero.preset;
    [
        p.field_66.unwrap_or(0),
        p.field_68.unwrap_or(0),
        p.field_6a.unwrap_or(0),
        p.field_6c.unwrap_or(0),
        p.field_6e.unwrap_or(0),
        p.field_70.unwrap_or(0),
    ]
}

fn empty_preview() -> Img {
    Rc::new(Image::new(PREVIEW_SIZE, PREVIEW_SIZE, [0, 0, 0, 0]))
}

/// Render `hero × weapon-slot` preview frames into a flat vector indexed by
/// `hero_index * PREVIEW_SLOTS + weapon_slot`.
///
/// `res` must already have `load_items_and_fonts` and an item-instance table
/// attached. The original reads the *current* runtime instance table; on a fresh
/// boot no `Game.N` has been loaded (`FUN_0043A870` has not run), so the caller
/// passes a zeroed `0x20000`-byte table.
pub fn compose_previews(res: &mut Res) -> Vec<Img> {
    let mut out = Vec::with_capacity(HEROES.len() * PREVIEW_SLOTS);
    for hero in HEROES.iter() {
        let equipment = preview_equipment(hero);
        for slot in 0..PREVIEW_SLOTS {
            let frame = res.paper_doll_frame(
                &equipment,
                slot as u8,
                true,
                hero.visual_index as usize,
                hero.palette_id(),
                0,
                5,
                0,
            );
            out.push(frame.unwrap_or_else(empty_preview));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_loadouts_match_original_hero_handlers() {
        assert_eq!(preview_equipment(&HEROES[0]), [3, 4, 0, 1, 8, 11]);
        assert_eq!(preview_equipment(&HEROES[1]), [0, 4, 5, 0, 7, 10]);
        assert_eq!(preview_equipment(&HEROES[2]), [0, 4, 5, 2, 6, 9]);
    }
}
