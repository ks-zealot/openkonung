//! Original Konung item decoding.
//!
//! Two independent original data sources are combined:
//!
//! 1. **Item instances** — the *loaded save file* (`Game.0/1/2`) at offset 0,
//!    16 bytes each, indexed by the item-instance id stored in
//!    `Entity+0x66`/`+0x72`/`+0xB2`/`+0xCC`. `+0x00` = class, `+0x03` =
//!    subtype. The original loader (`FUN_0043a870`, string `GAME.` @ `0x450747`)
//!    reads `0x20000` bytes (= 8192 × 16) of this table from `Game.N`; the
//!    separate `Game.res` file is **not** the instance table for a loaded game.
//! 2. **Item templates / names** — records tables in `konung.res` DGROUP.
//!    The original selector (`FUN_0042ba40`) chooses a table by the instance
//!    *class* and indexes it by the *subtype*. The **icon/weight fields** are
//!    addressed from that selector base (icon = `u16@+0x02`, weight =
//!    `u16@+0x00`, both used directly by the strip renderer), while the CP866
//!    **name** lives in a parallel table with a *different* base, read at `+0`
//!    by the detail routine (`FUN_0042f5f8`).
//!
//! ```text
//! class 0..=5 or 12 -> icon/weight 0x459B88  name 0x459B50  stride  64
//! class 6..=8       -> icon/weight 0x45812A  name 0x4580B0  stride 128
//! class 9           -> icon/weight 0x4594CA  name 0x459450  stride 128
//! class 10..=11     -> icon/weight 0x460996  name 0x46091C  stride 128
//! ```
//!
//! For the stride-`0x40` table (classes `0..=5|12`) the record matches
//! `KONUNG_SOURCE_OF_TRUTH.md` §20 exactly: `name[44]@+00`, combat value
//! `@+0x2C`, durability `@+0x2E`, requirement `@+0x30/+0x32`, range `@+0x34`,
//! price `@+0x36`, weight `@+0x38`, inventory icon `@+0x3A`, world sprite
//! `@+0x3C`, equipped-visual `@+0x3E`.

use crate::names::decode_cp866;
use crate::res::i16 as rd_i16;
use crate::res::u16 as rd_u16;

/// Size of a save-file item-instance record (`Game.0/1/2`, offset 0).
pub const ITEM_INSTANCE_SIZE: usize = 16;

/// `konung.res` DGROUP mapping: VA `0x450000` == file offset `0x36200`.
const ITEM_DGROUP_VA: u32 = 0x450000;
const ITEM_DGROUP_FILE: usize = 0x36200;

/// Icon/weight table bases used by the original selector `FUN_0042ba40`
/// (`icon = u16@+0x02`, `weight = u16@+0x00`).
const TABLE1_VA: u32 = 0x459B88;
const TABLE2_VA: u32 = 0x45812A;
const TABLE3_VA: u32 = 0x4594CA;
const TABLE4_VA: u32 = 0x460996;

/// Parallel CP866 **name** table bases read by `FUN_0042f5f8` at `+0x00`.
const NAME1_VA: u32 = 0x459B50;
const NAME2_VA: u32 = 0x4580B0;
const NAME3_VA: u32 = 0x459450;
const NAME4_VA: u32 = 0x46091C;

/// Combat-value field of the stride-`0x40` prototype record (`name[44]@+00`
/// makes combat value `0x459B50 + 0x2C`). Read by `FUN_0041ab7c` (armor
/// contribution) and `FUN_0041aea8` (weapon base) as `u16@0x459B7C +
/// subtype*0x40`; only defined for classes `0..=5|12`.
const ARMOR_TABLE_VA: u32 = 0x459B7C;
/// Stride of the `0x459B7C` combat-value table.
const ARMOR_STRIDE: usize = 64;

/// Table `(icon_base_va, stride, name_base_va)` for an item class.
fn table_for(class: u8) -> Option<(u32, usize, u32)> {
    match class {
        0..=5 | 12 => Some((TABLE1_VA, 64, NAME1_VA)),
        6..=8 => Some((TABLE2_VA, 128, NAME2_VA)),
        9 => Some((TABLE3_VA, 128, NAME3_VA)),
        10..=11 => Some((TABLE4_VA, 128, NAME4_VA)),
        _ => None,
    }
}

fn va_to_file(va: u32) -> usize {
    (ITEM_DGROUP_FILE as i64 + (va as i64 - ITEM_DGROUP_VA as i64)) as usize
}

/// The class/subtype decoded from an item-instance record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemInstance {
    pub class: u8,
    pub subtype: u8,
}

/// A fully resolved item: instance type plus its original icon, weight, combat
/// rating and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInfo {
    pub class: u8,
    pub subtype: u8,
    /// INTR sprite index (from the original template record at `+0x02`).
    pub icon: u16,
    /// Weight in original internal units (`u16` at `+0x00`; displayed `/1000`).
    pub weight: u16,
    /// Combat rating (`u16` at `0x459B7C + subtype*0x40`); summed as armor over
    /// equipment slots 3..=5 and used as the weapon base for the attack value.
    pub rating: u16,
    /// `equipped_visual` (`i16` at the prototype record `+0x3E`): the 0..=23
    /// GRAPH human-overlay block index the actor renderer composites for an
    /// equipped item. `-1` means no overlay (and for classes `6..=11`, which
    /// have no such field in their stride-0x80 record).
    pub equipped_visual: i16,
    /// Original CP866 name decoded to Unicode.
    pub name: String,
}

/// Parsed item-template database sourced from `konung.res`.
pub struct ItemDb {
    konung: Vec<u8>,
}

impl ItemDb {
    /// Own a copy of `konung.res` and resolve templates from it.
    pub fn from_konung(konung: &[u8]) -> ItemDb {
        ItemDb {
            konung: konung.to_vec(),
        }
    }

    /// Decode the class/subtype of an item instance in a save file's instance
    /// table (`Game.0/1/2`, offset 0).
    ///
    /// Id `0` is the empty/absent slot and yields `None`; out-of-range ids and
    /// truncated buffers also yield `None` (never a panic).
    pub fn decode_instance(game: &[u8], id: u16) -> Option<ItemInstance> {
        if id == 0 {
            return None;
        }
        let o = id as usize * ITEM_INSTANCE_SIZE;
        if o + ITEM_INSTANCE_SIZE > game.len() {
            return None;
        }
        Some(ItemInstance {
            class: game[o],
            subtype: game[o + 3],
        })
    }

    /// Resolve a `(class, subtype)` pair to its original icon, weight, combat
    /// rating and name.
    pub fn info(&self, class: u8, subtype: u8) -> Option<ItemInfo> {
        let (base_va, stride, name_va) = table_for(class)?;
        let o = va_to_file(base_va) + subtype as usize * stride;
        if o + stride > self.konung.len() {
            return None;
        }
        let icon = rd_u16(&self.konung, o + 2) as u16;
        let weight = rd_u16(&self.konung, o) as u16;
        // Combat value lives only in the stride-0x40 (classes 0..=5|12) record.
        let rating = if matches!(class, 0..=5 | 12) {
            let rating_o = va_to_file(ARMOR_TABLE_VA) + subtype as usize * ARMOR_STRIDE;
            if rating_o + 2 <= self.konung.len() {
                rd_u16(&self.konung, rating_o) as u16
            } else {
                0
            }
        } else {
            0
        };
        // The name is a separate parallel table, read at +0.
        let no = va_to_file(name_va) + subtype as usize * stride;
        if no + stride > self.konung.len() {
            return None;
        }
        let raw = &self.konung[no..no + stride];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let name = decode_cp866(&raw[..end]);
        // `equipped_visual` (`+0x3E`) exists only in the stride-0x40 prototype
        // record (classes 0..=5|12); the stride-0x80 records have no such field.
        let equipped_visual = if matches!(class, 0..=5 | 12) {
            let ov = no + 0x3E;
            if ov + 2 <= self.konung.len() {
                rd_i16(&self.konung, ov) as i16
            } else {
                -1
            }
        } else {
            -1
        };
        Some(ItemInfo {
            class,
            subtype,
            icon,
            weight,
            rating,
            equipped_visual,
            name,
        })
    }

    /// Resolve an item-instance id against a save file's instance table
    /// (`Game.0/1/2`, offset 0). The `game` slice must be the loaded save.
    pub fn resolve(&self, game: &[u8], id: u16) -> Option<ItemInfo> {
        let inst = Self::decode_instance(game, id)?;
        self.info(inst.class, inst.subtype)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn load(name: &str) -> Option<Vec<u8>> {
        std::fs::read(install_dir().join(name)).ok()
    }

    fn db_and_game() -> Option<(ItemDb, Vec<u8>)> {
        let konung = load("konung.res")?;
        // Item instances come from the loaded save file, not Game.res.
        let game = load("Game.1")?;
        Some((ItemDb::from_konung(&konung), game))
    }

    #[test]
    fn instance_decode_reads_class_and_subtype() {
        let mut game = vec![0u8; 16 * 4];
        game[16] = 1; // id 1 class
        game[16 + 3] = 6; // id 1 subtype
        let e = ItemDb::decode_instance(&game, 1).unwrap();
        assert_eq!((e.class, e.subtype), (1, 6));
        // id 0 is the empty slot.
        assert!(ItemDb::decode_instance(&game, 0).is_none());
        // out of range.
        assert!(ItemDb::decode_instance(&game, 99).is_none());
        // truncated.
        assert!(ItemDb::decode_instance(&[0u8; 8], 1).is_none());
    }

    #[test]
    fn table_selection_by_class() {
        assert_eq!(table_for(0).unwrap().1, 64);
        assert_eq!(table_for(5).unwrap().1, 64);
        assert_eq!(table_for(12).unwrap().1, 64);
        assert_eq!(table_for(0).unwrap().2, NAME1_VA);
        assert_eq!(table_for(12).unwrap().2, NAME1_VA);
        assert_eq!(table_for(6).unwrap().1, 128);
        assert_eq!(table_for(8).unwrap().1, 128);
        assert_eq!(table_for(6).unwrap().2, NAME2_VA);
        assert_eq!(table_for(9).unwrap().0, TABLE3_VA);
        assert_eq!(table_for(9).unwrap().2, NAME3_VA);
        assert_eq!(table_for(10).unwrap().0, TABLE4_VA);
        assert_eq!(table_for(11).unwrap().0, TABLE4_VA);
        assert_eq!(table_for(11).unwrap().2, NAME4_VA);
        assert!(table_for(13).is_none());
    }

    #[test]
    fn real_starting_item_mapping() {
        let Some((db, game)) = db_and_game() else {
            eprintln!("skipping: konung.res / Game.1 not present");
            return;
        };
        // Dragomir's Game.1 Entity[0] loadout: (id, class, subtype, INTR icon).
        let expected = [
            (1u16, 2u8, 18u8, 150u16),
            (2, 11, 25, 99),
            (3, 9, 5, 71),
            (4, 9, 5, 71),
            (5, 9, 5, 71),
            (6, 9, 5, 71),
            (7, 9, 5, 71),
        ];
        for (id, class, subtype, icon) in expected {
            let info = db.resolve(&game, id).unwrap_or_else(|| panic!("item {id}"));
            assert_eq!((info.class, info.subtype), (class, subtype), "item {id} type");
            assert_eq!(info.icon, icon, "item {id} icon");
        }
    }

    #[test]
    fn real_starting_item_names() {
        let Some((db, game)) = db_and_game() else { return };
        let n = |id| db.resolve(&game, id).unwrap().name;
        assert_eq!(n(1), "Нож");
        assert_eq!(n(2), "Правая часть амулета Дракона");
        assert_eq!(n(3), "Лечебный бальзам");
        assert_eq!(n(7), "Лечебный бальзам");
    }

    #[test]
    fn real_starting_item_weight_and_rating() {
        let Some((db, game)) = db_and_game() else { return };
        // (item id, weight @ icon_base+0x00, combat value @ 0x459B7C + subtype*0x40)
        let expected = [
            (1u16, 200u16, 60u16),
            (2, 100, 0),
            (3, 500, 0),
        ];
        for (id, weight, rating) in expected {
            let info = db.resolve(&game, id).unwrap_or_else(|| panic!("item {id}"));
            assert_eq!(info.weight, weight, "item {id} weight");
            assert_eq!(info.rating, rating, "item {id} rating");
        }
    }

    #[test]
    fn equipped_visual_field() {
        let Some((db, game)) = db_and_game() else { return };
        let ov = |id| db.resolve(&game, id).unwrap().equipped_visual;
        // Game.1: id 1 = Нож (class2/sub18), id 288 = Нож, id 289 = Кольчуга.
        assert_eq!(ov(1), 4);
        assert_eq!(ov(288), 4);
        assert_eq!(ov(289), 8);
        // Non-equipment classes (amulet class 11, balm class 9) have no overlay.
        assert_eq!(ov(2), -1);
        assert_eq!(ov(3), -1);
    }

    #[test]
    fn unknown_class_is_none() {
        let Some((db, _)) = db_and_game() else { return };
        // Class 13 has no table in the original selector.
        assert!(db.info(13, 0).is_none());
    }
}
