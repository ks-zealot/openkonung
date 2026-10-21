//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use std::collections::HashSet;

use super::*;
use crate::actors::{ActorMotion, PopulationBuild};
use crate::combat::{self, CombatStats, CombatTarget, MeleeRoll};
use crate::coords::{direction_from_cells, neighbors};

impl PlayableApp {
    pub(super) fn combatant_stats(&self, t: CombatTarget) -> Option<CombatStats> {
        match t {
            CombatTarget::Player => Some(CombatStats::from_player(&self.player)),
            CombatTarget::Party(i) => self.party.get(i).map(|m| CombatStats::from_entity(&m.entity)),
            CombatTarget::World(id) => self.combatant_entity(id).map(|e| CombatStats::from_entity(&e)),
        }
    }

    pub(super) fn combatant_entity(&self, id: usize) -> Option<crate::population::Entity> {
        self.population_cache
            .get(&self.current_map)?
            .actors
            .iter()
            .find(|a| a.entity.index == id)
            .map(|a| a.entity.clone())
    }

    pub(super) fn combatant_level(&self, t: CombatTarget) -> u8 {
        match t {
            CombatTarget::Player => self.player.level,
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.stats.level).unwrap_or(1),
            CombatTarget::World(id) => self
                .combatant_entity(id)
                .map(|e| e.stats.level)
                .unwrap_or(1),
        }
    }

    pub(super) fn combatant_cell(&self, t: CombatTarget) -> Option<(i32, i32)> {
        match t {
            CombatTarget::Player => Some((self.player_x.round() as i32, self.player_y.round() as i32)),
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.motion.cell),
            CombatTarget::World(id) => self
                .population_cache
                .get(&self.current_map)?
                .motion
                .get(&id)
                .map(|m| m.cell),
        }
    }

    pub(super) fn combatant_facing(&self, t: CombatTarget) -> u8 {
        match t {
            CombatTarget::Player => (self.player_dir.max(0) as u8) & 7,
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.motion.facing).unwrap_or(0),
            CombatTarget::World(id) => self
                .population_cache
                .get(&self.current_map)
                .and_then(|p| p.motion.get(&id))
                .map(|m| m.facing)
                .unwrap_or(0),
        }
    }

    pub(super) fn combatant_dead(&self, t: CombatTarget) -> bool {
        match t {
            CombatTarget::Player => self.player.dead || self.player.hp_internal == 0,
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.motion.dead).unwrap_or(true),
            CombatTarget::World(id) => self
                .population_cache
                .get(&self.current_map)
                .and_then(|p| p.motion.get(&id))
                .map(|m| m.dead)
                .unwrap_or(true),
        }
    }

    fn set_combat_facing(&mut self, t: CombatTarget, dir: u8) {
        match t {
            CombatTarget::Player => self.player_dir = (dir & 7) as i32,
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.motion.facing = dir & 7;
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(m) = pop.motion.get_mut(&id) {
                        m.facing = dir & 7;
                    }
                }
            }
        }
    }

    fn set_combat_hp(&mut self, t: CombatTarget, hp: u16) {
        match t {
            CombatTarget::Player => self.player.hp_internal = hp,
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.stats.hp_internal = hp;
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.stats.hp_internal = hp;
                    }
                }
            }
        }
    }

    fn add_combat_poison(&mut self, t: CombatTarget, amount: u16) {
        if amount == 0 {
            return;
        }
        match t {
            CombatTarget::Player => {
                self.player.periodic_damage = self.player.periodic_damage.saturating_add(amount)
            }
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.stats.periodic_damage =
                        m.entity.stats.periodic_damage.saturating_add(amount);
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.stats.periodic_damage =
                            a.entity.stats.periodic_damage.saturating_add(amount);
                    }
                }
            }
        }
    }

    fn combatant_equipment(&self, t: CombatTarget) -> Option<[u16; 6]> {
        match t {
            CombatTarget::Player => Some(self.player.equipment),
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.equipment),
            CombatTarget::World(id) => self.combatant_entity(id).map(|e| e.equipment),
        }
    }

    fn clear_equipment_slot(&mut self, t: CombatTarget, slot: usize) {
        if slot >= 6 {
            return;
        }
        match t {
            CombatTarget::Player => self.player.equipment[slot] = 0,
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.equipment[slot] = 0;
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.equipment[slot] = 0;
                    }
                }
            }
        }
    }

    fn set_combat_message(&mut self, message: impl Into<String>) {
        self.combat_message = Some(message.into());
        self.combat_message_t = 2.5;
    }

    /// Apply the `FUN_0041c3cc` armor durability writes for one landed hit.
    fn apply_armor_wear(&mut self, t: CombatTarget, incoming: i32, armor: i32) {
        let Some(ids) = self.combatant_equipment(t) else {
            return;
        };
        for slot in 3..6 {
            let id = ids[slot];
            if id == 0 {
                continue;
            }
            let rating = self
                .world
                .res
                .item_info(id)
                .map(|i| i.rating)
                .unwrap_or(0);
            if rating == 0 {
                continue;
            }
            let loss = combat::armor_wear(incoming, armor, rating);
            if self.world.res.damage_item_durability(id, loss) {
                self.clear_equipment_slot(t, slot);
                self.set_combat_message("Снаряжение сломано");
            }
        }
    }

    /// Apply the `FUN_0041c3cc` attacker weapon durability/charge writes.
    fn apply_weapon_wear(&mut self, t: CombatTarget, incoming: i32) {
        let Some(ids) = self.combatant_equipment(t) else {
            return;
        };
        let Some(stats) = self.combatant_stats(t) else {
            return;
        };
        let Some(slot) = stats.weapon_slot_index() else {
            return;
        };
        let id = ids[slot];
        if id == 0 {
            return;
        }
        self.world.res.decrement_item_charge(id);
        let loss = combat::weapon_wear(incoming);
        if self.world.res.damage_item_durability(id, loss) {
            self.clear_equipment_slot(t, slot);
            self.set_combat_message("Оружие сломано");
        }
    }

    /// Resolve one melee attack and apply HP, poison, equipment wear and death.
    pub(super) fn resolve_attack(
        &mut self,
        attacker: CombatTarget,
        defender: CombatTarget,
    ) -> Option<MeleeRoll> {
        if self.combatant_dead(attacker) || self.combatant_dead(defender) {
            return None;
        }
        self.combat_attacks = self.combat_attacks.saturating_add(1);
        let a = self.combatant_stats(attacker)?;
        let d = self.combatant_stats(defender)?;
        let af = self.combatant_facing(attacker);
        let df = self.combatant_facing(defender);
        let (roll, armor) = {
            let res = &self.world.res;
            let db = res.item_db.as_ref()?;
            let roll = combat::resolve_melee(
                db,
                &res.save_game,
                &a,
                af,
                &d,
                df,
                &mut self.combat_rng,
            );
            let armor = combat::armor_total(db, &res.save_game, &d);
            (roll, armor)
        };
        if !roll.hit {
            if roll.dodged {
                self.set_combat_message("Уклонение");
            }
            return Some(roll);
        }
        self.combat_hits = self.combat_hits.saturating_add(1);
        let hp_before = d.hp;
        if roll.lethal {
            self.kill_combatant(defender, self.combatant_level(attacker), 0);
        } else {
            self.set_combat_hp(defender, hp_before.saturating_sub(roll.damage));
            self.add_combat_poison(defender, roll.poison);
        }
        self.apply_armor_wear(defender, roll.incoming as i32, armor);
        self.apply_weapon_wear(attacker, roll.incoming as i32);
        Some(roll)
    }

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
                let cell = self.combatant_cell(CombatTarget::World(id)).unwrap_or((0, 0));
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
            }
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
        }
    }

    /// Nearest free cell beside `target` for a melee approach.
    pub(super) fn melee_goal(
        from: (i32, i32),
        target: (i32, i32),
        collision: &[u16],
        occupied: &HashSet<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        let mut best: Option<((i32, i32), i32)> = None;
        for (_, nx, ny) in neighbors(target.0, target.1) {
            let c = (nx, ny);
            if c == from {
                return Some(c);
            }
            if !crate::actors::is_free(collision, c.0, c.1) || occupied.contains(&c) {
                continue;
            }
            let d = crate::actors::follow_distance(from, c);
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((c, d));
            }
        }
        best.map(|(c, _)| c)
    }

    fn is_adjacent(a: (i32, i32), b: (i32, i32)) -> bool {
        a != b && neighbors(a.0, a.1).iter().any(|&(_, x, y)| (x, y) == b)
    }

    /// Right-click attack order (`FUN_0042260C`): only selected units already in
    /// battle stance receive it. Returns false when nothing was ordered.
    pub(super) fn issue_attack_order(&mut self, entity_id: usize) -> bool {
        if self.player.dead {
            return false;
        }
        let mut ordered = false;
        if self.party_selected.get(0).copied().unwrap_or(false)
            && crate::anim::battle_from_flags(self.player.flags_b)
        {
            self.player_attack_target = Some(CombatTarget::World(entity_id));
            self.player_attack_frame = 0;
            self.player_attack_frames = 0;
            self.target = None;
            self.player_path.clear();
            self.player_active = false;
            self.player_render_x = self.player_x;
            self.player_render_y = self.player_y;
            ordered = true;
        }
        for (i, m) in self.party.iter_mut().enumerate() {
            if self.party_selected.get(i + 1).copied().unwrap_or(false)
                && !m.motion.dead
                && crate::anim::battle_from_flags(m.entity.flags_b)
            {
                m.motion.combat_target = Some(CombatTarget::World(entity_id));
                m.motion.attack_frames = 0;
                m.motion.frame = 0;
                m.motion.path.clear();
                m.motion.target = None;
                m.motion.walking = false;
                m.motion.render_cell = m.motion.cell;
                ordered = true;
            }
        }
        if ordered {
            self.engage_world_group(entity_id, CombatTarget::Player);
        }
        ordered
    }

    /// Wake the target actor's whole object group (original group-activity gate).
    fn engage_world_group(&mut self, entity_id: usize, target: CombatTarget) {
        let loc = self.current_map;
        let Some(pop) = self.population_cache.get_mut(&loc) else {
            return;
        };
        let Some((group, object_family)) = pop
            .actors
            .iter()
            .find(|a| a.entity.index == entity_id)
            .map(|a| (a.source_group_id, a.entity.archetype & 0x40 != 0))
        else {
            return;
        };
        if !object_family {
            if let Some(m) = pop.motion.get_mut(&entity_id) {
                if !m.dead {
                    m.combat_target = Some(target);
                }
            }
            return;
        }
        for a in pop.actors.iter() {
            if a.source_group_id != group || a.entity.archetype & 0x40 == 0 {
                continue;
            }
            if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                if !m.dead {
                    m.combat_target = Some(target);
                }
            }
        }
    }

    fn advance_motion_walk(
        m: &mut ActorMotion,
        collision: &[u16],
        occupied: &mut HashSet<(i32, i32)>,
    ) -> bool {
        if !m.walking {
            return false;
        }
        let wf = if m.walk_frames == 0 {
            crate::actors::FALLBACK_WALK_FRAMES
        } else {
            m.walk_frames
        };
        m.frame = m.frame.saturating_add(1);
        if m.frame >= wf {
            m.frame = 0;
            if !m.path.is_empty() {
                crate::actors::begin_actor_segment(m, occupied, collision);
            } else {
                m.walking = false;
                m.render_cell = m.cell;
                m.target = None;
            }
        }
        m.walking
    }

    fn start_motion_walk(
        m: &mut ActorMotion,
        collision: &[u16],
        occupied: &mut HashSet<(i32, i32)>,
        goal: (i32, i32),
    ) {
        m.target = Some(goal);
        let bounds = (0, 0, crate::coords::RUNTIME_W - 1, crate::coords::RUNTIME_H - 1);
        match crate::actors::find_path(collision, occupied, m.cell, goal, bounds) {
            Some(p) if !p.is_empty() => {
                m.path = p;
                crate::actors::begin_actor_segment(m, occupied, collision);
            }
            _ => {
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
            }
        }
    }

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

    fn update_player_combat(&mut self) {
        if self.player.dead {
            return;
        }
        let Some(target) = self.player_attack_target else {
            self.player_attack_frame = 0;
            return;
        };
        if self.combatant_dead(target) {
            self.player_attack_target = None;
            self.player_attack_frame = 0;
            return;
        }
        let Some(tc) = self.combatant_cell(target) else {
            self.player_attack_target = None;
            return;
        };
        let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
        if Self::is_adjacent(pc, tc) {
            self.player_path.clear();
            self.player_active = false;
            self.player_render_x = self.player_x;
            self.player_render_y = self.player_y;
            let dir = direction_from_cells(tc.0, tc.1, pc.0, pc.1);
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

    fn update_party_combat(&mut self) {
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
            };
            let Some(tc) = target_cell else {
                continue;
            };
            let target_dead = match target {
                CombatTarget::Player => player_dead,
                CombatTarget::Party(j) => party_dead.get(j).copied().unwrap_or(true),
                CombatTarget::World(id) => !world_cells.contains_key(&id),
            };
            if target_dead {
                m.motion.combat_target = None;
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
        for (i, target) in attacks {
            self.resolve_attack(CombatTarget::Party(i), target);
        }
    }

    fn update_world_combat(&mut self) {
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
            };
            let Some(tc) = target_cell else {
                continue;
            };
            let target_dead = match target {
                CombatTarget::Player => player_dead,
                CombatTarget::Party(j) => party_dead.get(j).copied().unwrap_or(true),
                CombatTarget::World(_) => true,
            };
            if target_dead {
                m.combat_target = None;
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
        for (id, target) in attacks {
            self.resolve_attack(CombatTarget::World(id), target);
        }
    }

    fn apply_dot(&mut self) {
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

    fn advance_all_death_frames(&mut self) {
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

    fn combat_tick(&mut self) {
        self.combat_frame = self.combat_frame.wrapping_add(1);
        if self.combat_frame & 0xF == 0 {
            self.apply_dot();
        }
        self.advance_all_death_frames();
        self.update_player_combat();
        self.update_party_combat();
        self.update_world_combat();
    }

    /// Diagnostic seed override for the smoke test.
    pub fn combat_set_seed(&mut self, seed: u32) {
        self.combat_rng = combat::OriginalRng::new(seed);
    }

    /// Diagnostic setup: move one live object-family actor beside the
    /// protagonist, make it a weak poison carrier, put the leader in battle
    /// stance and issue the attack order. Returns the actor's source Entity id
    /// plus one, or 0.
    pub fn combat_test_setup(&mut self) -> u32 {
        self.ensure_population(self.current_map);
        let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
        let loc = self.current_map;
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let free: Vec<(i32, i32)> = neighbors(pc.0, pc.1)
            .iter()
            .map(|&(_, x, y)| (x, y))
            .filter(|&(x, y)| crate::actors::is_free(collision, x, y))
            .collect();
        let Some(cell) = free.first().copied() else {
            return 0;
        };
        let mut chosen = None;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for a in pop.actors.iter_mut() {
                if a.entity.archetype & 0x40 == 0 {
                    continue;
                }
                if pop.motion.get(&a.entity.index).is_some_and(|m| m.dead) {
                    continue;
                }
                a.entity.stats.hp_internal = 120;
                a.entity.stats.strength_current = 1;
                a.entity.stats.strength_base = 1;
                a.entity.stats.dexterity_current = 0;
                a.entity.stats.dexterity_base = 0;
                a.entity.stats.endurance_current = 1;
                a.entity.stats.endurance_base = 1;
                a.entity.stats.base_armor = 0;
                a.entity.stats.periodic_damage = 0;
                a.entity.stats.raw_0x52 = 2;
                a.entity.stats.money = 7;
                a.entity.equipment = [0; 6];
                a.entity.inventory = [0; 32];
                a.entity.inventory[0] = 3;
                if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                    m.cell = cell;
                    m.render_cell = cell;
                    m.path.clear();
                    m.target = None;
                    m.walking = false;
                    m.dead = false;
                    m.combat_target = None;
                    m.frame = 0;
                }
                chosen = Some(a.entity.index);
                break;
            }
        }
        let Some(id) = chosen else {
            return 0;
        };
        self.combat_test_target = Some(id);
        self.player.hp_internal = self.player.hp_internal.max(1);
        self.player.dead = false;
        self.player.flags_b |= crate::anim::FLAG_BATTLE;
        self.party_selected[0] = true;
        if self.player.equipment[2] == 0 {
            self.player.equipment[2] = 1;
        }
        if self.issue_attack_order(id) {
            (id as u32).saturating_add(1)
        } else {
            0
        }
    }

    /// Diagnostic: kill the protagonist to exercise the death row.
    pub fn combat_debug_kill_player(&mut self) -> u32 {
        self.kill_combatant(CombatTarget::Player, 1, 0);
        self.player.dead as u32
    }

    /// Diagnostic: force one player -> staged-target melee resolution and
    /// return the final damage (0 when no target/roll).
    pub fn combat_debug_force(&mut self) -> u32 {
        let Some(id) = self.combat_test_target else {
            return 0;
        };
        self.resolve_attack(CombatTarget::Player, CombatTarget::World(id))
            .map(|r| if r.hit { r.damage as u32 } else { 0 })
            .unwrap_or(0)
    }

    /// Diagnostic: teleport onto the newest corpse marker and run the pickup.
    pub fn combat_debug_collect(&mut self) -> u32 {
        let Some(cell) = self.active_world_objects.last().map(|o| o.cell()) else {
            return 0;
        };
        self.player_x = cell.0 as f64;
        self.player_y = cell.1 as f64;
        self.player_render_x = self.player_x;
        self.player_render_y = self.player_y;
        self.try_pickup_here();
        1
    }

    /// Diagnostic report consumed by `tools/combat_smoke.mjs`.
    pub fn combat_debug_report(&self) -> String {
        let target = self.combat_test_target;
        let (target_hp, target_dead, target_action, target_frame) = target
            .and_then(|id| {
                self.population_cache.get(&self.current_map).map(|p| {
                    (
                        p.actors
                            .iter()
                            .find(|a| a.entity.index == id)
                            .map(|a| a.entity.stats.hp_internal)
                            .unwrap_or(0),
                        p.motion.get(&id).map(|m| m.dead).unwrap_or(true),
                        p.motion.get(&id).map(|m| m.action).unwrap_or(0),
                        p.motion.get(&id).map(|m| m.frame).unwrap_or(0),
                    )
                })
            })
            .unwrap_or((0, true, 0, 0));
        let weapon = self.player.equipment[2];
        let durability = self
            .world
            .res
            .item_durability(weapon)
            .map(|(c, _)| c)
            .unwrap_or(0.0);
        let inventory = self.player.inventory.iter().filter(|&&id| id != 0).count();
        format!(
            "player_hp={} player_dead={} player_attack_frames={} player_attack_frame={} target_id={} target_hp={} target_dead={} target_action={} target_frame={} loot={} weapon={} weapon_durability={:.4} money={} inventory={} attacks={} hits={} player_reaction={}",
            self.player.hp_internal,
            self.player.dead as u8,
            self.player_attack_frames,
            self.player_attack_frame,
            target.map(|v| v as i64).unwrap_or(-1),
            target_hp,
            target_dead as u8,
            target_action,
            target_frame,
            self.active_world_objects.len(),
            weapon,
            durability,
            self.player.money,
            inventory,
            self.combat_attacks,
            self.combat_hits,
            combat::reaction(&CombatStats::from_player(&self.player))
        )
    }

    /// Fixed-rate combat update (60 ms logic ticks, capped at 10 per frame).
    pub(super) fn update_combat(&mut self, dt: f64) {
        self.combat_step_t += dt.max(0.0) * LOGIC_TICK_HZ;
        let mut ticks = self.combat_step_t.floor() as i64;
        self.combat_step_t -= ticks as f64;
        if ticks <= 0 {
            return;
        }
        ticks = ticks.min(10);
        if self.combat_message_t > 0.0 {
            self.combat_message_t -= ticks as f64 / LOGIC_TICK_HZ;
            if self.combat_message_t <= 0.0 {
                self.combat_message = None;
            }
        }
        for _ in 0..ticks {
            self.combat_tick();
        }
    }
}
