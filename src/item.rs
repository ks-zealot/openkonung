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

pub use crate::constants::exe_tables::ITEM_INSTANCE_SIZE;

use crate::constants::exe_tables::ITEM_DGROUP_VA;
use crate::constants::exe_tables::ITEM_DGROUP_FILE;

use crate::constants::exe_tables::TABLE1_VA;
use crate::constants::exe_tables::TABLE2_VA;
use crate::constants::exe_tables::TABLE3_VA;
use crate::constants::exe_tables::TABLE4_VA;

use crate::constants::exe_tables::NAME1_VA;
use crate::constants::exe_tables::NAME2_VA;
use crate::constants::exe_tables::NAME3_VA;
use crate::constants::exe_tables::NAME4_VA;

use crate::constants::exe_tables::ARMOR_TABLE_VA;
use crate::constants::exe_tables::ARMOR_STRIDE;

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
    /// World/ground sprite id (`FUN_00438CC8`). The original loader resolves
    /// this from the icon/weight table at `+4` of the class record, except
    /// class 12 (stackable arrows) = `0x99` and classes 10/other = `0xA3`.
    /// Class 9 subtypes 1/2 (Земляной орех / Белый корень) are `0xA0`/`0xA8`.
    pub world_sprite: u16,
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
        // `FUN_00438CC8`: per-class world sprite; class 10 and unknown classes
        // fall through to the generic loot icon `0xA3`, class 12 is hardcoded.
        let world_sprite = match class {
            0..=9 | 11 => rd_u16(&self.konung, o + 4) as u16,
            12 => 0x99,
            _ => 0xA3,
        };
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
            world_sprite,
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
mod tests;
