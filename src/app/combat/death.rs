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
    /// Enter the death row and (for world actors) spawn one corpse marker.
    pub(super) fn kill_combatant(&mut self, target: CombatTarget, killer_level: u8, bonus: i32) {
        if self.combatant_dead(target) {
            return;
        }
        let dir = self.combatant_facing(target) as usize;
        match target {
            CombatTarget::Player => {
                self.player.hp_internal = 0;
                self.player.dead = true;
                self.player_attack_target = None;
                self.player_attack_frame = 0;
                self.player_death_frame = 0;
                let battle = crate::anim::battle_from_flags(self.player.flags_b);
                self.player_death_frames = self
                    .world
                    .res
                    .action_cycle_frames(
                        self.player.entity_class(),
                        self.player.hero.visual_index,
                        dir,
                        crate::anim::death_action(battle),
                    )
                    .unwrap_or(1)
                    .min(u8::MAX as usize) as u8;
                self.play_actor_action(
                    self.player.entity_class(),
                    crate::anim::death_action(battle) as u8,
                    self.player.raw_0x12,
                    true,
                );
            }
            CombatTarget::Party(i) => {
                let battle = self
                    .party
                    .get(i)
                    .map(|m| crate::anim::battle_from_flags(m.entity.flags_b))
                    .unwrap_or(false);
                let frames = self
                    .party
                    .get(i)
                    .and_then(|m| {
                        self.world.res.action_cycle_frames(
                            m.entity.archetype,
                            m.entity.visual_index,
                            dir,
                            crate::anim::death_action(battle),
                        )
                    })
                    .unwrap_or(1)
                    .min(u8::MAX as usize) as u8;
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.stats.hp_internal = 0;
                    m.motion.dead = true;
                    m.motion.combat_target = None;
                    m.motion.path.clear();
                    m.motion.walking = false;
                    m.motion.render_cell = m.motion.cell;
                    m.motion.action = crate::anim::death_action(battle) as u8;
                    m.motion.frame = 0;
                    m.motion.death_frames = frames;
                }
                if let Some(m) = self.party.get(i) {
                    self.play_actor_action(
                        m.entity.archetype,
                        crate::anim::death_action(battle) as u8,
                        m.entity.raw_0x12,
                        false,
                    );
                }
            }
            CombatTarget::World(id) => {
                let Some(entity) = self.combatant_entity(id) else {
                    return;
                };
                let battle = crate::anim::battle_from_flags(entity.flags_b);
                let frames = self
                    .world
                    .res
                    .action_cycle_frames(
                        entity.archetype,
                        entity.visual_index,
                        dir,
                        crate::anim::death_action(battle),
                    )
                    .unwrap_or(1)
                    .min(u8::MAX as usize) as u8;
                let cell = self
                    .combatant_cell(CombatTarget::World(id))
                    .unwrap_or((0, 0));
                self.spawn_corpse_loot(&entity, cell, killer_level, bonus);
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.stats.hp_internal = 0;
                    }
                    if let Some(m) = pop.motion.get_mut(&id) {
                        m.dead = true;
                        m.combat_target = None;
                        m.path.clear();
                        m.walking = false;
                        m.render_cell = m.cell;
                        m.action = crate::anim::death_action(battle) as u8;
                        m.frame = 0;
                        m.death_frames = frames;
                    }
                }
                self.play_actor_action(
                    entity.archetype,
                    crate::anim::death_action(battle) as u8,
                    entity.raw_0x12,
                    false,
                );
            }
            CombatTarget::Prop(_) => {}
        }
    }

    /// Advance one dead combatant's death row to its final frame.
    fn advance_death_frame(&mut self, t: CombatTarget) {
        match t {
            CombatTarget::Player => {
                if self.player_death_frame + 1 < self.player_death_frames.max(1) {
                    self.player_death_frame += 1;
                }
            }
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    if m.motion.dead && m.motion.frame + 1 < m.motion.death_frames.max(1) {
                        m.motion.frame += 1;
                    }
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(m) = pop.motion.get_mut(&id) {
                        if m.dead && m.frame + 1 < m.death_frames.max(1) {
                            m.frame += 1;
                        }
                    }
                }
            }
            CombatTarget::Prop(_) => {}
        }
    }

    pub(super) fn advance_all_death_frames(&mut self) {
        if self.player.dead {
            self.advance_death_frame(CombatTarget::Player);
        }
        for i in 0..self.party.len() {
            if self.party.get(i).is_some_and(|m| m.motion.dead) {
                self.advance_death_frame(CombatTarget::Party(i));
            }
        }
        let dead: Vec<usize> = self
            .population_cache
            .get(&self.current_map)
            .map(|p| {
                p.motion
                    .iter()
                    .filter(|(_, m)| m.dead)
                    .map(|(&id, _)| id)
                    .collect()
            })
            .unwrap_or_default();
        for id in dead {
            self.advance_death_frame(CombatTarget::World(id));
        }
    }
}
