//! Settlement merchant restock (`FUN_004175FC` role 2, `FUN_004179C8`
//! role 4). Restock is skipped while the exchange window is open, exactly like
//! the original (`_DAT_0081d610 != 7`).

use super::*;
use crate::inventory::{requirement_ok, OwnerSnapshot, SlotsRef};
use crate::population::SettlementTrade;
use crate::res::Res;

const ROLE2_PERIOD: i32 = 0x5A0;
const RECIPE_LEVEL_BASE: u32 = 0x45A2FC;
const RECIPE_LEVEL_MID: u32 = 0x459ABC;
const RECIPE_LEVEL_HIGH: u32 = 0x45A0FC;

/// Equipment kit index (`0..=2`) from the settlement group class low bits.
///
/// Selects the recipe-level table rows and the role-4 class/subtype map.
fn kit_from_class(class: u8) -> usize {
    match class & 3 {
        0 => 2,
        1 => 1,
        _ => 0,
    }
}

/// Smithing level threshold of one role-4 stock slot.
///
/// Read from the three recipe tables at VAs `0x45A2FC` (ammo slots 0..2),
/// `0x459ABC` (slots 3..11) and `0x45A0FC` (slots 12..), stride `0x40`, value at
/// `+2`. Returns 0 when the item database is not loaded.
fn recipe_level(res: &Res, kit: usize, slot: usize) -> u16 {
    let Some(db) = res.item_db.as_ref() else {
        return 0;
    };
    if slot < 3 {
        db.read_va_u16(RECIPE_LEVEL_BASE, slot * 0x40 + 2)
    } else if slot < 12 {
        db.read_va_u16(RECIPE_LEVEL_MID, (kit * 9 + slot) * 0x40 + 2)
    } else {
        db.read_va_u16(RECIPE_LEVEL_HIGH, (kit * 11 + slot) * 0x40 + 2)
    }
}

/// Allocate one class-9 instance of `input` (subtype `2/0/1`) and run the
/// role-2 craft row `(input, 3)` on it; the product is subtype `5/6/7`.
///
/// Returns `None` when the instance table is full (the original then aborts the
/// whole pass without consuming the quota).
fn restock_unit(res: &mut Res, healing: &mut u16, subtype: u8) -> Option<u16> {
    crate::inventory::craft_restock(res, healing, subtype)
}

/// `FUN_004175FC`: role-2 merchant (herb/potion) restock for `passes` ticks.
///
/// The `0x5A0` period timer resets all three quotas; the intermediate values
/// `0x438`, `0x2D0` and `0x168` re-open one or two of them. Every empty stock
/// slot then produces a herb, or combines two stocked herbs through
/// [`crate::inventory::craft`] with the producer's healing skill.
fn tick_role2(res: &mut Res, t: &mut SettlementTrade, passes: u64, healing: &mut u16) {
    let mut n = passes;
    while n > 0 {
        n -= 1;
        t.timer2 -= 1;
        if t.timer2 < 1 {
            t.timer2 = ROLE2_PERIOD;
            t.quota0 = 1;
            t.quota1 = 1;
            t.quota2 = 1;
        } else if t.timer2 == 0x2D0 {
            t.quota1 = 1;
            t.quota2 = 1;
        } else if t.timer2 == 0x438 || t.timer2 == 0x168 {
            t.quota2 = 1;
        }

        for combined in 5usize..28 {
            let stock_index = combined.saturating_sub(3);
            if stock_index >= t.stock2.len() || t.stock2[stock_index] != 0 {
                continue;
            }
            let case = if combined <= 13 {
                combined
            } else {
                combined - 14
            };
            let produced = match case {
                5 => {
                    if t.quota2 > 0 {
                        match restock_unit(res, healing, 2) {
                            Some(id) => {
                                t.quota2 -= 1;
                                Some(id)
                            }
                            // Original `return 0`: the pass aborts, quota intact.
                            None => return,
                        }
                    } else {
                        None
                    }
                }
                6 => {
                    if t.quota0 > 0 {
                        match restock_unit(res, healing, 0) {
                            Some(id) => {
                                t.quota0 -= 1;
                                Some(id)
                            }
                            None => return,
                        }
                    } else {
                        None
                    }
                }
                7 => {
                    if t.quota1 > 0 {
                        match restock_unit(res, healing, 1) {
                            Some(id) => {
                                t.quota1 -= 1;
                                Some(id)
                            }
                            None => return,
                        }
                    } else {
                        None
                    }
                }
                8..=13 => {
                    let (a, b): (usize, usize) = match case {
                        8 => (0x13, 0x14),
                        9 => (0x13, 0x15),
                        10 => (0x13, 0x17),
                        11 => (0x14, 0x15),
                        12 => (0x14, 0x17),
                        _ => (0x15, 0x16),
                    };
                    let ai = a.saturating_sub(3);
                    let bi = b.saturating_sub(3);
                    if ai >= t.stock2.len() || bi >= t.stock2.len() {
                        None
                    } else {
                        let a_id = t.stock2[ai];
                        let b_id = t.stock2[bi];
                        if a_id == 0 || b_id == 0 {
                            None
                        } else if crate::inventory::craft(res, healing, a_id, b_id).is_some() {
                            t.stock2[ai] = 0;
                            t.stock2[bi] = 0;
                            Some(a_id)
                        } else {
                            None
                        }
                    }
                }
                _ => None,
            };
            if let Some(id) = produced {
                t.stock2[stock_index] = id;
            }
        }
    }
}

/// Class and subtype of the role-4 stock item for `kit`/`slot`.
///
/// Slots 0..2 are class-12 stacks, 3..11 the class 2/0/1 gear rows, and 12.. the
/// class 3/5/4 armor rows.
fn role4_class_subtype(kit: usize, slot: usize) -> (u8, u8) {
    if slot < 3 {
        (12, (0x1E + slot) as u8)
    } else if slot < 12 {
        let class = if slot < 6 {
            2
        } else if slot < 9 {
            0
        } else {
            1
        };
        (class, (kit * 9 + slot - 3) as u8)
    } else {
        let class = if slot < 15 {
            3
        } else if slot < 19 {
            5
        } else {
            4
        };
        (class, (kit * 11 + slot + 0x16) as u8)
    }
}

/// Allocate the role-4 stock item for one slot and place it in the stock row.
///
/// Class-12 stacks start at 30 units, other items at their template durability.
/// Returns false when the instance table is full.
fn create_role4_item(res: &mut Res, t: &mut SettlementTrade, kit: usize, slot: usize) -> bool {
    let (class, subtype) = role4_class_subtype(kit, slot);
    if class == 12 {
        let Some(id) = res.alloc_item_instance(12, subtype, 0, -1, 0.0, 0, 0) else {
            return false;
        };
        res.set_item_quantity(id, 30);
        t.stock4[slot] = id;
        return true;
    }
    let durability = res
        .item_info_for(class, subtype)
        .map(|i| i.durability as f32)
        .unwrap_or(1.0)
        .max(1.0);
    let Some(id) = res.alloc_item_instance(class, subtype, 0, -1, durability, 0, 0) else {
        return false;
    };
    t.stock4[slot] = id;
    true
}

/// Equipment slot the role-4 item class occupies.
///
/// Follows the original class-equals-slot rule: class 2 fits slot 2, class 0
/// slot 0, class 1 slot 1, and classes 3/4/5 their own armor slots. Slots 0..2
/// (class-12 ammo) never reach this map because they go to `Entity+0xCC`.
fn role4_equip_slot(slot: usize) -> usize {
    if slot < 6 {
        2
    } else if slot < 9 {
        0
    } else if slot < 12 {
        1
    } else if slot < 15 {
        3
    } else if slot < 19 {
        5
    } else {
        4
    }
}

/// `FUN_004179C8` distribution branch: when no affordable empty stock slot is
/// available, move an existing stock item onto the first settlement group
/// member that can use it, replacing lower-subtype gear.
fn role4_distribute(
    res: &mut Res,
    t: &mut SettlementTrade,
    tables: &mut GameTables,
    group_id: u8,
    kit: usize,
    smithing: u16,
    party_ids: &HashSet<usize>,
) -> Option<usize> {
    let (first, count) = {
        let group = tables.groups.get(group_id as usize)?;
        (group.first_entity as usize, group.entity_count as usize)
    };
    for slot in 0..t.stock4.len() {
        let item = t.stock4[slot];
        if item == 0 {
            continue;
        }
        let level = recipe_level(res, kit, slot) as u32;
        if level > smithing as u32 * 10 {
            continue;
        }
        let (_, new_subtype) = role4_class_subtype(kit, slot);
        let equip_slot = role4_equip_slot(slot);
        for member in first..first.saturating_add(count) {
            if party_ids.contains(&member) {
                continue;
            }
            let Some(entity) = tables.entities.get(member) else {
                continue;
            };
            // `FUN_004182D4` runs `FUN_0041C69C` before the requirement test, so
            // the member's derived stats include their current gear bonuses.
            let stored = crate::character::effects::StoredStats {
                dexterity: entity.stats.dexterity_current,
                charisma: entity.stats.charisma_current,
                hp: entity.stats.hp_internal,
                strength: entity.stats.strength_current,
                endurance: entity.stats.endurance_current,
            };
            let extra = [
                if entity.state_d2 == 0 || entity.state_d2 == 0xFF {
                    None
                } else {
                    Some(entity.state_d2 as i8)
                },
                None,
            ];
            let derived = crate::character::effects::recompute(
                res,
                &entity.equipment,
                &entity.additional_equipment,
                extra,
                stored,
            );
            let entity = tables.entities.get_mut(member).unwrap();
            crate::character::effects::apply(&mut entity.stats, &derived);
            let entity = &tables.entities[member];
            let snap = OwnerSnapshot::from_entity(entity);
            let slots = SlotsRef {
                equipment: &entity.equipment,
                inventory: &entity.inventory,
                additional: &entity.additional_equipment,
                special: entity.special_equipment,
            };
            if !requirement_ok(&snap, res, &slots, item) {
                continue;
            }
            if slot < 3 {
                let Some(entity) = tables.entities.get_mut(member) else {
                    continue;
                };
                if entity.special_equipment != 0 {
                    continue;
                }
                entity.special_equipment = item;
                t.stock4[slot] = 0;
                t.last_slot = slot as i8;
                t.timer4 = 1;
                return Some(member);
            }
            let old = entity.equipment[equip_slot];
            let old_subtype = if old == 0 {
                0
            } else {
                res.item_class_subtype(old).map(|(_, s)| s).unwrap_or(0)
            };
            if old != 0 && old_subtype >= new_subtype {
                continue;
            }
            if old != 0 {
                res.set_item_class(old, 0xFF);
            }
            let Some(entity) = tables.entities.get_mut(member) else {
                continue;
            };
            entity.equipment[equip_slot] = item;
            t.stock4[slot] = 0;
            t.last_slot = slot as i8;
            t.timer4 = 1;
            return Some(member);
        }
    }
    None
}

/// `FUN_004179C8` branch A production delay: `round(level * 60 / sqrt(smithing))`.
///
/// The original evaluates this in x87 and stores it with `FISTP`; with
/// `smithing == 0` the division yields `+inf`, which stores as `0x80000000`.
/// Branch B then wraps it to `0x7FFFFFFF`, so a skill-less producer never
/// completes a slot — reproduced by [`f64`]-free integer arithmetic below.
fn role4_delay(level: u16, smithing: u16) -> i32 {
    if smithing == 0 {
        return i32::MIN;
    }
    let v = level as f32 * 60.0 / (smithing as f32).sqrt();
    v.round_ties_even() as i32
}

/// `FUN_004179C8` branch B skill growth:
/// `smithing = round(sqrt(smithing^2 + q / 24))` (`0x45001C` = `1/24`).
///
/// `q` is the recipe level for ammo slots and
/// `round(1 + quality / sqrt(smithing))` otherwise. A zero skill divides by
/// zero; the resulting x87 NaN store leaves the `u16` at `0`.
fn role4_grow(smithing: u16, q: i32) -> u16 {
    if smithing == 0 {
        return 0;
    }
    let s = smithing as f64;
    let v = (s * s + q as f64 / 24.0).sqrt().round_ties_even();
    v.clamp(0.0, u16::MAX as f64) as u16
}

/// `FUN_004179C8`: role-4 merchant (equipment) restock for `passes` ticks.
///
/// With no slot in production, pick the first empty slot whose recipe level the
/// producer's smithing skill (`skill * 10`) covers; when none is affordable the
/// existing stock is distributed onto settlement members instead. A reserved
/// slot completes after `round(level * 60 / sqrt(smithing))` passes and raises
/// the producer's smithing through [`role4_grow`].
///
/// # Returns
///
/// The settlement member that received distributed stock, if any.
fn tick_role4(
    res: &mut Res,
    t: &mut SettlementTrade,
    passes: u64,
    kit: usize,
    smithing: &mut u16,
    tables: &mut Option<GameTables>,
    group_id: u8,
    party_ids: &HashSet<usize>,
) -> Option<usize> {
    let mut changed = None;
    let mut n = passes;
    while n > 0 {
        n -= 1;
        if t.last_slot < 0 {
            let mut chosen = None;
            for slot in 0..t.stock4.len() {
                if t.stock4[slot] == 0 {
                    let level = recipe_level(res, kit, slot);
                    if level as u32 <= *smithing as u32 * 10 {
                        chosen = Some((slot, level));
                        break;
                    }
                }
            }
            if let Some((slot, level)) = chosen {
                t.last_slot = slot as i8;
                t.timer4 = role4_delay(level, *smithing);
            } else if let Some(tables) = tables.as_mut() {
                if let Some(member) =
                    role4_distribute(res, t, tables, group_id, kit, *smithing, party_ids)
                {
                    changed = Some(member);
                }
            }
        } else {
            // x87 `DEC` wraps: `INT_MIN` becomes `INT_MAX` (never completes).
            t.timer4 = t.timer4.wrapping_sub(1);
            if t.timer4 <= 0 {
                let slot = t.last_slot as usize;
                t.last_slot = -1;
                if slot < t.stock4.len() && t.stock4[slot] == 0 {
                    if create_role4_item(res, t, kit, slot) {
                        let q = if slot < 3 {
                            recipe_level(res, kit, slot) as i32
                        } else {
                            let quality = t
                                .stock4
                                .get(slot)
                                .and_then(|&id| res.item_quality(id))
                                .unwrap_or(0.0);
                            let skill = (*smithing as f32).sqrt();
                            (1.0 + quality / skill).round_ties_even() as i32
                        };
                        *smithing = role4_grow(*smithing, q);
                    }
                }
            }
        }
    }
    changed
}

impl PlayableApp {
    /// Advance every settlement's merchant restock by `passes` settlement
    /// passes (one per 16 original frames, `FUN_0041D048`). Runs for all 20
    /// settlements, not only the current location.
    pub(super) fn tick_settlement_trade(&mut self, passes: u64) {
        if passes == 0 || self.trade.is_some() {
            return;
        }
        for si in 0..self.settlements.len() {
            let loc = self.settlements[si].loc_id;
            let group_class = self
                .tables
                .as_ref()
                .and_then(|t| t.groups.get(self.settlements[si].group_id as usize))
                .map(|g| g.class_1e)
                .unwrap_or(0);
            let kit = kit_from_class(group_class);
            let Some(ti) = self.settlement_trade.iter().position(|t| t.index == si) else {
                continue;
            };
            // The producer words are `waypoint[1].owner` (role 2 restock) and
            // `waypoint[3].owner` (role 4 equipment); `SettlementTrade` only
            // mirrors them for save/legacy export.
            let prod2 = self.settlements[si].producer(1) as usize;
            let prod4 = self.settlements[si].producer(3) as usize;
            let mut heal = self
                .tables
                .as_ref()
                .and_then(|t| t.entities.get(prod2))
                .map(|e| e.stats.healing)
                .unwrap_or(0);
            let mut smith = self
                .tables
                .as_ref()
                .and_then(|t| t.entities.get(prod4))
                .map(|e| e.stats.smithing)
                .unwrap_or(0);

            let group_id = self.settlements[si].group_id;
            let party_ids: HashSet<usize> = self.party.iter().map(|m| m.entity.index).collect();
            let PlayableApp {
                world,
                settlement_trade,
                tables,
                population_cache,
                ..
            } = self;
            let res = &mut world.res;
            let t = &mut settlement_trade[ti];
            if prod2 != 0 {
                tick_role2(res, t, passes, &mut heal);
            }
            let changed = if prod4 != 0 {
                tick_role4(
                    res, t, passes, kit, &mut smith, tables, group_id, &party_ids,
                )
            } else {
                None
            };

            if let Some(tables) = tables.as_mut() {
                if let Some(e) = tables.entities.get_mut(prod2) {
                    e.stats.healing = heal;
                }
                if let Some(e) = tables.entities.get_mut(prod4) {
                    e.stats.smithing = smith;
                }
            }
            if let Some(member) = changed {
                if let Some(tables) = tables.as_ref() {
                    if let Some(entity) = tables.entities.get(member) {
                        if let Some(pop) = population_cache.get_mut(&(loc as u32)) {
                            if let Some(actor) = pop
                                .actors
                                .iter_mut()
                                .find(|a| a.source_entity_id() == member)
                            {
                                actor.entity = entity.clone();
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{role4_delay, role4_grow};

    /// `FUN_004179C8` branch A: `round(level*60 / sqrt(smithing))`, with the
    /// x87 `FISTP` of `+inf` (division by a zero skill) storing `INT_MIN`.
    #[test]
    fn role4_delay_matches_the_x87_formula() {
        assert_eq!(role4_delay(1, 9), 20);
        assert_eq!(role4_delay(60, 100), 360);
        assert_eq!(role4_delay(60, 400), 180);
        assert_eq!(role4_delay(9, 0), i32::MIN);
    }

    /// `FUN_004179C8` branch B: `round(sqrt(smithing^2 + q/24))`; a zero skill
    /// makes the division NaN and leaves the stored `u16` at zero.
    #[test]
    fn role4_grow_uses_the_quarter_day_sqrt() {
        assert_eq!(role4_grow(1, 30), 2); // sqrt(2.25) = 1.5 -> ties-even 2
        assert_eq!(role4_grow(2, 30), 2); // 2.29 -> 2
        assert_eq!(role4_grow(3, 30), 3); // 3.20 -> 3
        assert_eq!(role4_grow(100, 30), 100); // 100.06 -> 100
        assert_eq!(role4_grow(0, 30), 0); // NaN store
    }
}
