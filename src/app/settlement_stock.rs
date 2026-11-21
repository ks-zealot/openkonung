//! Settlement map-entry stock seeding (`FUN_00418E6C`).
//!
//! `FUN_0043ADF0` (mode 0) calls the original for **every settlement of the
//! loaded location**, right after the target-list pass. It fills the stock that
//! the restock producers (`FUN_004175FC` role 2, `FUN_004179C8` role 4) later
//! top up and that role 3 — which has no producer at all — sells.
//!
//! The ladder below follows the decompiled blocks one by one. The RNG draw
//! order is preserved because it consumes the shared game `rand()` stream:
//! role-2 jars, the three class-6/7/8 goods, the class-11 trinkets, the three
//! weapon slots, the six armor slots, then the fixed eighteen-slot block.

use super::*;
use crate::combat::loot::level_tier;
use crate::actors::DeterministicRng;
use crate::res::Res;

/// `FUN_00417190`: `(x + 1) & 3` mapped to the `0..=2` kit range.
fn kit_17190(x: u32) -> u8 {
    match (x + 1) & 3 {
        0 => 2,
        1 => 1,
        _ => 0,
    }
}

/// `FUN_004170E0`: `(rand() % 2 + x + 1) % 3` mapped to the `0..=2` kit range.
fn kit_170e0(rng: &mut DeterministicRng, x: u32) -> u8 {
    match (rng.next_u32() % 2 + x + 1) % 3 {
        0 => 2,
        1 => 1,
        _ => 0,
    }
}

/// `0` when the span is below two, otherwise `rand() % span`.
fn span_draw(rng: &mut DeterministicRng, span: u32) -> u8 {
    if span < 2 {
        0
    } else {
        (rng.next_u32() % span) as u8
    }
}

/// Prototype durability (`u16@+0x2E`) of one subtype record.
fn durability(res: &Res, class: u8, subtype: u8) -> f32 {
    res.item_info_for(class, subtype)
        .map(|i| i.durability as f32)
        .unwrap_or(0.0)
}

/// Allocate a good the original leaves without a durability (`class 6/7/8/11`).
fn alloc_plain(res: &mut Res, class: u8, subtype: u8) -> Option<u16> {
    res.alloc_item_instance(class, subtype, 0, -1, 0.0, 0, 0)
}

/// Allocate a weapon/armor piece with `+0x04 = +0x08 = prototype durability`.
fn alloc_gear(res: &mut Res, class: u8, subtype: u8) -> Option<u16> {
    let dur = durability(res, class, subtype);
    res.alloc_item_instance(class, subtype, 0, -1, dur, 0, 0)
}

/// Unconditional condition roll: `((rand%4)+1)*10 + span*3 + rand%2`.
fn cond_plus_one(rng: &mut DeterministicRng, level: u8) -> i8 {
    let span = span_draw(rng, level as u32 / 8 + 1);
    let tens = rng.next_u32() % 4;
    let units = rng.next_u32() % 2;
    ((tens + 1) * 10 + span as u32 * 3 + units) as i8
}

/// Unconditional condition roll without the `+1` on the tens digit.
fn cond_plain(rng: &mut DeterministicRng, level: u8) -> i8 {
    let span = span_draw(rng, level as u32 / 8 + 1);
    let tens = rng.next_u32() % 4;
    let units = rng.next_u32() % 2;
    (tens * 10 + span as u32 * 3 + units) as i8
}

/// `FUN_00418E6C` body for one settlement.
///
/// `kit_class` is `Group+0x1E & 3`; `level` is the leader's level
/// (`Entity+0x30 >> 24`). The original aborts the whole pass when the instance
/// table is full, which is why every allocation failure returns.
fn seed_one(
    res: &mut Res,
    t: &mut crate::population::SettlementTrade,
    rng: &mut DeterministicRng,
    level: u8,
    kit_class: u8,
) {
    // Role 2 (`+0x412`): 1..3 empty jars into elements 0 and 14 (the original
    // measures the array from `+0x40C`, hence the `0x1C` stride).
    let mut jars = rng.next_u32() % 3 + 1;
    for slot in [0usize, 14] {
        if jars == 0 || t.stock2[slot] != 0 {
            continue;
        }
        let Some(id) = alloc_plain(res, 9, 3) else {
            return;
        };
        jars -= 1;
        t.stock2[slot] = id;
    }

    // Role 3 (`+0x444`) slots 0..2 at 70%: class 8/7/6 goods.
    for slot in 0..3usize {
        if t.stock3[slot] != 0 || rng.next_u32() % 100 <= 0x46 {
            continue;
        }
        let pick = rng.next_u32() % 0x1e;
        let (class, base) = if pick < 10 {
            (8u8, 6u8)
        } else if pick < 20 {
            (7, 3)
        } else {
            (6, 0)
        };
        let subtype = base + span_draw(rng, level as u32 / 5);
        let Some(id) = alloc_plain(res, class, subtype) else {
            return;
        };
        let cond = cond_plus_one(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[slot] = id;
    }

    // Slots 3..4 at 50%: class-11 trinkets.
    for slot in 3..5usize {
        if t.stock3[slot] != 0 || rng.next_u32() % 100 <= 0x32 {
            continue;
        }
        let subtype = (rng.next_u32() % 10) as u8;
        let Some(id) = alloc_plain(res, 0x0b, subtype) else {
            return;
        };
        t.stock3[slot] = id;
    }

    // Slots 5..7 at 30%: melee weapons, `FUN_004170E0` kit, 10% condition.
    for slot in 5..8usize {
        if t.stock3[slot] != 0 || rng.next_u32() % 100 <= 0x1e {
            continue;
        }
        let off = span_draw(rng, level as u32 / 5 + 1);
        let kit = kit_170e0(rng, kit_class as u32);
        let prefix = rng.next_u32() % 2;
        let subtype = (prefix * 3) as u8 + off + kit * 9;
        let class = if subtype % 9 < 3 { 2 } else { 0 };
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        if rng.next_u32() % 0x65 < 10 {
            let tier = level_tier(level) as u32;
            let off2 = if tier < 2 { 0 } else { rng.next_u32() % tier };
            let units = rng.next_u32() % 5;
            res.set_item_condition(id, (off2 + units * 10) as i8);
        }
        t.stock3[slot] = id;
    }

    // Slots 8..13 at 50%: armor suits (class 3/5/4), 7% condition.
    for slot in 8..14usize {
        if t.stock3[slot] != 0 || rng.next_u32() % 100 <= 0x32 {
            continue;
        }
        let pick = rng.next_u32() % 3;
        let (class, span, base) = match pick {
            0 => (3u8, level as u32 / 5 + 1, 0x22u8),
            1 => (5, level as u32 / 4 + 1, 0x25),
            _ => (4, level as u32 / 4 + 1, 0x29),
        };
        let off = span_draw(rng, span);
        let kit = kit_170e0(rng, kit_class as u32);
        let subtype = off + base + kit * 11;
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        if rng.next_u32() % 100 < 7 {
            let tier = level_tier(level) as u32;
            let off2 = if tier < 2 { 0 } else { rng.next_u32() % tier };
            let units = rng.next_u32() % 5;
            res.set_item_condition(id, (off2 + units * 10) as i8);
        }
        t.stock3[slot] = id;
    }

    // Slots 14..16: class 7 / 6 / 8 goods with an unconditional condition.
    for (slot, class, base) in [(14usize, 7u8, 3u8), (15, 6, 0), (16, 8, 6)] {
        let subtype = base + span_draw(rng, level as u32 / 5);
        let Some(id) = alloc_plain(res, class, subtype) else {
            return;
        };
        let cond = cond_plus_one(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[slot] = id;
    }

    // Slots 17..18: melee weapons with `FUN_00417190(kit_class)` / `+1`.
    for k in 0..2u32 {
        let slot = 17 + k as usize;
        let off = span_draw(rng, level as u32 / 5 + 1);
        let kit = kit_17190(kit_class as u32 + k);
        let prefix = rng.next_u32() % 2;
        let subtype = (prefix * 3) as u8 + off + kit * 9;
        let class = if subtype % 9 < 3 { 2 } else { 0 };
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        t.stock3[slot] = id;
    }

    // Slots 19..20: bows (class 1), same kit alternation, no condition.
    for k in 0..2u32 {
        let slot = 19 + k as usize;
        let off = span_draw(rng, level as u32 / 5 + 1);
        let kit = kit_17190(kit_class as u32 + k);
        let subtype = off + 6 + kit * 9;
        let Some(id) = alloc_gear(res, 1, subtype) else {
            return;
        };
        t.stock3[slot] = id;
    }

    // Slots 21..26: two armor suits (class 3/5/4) with kit / kit+1.
    for (slot, class, span, base, kit_add) in [
        (21usize, 3u8, level as u32 / 5 + 1, 0x22u8, 0u32),
        (22, 5, level as u32 / 4 + 1, 0x25, 0),
        (23, 4, level as u32 / 4 + 1, 0x29, 0),
        (24, 3, level as u32 / 5 + 1, 0x22, 1),
        (25, 5, level as u32 / 4 + 1, 0x25, 1),
        (26, 4, level as u32 / 4 + 1, 0x29, 1),
    ] {
        let off = span_draw(rng, span);
        let kit = kit_17190(kit_class as u32 + kit_add);
        let subtype = off + base + kit * 11;
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        t.stock3[slot] = id;
    }

    // Slot 27: plain melee weapon (`rand%2` prefix), condition without `+1`.
    {
        let off = span_draw(rng, level as u32 / 5 + 1);
        let prefix = rng.next_u32() % 2;
        let subtype = (prefix * 3) as u8 + off;
        let class = if subtype % 9 < 3 { 2 } else { 0 };
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        let cond = cond_plain(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[27] = id;
    }

    // Slot 28: plain bow (`rand%3` kit), condition without `+1`.
    {
        let off = span_draw(rng, level as u32 / 5 + 1);
        let prefix = rng.next_u32() % 3;
        let subtype = (prefix * 9) as u8 + off + 6;
        let Some(id) = alloc_gear(res, 1, subtype) else {
            return;
        };
        let cond = cond_plain(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[28] = id;
    }

    // Slots 29..30: plain armor (`rand%3` kit), condition without `+1`.
    for (slot, class, span, base) in [
        (29usize, 3u8, level as u32 / 5 + 1, 0x22u8),
        (30, 5, level as u32 / 4 + 1, 0x25),
    ] {
        let off = span_draw(rng, span);
        let prefix = rng.next_u32() % 3;
        let subtype = (prefix * 11) as u8 + off + base;
        let Some(id) = alloc_gear(res, class, subtype) else {
            return;
        };
        let cond = cond_plain(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[slot] = id;
    }

    // Slot 31: plain armor, condition with the `+1` tens digit restored.
    {
        let off = span_draw(rng, level as u32 / 4 + 1);
        let prefix = rng.next_u32() % 3;
        let subtype = (prefix * 11) as u8 + off + 0x29;
        let Some(id) = alloc_gear(res, 4, subtype) else {
            return;
        };
        let cond = cond_plus_one(rng, level);
        res.set_item_condition(id, cond);
        t.stock3[31] = id;
    }
}

impl PlayableApp {
    /// Seed every settlement of `loc` (`FUN_00418E6C`).
    ///
    /// Runs once per map build, i.e. on the original mode-0 entry. Already
    /// occupied stock slots are left alone, like the original's `== 0` tests.
    /// The original draws from the global `rand()`; entry-time rolls use the
    /// per-location stream (`build.rng`), the same convention as placement and
    /// population growth, so combat sequences are not perturbed.
    pub(super) fn seed_settlement_stock(&mut self, loc: u32, rng: &mut DeterministicRng) {
        if self.tables.is_none() || self.settlements.is_empty() {
            return;
        }
        let level = self.player.level;
        let rows: Vec<(usize, u8)> = {
            let tables = self.tables.as_ref().expect("checked above");
            self.settlements
                .iter()
                .enumerate()
                .filter(|(_, s)| s.loc_id as u32 == loc)
                .filter_map(|(si, s)| {
                    let ti = self
                        .settlement_trade
                        .iter()
                        .position(|t| t.index == si)?;
                    let kit = tables
                        .groups
                        .get(s.group_id as usize)
                        .map(|g| g.class_1e & 3)
                        .unwrap_or(0);
                    Some((ti, kit))
                })
                .collect()
        };
        let PlayableApp {
            world,
            settlement_trade,
            ..
        } = self;
        for (ti, kit) in rows {
            let Some(t) = settlement_trade.get_mut(ti) else {
                continue;
            };
            seed_one(&mut world.res, t, rng, level, kit);
        }
    }
}
