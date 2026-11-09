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

use crate::constants::exe_tables::ITEM_DGROUP_FILE;
use crate::constants::exe_tables::ITEM_DGROUP_VA;

use crate::constants::exe_tables::TABLE1_VA;
use crate::constants::exe_tables::TABLE2_VA;
use crate::constants::exe_tables::TABLE3_VA;
use crate::constants::exe_tables::TABLE4_VA;

use crate::constants::exe_tables::NAME1_VA;
use crate::constants::exe_tables::NAME2_VA;
use crate::constants::exe_tables::NAME3_VA;
use crate::constants::exe_tables::NAME4_VA;

use crate::constants::exe_tables::ARMOR_STRIDE;
use crate::constants::exe_tables::ARMOR_TABLE_VA;
use crate::constants::exe_tables::ITEM_CONDITION_VALUE_STRIDE;
use crate::constants::exe_tables::ITEM_CONDITION_VALUE_VA;
use crate::constants::exe_tables::ITEM_DURABILITY_OFF;
use crate::constants::exe_tables::ITEM_LIFETIME_OFF;
use crate::constants::exe_tables::ITEM_MODIFIER_COUNT;
use crate::constants::exe_tables::ITEM_MODIFIER_STRIDE;
use crate::constants::exe_tables::ITEM_MODIFIER_TABLE_VA;
use crate::constants::exe_tables::SETTLEMENT_PROP_STRIDE;
use crate::constants::exe_tables::SETTLEMENT_PROP_TABLE_VA;
use crate::constants::exe_tables::ITEM_RANGE_OFF;
use crate::constants::exe_tables::ITEM_REQUIREMENT_STAT_OFF;
use crate::constants::exe_tables::ITEM_REQUIREMENT_VALUE_OFF;
use crate::constants::exe_tables::ITEM_VALUE_OFF_128;
use crate::constants::exe_tables::ITEM_VALUE_OFF_64;

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

fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// One 16-byte item-instance record (`Game.N` offset 0).
///
/// `+0x02` is the byte the original `FUN_0041aea8` reads as the weapon
/// modifier-table index (`0..=17`, `-1` = none). The `+0x01` byte is retained
/// separately; it is not the damage modifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemInstance {
    pub class: u8,
    pub condition: i8,
    pub modifier_index: i8,
    pub subtype: u8,
    pub current_durability: f32,
    pub max_durability: f32,
    /// Raw `u32` at `+0x04`. For class 12 (ammo) and class 9 (potions/jars)
    /// this is the stack/charge quantity, not an IEEE float.
    pub quantity: u32,
    pub poison: u16,
    pub flags: u16,
}

/// A fully resolved item: instance type plus its original icon, weight, combat
/// rating and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInfo {
    pub class: u8,
    pub subtype: u8,
    /// INTR sprite index (from the original template record at `+0x02`).
    pub icon: u16,
    /// Weight in original internal units (`i16` at `+0x00`; displayed `/1000`).
    ///
    /// Signed: `FUN_0041B7E4` sign-extends the high word (`>> 0x10` on the
    /// dword load), so a negative template value *reduces* the carried total.
    /// The only such item in the shipped data is class 11 subtype 11
    /// «Сын Луны» (`0xD8F0` = −10000 = −10.0 kg).
    pub weight: i32,
    /// Combat rating (`u16` at `0x459B7C + subtype*0x40`); summed as armor over
    /// equipment slots 3..=5 and used as the weapon base for the attack value.
    pub rating: u16,
    /// Prototype durability (`u16@+0x2E`, stride-`0x40` records only).
    pub durability: u16,
    /// Requirement stat id (`u16@+0x30`, stride-`0x40` records only).
    pub requirement_stat: u16,
    /// Requirement value (`u16@+0x32`, stride-`0x40` records only).
    pub requirement_value: u16,
    /// Prototype reach (`u16@+0x34`, stride-`0x40` records only).
    pub range: u16,
    /// Projectile lifetime in frames (`u8@+0x08`, read by `FUN_0041BF44`).
    pub projectile_lifetime: u8,
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
        let ptr = &game[o..o + ITEM_INSTANCE_SIZE];
        Some(ItemInstance {
            class: ptr[0],
            condition: ptr[1] as i8,
            modifier_index: ptr[2] as i8,
            subtype: ptr[3],
            current_durability: rd_f32(ptr, 4),
            max_durability: rd_f32(ptr, 8),
            quantity: u32::from_le_bytes([ptr[4], ptr[5], ptr[6], ptr[7]]),
            poison: rd_u16(ptr, 12) as u16,
            flags: rd_u16(ptr, 14) as u16,
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
        let weight = rd_u16(&self.konung, o) as i16 as i32;
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
        // The name/prototype record is a separate parallel table read at +0.
        let no = va_to_file(name_va) + subtype as usize * stride;
        if no + stride > self.konung.len() {
            return None;
        }
        let (durability, requirement_stat, requirement_value, range, projectile_lifetime) =
            if stride == 64 {
                (
                    rd_u16(&self.konung, no + ITEM_DURABILITY_OFF) as u16,
                    rd_u16(&self.konung, no + ITEM_REQUIREMENT_STAT_OFF) as u16,
                    rd_u16(&self.konung, no + ITEM_REQUIREMENT_VALUE_OFF) as u16,
                    rd_u16(&self.konung, no + ITEM_RANGE_OFF) as u16,
                    self.konung[no + ITEM_LIFETIME_OFF],
                )
            } else {
                (0, 0, 0, 0, 0)
            };
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
            durability,
            requirement_stat,
            requirement_value,
            range,
            projectile_lifetime,
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

    /// Damage value of weapon modifier `index` against `target_archetype`
    /// (`FUN_0041aea8`: table `0x462798`, stride `0x40`). `None` when the index
    /// is absent or the record targets a different archetype.
    pub fn modifier_damage(&self, index: i8, target_archetype: u8) -> Option<u16> {
        if index < 0 || index as usize >= ITEM_MODIFIER_COUNT {
            return None;
        }
        let o = va_to_file(ITEM_MODIFIER_TABLE_VA) + index as usize * ITEM_MODIFIER_STRIDE;
        if o + 4 > self.konung.len() {
            return None;
        }
        if rd_u16(&self.konung, o + 2) as u16 != target_archetype as u16 {
            return None;
        }
        Some(rd_u16(&self.konung, o) as u16)
    }

    /// Prototype combat rating by subtype (`0x459B7C + subtype*0x40`), used by
    /// the projectile damage path where only the bow subtype is stored.
    pub fn rating_for_subtype(&self, subtype: u8) -> u16 {
        let o = va_to_file(ARMOR_TABLE_VA) + subtype as usize * ARMOR_STRIDE;
        if o + 2 <= self.konung.len() {
            rd_u16(&self.konung, o) as u16
        } else {
            0
        }
    }

    /// Base object record index for settlement target key `key`
    /// (`FUN_0043ADF0` `0x43B7A7`). The effective prop object id is
    /// `base + target.state + 30`.
    pub fn settlement_prop_base(&self, key: usize) -> Option<u16> {
        if key >= 0x80 {
            return None;
        }
        let o = va_to_file(SETTLEMENT_PROP_TABLE_VA) + key * SETTLEMENT_PROP_STRIDE + 2;
        if o + 2 <= self.konung.len() {
            Some(rd_u16(&self.konung, o) as u16)
        } else {
            None
        }
    }

    /// Base trade value of one item instance (`FUN_0041B2A8`).
    ///
    /// The value is the prototype `price` field (`+0x36` for stride-`0x40`
    /// records, `+0x78` for stride-`0x80` records). For classes `0..=8` a
    /// condition bonus from `0x458530 + condition*0x10` is added when the item
    /// is identified (`flags & 1 == 0`; bit 0 is the `FUN_0041BA08` unidentified
    /// marker set by generated loot). Class 12 multiplies by the raw `u32`
    /// stack quantity at `+0x04`; class 9 treats `+0x04` as a float charge
    /// count, multiplies it by a negative signed price, rounds, then shifts the
    /// result left by 3 (`FUN_0041B2A8`).
    pub fn instance_trade_value(&self, inst: &ItemInstance) -> i32 {
        let Some((_, stride, name_va)) = table_for(inst.class) else {
            return 0;
        };
        let no = va_to_file(name_va) + inst.subtype as usize * stride;
        let value_off = if stride == 64 {
            ITEM_VALUE_OFF_64
        } else {
            ITEM_VALUE_OFF_128
        };
        let mut value = if no + value_off + 2 <= self.konung.len() {
            rd_i16(&self.konung, no + value_off) as i32
        } else {
            0
        };
        match inst.class {
            0..=8 => {
                if inst.condition >= 0 && inst.flags & 1 == 0 {
                    let co = va_to_file(ITEM_CONDITION_VALUE_VA)
                        + inst.condition as usize * ITEM_CONDITION_VALUE_STRIDE;
                    if co + 2 <= self.konung.len() {
                        value += rd_i16(&self.konung, co) as i32;
                    }
                }
                value.max(1)
            }
            12 => value.saturating_mul(inst.quantity as i32),
            9 => {
                if value < 0 {
                    // `FUN_0041B2A8`: `fld dword [instance+4]` (the float
                    // charge count), negate, multiply by the negative price,
                    // round, then `<<3`.
                    value = (-(inst.current_durability as f64) * value as f64).round() as i32;
                }
                value.saturating_mul(8)
            }
            _ => value,
        }
    }

    /// Trade value of item instance `id` in the loaded save.
    pub fn instance_trade_value_by_id(&self, game: &[u8], id: u16) -> Option<i32> {
        let inst = Self::decode_instance(game, id)?;
        Some(self.instance_trade_value(&inst))
    }

    /// Raw `u16` at `va + extra` in the loaded `konung.res` image. Used for the
    /// settlement restock recipe-level tables.
    pub fn read_va_u16(&self, va: u32, extra: usize) -> u16 {
        let o = va_to_file(va) + extra;
        if o + 2 <= self.konung.len() {
            rd_u16(&self.konung, o) as u16
        } else {
            0
        }
    }

    /// CP866 name suffix of weapon modifier `index` (e.g. `", на муравьев"`).
    pub fn modifier_name(&self, index: i8) -> Option<String> {
        if index < 0 || index as usize >= ITEM_MODIFIER_COUNT {
            return None;
        }
        let o = va_to_file(ITEM_MODIFIER_TABLE_VA) + index as usize * ITEM_MODIFIER_STRIDE + 0x14;
        if o + 0x2C > self.konung.len() {
            return None;
        }
        let raw = &self.konung[o..o + 0x2C];
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        if end == 0 {
            return None;
        }
        Some(decode_cp866(&raw[..end]))
    }
}

#[cfg(test)]
mod tests;
