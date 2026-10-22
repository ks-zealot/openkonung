//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use std::collections::HashSet;

use super::super::*;
use crate::actors::PopulationBuild;
use crate::combat::CombatTarget;
use crate::coords::direction_from_cells;

impl PlayableApp {
    fn player_attack_frames(&self) -> u8 {
        let battle = crate::anim::battle_from_flags(self.player.flags_b);
        self.world
            .res
            .action_cycle_frames(
                self.player.entity_class(),
                self.player.hero.visual_index,
                (self.player_dir.max(0) as usize) & 7,
                crate::anim::attack_action(battle),
            )
            .unwrap_or(1)
            .min(u8::MAX as usize) as u8
    }

    pub(super) fn update_player_combat(&mut self) {
        if self.player.dead {
            return;
        }
        let Some(target) = self.player_attack_target else {
            self.player_attack_frame = 0;
            self.player_attack_ranged = false;
            return;
        };
        if self.combatant_dead(target) {
            self.player_attack_target = None;
            self.player_attack_frame = 0;
            self.player_attack_ranged = false;
            return;
        }
        let Some(tc) = self.combatant_cell(target) else {
            self.player_attack_target = None;
            return;
        };
        let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
        let dir = direction_from_cells(tc.0, tc.1, pc.0, pc.1);
        if self.player_attack_ranged {
            self.player_path.clear();
            self.player_active = false;
            self.player_render_x = self.player_x;
            self.player_render_y = self.player_y;
            self.set_combat_facing(CombatTarget::Player, dir);
            self.player_ranged_frames = self
                .world
                .res
                .action_cycle_frames(
                    self.player.entity_class(),
                    self.player.hero.visual_index,
                    dir as usize,
                    crate::anim::ACT_RANGED,
                )
                .unwrap_or(1)
                .min(u8::MAX as usize) as u8;
            let spawn_at = self.player_ranged_frames.saturating_sub(6);
            self.player_ranged_frame = self.player_ranged_frame.saturating_add(1);
            if self.player_ranged_frame > self.player_ranged_frames {
                self.player_ranged_frame = 1;
                self.player_ranged_fired = false;
                if !self.ranged_ready(CombatTarget::Player, false) {
                    self.player_attack_target = None;
                    self.player_attack_ranged = false;
                    return;
                }
            }
            if self.player_ranged_frame >= spawn_at.max(1) && !self.player_ranged_fired {
                let prop_target = match target {
                    CombatTarget::Prop(i) => Some(i),
                    _ => None,
                };
                self.player_ranged_fired = true;
                self.spawn_projectile(CombatTarget::Player, tc, prop_target);
            }
            return;
        }
        if Self::is_adjacent(pc, tc) {
            self.player_path.clear();
            self.player_active = false;
            self.player_render_x = self.player_x;
            self.player_render_y = self.player_y;
            self.set_combat_facing(CombatTarget::Player, dir);
            if self.player_attack_frames == 0 {
                self.player_attack_frames = self.player_attack_frames();
            }
            self.player_attack_frames = self.player_attack_frames.max(1);
            self.player_attack_frame = self.player_attack_frame.saturating_add(1);
            if self.player_attack_frame >= self.player_attack_frames {
                self.player_attack_frame = 0;
                self.resolve_attack(CombatTarget::Player, target);
            }
        } else {
            self.player_attack_frame = 0;
            if !self.player_active && self.player_path.is_empty() {
                let loc = self.current_map;
                let collision = self
                    .world
                    .loc(loc)
                    .map(|l| l.collision.as_slice())
                    .unwrap_or(&[]);
                let occupied = HashSet::new();
                if let Some(goal) = Self::melee_goal(pc, tc, collision, &occupied) {
                    if goal != pc {
                        self.set_player_target(goal.0, goal.1);
                    }
                }
            }
        }
    }

    pub(super) fn update_party_combat(&mut self) {
        if self.party.is_empty() {
            return;
        }
        let loc = self.current_map;
        let player_cell = (self.player_x.round() as i32, self.player_y.round() as i32);
        let player_dead = self.player.dead;
        let party_cells: Vec<(i32, i32)> = self.party.iter().map(|m| m.motion.cell).collect();
        let party_dead: Vec<bool> = self.party.iter().map(|m| m.motion.dead).collect();
        let world_cells: std::collections::HashMap<usize, (i32, i32)> = self
            .population_cache
            .get(&loc)
            .map(|p| {
                p.motion
                    .iter()
                    .filter(|(_, m)| !m.dead)
                    .map(|(&id, m)| (id, m.cell))
                    .collect()
            })
            .unwrap_or_default();
        let PlayableApp {
            world,
            party,
            party_selected: _,
            ..
        } = self;
        let collision = world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let mut occupied: HashSet<(i32, i32)> = party
            .iter()
            .filter(|m| !m.motion.dead)
            .map(|m| m.motion.cell)
            .collect();
        occupied.insert(player_cell);
        let mut attacks: Vec<(usize, CombatTarget)> = Vec::new();
        let mut shots: Vec<(usize, (i32, i32), Option<usize>)> = Vec::new();
        for (i, m) in party.iter_mut().enumerate() {
            if m.motion.dead {
                continue;
            }
            let Some(target) = m.motion.combat_target else {
                continue;
            };
            let target_cell = match target {
                CombatTarget::Player => Some(player_cell),
                CombatTarget::Party(j) => party_cells.get(j).copied(),
                CombatTarget::World(id) => world_cells.get(&id).copied(),
                CombatTarget::Prop(index) => world
                    .loc(loc)
                    .and_then(|l| l.props.iter().find(|p| p.ri == index))
                    .map(|p| (p.grid_x, p.grid_y)),
            };
            let Some(tc) = target_cell else {
                continue;
            };
            let target_dead = match target {
                CombatTarget::Player => player_dead,
                CombatTarget::Party(j) => party_dead.get(j).copied().unwrap_or(true),
                CombatTarget::World(id) => !world_cells.contains_key(&id),
                CombatTarget::Prop(_) => false,
            };
            if target_dead {
                m.motion.combat_target = None;
                continue;
            }
            if m.motion.ranged_order {
                m.motion.path.clear();
                m.motion.walking = false;
                m.motion.render_cell = m.motion.cell;
                let dir = direction_from_cells(tc.0, tc.1, m.motion.cell.0, m.motion.cell.1);
                m.motion.facing = dir;
                m.motion.action = crate::anim::ACT_RANGED as u8;
                if m.motion.attack_frames == 0 {
                    m.motion.attack_frames = world
                        .res
                        .action_cycle_frames(
                            m.entity.archetype,
                            m.entity.visual_index,
                            dir as usize,
                            crate::anim::ACT_RANGED,
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                let spawn_at = m.motion.attack_frames.saturating_sub(6);
                m.motion.frame = m.motion.frame.saturating_add(1);
                if m.motion.frame > m.motion.attack_frames.max(1) {
                    m.motion.frame = 1;
                    m.motion.ranged_fired = false;
                }
                if m.motion.frame >= spawn_at.max(1) && !m.motion.ranged_fired {
                    m.motion.ranged_fired = true;
                    let prop = match target {
                        CombatTarget::Prop(index) => Some(index),
                        _ => None,
                    };
                    shots.push((i, tc, prop));
                }
                continue;
            }
            if Self::is_adjacent(m.motion.cell, tc) {
                m.motion.path.clear();
                m.motion.walking = false;
                m.motion.render_cell = m.motion.cell;
                let dir = direction_from_cells(tc.0, tc.1, m.motion.cell.0, m.motion.cell.1);
                m.motion.facing = dir;
                let battle = crate::anim::battle_from_flags(m.entity.flags_b);
                if let Some(frames) = world.res.walk_cycle_frames(
                    m.entity.archetype,
                    m.entity.visual_index,
                    dir as usize,
                    battle,
                ) {
                    m.motion.walk_frames = frames.min(u8::MAX as usize) as u8;
                }
                if m.motion.attack_frames == 0 {
                    m.motion.attack_frames = world
                        .res
                        .action_cycle_frames(
                            m.entity.archetype,
                            m.entity.visual_index,
                            dir as usize,
                            crate::anim::attack_action(battle),
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                m.motion.action = crate::anim::attack_action(battle) as u8;
                m.motion.frame = m.motion.frame.saturating_add(1);
                if m.motion.frame >= m.motion.attack_frames.max(1) {
                    m.motion.frame = 0;
                    attacks.push((i, target));
                }
            } else {
                let battle = crate::anim::battle_from_flags(m.entity.flags_b);
                m.motion.action = crate::anim::walk_action(battle) as u8;
                if !m.motion.walking && m.motion.path.is_empty() {
                    if let Some(goal) = Self::melee_goal(m.motion.cell, tc, collision, &occupied) {
                        if goal != m.motion.cell {
                            Self::start_motion_walk(&mut m.motion, collision, &mut occupied, goal);
                        }
                    }
                }
                Self::advance_motion_walk(&mut m.motion, collision, &mut occupied);
            }
        }
        for (i, tc, prop) in shots {
            if !self.spawn_projectile(CombatTarget::Party(i), tc, prop) {
                if let Some(m) = self.party.get_mut(i) {
                    m.motion.combat_target = None;
                    m.motion.ranged_order = false;
                }
            }
        }
        for (i, target) in attacks {
            self.resolve_attack(CombatTarget::Party(i), target);
        }
    }

    pub(super) fn update_world_combat(&mut self) {
        let loc = self.current_map;
        let player_cell = (self.player_x.round() as i32, self.player_y.round() as i32);
        let player_dead = self.player.dead;
        let party_cells: Vec<(i32, i32)> = self.party.iter().map(|m| m.motion.cell).collect();
        let party_dead: Vec<bool> = self.party.iter().map(|m| m.motion.dead).collect();
        let PlayableApp {
            world,
            population_cache,
            ..
        } = self;
        let collision = world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let Some(pop) = population_cache.get_mut(&loc) else {
            return;
        };
        let mut occupied: HashSet<(i32, i32)> = pop
            .motion
            .values()
            .filter(|m| !m.dead)
            .map(|m| m.cell)
            .collect();
        occupied.insert(player_cell);
        for c in &party_cells {
            occupied.insert(*c);
        }
        let mut attacks: Vec<(usize, CombatTarget)> = Vec::new();
        let mut shots: Vec<(usize, (i32, i32), Option<usize>)> = Vec::new();
        let PopulationBuild { actors, motion, .. } = pop;
        for a in actors.iter_mut() {
            let id = a.entity.index;
            let Some(m) = motion.get_mut(&id) else {
                continue;
            };
            if m.dead {
                continue;
            }
            let Some(target) = m.combat_target else {
                continue;
            };
            let target_cell = match target {
                CombatTarget::Player => Some(player_cell),
                CombatTarget::Party(j) => party_cells.get(j).copied(),
                CombatTarget::World(_) => None,
                CombatTarget::Prop(index) => world
                    .loc(loc)
                    .and_then(|l| l.props.iter().find(|p| p.ri == index))
                    .map(|p| (p.grid_x, p.grid_y)),
            };
            let Some(tc) = target_cell else {
                continue;
            };
            let target_dead = match target {
                CombatTarget::Player => player_dead,
                CombatTarget::Party(j) => party_dead.get(j).copied().unwrap_or(true),
                CombatTarget::World(_) => true,
                CombatTarget::Prop(_) => false,
            };
            if target_dead {
                m.combat_target = None;
                continue;
            }
            if m.ranged_order {
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
                let dir = direction_from_cells(tc.0, tc.1, m.cell.0, m.cell.1);
                m.facing = dir;
                m.action = crate::anim::ACT_RANGED as u8;
                if m.attack_frames == 0 {
                    m.attack_frames = world
                        .res
                        .action_cycle_frames(
                            a.entity.archetype,
                            a.entity.visual_index,
                            dir as usize,
                            crate::anim::ACT_RANGED,
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                let spawn_at = m.attack_frames.saturating_sub(6);
                m.frame = m.frame.saturating_add(1);
                if m.frame > m.attack_frames.max(1) {
                    m.frame = 1;
                    m.ranged_fired = false;
                }
                if m.frame >= spawn_at.max(1) && !m.ranged_fired {
                    m.ranged_fired = true;
                    let prop = match target {
                        CombatTarget::Prop(index) => Some(index),
                        _ => None,
                    };
                    shots.push((id, tc, prop));
                }
                continue;
            }
            if Self::is_adjacent(m.cell, tc) {
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
                let dir = direction_from_cells(tc.0, tc.1, m.cell.0, m.cell.1);
                m.facing = dir;
                let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                if let Some(frames) = world.res.walk_cycle_frames(
                    a.entity.archetype,
                    a.entity.visual_index,
                    dir as usize,
                    battle,
                ) {
                    m.walk_frames = frames.min(u8::MAX as usize) as u8;
                }
                if m.attack_frames == 0 {
                    m.attack_frames = world
                        .res
                        .action_cycle_frames(
                            a.entity.archetype,
                            a.entity.visual_index,
                            dir as usize,
                            crate::anim::attack_action(battle),
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                m.action = crate::anim::attack_action(battle) as u8;
                m.frame = m.frame.saturating_add(1);
                if m.frame >= m.attack_frames.max(1) {
                    m.frame = 0;
                    attacks.push((id, target));
                }
            } else {
                let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                m.action = crate::anim::walk_action(battle) as u8;
                if !m.walking && m.path.is_empty() {
                    if let Some(goal) = Self::melee_goal(m.cell, tc, collision, &occupied) {
                        if goal != m.cell {
                            Self::start_motion_walk(m, collision, &mut occupied, goal);
                        }
                    }
                }
                Self::advance_motion_walk(m, collision, &mut occupied);
            }
        }
        for (id, tc, prop) in shots {
            if !self.spawn_projectile(CombatTarget::World(id), tc, prop) {
                if let Some(pop) = self.population_cache.get_mut(&loc) {
                    if let Some(m) = pop.motion.get_mut(&id) {
                        m.combat_target = None;
                        m.ranged_order = false;
                    }
                }
            }
        }
        for (id, target) in attacks {
            self.resolve_attack(CombatTarget::World(id), target);
        }
    }
}
