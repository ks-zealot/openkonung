//! Corpse-loot generation (`FUN_00418480`).
//!
//! Verified rules:
//! - money + the dead entity's 32 inventory ids (up to 22 slots);
//! - `Entity+0x52 != 0` always adds class-9 subtype-0 "Ядовитое жало" with the
//!   entity's poison value in instance `+0x0C`;
//! - a generated item is rolled only when `killer_level != 0 && rand()%101 > 85`
//!   (melee passes level 0, so melee corpses never get a random item).

use super::OriginalRng;
use crate::item::{ItemDb, ItemInfo};

/// New instance fields for one generated corpse item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootItemSpec {
    /// Item class byte (instance `+0x00`), e.g. `9` for the poison stinger.
    pub class: u8,
    /// Item subtype byte (instance `+0x03`), indexing the class prototype table.
    pub subtype: u8,
    /// `FUN_00418480` writes the generated quality roll to instance `+0x01`
    /// (condition), not the `+0x02` weapon-modifier index.
    pub condition: i8,
    /// Always `-1` for generated/monster loot; `+0x02` stays untouched.
    pub modifier: i8,
    /// Prototype durability copied from the item record (instance `+0x06`).
    pub durability: f32,
    /// Poison value (instance `+0x0C`); non-zero only for the stinger.
    pub poison: u16,
    /// Item flag word (instance `+0x0A`); bit `0x0001` = unidentified.
    pub flags: u16,
}

/// The full corpse contents produced by `FUN_00418480` before the marker is
/// written: copied inventory ids, newly allocated instances, and money.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LootPlan {
    /// Existing instance ids copied from the dead entity's inventory.
    pub existing: Vec<u16>,
    /// New instances to allocate in the shared item table.
    pub generated: Vec<LootItemSpec>,
    /// Total money placed on the corpse (may be negative? original stores the
    /// signed sum; callers clamp).
    pub money: i32,
}

impl LootPlan {
    /// True when the corpse would carry nothing at all.
    pub fn is_empty(&self) -> bool {
        self.existing.is_empty() && self.generated.is_empty() && self.money == 0
    }
}

/// `FUN_00417144`: generated-item modifier range by killer level.
pub fn level_tier(level: u8) -> u32 {
    if level < 4 {
        3
    } else if level < 7 {
        6
    } else if level < 10 {
        8
    } else {
        10
    }
}

/// Generated-item quality count per killer level: `level/5 + 1` (at least 1).
fn level_div(level: u8) -> u32 {
    level as u32 / 5 + 1
}

/// The prototype durability `f32` for a generated item, or `0.0` when the
/// class/subtype has no prototype record.
fn proto_durability(db: &ItemDb, class: u8, subtype: u8) -> f32 {
    db.info(class, subtype)
        .map(|i| i.durability as f32)
        .unwrap_or(0.0)
}

/// Unidentified-item roll for a generated drop: only `rand()%101 < 10` can
/// flag it, and then only when `identification < rand()%100`. Returns `1` when
/// the item starts unidentified.
fn unidentified(rng: &mut OriginalRng, identification: u16) -> u16 {
    if rng.roll() < 10 && (identification as u32) < rng.next_u32() % 100 {
        1
    } else {
        0
    }
}

/// Generated condition/quality value: a `tier`-bounded roll plus a `rand()%5`
/// decade term. Written to instance `+0x01`.
fn modifier(rng: &mut OriginalRng, tier: u32) -> i8 {
    let local = if tier < 2 { 0 } else { rng.next_u32() % tier };
    (local + (rng.next_u32() % 5) * 10) as i8
}

/// Build the original corpse-loot plan from the dead entity's authored state.
pub fn plan_corpse_loot(
    db: &ItemDb,
    rng: &mut OriginalRng,
    money: i32,
    inventory: &[u16; 32],
    entity_0x52: u16,
    killer_level: u8,
    bonus: i32,
    identification: u16,
) -> LootPlan {
    let mut plan = LootPlan::default();
    let inv: Vec<u16> = inventory.iter().copied().filter(|&id| id != 0).collect();
    if inv.is_empty() && money == 0 && entity_0x52 == 0 {
        return plan;
    }
    // `FUN_00418480`: money += signed entity money + rand()%bonus (bonus<2 -> 0).
    let extra = if bonus >= 2 {
        (rng.next_u32() % bonus as u32) as i32
    } else {
        0
    };
    plan.money = money.saturating_add(extra);
    for id in inv.into_iter().take(22) {
        plan.existing.push(id);
    }
    if plan.existing.len() >= 22 {
        return plan;
    }
    if entity_0x52 != 0 {
        plan.generated.push(LootItemSpec {
            class: 9,
            subtype: 0,
            condition: -1,
            modifier: -1,
            durability: 1.0,
            poison: entity_0x52,
            flags: 0,
        });
        if plan.existing.len() + plan.generated.len() >= 22 {
            return plan;
        }
    }
    if killer_level == 0 {
        return plan;
    }
    let roll = rng.roll();
    if roll <= 85 {
        return plan;
    }
    let tier = level_tier(killer_level);
    let div = level_div(killer_level);
    let local = if div < 2 { 0 } else { rng.next_u32() % div };
    let (class, subtype, durability, unidentified_chance) = if roll == 86 {
        let subtype = (local + (rng.next_u32() % 9) * 3) as u8;
        let class = match subtype % 9 {
            0..=2 => 2,
            3..=5 => 0,
            _ => 1,
        };
        (class, subtype, proto_durability(db, class, subtype), 10)
    } else if roll == 87 {
        let variant = rng.next_u32() % 3;
        let (class, base) = match variant {
            0 => (3, 0x22),
            1 => (5, 0x25),
            _ => (4, 0x28),
        };
        let subtype = (local + (rng.next_u32() % 3) * 11 + base) as u8;
        (class, subtype, proto_durability(db, class, subtype), 7)
    } else if roll < 0x5f {
        (8, (local + 6) as u8, 0.0, 7)
    } else if roll < 0x62 {
        (7, (local + 3) as u8, 0.0, 7)
    } else {
        (6, local as u8, 0.0, 7)
    };
    let mut flags = 0;
    if unidentified_chance > 0 && unidentified(rng, identification) != 0 {
        flags |= 1;
    }
    plan.generated.push(LootItemSpec {
        class,
        subtype,
        condition: modifier(rng, tier),
        modifier: -1,
        durability,
        poison: 0,
        flags,
    });
    plan
}

/// Name lookup helper used by diagnostics/tests.
pub fn item_name(db: &ItemDb, class: u8, subtype: u8) -> Option<String> {
    db.info(class, subtype).map(|i: ItemInfo| i.name)
}
