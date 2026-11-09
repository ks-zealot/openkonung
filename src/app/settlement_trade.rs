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

fn kit_from_class(class: u8) -> usize {
    match class & 3 {
        0 => 2,
        1 => 1,
        _ => 0,
    }
}

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

fn simple_herb(res: &mut Res, subtype: u8) -> Option<u16> {
    res.alloc_item_instance(9, subtype, 0, -1, 1.0, 0, 0)
}

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
                        t.quota2 -= 1;
                        simple_herb(res, 2)
                    } else {
                        None
                    }
                }
                6 => {
                    if t.quota0 > 0 {
                        t.quota0 -= 1;
                        simple_herb(res, 0)
                    } else {
                        None
                    }
                }
                7 => {
                    if t.quota1 > 0 {
                        t.quota1 -= 1;
                        simple_herb(res, 1)
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
            let Some(entity) = tables.entities.get(member) else {
                continue;
            };
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

fn tick_role4(
    res: &mut Res,
    t: &mut SettlementTrade,
    passes: u64,
    kit: usize,
    smithing: &mut u16,
    tables: &mut Option<GameTables>,
    group_id: u8,
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
                        chosen = Some(slot);
                        break;
                    }
                }
            }
            if let Some(slot) = chosen {
                t.last_slot = slot as i8;
                t.timer4 = 1;
            } else if let Some(tables) = tables.as_mut() {
                if let Some(member) = role4_distribute(res, t, tables, group_id, kit, *smithing) {
                    changed = Some(member);
                }
            }
        } else {
            t.timer4 -= 1;
            if t.timer4 <= 0 {
                let slot = t.last_slot as usize;
                t.last_slot = -1;
                if slot < t.stock4.len() && t.stock4[slot] == 0 {
                    if create_role4_item(res, t, kit, slot) {
                        *smithing = (*smithing).saturating_add(1).min(500);
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
                tick_role4(res, t, passes, kit, &mut smith, tables, group_id)
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
