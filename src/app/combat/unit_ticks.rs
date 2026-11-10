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
        self.world
            .res
            .action_cycle_frames(
                self.player.entity_class(),
                self.player.hero.visual_index,
                (self.player_dir.max(0) as usize) & 7,
                crate::anim::ACT_MELEE,
            )
            .unwrap_or(1)
            .min(u8::MAX as usize) as u8
    }

    /// `FUN_0041EB48` adjacent acquisition: the first live object-family actor
    /// in an adjacent cell whose current target is `me`.
    fn adjacent_world_attacker(&self, me: CombatTarget, cell: (i32, i32)) -> Option<usize> {
        let pop = self.population_cache.get(&self.current_map)?;
        pop.actors.iter().find_map(|a| {
            if a.entity.archetype & 0x40 == 0 {
                return None;
            }
            let m = pop.motion.get(&a.entity.index)?;
            if m.dead || m.combat_target != Some(me) {
                return None;
            }
            Self::is_adjacent(cell, m.cell).then_some(a.entity.index)
        })
    }

    pub(in crate::app) fn update_player_combat(&mut self) {
        if self.player.dead {
            return;
        }
        // `FUN_0041EB48`: an idle unit automatically acquires an adjacent
        // enemy that is already targeting it. This is the original retaliation
        // path that lets the party fight back while walking through a mob
        // group without a manual right-click order.
        if self.player_attack_target.is_none() && !self.player_active && self.player_path.is_empty()
        {
            let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
            if let Some(id) = self.adjacent_world_attacker(CombatTarget::Player, pc) {
                self.player.flags_b |= crate::anim::FLAG_BATTLE;
                self.player_attack_target = Some(CombatTarget::World(id));
                self.player_attack_frame = 0;
                self.player_attack_frames = 0;
                self.player_attack_ranged = false;
                self.player_ranged_frame = 0;
                self.player_ranged_frames = 0;
                self.player_ranged_fired = false;
                self.target = None;
                self.player_path.clear();
                self.player_render_x = self.player_x;
                self.player_render_y = self.player_y;
            }
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
            if self.player_ranged_frame == 0 {
                self.play_actor_action(
                    self.player.entity_class(),
                    crate::anim::ACT_RANGED as u8,
                    self.player.raw_0x12,
                    true,
                    Some((self.player_x.round() as i32, self.player_y.round() as i32)),
                );
            }
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
            if self.player_attack_frame == 0 {
                self.play_actor_action(
                    self.player.entity_class(),
                    crate::anim::ACT_MELEE as u8,
                    self.player.raw_0x12,
                    true,
                    Some((self.player_x.round() as i32, self.player_y.round() as i32)),
                );
            }
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
        let party_retaliate: Vec<Option<usize>> = (0..self.party.len())
            .map(|i| {
                let cell = party_cells.get(i).copied().unwrap_or((0, 0));
                self.adjacent_world_attacker(CombatTarget::Party(i), cell)
            })
            .collect();
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
        let mut action_sounds: Vec<(u8, u8, u8, (i32, i32))> = Vec::new();
        for (i, m) in party.iter_mut().enumerate() {
            if m.motion.dead {
                continue;
            }
            // `FUN_0041EB48`: retaliate against an adjacent attacker while the
            // companion is idle (no manual order).
            if m.motion.combat_target.is_none() && !m.motion.walking && m.motion.path.is_empty() {
                if let Some(id) = party_retaliate.get(i).copied().flatten() {
                    m.entity.flags_b |= crate::anim::FLAG_BATTLE;
                    m.motion.combat_target = Some(CombatTarget::World(id));
                    m.motion.attack_frames = 0;
                    m.motion.frame = 0;
                    m.motion.ranged_order = false;
                    m.motion.ranged_fired = false;
                    m.motion.render_cell = m.motion.cell;
                }
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
                    action_sounds.push((
                        m.entity.archetype,
                        crate::anim::ACT_RANGED as u8,
                        m.entity.raw_0x12,
                        m.motion.cell,
                    ));
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
                            crate::anim::ACT_MELEE,
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                m.motion.action = crate::anim::ACT_MELEE as u8;
                if m.motion.frame == 0 {
                    action_sounds.push((
                        m.entity.archetype,
                        crate::anim::ACT_MELEE as u8,
                        m.entity.raw_0x12,
                        m.motion.cell,
                    ));
                }
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
        for (archetype, action, weapon_slot, cell) in action_sounds {
            self.play_actor_action(archetype, action, weapon_slot, false, Some(cell));
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

    pub(in crate::app) fn update_world_combat(&mut self) {
        let loc = self.current_map;
        let player_cell = (self.player_x.round() as i32, self.player_y.round() as i32);
        let player_dead = self.player.dead;
        let party_cells: Vec<(i32, i32)> = self.party.iter().map(|m| m.motion.cell).collect();
        let party_dead: Vec<bool> = self.party.iter().map(|m| m.motion.dead).collect();
        // `FUN_00412E00`: a humanoid actor holding a bow + ammo switches to the
        // ranged action row (4) as soon as it owns a combat target. Resolve the
        // capability before taking the mutable population borrow.
        let world_ids: Vec<usize> = self
            .population_cache
            .get(&loc)
            .map(|p| p.actors.iter().map(|a| a.entity.index).collect())
            .unwrap_or_default();
        let ranged_caps: HashSet<usize> = world_ids
            .into_iter()
            .filter(|&id| self.ranged_ready(CombatTarget::World(id), false))
            .collect();
        let PlayableApp {
            world,
            population_cache,
            combat_rng,
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
        let mut action_sounds: Vec<(u8, u8, u8, (i32, i32))> = Vec::new();
        let world_state: HashMap<usize, ((i32, i32), bool)> = pop
            .motion
            .iter()
            .map(|(&id, m)| (id, (m.cell, m.dead)))
            .collect();
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
                CombatTarget::World(id) => world_state.get(&id).map(|&(cell, _)| cell),
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
                CombatTarget::World(id) => {
                    world_state.get(&id).map(|&(_, dead)| dead).unwrap_or(true)
                }
                CombatTarget::Prop(_) => false,
            };
            if target_dead {
                m.combat_target = None;
                continue;
            }
            if !m.ranged_order && ranged_caps.contains(&id) {
                m.ranged_order = true;
                m.ranged_fired = false;
                m.attack_frames = 0;
                m.frame = 0;
                m.path.clear();
                m.target = None;
                m.walking = false;
                m.render_cell = m.cell;
            }
            if m.ranged_order {
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
                let dir = direction_from_cells(tc.0, tc.1, m.cell.0, m.cell.1);
                m.facing = dir;
                m.action = crate::anim::ACT_RANGED as u8;
                if m.attack_frames == 0 {
                    action_sounds.push((
                        a.entity.archetype,
                        crate::anim::ACT_RANGED as u8,
                        a.entity.raw_0x12,
                        m.cell,
                    ));
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
                            crate::anim::ACT_MELEE,
                        )
                        .unwrap_or(1)
                        .min(u8::MAX as usize) as u8;
                }
                m.action = crate::anim::ACT_MELEE as u8;
                if m.frame == 0 {
                    action_sounds.push((
                        a.entity.archetype,
                        crate::anim::ACT_MELEE as u8,
                        a.entity.raw_0x12,
                        m.cell,
                    ));
                }
                m.frame = m.frame.saturating_add(1);
                if m.frame >= m.attack_frames.max(1) {
                    m.frame = 0;
                    attacks.push((id, target));
                }
            } else {
                let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                let is_mo = matches!(a.entity.archetype, 0x4D | 0x4F);
                let was_walking = m.walking;
                let mut hold = false;
                if is_mo && !m.walking {
                    if a.entity.raw_0x12 > 0 {
                        a.entity.raw_0x12 -= 1;
                        hold = a.entity.raw_0x12 > 0;
                    }
                    if !hold {
                        if !m.path.is_empty() {
                            crate::actors::begin_actor_segment(m, &mut occupied, collision);
                        } else if let Some(goal) =
                            Self::melee_goal(m.cell, tc, collision, &occupied)
                        {
                            if goal != m.cell {
                                Self::start_motion_walk(m, collision, &mut occupied, goal);
                            }
                        }
                    }
                } else if !is_mo && !m.walking && m.path.is_empty() {
                    if let Some(goal) = Self::melee_goal(m.cell, tc, collision, &occupied) {
                        if goal != m.cell {
                            Self::start_motion_walk(m, collision, &mut occupied, goal);
                        }
                    }
                }
                let walking = if is_mo {
                    Self::advance_motion_walk_cycle(m, collision, &mut occupied)
                } else {
                    Self::advance_motion_walk(m, collision, &mut occupied)
                };
                if is_mo && was_walking && !walking && a.entity.raw_0x12 == 0 {
                    // `FUN_004123BC` walk-case M/O delay: ceiling is authored
                    // `Entity+0xD7`; one random sign draw then one modulus.
                    let ceiling = world.res.entity_raw_d7(id) as u32;
                    let span = if ceiling < 2 {
                        0
                    } else {
                        combat_rng.next_u32() % ceiling
                    };
                    a.entity.raw_0x12 = (combat_rng.next_u32() % 2) as u8 * span as u8 + 1;
                }
                if walking {
                    m.action = crate::anim::walk_action(battle) as u8;
                } else {
                    m.action = crate::anim::idle_action(battle) as u8;
                    m.frame = 0;
                }
            }
        }
        for (archetype, action, weapon_slot, cell) in action_sounds {
            self.play_actor_action(archetype, action, weapon_slot, false, Some(cell));
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
