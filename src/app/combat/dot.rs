//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use super::super::*;
use crate::combat::CombatTarget;

impl PlayableApp {
    pub(super) fn apply_dot(&mut self) {
        if !self.player.dead && self.player.hp_internal > 0 && self.player.periodic_damage > 0 {
            let hp = self
                .player
                .hp_internal
                .saturating_sub(self.player.periodic_damage);
            if hp == 0 {
                self.kill_combatant(CombatTarget::Player, self.player.level, 0);
            } else {
                self.player.hp_internal = hp;
            }
        }
        for i in 0..self.party.len() {
            let (hp, dot, dead) = match self.party.get(i) {
                Some(m) => (
                    m.entity.stats.hp_internal,
                    m.entity.stats.periodic_damage,
                    m.motion.dead,
                ),
                None => continue,
            };
            if dead || hp == 0 || dot == 0 {
                continue;
            }
            let next = hp.saturating_sub(dot);
            if next == 0 {
                self.kill_combatant(CombatTarget::Party(i), self.player.level, 0);
            } else if let Some(m) = self.party.get_mut(i) {
                m.entity.stats.hp_internal = next;
            }
        }
        let ids: Vec<usize> = self
            .population_cache
            .get(&self.current_map)
            .map(|p| p.actors.iter().map(|a| a.entity.index).collect())
            .unwrap_or_default();
        for id in ids {
            let dead = self
                .population_cache
                .get(&self.current_map)
                .and_then(|p| p.motion.get(&id))
                .map(|m| m.dead)
                .unwrap_or(true);
            let (hp, dot) = match self.combatant_entity(id) {
                Some(e) => (e.stats.hp_internal, e.stats.periodic_damage),
                None => continue,
            };
            if dead || hp == 0 || dot == 0 {
                continue;
            }
            let next = hp.saturating_sub(dot);
            if next == 0 {
                self.kill_combatant(CombatTarget::World(id), self.player.level, 0);
            } else if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                    a.entity.stats.hp_internal = next;
                }
            }
        }
    }
}
