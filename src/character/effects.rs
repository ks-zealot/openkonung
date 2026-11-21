//! `FUN_0041C69C`: equipment condition-bonus recalculation.
//!
//! Every identified item whose instance `+0x01` condition byte is `>= 0`
//! contributes the three `(stat, op, i16 value)` triples of its 16-byte record
//! at `0x458530` (`+4/+5/+6`, `+8/+9/+10`, `+12/+13/+14`). The per-stat
//! accumulator `0x81B930` starts at 0 and the active byte `0x81CFB8` at 1:
//! `active &= op`; when the byte clears, the accumulator is overwritten with
//! the value, otherwise the value is added. The entity's own `+0xD2`/`+0xD3`
//! bytes (`state_d2`/`state_d3`) are two extra condition indices.
//!
//! Outputs: stats 2/3/5/6 are added to the **derived** dexterity/charisma/
//! strength/endurance (`+0x54/+0x58/+0x5E/+0x62`, taken from the stored
//! `+0x56/+0x5A/+0x60/+0x64`), stat 4 adds `value << 4` to HP (`+0x5C`,
//! capped at 1600), and stats 7/8/9 are the armor/damage/attack set bonuses
//! (`+0xC4/+0xC6/+0xC8`) gated by the `+0xD1` add flags (bits 0/1/2).

use crate::constants::exe_tables::{ITEM_CONDITION_VALUE_STRIDE, ITEM_CONDITION_VALUE_VA};
use crate::item::va_to_file;
use crate::res::Res;

/// The stored (`+0x56/+0x5A/+0x60/+0x64`, `+0x5C`) stat values that potions,
/// training and the level-up commands modify.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoredStats {
    /// `Entity+0x56` stored dexterity.
    pub dexterity: u16,
    /// `Entity+0x5A` stored charisma.
    pub charisma: u16,
    /// `Entity+0x5C` HP (internal 16x scale).
    pub hp: u16,
    /// `Entity+0x60` stored strength.
    pub strength: u16,
    /// `Entity+0x64` stored endurance.
    pub endurance: u16,
}

/// Values produced by the recalculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recompute {
    /// Derived `+0x54` dexterity.
    pub dexterity: u16,
    /// Derived `+0x58` charisma.
    pub charisma: u16,
    /// `+0x5C` HP after the stat-4 bonus (capped at 1600).
    pub hp: u16,
    /// Derived `+0x5E` strength.
    pub strength: u16,
    /// Derived `+0x62` endurance (minimum 1).
    pub endurance: u16,
    /// `+0xC4` armor set bonus (`i16`).
    pub armor_bonus: i16,
    /// `+0xC6` damage set bonus (`i16`).
    pub damage_bonus: i16,
    /// `+0xC8` attack set bonus (`i16`).
    pub attack_bonus: i16,
    /// `+0xD1` add-vs-replace flags, bits 0/1/2 for stats 7/8/9.
    pub bonus_flags: u8,
}

/// Read the 16-byte condition record at `0x458530 + condition*0x10`.
pub(crate) fn condition_record(res: &Res, condition: u8) -> Option<&[u8]> {    let o = va_to_file(ITEM_CONDITION_VALUE_VA)
        + condition as usize * ITEM_CONDITION_VALUE_STRIDE;
    if o + ITEM_CONDITION_VALUE_STRIDE > res.konung.len() {
        return None;
    }
    Some(&res.konung[o..o + ITEM_CONDITION_VALUE_STRIDE])
}

/// `FUN_0041C69C` for one owner: the six equipment slots, the five additional
/// slots and the two entity condition bytes.
pub fn recompute(
    res: &Res,
    equipment: &[u16; 6],
    additional: &[u16; 5],
    extra: [Option<i8>; 2],
    stored: StoredStats,
) -> Recompute {
    let mut acc = [0i32; 9];
    let mut active = [1u8; 9];

    let mut apply = |cond: i8| {
        if cond < 0 {
            return;
        }
        let Some(rec) = condition_record(res, cond as u8) else {
            return;
        };
        for base in [4usize, 8, 12] {
            let stat = rec[base] as usize;
            if stat == 0 || stat > 9 {
                continue;
            }
            let idx = stat - 1;
            active[idx] &= rec[base + 1];
            let value = i16::from_le_bytes([rec[base + 2], rec[base + 3]]) as i32;
            if active[idx] == 0 {
                acc[idx] = value;
            } else {
                acc[idx] = acc[idx].saturating_add(value);
            }
        }
    };

    for &id in equipment.iter().chain(additional.iter()) {
        let Some(inst) = res.item_instance(id) else {
            continue;
        };
        if inst.condition < 0 || inst.flags & 1 != 0 {
            continue;
        }
        apply(inst.condition);
    }
    for cond in extra.into_iter().flatten() {
        apply(cond);
    }

    // The original checks the accumulator first (`0` = no bonus at all), then
    // the active byte selects replace (`0`) vs add (`1`).
    let derived = |idx: usize, stored: u16| -> i32 {
        if acc[idx] == 0 {
            stored as i32
        } else if active[idx] == 0 {
            acc[idx]
        } else {
            acc[idx].saturating_add(stored as i32)
        }
    };

    let dexterity = derived(1, stored.dexterity).max(0).min(u16::MAX as i32) as u16;
    let charisma = derived(2, stored.charisma).max(0).min(u16::MAX as i32) as u16;
    let strength = derived(4, stored.strength).max(0).min(u16::MAX as i32) as u16;
    let endurance = derived(5, stored.endurance).max(1).min(u16::MAX as i32) as u16;

    let hp_bonus = acc[3].saturating_mul(16);
    let mut hp = stored.hp as i32;
    if hp_bonus != 0 {
        hp = if active[3] == 0 {
            hp_bonus
        } else {
            hp.saturating_add(hp_bonus)
        };
    }
    let hp = hp.min(1600).max(0) as u16;

    let mut bonus_flags = 0u8;
    for bit in 0..3 {
        if active[6 + bit] != 0 {
            bonus_flags |= 1 << bit;
        }
    }

    Recompute {
        dexterity,
        charisma,
        hp,
        strength,
        endurance,
        armor_bonus: acc[6] as i16,
        damage_bonus: acc[7] as i16,
        attack_bonus: acc[8] as i16,
        bonus_flags,
    }
}

/// Write one [`Recompute`] result into an `EntityStats` record.
pub fn apply(stats: &mut crate::population::EntityStats, r: &Recompute) {
    stats.dexterity_base = r.dexterity;
    stats.charisma_base = r.charisma;
    stats.hp_internal = r.hp;
    stats.strength_base = r.strength;
    stats.endurance_base = r.endurance;
    stats.armor_bonus = r.armor_bonus;
    stats.damage_bonus = r.damage_bonus;
    stats.attack_bonus = r.attack_bonus;
    stats.bonus_flags = r.bonus_flags;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::res::Res;

    /// Load the original install resources, or `None` when they are absent.
    fn test_res() -> Option<Res> {
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

    /// Allocate an item with a condition record index (`-1` = none).
    fn alloc(res: &mut Res, class: u8, subtype: u8, condition: i8, flags: u16) -> u16 {
        res.alloc_item_instance(class, subtype, condition, -1, 1.0, 0, flags)
            .expect("free instance")
    }

    fn base() -> StoredStats {
        StoredStats {
            dexterity: 10,
            charisma: 10,
            hp: 100,
            strength: 10,
            endurance: 10,
        }
    }

    /// No items: every derived value equals the stored one.
    #[test]
    fn empty_equipment_keeps_the_stored_stats() {
        let Some(res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let r = recompute(&res, &[0; 6], &[0; 5], [None, None], base());
        assert_eq!(
            (r.dexterity, r.charisma, r.hp, r.strength, r.endurance),
            (10, 10, 100, 10, 10)
        );
        assert_eq!(r.armor_bonus, 0);
        // `0x81CFB8` is initialised to 1, so the add flags start set; the
        // bonus value (0 here) is what gates their use.
        assert_eq!(r.bonus_flags, 0b111);
    }

    /// Condition 40 (`0x458530 + 40*0x10`) is stat 7 `+4`: the armor bonus, with
    /// the add flag set (bit 0).
    #[test]
    fn armor_condition_record_sets_the_c4_bonus() {
        let Some(mut res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let armor = alloc(&mut res, 3, 34, 40, 0);
        let mut equipment = [0u16; 6];
        equipment[3] = armor;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], base());
        assert_eq!(r.armor_bonus, 4);
        assert_eq!(r.bonus_flags & 1, 1);
    }

    /// Condition 53 is stat 4 `+80`: HP gains `80 << 4`, capped at 1600.
    #[test]
    fn hp_condition_adds_sixteen_times_the_value() {
        let Some(mut res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let armor = alloc(&mut res, 3, 34, 53, 0);
        let mut equipment = [0u16; 6];
        equipment[4] = armor;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], base());
        assert_eq!(r.hp, 100 + 80 * 16);
        // Already near full: clamped at the original 1600 internal cap.
        let mut full = base();
        full.hp = 1500;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], full);
        assert_eq!(r.hp, 1600);
    }

    /// Condition 60 has `(2, op 0, 100)` and `(6, op 1, -20)`: the first
    /// occurrence replaces (op 0 clears the active byte), the second adds.
    #[test]
    fn condition_ops_replace_or_add() {
        let Some(mut res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let item = alloc(&mut res, 0, 0, 60, 0);
        let mut equipment = [0u16; 6];
        equipment[0] = item;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], base());
        assert_eq!(r.dexterity, 100, "op 0 replaces the derived value");
        assert_eq!(r.endurance, 1, "10 - 20 clamped at the original minimum 1");
        assert_eq!(r.charisma, 10);
    }

    /// The entity's own `+0xD2` is one more condition index (record 19 =
    /// strength -5; the doll writes `0x13` there).
    #[test]
    fn state_d2_applies_a_condition_record() {
        let Some(res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let r = recompute(&res, &[0; 6], &[0; 5], [Some(0x13), None], base());
        assert_eq!(r.strength, 5);
    }

    /// Unidentified items (`+0x0E & 1`) contribute nothing.
    #[test]
    fn unidentified_items_are_ignored() {
        let Some(mut res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let armor = alloc(&mut res, 3, 34, 40, 1);
        let mut equipment = [0u16; 6];
        equipment[3] = armor;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], base());
        assert_eq!(r.armor_bonus, 0);
    }

    /// Two records on the same stat accumulate (`2 * 4` armor, flag stays set).
    #[test]
    fn same_stat_records_accumulate() {
        let Some(mut res) = test_res() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let a = alloc(&mut res, 3, 34, 40, 0);
        let b = alloc(&mut res, 3, 34, 40, 0);
        let mut equipment = [0u16; 6];
        equipment[3] = a;
        equipment[4] = b;
        let r = recompute(&res, &equipment, &[0; 5], [None, None], base());
        assert_eq!(r.armor_bonus, 8);
        assert_eq!(r.bonus_flags & 1, 1);
    }
}
