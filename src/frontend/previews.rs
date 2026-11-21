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
//!
//! The compositor resolves the equipment ids through the live item-instance
//! table (`0x659B0C`). Before a `Game.N` is loaded the table is not the gameplay
//! save: the original New-Game initialisation (`FUN_00436214` case 3) writes a
//! static 12-entry default kit into it immediately before calling
//! `FUN_0042EA6C`. [`preview_instance_table`] reproduces that kit.

use crate::hero::{HeroTemplate, HEROES};
use crate::res::{Image, Img, Res};
use std::rc::Rc;

/// Number of pre-rendered active-weapon-slot variants per hero.
pub const PREVIEW_SLOTS: usize = 3;
/// Size of the offscreen preview surface (`FUN_0042EA6C`).
pub const PREVIEW_SIZE: usize = 160;

/// The static default kit written to the item-instance table (`0x659B0C`) by
/// `FUN_00436214` case 3 (New Game) before `FUN_0042EA6C` composes the preview.
///
/// Index = item-instance id; value = `(class, subtype)`. Every other byte of the
/// 16-byte instance record is zero. The prototypes come from `konung.res` and
/// give Dragomir's screenshot loadout: leather helmet, chainmail, mace, bow,
/// shield (ids `3,4,0,1,8,11`).
pub const DEFAULT_PREVIEW_INSTANCES: [(u8, u8); 12] = [
    (0x00, 0x00),
    (0x03, 0x24),
    (0x03, 0x22),
    (0x00, 0x03),
    (0x01, 0x06),
    (0x02, 0x00),
    (0x04, 0x34),
    (0x04, 0x2B),
    (0x04, 0x37),
    (0x05, 0x25),
    (0x05, 0x26),
    (0x05, 0x3D),
];

/// Build the `0x20000`-byte item-instance table used by the New-Hero preview.
pub fn preview_instance_table() -> Vec<u8> {
    let mut table = vec![0u8; 0x20000];
    for (id, &(class, subtype)) in DEFAULT_PREVIEW_INSTANCES.iter().enumerate() {
        let o = id * 16;
        table[o] = class;
        table[o + 3] = subtype;
    }
    table
}

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

/// Fully transparent 160x160 placeholder used when the paper-doll compositor
/// rejects a preview variant, so the blit still has a correctly sized surface.
fn empty_preview() -> Img {
    Rc::new(Image::new(PREVIEW_SIZE, PREVIEW_SIZE, [0, 0, 0, 0]))
}

/// Render `hero × weapon-slot` preview frames into a flat vector indexed by
/// `hero_index * PREVIEW_SLOTS + weapon_slot`.
///
/// `res` must already have `load_items_and_fonts` and the New-Game default
/// instance table ([`preview_instance_table`]) attached. The original reads the
/// *current* runtime instance table, which `FUN_00436214` case 3 fills with that
/// static kit before drawing the creation entity.
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

    /// Each hero's six `Entity+0x66..+0x70` preview item ids match the
    /// original per-hero handler loadout.
    #[test]
    fn preview_loadouts_match_original_hero_handlers() {
        assert_eq!(preview_equipment(&HEROES[0]), [3, 4, 0, 1, 8, 11]);
        assert_eq!(preview_equipment(&HEROES[1]), [0, 4, 5, 0, 7, 10]);
        assert_eq!(preview_equipment(&HEROES[2]), [0, 4, 5, 2, 6, 9]);
    }

    /// The table bytes match the `FUN_00436214` case-3 writes at `0x659B0C`.
    #[test]
    fn default_preview_instances_match_the_original_new_game_kit() {
        let table = preview_instance_table();
        assert_eq!(table.len(), 0x20000);
        for (id, &(class, subtype)) in DEFAULT_PREVIEW_INSTANCES.iter().enumerate() {
            let o = id * 16;
            assert_eq!(table[o], class, "instance {id} class");
            assert_eq!(table[o + 3], subtype, "instance {id} subtype");
            assert_eq!(&table[o + 1..o + 3], &[0, 0], "instance {id} spacer");
            assert_eq!(&table[o + 4..o + 16], &[0u8; 12], "instance {id} body");
        }
        // Dragomir's six preview ids resolve through this table to the armour,
        // helmet, mace, bow and shield seen in the live character-creation screen.
        let classes: Vec<(u8, u8)> = preview_equipment(&HEROES[0])
            .iter()
            .map(|&id| {
                let o = id as usize * 16;
                (table[o], table[o + 3])
            })
            .collect();
        assert_eq!(
            classes,
            [(0, 3), (1, 6), (0, 0), (3, 0x24), (4, 0x37), (5, 0x3D)]
        );
    }
}
