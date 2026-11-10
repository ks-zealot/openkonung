//! Settlement target timer/state pass (`FUN_0041D048`).
//!
//! The original settlement branch runs once per 16 original frames
//! (`(frame + 7) & 0xF == 0`). For every target entry
//! (`+0 key`, `+1 state`, `+2 prop_index`, `+6 timer`, stride 8) it advances a
//! timer-driven state machine:
//!
//! ```text
//! timer > 0, state < 3, workers > 0:
//!     timer -= 1
//!     if timer == 0:
//!         state += 1
//!         if state != 3: changed = true; timer = value * 60 / workers
//!         update_prop()
//! timer > 0 (other states, or no workers):
//!     timer -= 1
//!     if timer == 0:
//!         state += 1
//!         if state != 6: timer = value
//!         update_prop()
//! timer == 0, state == 6, workers > 0:
//!     changed = true; state = 0; timer = value * 60 / workers; update_prop()
//! ```
//!
//! `changed` (`local_30`) gates the barracks training pass for that settlement.
//! `update_prop` is `FUN_004172AC`, only when the settlement is on the current
//! map; it sets the prop object to `base(key) + state + 30`.
//!
//! The worker count is computed before the defense/claim side effects and
//! passed in by [`PlayableApp::tick_settlement_pass`]:
//!
//! ```text
//! free   = members: visual<3, visible, kind != 3/9
//! roles  = free with archetype & 0x38
//! owners = nonzero waypoints[0..4].owner
//! workers = group.activity_1d == 1 ? 0 : free - (roles + owners)
//! ```

use super::*;
use crate::constants::exe_tables::SETTLEMENT_TARGET_TABLE;
use crate::population::SettlementTarget;

/// Worker counts for one settlement (`FUN_0041D048`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SettlementWorkers {
    /// Visible low-visual members (`local_24` before the role/owner subtraction).
    pub free: i32,
    /// Free workers used to scale target timers.
    pub workers: i32,
}

/// Runtime action row (`Entity+0x00`) and dead flag for an entity.
///
/// `Entity+0x00` is the **action row**, not a separate "kind": `3`/`9` are the
/// death rows. The remake stores the authored value in [`Entity`](crate::population::Entity) `kind` and the
/// live row in [`crate::actors::ActorMotion::action`]; the population cache is
/// therefore authoritative for cached locations, and the authored byte is only
/// a fallback for locations that were never built.
pub(super) fn runtime_action(
    app: &PlayableApp,
    loc: u32,
    id: usize,
    authored_action: u8,
) -> (u8, bool) {
    if let Some(pop) = app.population_cache.get(&loc) {
        if let Some(motion) = pop.motion.get(&id) {
            return (motion.action, motion.dead);
        }
    }
    (authored_action, matches!(authored_action, 3 | 9))
}

impl PlayableApp {
    /// `FUN_0041D048` worker count for one settlement. Uses the runtime action
    /// row / dead flag from the location cache so runtime deaths are excluded.
    pub(super) fn settlement_workers(&self, si: usize) -> SettlementWorkers {
        let Some(settlement) = self.settlements.get(si) else {
            return SettlementWorkers::default();
        };
        let Some(tables) = self.tables.as_ref() else {
            return SettlementWorkers::default();
        };
        let Some(group) = tables.groups.get(settlement.group_id as usize) else {
            return SettlementWorkers::default();
        };
        let Ok(range) = group.entity_range() else {
            return SettlementWorkers::default();
        };
        let loc = settlement.loc_id as u32;
        // Recruited companions keep their source table row as an overlay; the
        // original `FUN_00431820` compacts the old group. Exclude their ids so
        // they do not inflate the free/role counts.
        let party_ids: HashSet<usize> = self.party.iter().map(|m| m.entity.index).collect();
        let mut free = 0i32;
        let mut roles = 0i32;
        for id in range {
            if party_ids.contains(&id) {
                continue;
            }
            let Some(entity) = tables.entities.get(id) else {
                continue;
            };
            let (action, dead) = runtime_action(self, loc, id, entity.kind);
            if dead
                || matches!(action, 3 | 9)
                || entity.visual_index >= 3
                || entity.archetype & 0x80 != 0
            {
                continue;
            }
            free += 1;
            if entity.archetype & 0x38 != 0 {
                roles += 1;
            }
        }
        let owners = settlement
            .waypoints
            .iter()
            .take(5)
            .filter(|w| w.owner != 0)
            .count() as i32;
        let workers = if group.activity_1d == 1 {
            0
        } else {
            free - (roles + owners)
        };
        SettlementWorkers { free, workers }
    }

    /// Run one target timer pass for every settlement. `workers` is indexed by
    /// settlement index and must be computed **before** the defense/claim
    /// side effects (the original computes `local_24` first). Returns the
    /// settlement indices whose target pass set `local_30` (`changed`).
    pub(super) fn tick_settlement_targets(
        &mut self,
        workers: &[SettlementWorkers],
    ) -> HashSet<usize> {
        let mut blocked = HashSet::new();
        for si in 0..self.settlements.len() {
            let available = workers.get(si).map(|w| w.workers).unwrap_or(0);
            let loc = self.settlements[si].loc_id as u32;
            let targets = self.settlements[si].targets.clone();
            let mut changed = false;
            for (ti, target) in targets.into_iter().enumerate() {
                if target.key >= 0x80 || (target.key as usize) >= SETTLEMENT_TARGET_TABLE.len() {
                    continue;
                }
                let value = SETTLEMENT_TARGET_TABLE[target.key as usize].1;
                let mut timer = target.timer;
                let mut state = target.state;
                let mut prop_update = false;
                if timer > 0 {
                    if state < 3 && available > 0 {
                        timer -= 1;
                        if timer == 0 {
                            state = state.saturating_add(1);
                            if state != 3 {
                                changed = true;
                                timer = ((value as i32 * 0x3C) / available) as u16;
                            }
                            prop_update = true;
                        }
                    } else {
                        timer -= 1;
                        if timer == 0 {
                            state = state.saturating_add(1);
                            if state != 6 {
                                timer = value;
                            }
                            prop_update = true;
                        }
                    }
                } else if state == 6 && available > 0 {
                    changed = true;
                    state = 0;
                    timer = ((value as i32 * 0x3C) / available) as u16;
                    prop_update = true;
                }
                // The original decrements the record in place, so a plain
                // countdown must be written back even when it does not fire.
                if state != target.state || timer != target.timer {
                    self.settlements[si].targets[ti].state = state;
                    self.settlements[si].targets[ti].timer = timer;
                }
                if prop_update && loc == self.current_map {
                    let updated: SettlementTarget = self.settlements[si].targets[ti];
                    self.apply_settlement_prop(loc, &updated);
                }
            }
            if changed {
                blocked.insert(si);
            }
        }
        blocked
    }
}
