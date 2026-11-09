//! Settlement defense/claim/defeat/random-event block (`FUN_0041D048`).
//!
//! Runs before the target timer pass in [`PlayableApp::tick_settlement_pass`],
//! once per 16 original frames. It covers the parts of the original settlement
//! branch that surround the target loop:
//!
//! - no free low-visual members → clear `Group+0x1D`, settlement flags `0x15`
//!   and the `+0x14` event counter;
//! - `FUN_00416670` waypoint claim: an armed party member with a different
//!   visual family inside the group's rectangle A sets `Group+0x1A = 0`
//!   (party group) and `Group+0x1D = 1`, otherwise clears the activity;
//! - defeat check: when `Group+0x1D == 1` and the settlement is not already
//!   defeated, compare the `FUN_0041B658` party power sum against the
//!   settlement defender power sum; `party / defenders >= 2.0` conquers it;
//! - `+0x14` event counter: every `0x2760` passes add the recovered money-pool
//!   trickle (`entity_count*5 + target_base0*20*sqrt(leader.trade) + 1`).
//!
//! The target timer pass itself is in [`super::settlement_targets`].

use super::settlement_targets::runtime_action;
use super::*;
use crate::combat::{combat_power, CombatStats};
use crate::video::{self, VideoAfter, VideoClip};

/// `FUN_00416670`: does this actor look like an armed stranger?
fn claimable(
    res: &crate::res::Res,
    visual: u8,
    class_low: u8,
    equipment: &[u16; 6],
    inventory: &[u16; 32],
) -> bool {
    if visual == class_low {
        return false;
    }
    if equipment[0] != 0 || equipment[1] != 0 || equipment[2] != 0 {
        return true;
    }
    inventory
        .iter()
        .any(|&id| id != 0 && res.item_class(id).map(|c| c < 3).unwrap_or(false))
}

fn inside(rect: &crate::population::Rect, cell: (i32, i32)) -> bool {
    let (x, y) = cell;
    rect.x1 as i32 <= x
        && x <= rect.x2 as i32
        && rect.y1 as i32 <= y
        && y <= rect.y2 as i32
}

impl PlayableApp {
    /// `if (free == 0)`: clear the group activity, the settlement alert flags
    /// (`0x15`) and the `+0x14` counter.
    pub(super) fn settlement_no_worker_clear(&mut self, si: usize) {
        let Some(group_id) = self.settlements.get(si).map(|s| s.group_id as usize) else {
            return;
        };
        if let Some(tables) = self.tables.as_mut() {
            if let Some(group) = tables.groups.get_mut(group_id) {
                group.activity_1d = 0;
            }
        }
        if let Some(s) = self.settlements.get_mut(si) {
            s.flags_4b2 &= 0xEA;
            s.progress_14 = 0;
        }
    }

    /// `FUN_00416670`: the settlement claims an armed foreign party member
    /// inside its rectangle A, waking the group and targeting the party.
    pub(super) fn settlement_claim(&mut self, si: usize) {
        let Some(settlement) = self.settlements.get(si) else {
            return;
        };
        if settlement.flags_4b2 & 5 == 0 {
            return;
        }
        let group_id = settlement.group_id as usize;
        let Some(group) = self
            .tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .cloned()
        else {
            return;
        };
        let rect = group.rect_a;
        let class_low = group.class_1e & 3;
        let hostile = settlement.flags_4b2 & 1 != 0;

        let mut claimed = false;
        let leader_cell = (
            self.player_x.round() as i32,
            self.player_y.round() as i32,
        );
        if inside(&rect, leader_cell)
            && (hostile
                || claimable(
                    &self.world.res,
                    self.player.hero.visual_index,
                    class_low,
                    &self.player.equipment,
                    &self.player.inventory,
                ))
        {
            claimed = true;
        }
        if !claimed {
            for member in &self.party {
                if !inside(&rect, member.motion.cell) {
                    continue;
                }
                if hostile
                    || claimable(
                        &self.world.res,
                        member.entity.visual_index,
                        class_low,
                        &member.entity.equipment,
                        &member.entity.inventory,
                    )
                {
                    claimed = true;
                    break;
                }
            }
        }
        if let Some(tables) = self.tables.as_mut() {
            if let Some(group) = tables.groups.get_mut(group_id) {
                if claimed {
                    group.target_group = 0;
                    group.activity_1d = 1;
                } else {
                    group.activity_1d = 0;
                }
            }
        }
    }

    /// `FUN_0041D048` defeat check: `party_power / defender_power >= 2.0`
    /// (`_DAT_004501A8`). A zero defender power is an infinite ratio (defeat
    /// when the party has any power), matching the x87 division.
    pub(super) fn settlement_defeat_check(&mut self, si: usize) -> bool {
        let Some(settlement) = self.settlements.get(si) else {
            return false;
        };
        if settlement.flags_4b2 & 0x80 != 0 {
            return false;
        }
        let group_id = settlement.group_id as usize;
        let Some(group) = self
            .tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .cloned()
        else {
            return false;
        };
        if group.activity_1d != 1 {
            return false;
        }
        let Some(db) = self.world.res.item_db.as_ref() else {
            return false;
        };
        let save = &self.world.res.save_game;
        let party_power = combat_power(db, save, &CombatStats::from_player(&self.player)) as i64
            + self
                .party
                .iter()
                .map(|m| combat_power(db, save, &CombatStats::from_entity(&m.entity)) as i64)
                .sum::<i64>();
        let loc = settlement.loc_id as u32;
        let party_ids: HashSet<usize> = self.party.iter().map(|m| m.entity.index).collect();
        let mut defender_power = 0i64;
        if let Ok(range) = group.entity_range() {
            for id in range {
                if party_ids.contains(&id) {
                    continue;
                }
                let Some(entity) = self.tables.as_ref().and_then(|t| t.entities.get(id)) else {
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
                defender_power += combat_power(db, save, &CombatStats::from_entity(entity)) as i64;
            }
        }
        let defeated = if defender_power <= 0 {
            party_power > 0
        } else {
            party_power as f64 / defender_power as f64 >= 2.0
        };
        if !defeated {
            return false;
        }
        self.apply_settlement_defeat(si, group_id);
        true
    }

    fn apply_settlement_defeat(&mut self, si: usize, group_id: usize) {
        // Group activity off; `Entity+0x00` 3/9 records are **death action
        // rows** and are frozen on their final frame and hidden
        // (`archetype |= 0x80`). Live members are disarmed and idled.
        let loc = self.settlements.get(si).map(|s| s.loc_id).unwrap_or(0) as u32;
        let range = self
            .tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .and_then(|g| g.entity_range().ok());
        // The authored tables only carry `Entity+0x00`; the live action row is
        // in the location cache, so resolve it before mutating the tables.
        let runtime: HashMap<usize, (u8, bool)> = range
            .as_ref()
            .map(|range| {
                range
                    .clone()
                    .filter_map(|id| {
                        let authored = self
                            .tables
                            .as_ref()
                            .and_then(|t| t.entities.get(id))
                            .map(|e| e.kind)?;
                        Some((id, runtime_action(self, loc, id, authored)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(tables) = self.tables.as_mut() {
            if let Some(group) = tables.groups.get_mut(group_id) {
                group.activity_1d = 0;
            }
            for (&id, &(action, dead)) in &runtime {
                let Some(entity) = tables.entities.get_mut(id) else {
                    continue;
                };
                if dead || matches!(action, 3 | 9) {
                    entity.flags_a = 0;
                    entity.archetype |= 0x80;
                } else if entity.archetype & 0x80 == 0 {
                    entity.flags_a = 0;
                }
            }
        }
        let PlayableApp {
            world,
            population_cache,
            ..
        } = self;
        if let Some(pop) = population_cache.get_mut(&loc) {
            let actor_ids: Vec<usize> = pop
                .actors
                .iter()
                .filter(|a| a.source_group_id == group_id)
                .map(|a| a.entity.index)
                .collect();
            for id in actor_ids {
                let Some(actor) = pop.actors.iter().find(|a| a.entity.index == id) else {
                    continue;
                };
                // The cache is authoritative for the live action row.
                let (action, dead, known_frames) = pop
                    .motion
                    .get(&id)
                    .map(|m| (m.action, m.dead, m.death_frames))
                    .unwrap_or((
                        actor.entity.kind,
                        matches!(actor.entity.kind, 3 | 9),
                        0,
                    ));
                let battle = crate::anim::battle_from_flags(actor.entity.flags_b);
                let death = dead || matches!(action, 3 | 9);
                // Preserve the current death row (3/9); otherwise use the
                // stance's death action.
                let death_action = if matches!(action, 3 | 9) {
                    action
                } else {
                    crate::anim::death_action(battle) as u8
                };
                let death_frames = if matches!(action, 3 | 9) && known_frames > 0 {
                    // `Entity+0xD7` is already the current action's frame count.
                    known_frames
                } else {
                    world
                        .res
                        .action_cycle_frames(
                            actor.entity.archetype,
                            actor.entity.visual_index,
                            (actor.entity.facing & 7) as usize,
                            death_action as usize,
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8
                };
                if let Some(actor) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                    actor.entity.flags_a = 0;
                    if death {
                        actor.entity.archetype |= 0x80;
                    }
                }
                if let Some(motion) = pop.motion.get_mut(&id) {
                    motion.combat_target = None;
                    motion.path.clear();
                    motion.target = None;
                    motion.walking = false;
                    if death {
                        // `Entity+0x10 = Entity+0xD7 - 1`: hold the final
                        // frame of the death action.
                        motion.dead = true;
                        motion.action = death_action;
                        motion.death_frames = death_frames.max(1);
                        motion.frame = death_frames.saturating_sub(1);
                    } else {
                        motion.action = crate::anim::idle_action(battle) as u8;
                        motion.frame = 0;
                    }
                }
            }
        }
        if let Some(settlement) = self.settlements.get_mut(si) {
            settlement.flags_4b2 |= 0x80;
            settlement.flags_4b2 &= 0xEA;
            settlement.progress_14 = 0;
            settlement.money_pool_10 = 0;
        }
        for trade in self.settlement_trade.iter_mut().filter(|t| t.index == si) {
            trade.flags_4b2 |= 0x80;
            trade.flags_4b2 &= 0xEA;
        }
        // The party stops fighting (`Entity+0x06/+0x02 = 0`, idle).
        self.player_attack_target = None;
        self.player_path.clear();
        self.player_active = false;
        for member in &mut self.party {
            member.motion.combat_target = None;
            member.motion.path.clear();
            member.motion.target = None;
            member.motion.walking = false;
        }
        // `FUN_0042ADE4("DEFEAT<location>.FLM")`.
        video::request(VideoClip::Defeat(loc as u8), VideoAfter::Gameplay);
    }

    /// `+0x14` event counter: every `0x2760` passes (when settlement flag
    /// `0x10` is clear) add the recovered money-pool trickle. `FUN_0043EB2E`
    /// is `sqrt`; `FUN_0043eaae` is a plain round.
    pub(super) fn settlement_random_event(&mut self, si: usize) {
        let Some(settlement) = self.settlements.get(si) else {
            return;
        };
        if settlement.flags_4b2 & 0x10 != 0 {
            return;
        }
        let group_id = settlement.group_id as usize;
        // `FUN_00431820` removes a recruited member from its old group; the
        // remake keeps that row as an overlay, so subtract party members whose
        // source row still belongs to this group from the authored count.
        let authored_count = self
            .tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .map(|g| g.entity_count as i32)
            .unwrap_or(0);
        let party_in_group = self
            .tables
            .as_ref()
            .map(|t| {
                self.party
                    .iter()
                    .filter(|m| {
                        t.entities
                            .get(m.entity.index)
                            .map(|e| e.group_id as usize == group_id)
                            .unwrap_or(false)
                    })
                    .count() as i32
            })
            .unwrap_or(0);
        let count = authored_count.saturating_sub(party_in_group).max(0) as f64;
        let base = settlement.target_base0 as f64;
        let trade = self.player.trade as f64;
        let Some(settlement) = self.settlements.get_mut(si) else {
            return;
        };
        settlement.progress_14 = settlement.progress_14.saturating_add(1);
        if settlement.progress_14 < 0x2760 {
            return;
        }
        let add = (count * 5.0 + base * 20.0 * trade.sqrt() + 1.0).round_ties_even() as i32;
        settlement.money_pool_10 = settlement.money_pool_10.saturating_add(add);
        settlement.progress_14 = 0;
    }
}
