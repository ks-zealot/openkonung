//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).
//!
//! This file: the per-unit attack cycles for the leader, the party and the world
//! actors.

use std::collections::HashSet;

use super::super::*;
use crate::actors::PopulationBuild;
use crate::combat::CombatTarget;
use crate::coords::direction_from_cells;

impl PlayableApp {
    /// Frame count of the protagonist's melee row for the current facing and
    /// stance (`Entity+0xD7` equivalent), read from the GRAPH tables.
    ///
    /// Falls back to `1` when the visual family cannot be resolved, so an attack
    /// always resolves on a frame boundary.
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

    /// One logic tick of the protagonist's attack cycle.
    ///
    /// * idle with an adjacent attacker -> `FUN_0041EB48` retaliation, which
    ///   auto-acquires the enemy and enters battle stance;
    /// * ranged order -> action row `4`, one arrow spawned on frame
    ///   `count - 6` of every cycle;
    /// * melee order -> face the adjacent target, play the melee row and resolve
    ///   the hit on its last frame (`FUN_004123BC` case 5);
    /// * not adjacent -> walk to the nearest free neighbour of the target.
    ///
    /// A manual order writes no move destination, so the walk branch only runs
    /// while the leader is idle.
    pub(in crate::app) fn update_player_combat(&mut self) {
        if self.player.dead {
            return;
        }
        // `FUN_0041E2D4`: the leader's persistent engage order also re-acquires
        // the next hostile after a kill (the original runs the same group-0
        // actor logic for entity 0).
        if self.player_engaged {
            let missing = self
                .player_attack_target
                .map_or(true, |t| self.combatant_dead(t));
            if missing && !self.player_active && self.player_path.is_empty() {
                let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
                let ranged = self.ranged_ready(CombatTarget::Player, false);
                match self.acquire_next_hostile(CombatTarget::Player, pc, ranged) {
                    Some(target) => {
                        self.player_attack_target = Some(target);
                        self.player_attack_frame = 0;
                        self.player_attack_frames = 0;
                        self.player_attack_ranged = ranged;
                        self.player_ranged_frame = 0;
                        self.player_ranged_frames = 0;
                        self.player_ranged_fired = false;
                        self.target = None;
                    }
                    None => {
                        // `+0x02 &= 0xFE`: no hostile left, the order ends.
                        self.player_engaged = false;
                        self.player_attack_target = None;
                        self.player_attack_ranged = false;
                    }
                }
            }
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

    /// One logic tick of every recruited companion's attack cycle.
    ///
    /// The same rules as the protagonist, applied to all members in one pass: the
    /// target cell is resolved from the live population, `FUN_0041EB48`
    /// retaliation applies to idle members, and the resulting attacks, arrow
    /// spawns and action sounds are queued so they can be applied after the
    /// mutable party borrow ends.
    pub(in crate::app) fn update_party_combat(&mut self) {
        if self.party.is_empty() {
            return;
        }
        // `FUN_0041E2D4`: a unit holding the persistent engage order re-acquires
        // the next hostile after its target dies (or leaves the map). The
        // original runs this from `FUN_0041ECC8` on the idle/walk tick, one
        // member at a time, so an earlier member's fresh target is already
        // visible to `FUN_00420190` for the next one. A unit still walking
        // finishes its current segment first.
        for i in 0..self.party.len() {
            let needs_target = match self.party.get(i) {
                Some(m) => {
                    !m.motion.dead
                        && m.motion.engaged
                        && !m.motion.walking
                        && m.motion
                            .combat_target
                            .map_or(true, |t| self.combatant_dead(t))
                }
                None => false,
            };
            if !needs_target {
                continue;
            }
            let cell = self.party[i].motion.cell;
            let ranged = self.ranged_ready(CombatTarget::Party(i), false);
            let target = self.acquire_next_hostile(CombatTarget::Party(i), cell, ranged);
            let Some(m) = self.party.get_mut(i) else {
                continue;
            };
            match target {
                Some(target) => {
                    m.motion.combat_target = Some(target);
                    m.motion.attack_frames = 0;
                    m.motion.frame = 0;
                    m.motion.ranged_order = ranged;
                    m.motion.ranged_fired = false;
                    m.motion.path.clear();
                    m.motion.target = None;
                    m.motion.walking = false;
                    m.motion.render_cell = m.motion.cell;
                }
                None => {
                    // `+0x02 &= 0xFE`: no hostile left, the order ends.
                    m.motion.combat_target = None;
                    m.motion.engaged = false;
                    m.motion.ranged_order = false;
                    m.motion.ranged_fired = false;
                }
            }
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

    /// One logic tick of world-actor combat.
    ///
    /// A humanoid holding a bow plus ammo switches to the ranged row as soon as it
    /// owns a target (`FUN_00412E00`); otherwise the unit faces the target, plays
    /// the melee row and resolves on its last frame, or walks to a free
    /// neighbour. `archetype 0x4D/0x4F` units spend their authored M/O delay in
    /// `Entity+0x12` between cycles and re-roll it from the `Entity+0xD7`
    /// ceiling, exactly like the original walk case.
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
