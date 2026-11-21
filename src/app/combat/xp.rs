//! Kill rewards: `FUN_0041079C` experience and the mastery gain.
//!
//! Both kill paths share one shape:
//!
//! * melee — `FUN_004123BC` case 5: `FUN_0041079C(self, power/4)`,
//!   `self.melee_mastery(+0x40) += power`, `FUN_00418480(target, 0, 0)`;
//! * ranged — `FUN_00420F30`: `FUN_0041079C(shooter, power/4)`,
//!   `shooter.archery_mastery(+0x44) += power`, `FUN_00418480(target, power, kind)`.
//!
//! `power` is the victim's `FUN_0041B658` combat-power metric. A kill therefore
//! pays **experience**, not money; money from a body is the corpse loot.

use super::super::*;
use crate::character::{class_index, grant_xp, growth_index_raw, XpView};
use crate::combat::CombatTarget;
use crate::constants::combat::KILL_XP_DIVISOR;

impl PlayableApp {
    /// `FUN_0041079C` for the leader, a companion or a world actor.
    pub(in crate::app) fn grant_combat_xp(&mut self, who: CombatTarget, amount: i32) -> bool {
        match who {
            CombatTarget::Player => grant_xp(
                XpView {
                    class_index: Some(class_index(self.player.class)),
                    temp_effect: self.player.temp_effect_timer,
                    flags_b: self.player.flags_b,
                    level: &mut self.player.level,
                    available_xp: &mut self.player.available_xp,
                    invested_xp: &mut self.player.invested_xp,
                    dexterity: &mut self.player.dexterity_current,
                    charisma: &mut self.player.charisma_current,
                    strength: &mut self.player.strength_current,
                    endurance: &mut self.player.endurance_current,
                },
                amount,
            ),
            CombatTarget::Party(i) => {
                let Some(m) = self.party.get_mut(i) else {
                    return false;
                };
                grant_xp(
                    XpView {
                        class_index: growth_index_raw(m.entity.archetype),
                        temp_effect: m.entity.stats.temp_effect_timer,
                        flags_b: m.entity.flags_b,
                        level: &mut m.entity.stats.level,
                        available_xp: &mut m.entity.stats.available_xp,
                        invested_xp: &mut m.entity.stats.invested_xp,
                        dexterity: &mut m.entity.stats.dexterity_current,
                        charisma: &mut m.entity.stats.charisma_current,
                        strength: &mut m.entity.stats.strength_current,
                        endurance: &mut m.entity.stats.endurance_current,
                    },
                    amount,
                )
            }
            CombatTarget::World(id) => {
                let loc = self.current_map;
                let Some(pop) = self.population_cache.get_mut(&loc) else {
                    return false;
                };
                let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) else {
                    return false;
                };
                grant_xp(
                    XpView {
                        class_index: growth_index_raw(a.entity.archetype),
                        temp_effect: a.entity.stats.temp_effect_timer,
                        flags_b: a.entity.flags_b,
                        level: &mut a.entity.stats.level,
                        available_xp: &mut a.entity.stats.available_xp,
                        invested_xp: &mut a.entity.stats.invested_xp,
                        dexterity: &mut a.entity.stats.dexterity_current,
                        charisma: &mut a.entity.stats.charisma_current,
                        strength: &mut a.entity.stats.strength_current,
                        endurance: &mut a.entity.stats.endurance_current,
                    },
                    amount,
                )
            }
            CombatTarget::Prop(_) => false,
        }
    }

    /// Pay a kill: `power / 4` experience and `power` into the matching mastery.
    ///
    /// `melee` selects `Entity+0x40` (melee mastery) over `Entity+0x44`
    /// (archery mastery). `power <= 0` is ignored, exactly like the call sites.
    pub(in crate::app) fn award_kill_reward(
        &mut self,
        owner: CombatTarget,
        power: i32,
        melee: bool,
    ) {
        if power <= 0 {
            return;
        }
        self.grant_combat_xp(owner, power / KILL_XP_DIVISOR);
        let skill = power.clamp(0, u16::MAX as i32) as u16;
        match owner {
            CombatTarget::Player => {
                if melee {
                    self.player.melee_mastery = self.player.melee_mastery.saturating_add(skill);
                } else {
                    self.player.archery_mastery =
                        self.player.archery_mastery.saturating_add(skill);
                }
            }
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    let mastery = if melee {
                        &mut m.entity.stats.melee_mastery
                    } else {
                        &mut m.entity.stats.archery_mastery
                    };
                    *mastery = mastery.saturating_add(skill);
                }
            }
            CombatTarget::World(id) => {
                let loc = self.current_map;
                if let Some(pop) = self.population_cache.get_mut(&loc) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        let mastery = if melee {
                            &mut a.entity.stats.melee_mastery
                        } else {
                            &mut a.entity.stats.archery_mastery
                        };
                        *mastery = mastery.saturating_add(skill);
                    }
                }
            }
            CombatTarget::Prop(_) => {}
        }
    }
}
