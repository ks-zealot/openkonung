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
use super::acquire::armed_los;

/// Facing written before an attack.
///
/// The original action machines never turn toward the attack target: a unit
/// attacks with the facing it last walked into (`FUN_0041435C` stores the
/// queue-head direction). For an **adjacent** target the remake therefore uses
/// the original `FUN_00438640` probe, which returns the step that actually
/// reaches the cell; for a distant (ranged) target it keeps the sector formula
/// `FUN_0043897C` so an archer still faces the right way.
fn attack_facing(tc: (i32, i32), pc: (i32, i32)) -> u8 {
    if PlayableApp::is_adjacent(pc, tc) {
        crate::coords::face_direction(tc.0, tc.1, pc.0, pc.1)
    } else {
        direction_from_cells(tc.0, tc.1, pc.0, pc.1)
    }
}

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

    /// `FUN_0041EB48` adjacent acquisition.
    ///
    /// The original walks the eight neighbours of `cell` through the collision
    /// grid and accepts the first occupant whose marker is a real entity that
    /// (a) belongs to a **different group**, (b) still holds its engage order
    /// (`Entity+0x02 & 1`) and (c) targets `me` (`Entity+0x06`). There is no
    /// object-family filter: any other-group actor qualifies.
    fn adjacent_world_attacker(&self, me: CombatTarget, cell: (i32, i32)) -> Option<usize> {
        let pop = self.population_cache.get(&self.current_map)?;
        pop.actors.iter().find_map(|a| {
            // The leader and the companions are group 0; `0x123` writes
            // `Entity+0x05 = 0` when a companion joins.
            if a.entity.group_id == 0 {
                return None;
            }
            let m = pop.motion.get(&a.entity.index)?;
            if m.dead || m.combat_target != Some(me) {
                return None;
            }
            // `Entity+0x02 & 1`: the attacker still holds a mode-1 engage order
            // (`FUN_004142D4(...,1)` writes the whole order byte).
            if !m.engaged {
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
        // `FUN_00421B50` + `FUN_004116CC`: the leader only takes its AI turn when
        // the frame budget allows and the cursor is behind it (one-based order).
        // The attack cycle below still advances; the gate only stops new
        // acquisition/retaliation, like `FUN_0041ECC8`.
        let take_turn = self.ai_schedule.allows(1);
        if take_turn {
            self.ai_schedule.serve(1);
        }
        // `FUN_0041E2D4`: the leader's persistent engage order also re-acquires
        // the next hostile after a kill (the original runs the same group-0
        // actor logic for entity 0).
        if take_turn && self.player_engaged {
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
                        // `FUN_0041E2D4` clears bit 4 (`+0x02 &= 0xFB`) on a
                        // successful re-acquire: the ranged order has to be
                        // re-established by a new command.
                        self.player_attack_ranged = false;
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
        if take_turn
            && self.player_attack_target.is_none()
            && !self.player_engaged
            && !self.player_active
            && self.player_path.is_empty()
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
        let dir = attack_facing(tc, pc);
        if self.player_attack_ranged {
            // `FUN_0041ECC8` `+0x02 & 4` branch: re-probe every cycle. The
            // original clears the target and, for the leader, bit 4 itself
            // (`+0x02 &= 0xFB`) whenever the probe is not a clear `1`.
            if !matches!(target, CombatTarget::Prop(_)) {
                let los = self.world.collision(self.current_map).and_then(|collision| {
                    armed_los(
                        &self.world.res,
                        collision,
                        self.player.raw_0x12,
                        &self.player.equipment,
                        self.player.special_equipment,
                        pc,
                        tc,
                    )
                });
                if !matches!(los, Some(crate::combat::los::LineOfFire::Clear)) {
                    self.player_attack_target = None;
                    self.player_attack_ranged = false;
                    self.player_attack_frame = 0;
                    return;
                }
            }
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
                let collision = self.world.collision_mut(loc).unwrap_or(&mut []);
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
        // `FUN_004116CC`: at most one AI turn per member per frame, in group-0
        // order (`entity.index + 1`, the original's address comparison). A unit
        // that is skipped this frame keeps attacking; only new acquisition and
        // retaliation are gated.
        let mut take_turn = vec![false; self.party.len()];
        for i in 0..self.party.len() {
            let order = self.party[i].entity.index + 1;
            if self.ai_schedule.allows(order) {
                self.ai_schedule.serve(order);
                take_turn[i] = true;
            }
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
            if !needs_target || !take_turn.get(i).copied().unwrap_or(false) {
                continue;
            }
            let cell = self.party[i].motion.cell;
            let ranged = self.ranged_ready(CombatTarget::Party(i), false);
            // `FUN_00410E84` charges one action when the armed pick runs.
            if ranged {
                self.ai_schedule.charge();
            }
            let target = self.acquire_next_hostile(CombatTarget::Party(i), cell, ranged);
            let marker = target
                .and_then(|t| self.combat_target_marker(t))
                .unwrap_or(0);
            let Some(m) = self.party.get_mut(i) else {
                continue;
            };
            match target {
                Some(target) => {
                    m.motion.combat_target = Some(target);
                    m.motion.target_marker = marker;
                    m.motion.attack_frames = 0;
                    m.motion.frame = 0;
                    // `FUN_0041E2D4` clears bit 4 on a successful re-acquire.
                    m.motion.ranged_order = false;
                    m.motion.ranged_fired = false;
                    m.motion.path.clear();
                    m.motion.target = None;
                    m.motion.walking = false;
                    m.motion.render_cell = m.motion.cell;
                }
                None => {
                    // `+0x02 &= 0xFE`: no hostile left, the order ends.
                    m.motion.combat_target = None;
                    m.motion.target_marker = 0;
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
                // `FUN_0041EB48` runs inside the gated `FUN_0041ECC8` branch.
                if !take_turn.get(i).copied().unwrap_or(false) {
                    return None;
                }
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
        let prop_cells: HashMap<usize, (i32, i32)> = world
            .loc(loc)
            .map(|l| {
                l.props
                    .iter()
                    .map(|p| (p.ri, (p.grid_x, p.grid_y)))
                    .collect()
            })
            .unwrap_or_default();
        let Some((collision, res)) = world.collision_and_res_mut(loc) else {
            return;
        };
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
            if m.motion.combat_target.is_none()
                && !m.motion.engaged
                && !m.motion.walking
                && m.motion.path.is_empty()
            {
                if let Some(id) = party_retaliate.get(i).copied().flatten() {
                    m.entity.flags_b |= crate::anim::FLAG_BATTLE;
                    m.motion.combat_target = Some(CombatTarget::World(id));
                    m.motion.target_marker = crate::actors::occupancy::marker(id);
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
                CombatTarget::Prop(index) => prop_cells.get(&index).copied(),
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
                m.motion.target_marker = 0;
                continue;
            }
            if m.motion.ranged_order {
                // `FUN_0041ECC8` `+0x02 & 4`: probe before every ranged cycle.
                // Blocked (`0`) clears the target but keeps the order for a
                // companion; too close (`-1`) or an unarmed unit clears bit 4;
                // the gate failure keeps the target untouched. Props are a
                // mouse-path target, not an `Entity+0x06` one, so they skip the
                // actor-logic probe.
                if !matches!(target, CombatTarget::Prop(_)) {
                    let los = armed_los(
                        res,
                        collision,
                        m.entity.raw_0x12,
                        &m.entity.equipment,
                        m.entity.special_equipment,
                        m.motion.cell,
                        tc,
                    );
                    match los {
                        Some(crate::combat::los::LineOfFire::Clear) => {}
                        Some(crate::combat::los::LineOfFire::Blocked) => {
                            m.motion.combat_target = None;
                            m.motion.target_marker = 0;
                            continue;
                        }
                        Some(crate::combat::los::LineOfFire::TooClose) => {
                            m.motion.combat_target = None;
                            m.motion.target_marker = 0;
                            m.motion.ranged_order = false;
                            continue;
                        }
                        None => {
                            m.motion.ranged_order = false;
                            continue;
                        }
                    }
                }
                m.motion.path.clear();
                m.motion.walking = false;
                m.motion.render_cell = m.motion.cell;
                let dir = attack_facing(tc, m.motion.cell);
                m.motion.facing = dir;
                m.motion.action = crate::anim::ACT_RANGED as u8;
                if m.motion.attack_frames == 0 {
                    action_sounds.push((
                        m.entity.archetype,
                        crate::anim::ACT_RANGED as u8,
                        m.entity.raw_0x12,
                        m.motion.cell,
                    ));
                    m.motion.attack_frames = res
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
                let dir = attack_facing(tc, m.motion.cell);
                m.motion.facing = dir;
                let battle = crate::anim::battle_from_flags(m.entity.flags_b);
                if let Some(frames) = res.walk_cycle_frames(
                    m.entity.archetype,
                    m.entity.visual_index,
                    dir as usize,
                    battle,
                ) {
                    m.motion.walk_frames = frames.min(u8::MAX as usize) as u8;
                }
                if m.motion.attack_frames == 0 {
                    m.motion.attack_frames = res
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
                            Self::start_motion_walk(
                                &mut m.motion,
                                m.entity.index,
                                collision,
                                &mut occupied,
                                goal,
                            );
                        }
                    }
                }
                Self::advance_motion_walk(&mut m.motion, m.entity.index, collision, &mut occupied);
            }
        }
        for (archetype, action, weapon_slot, cell) in action_sounds {
            self.play_actor_action(archetype, action, weapon_slot, false, Some(cell));
        }
        for (i, tc, prop) in shots {
            if !self.spawn_projectile(CombatTarget::Party(i), tc, prop) {
                if let Some(m) = self.party.get_mut(i) {
                    m.motion.combat_target = None;
                    m.motion.target_marker = 0;
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
        let prop_cells: HashMap<usize, (i32, i32)> = world
            .loc(loc)
            .map(|l| {
                l.props
                    .iter()
                    .map(|p| (p.ri, (p.grid_x, p.grid_y)))
                    .collect()
            })
            .unwrap_or_default();
        let Some((collision, res)) = world.collision_and_res_mut(loc) else {
            return;
        };
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
                CombatTarget::Prop(index) => prop_cells.get(&index).copied(),
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
                m.target_marker = 0;
                m.engaged = false;
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
                // `FUN_0041ECC8` `+0x02 & 4`: probe before every ranged cycle.
                // A blocked line clears the target (the order stays set for a
                // world unit), too close or an unarmed unit clears bit 4.
                // Props are a mouse-path target, not an `Entity+0x06` one.
                if !matches!(target, CombatTarget::Prop(_)) {
                    let los = armed_los(
                        res,
                        collision,
                        a.entity.raw_0x12,
                        &a.entity.equipment,
                        a.entity.special_equipment,
                        m.cell,
                        tc,
                    );
                    match los {
                        Some(crate::combat::los::LineOfFire::Clear) => {}
                        Some(crate::combat::los::LineOfFire::Blocked) => {
                            m.combat_target = None;
                            m.target_marker = 0;
                            continue;
                        }
                        Some(crate::combat::los::LineOfFire::TooClose) => {
                            m.combat_target = None;
                            m.target_marker = 0;
                            m.ranged_order = false;
                            continue;
                        }
                        None => {
                            m.ranged_order = false;
                            continue;
                        }
                    }
                }
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
                let dir = attack_facing(tc, m.cell);
                m.facing = dir;
                m.action = crate::anim::ACT_RANGED as u8;
                if m.attack_frames == 0 {
                    action_sounds.push((
                        a.entity.archetype,
                        crate::anim::ACT_RANGED as u8,
                        a.entity.raw_0x12,
                        m.cell,
                    ));
                    m.attack_frames = res
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
                let dir = attack_facing(tc, m.cell);
                m.facing = dir;
                let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                if let Some(frames) = res.walk_cycle_frames(
                    a.entity.archetype,
                    a.entity.visual_index,
                    dir as usize,
                    battle,
                ) {
                    m.walk_frames = frames.min(u8::MAX as usize) as u8;
                }
                if m.attack_frames == 0 {
                    m.attack_frames = res
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
                            crate::actors::begin_actor_segment(m, id, &mut occupied, collision);
                        } else if let Some(goal) =
                            Self::melee_goal(m.cell, tc, collision, &occupied)
                        {
                            if goal != m.cell {
                                Self::start_motion_walk(m, id, collision, &mut occupied, goal);
                            }
                        }
                    }
                } else if !is_mo && !m.walking && m.path.is_empty() {
                    if let Some(goal) = Self::melee_goal(m.cell, tc, collision, &occupied) {
                        if goal != m.cell {
                            Self::start_motion_walk(m, id, collision, &mut occupied, goal);
                        }
                    }
                }
                let walking = if is_mo {
                    Self::advance_motion_walk_cycle(m, collision, &mut occupied)
                } else {
                    Self::advance_motion_walk(m, id, collision, &mut occupied)
                };
                if is_mo && was_walking && !walking && a.entity.raw_0x12 == 0 {
                    // `FUN_004123BC` walk-case M/O delay: ceiling is authored
                    // `Entity+0xD7`; one random sign draw then one modulus.
                    let ceiling = res.entity_raw_d7(id) as u32;
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
                        m.target_marker = 0;
                        m.ranged_order = false;
                    }
                }
            }
        }
        for (id, target) in attacks {
            self.resolve_attack(CombatTarget::World(id), target);
            // `FUN_004123BC` case 5: a **surviving** target leaves a non-`0x49`
            // attacker free to attempt the step-aside (`FUN_004101E4`); a kill
            // breaks out of the case before it.
            if !self.combatant_dead(target) {
                self.post_attack_reposition(id);
            }
        }
    }

    /// `FUN_004123BC` case 5 tail: the post-attack step-aside.
    ///
    /// The case ends with
    /// `if (archetype != 0x49 || FUN_004101E4(...) == 0 || FUN_004142D4(...) == 0) advance;`
    /// so **only archetype `0x49`** runs the offset probe: it draws one
    /// `rand()%4`, and on success `FUN_004142D4(cell + offset, 8)` writes the
    /// whole order byte (target, engage bit and ranged order are dropped) and
    /// the unit walks there. Every other archetype just advances its action.
    pub(in crate::app) fn post_attack_reposition(&mut self, id: usize) {
        let loc = self.current_map;
        let Some((archetype, cell)) = self.population_cache.get(&loc).and_then(|p| {
            let a = p.actors.iter().find(|a| a.entity.index == id)?;
            let cell = p.motion.get(&id)?.cell;
            Some((a.entity.archetype, cell))
        }) else {
            return;
        };
        if archetype != 0x49 {
            return;
        }
        let PlayableApp {
            world,
            population_cache,
            combat_rng,
            ..
        } = self;
        let Some((collision, _res)) = world.collision_and_res_mut(loc) else {
            return;
        };
        let Some((dx, dy)) = super::approach::approach_offset(cell, collision, combat_rng) else {
            return;
        };
        let Some(pop) = population_cache.get_mut(&loc) else {
            return;
        };
        let mut occupied: HashSet<(i32, i32)> = pop
            .motion
            .values()
            .filter(|m| !m.dead)
            .map(|m| m.cell)
            .collect();
        let Some(m) = pop.motion.get_mut(&id) else {
            return;
        };
        m.combat_target = None;
        m.target_marker = 0;
        m.engaged = false;
        m.ranged_order = false;
        m.ranged_fired = false;
        m.attack_frames = 0;
        Self::start_motion_walk(
            m,
            id,
            collision,
            &mut occupied,
            (cell.0 + dx, cell.1 + dy),
        );
    }
}
