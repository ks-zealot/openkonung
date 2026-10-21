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
            CombatTarget::Prop(_) => None,
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

    pub(super) fn prop_cell(&self, index: usize) -> Option<(i32, i32)> {
        let loc = self.world.loc(self.current_map)?;
        loc.props
            .iter()
            .find(|p| p.ri == index)
            .map(|p| (p.grid_x, p.grid_y))
    }

    pub(super) fn combatant_level(&self, t: CombatTarget) -> u8 {
        match t {
            CombatTarget::Player => self.player.level,
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.stats.level).unwrap_or(1),
            CombatTarget::World(id) => self
                .combatant_entity(id)
                .map(|e| e.stats.level)
                .unwrap_or(1),
            CombatTarget::Prop(_) => 1,
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
            CombatTarget::Prop(index) => self.prop_cell(index),
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
            CombatTarget::Prop(_) => 0,
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
            CombatTarget::Prop(_) => false,
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
            CombatTarget::Prop(_) => {}
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
            CombatTarget::Prop(_) => {}
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
            CombatTarget::Prop(_) => {}
        }
    }

    fn combatant_equipment(&self, t: CombatTarget) -> Option<[u16; 6]> {
        match t {
            CombatTarget::Player => Some(self.player.equipment),
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.equipment),
            CombatTarget::World(id) => self.combatant_entity(id).map(|e| e.equipment),
            CombatTarget::Prop(_) => None,
        }
    }

    fn combatant_special(&self, t: CombatTarget) -> u16 {
        match t {
            CombatTarget::Player => self.player.special_equipment,
            CombatTarget::Party(i) => self
                .party
                .get(i)
                .map(|m| m.entity.special_equipment)
                .unwrap_or(0),
            CombatTarget::World(id) => self
                .combatant_entity(id)
                .map(|e| e.special_equipment)
                .unwrap_or(0),
            CombatTarget::Prop(_) => 0,
        }
    }

    fn clear_combatant_special(&mut self, t: CombatTarget) {
        match t {
            CombatTarget::Player => self.player.special_equipment = 0,
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.special_equipment = 0;
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.special_equipment = 0;
                    }
                }
            }
            CombatTarget::Prop(_) => {}
        }
    }

    /// True when the unit can fire: battle stance, bow in `Equipment[1]`, ammo
    /// in `+0xCC` with quantity > 0. `require_plain_ammo` mirrors the original
    /// prop-order gate (`ammo instance +0x02 == 0`).
    fn ranged_ready(&self, t: CombatTarget, require_plain_ammo: bool) -> bool {
        let Some(stats) = self.combatant_stats(t) else {
            return false;
        };
        if stats.weapon_slot != 1 || stats.equipment[1] == 0 {
            return false;
        }
        if !crate::anim::battle_from_flags(match t {
            CombatTarget::Player => self.player.flags_b,
            CombatTarget::Party(i) => self
                .party
                .get(i)
                .map(|m| m.entity.flags_b)
                .unwrap_or(0),
            CombatTarget::World(id) => self
                .combatant_entity(id)
                .map(|e| e.flags_b)
                .unwrap_or(0),
            CombatTarget::Prop(_) => 0,
        }) {
            return false;
        }
        let ammo = self.combatant_special(t);
        if ammo == 0 {
            return false;
        }
        let Some(inst) = self.world.res.item_instance(ammo) else {
            return false;
        };
        if inst.class != 12 || self.world.res.item_quantity(ammo).unwrap_or(0) == 0 {
            return false;
        }
        if require_plain_ammo && inst.modifier_index != 0 {
            return false;
        }
        self.world
            .res
            .item_info(stats.equipment[1])
            .map(|info| info.class == 1 && info.range > 0)
            .unwrap_or(false)
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
            CombatTarget::Prop(_) => {}
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
        let target = CombatTarget::World(entity_id);
        let mut ordered = false;
        if self.party_selected.get(0).copied().unwrap_or(false)
            && crate::anim::battle_from_flags(self.player.flags_b)
        {
            let ranged = self.ranged_ready(CombatTarget::Player, false);
            self.player_attack_target = Some(target);
            self.player_attack_frame = 0;
            self.player_attack_frames = 0;
            self.player_attack_ranged = ranged;
            self.player_ranged_frame = 0;
            self.player_ranged_frames = 0;
            self.player_ranged_fired = false;
            if ranged {
                self.player_path.clear();
                self.player_active = false;
                self.player_render_x = self.player_x;
                self.player_render_y = self.player_y;
            } else {
                self.target = None;
                self.player_path.clear();
                self.player_active = false;
                self.player_render_x = self.player_x;
                self.player_render_y = self.player_y;
            }
            ordered = true;
        }
        let party_ranged: Vec<bool> = (0..self.party.len())
            .map(|i| self.ranged_ready(CombatTarget::Party(i), false))
            .collect();
        for (i, m) in self.party.iter_mut().enumerate() {
            if self.party_selected.get(i + 1).copied().unwrap_or(false)
                && !m.motion.dead
                && crate::anim::battle_from_flags(m.entity.flags_b)
            {
                let ranged = party_ranged.get(i).copied().unwrap_or(false);
                m.motion.combat_target = Some(target);
                m.motion.attack_frames = 0;
                m.motion.frame = 0;
                m.motion.ranged_order = ranged;
                m.motion.ranged_fired = false;
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

    /// Prop/object order (`FUN_0042260C` branch 2b): only ranged-ready units
    /// whose ammo instance `+0x02 == 0` receive it. No move destination is
    /// written for actor/prop-target orders.
    pub(super) fn issue_prop_order(&mut self, prop_index: usize) -> bool {
        if self.player.dead {
            return false;
        }
        let target = CombatTarget::Prop(prop_index);
        let mut ordered = false;
        if self.party_selected.get(0).copied().unwrap_or(false)
            && self.ranged_ready(CombatTarget::Player, true)
        {
            self.player_attack_target = Some(target);
            self.player_attack_frame = 0;
            self.player_attack_ranged = true;
            self.player_ranged_frame = 0;
            self.player_ranged_frames = 0;
            self.player_ranged_fired = false;
            self.player_path.clear();
            self.player_active = false;
            self.player_render_x = self.player_x;
            self.player_render_y = self.player_y;
            ordered = true;
        }
        let party_ranged: Vec<bool> = (0..self.party.len())
            .map(|i| self.ranged_ready(CombatTarget::Party(i), true))
            .collect();
        for (i, m) in self.party.iter_mut().enumerate() {
            if self.party_selected.get(i + 1).copied().unwrap_or(false)
                && !m.motion.dead
                && party_ranged.get(i).copied().unwrap_or(false)
            {
                m.motion.combat_target = Some(target);
                m.motion.ranged_order = true;
                m.motion.ranged_fired = false;
                m.motion.attack_frames = 0;
                m.motion.frame = 0;
                m.motion.path.clear();
                m.motion.target = None;
                m.motion.walking = false;
                m.motion.render_cell = m.motion.cell;
                ordered = true;
            }
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

    fn combatant_render_cell(&self, t: CombatTarget) -> (f32, f32) {
        match t {
            CombatTarget::Player => (self.player_render_x as f32, self.player_render_y as f32),
            CombatTarget::Party(i) => self
                .party
                .get(i)
                .map(|m| (m.motion.render_cell.0 as f32, m.motion.render_cell.1 as f32))
                .unwrap_or((0.0, 0.0)),
            CombatTarget::World(id) => self
                .population_cache
                .get(&self.current_map)
                .and_then(|p| p.motion.get(&id))
                .map(|m| (m.render_cell.0 as f32, m.render_cell.1 as f32))
                .unwrap_or((0.0, 0.0)),
            CombatTarget::Prop(_) => (0.0, 0.0),
        }
    }

    /// Spawn one arrow (`FUN_0041BF44`) and consume one ammo charge plus 0.1 bow
    /// durability. Returns false when the pool/bow/ammo is unavailable.
    fn spawn_projectile(
        &mut self,
        shooter: CombatTarget,
        target_cell: (i32, i32),
        prop_target: Option<usize>,
    ) -> bool {
        if self.projectiles.len() >= crate::combat::projectile::POOL_CAP {
            return false;
        }
        let Some(stats) = self.combatant_stats(shooter) else {
            return false;
        };
        let Some(bow_id) = stats.equipment.get(1).copied().filter(|&id| id != 0) else {
            return false;
        };
        let Some(bow_info) = self.world.res.item_info(bow_id) else {
            return false;
        };
        let ammo = self.combatant_special(shooter);
        if ammo == 0 || self.world.res.item_quantity(ammo).unwrap_or(0) == 0 {
            return false;
        }
        let (bow, life, start, facing) = {
            let res = &self.world.res;
            let Some(db) = res.item_db.as_ref() else {
                return false;
            };
            let Some(bow) = crate::combat::bow_shot(db, &res.save_game, &stats) else {
                return false;
            };
            let start_cell = self.combatant_render_cell(shooter);
            let (lx, ly) = entity_to_local_px_f(start_cell.0 as f64, start_cell.1 as f64);
            let (mx, my) = crate::combat::projectile::muzzle_offset(
                (self.combatant_facing(shooter)) & 7,
            );
            (
                bow,
                bow_info.projectile_lifetime,
                (lx as f32 + mx, ly as f32 + my),
                self.combatant_facing(shooter) & 7,
            )
        };
        let (tx, ty) = entity_to_local_px(target_cell.0, target_cell.1);
        let target = (tx as f32, ty as f32 - crate::combat::projectile::AIM_Y_OFFSET);
        let projectile = crate::combat::projectile::spawn(
            shooter,
            start,
            target,
            facing,
            bow,
            life,
            prop_target,
        );
        self.projectiles.push(projectile);
        // Ammo is consumed only after a successful spawn (action row 4).
        let remaining = self.world.res.decrement_item_quantity(ammo).unwrap_or(0);
        if remaining == 0 {
            self.clear_combatant_special(shooter);
            self.set_combat_message("Стрелы закончились");
        }
        // `FUN_0041BF44`: bow current durability -= 0.1; break clears Equipment[1].
        if self.world.res.damage_item_durability(bow_id, 0.1) {
            self.clear_equipment_slot(shooter, 1);
            self.set_combat_message("Лук сломан");
        }
        true
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

    fn resolve_projectile_hit(
        &mut self,
        shooter: CombatTarget,
        defender: CombatTarget,
        bow: crate::combat::BowShot,
    ) {
        if self.combatant_dead(defender) {
            return;
        }
        let Some(dstats) = self.combatant_stats(defender) else {
            return;
        };
        let (roll, armor) = {
            let res = &self.world.res;
            let Some(db) = res.item_db.as_ref() else {
                return;
            };
            let roll = crate::combat::resolve_projectile(
                db,
                &res.save_game,
                &bow,
                &dstats,
                &mut self.combat_rng,
            );
            let armor = crate::combat::armor_total(db, &res.save_game, &dstats);
            (roll, armor)
        };
        if !roll.hit {
            return;
        }
        if roll.lethal {
            let power = {
                let res = &self.world.res;
                res.item_db
                    .as_ref()
                    .map(|db| crate::combat::combat_power(db, &res.save_game, &dstats))
                    .unwrap_or(0)
            };
            let killer_level = self.combatant_level(shooter);
            self.kill_combatant(defender, killer_level, power);
            self.award_ranged_kill(shooter, power);
        } else {
            self.set_combat_hp(defender, dstats.hp.saturating_sub(roll.damage));
            self.add_combat_poison(defender, roll.poison);
        }
        self.apply_armor_wear(defender, roll.incoming as i32, armor);
    }

    fn award_ranged_kill(&mut self, owner: CombatTarget, power: i32) {
        if power <= 0 {
            return;
        }
        let money = power / 4;
        let skill = power.clamp(0, u16::MAX as i32) as u16;
        match owner {
            CombatTarget::Player => {
                self.player.money = self.player.money.saturating_add(money);
                self.player.archery_mastery = self.player.archery_mastery.saturating_add(skill);
            }
            CombatTarget::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.stats.money = m.entity.stats.money.saturating_add(money);
                    m.entity.stats.archery_mastery =
                        m.entity.stats.archery_mastery.saturating_add(skill);
                }
            }
            CombatTarget::World(id) => {
                if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                    if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                        a.entity.stats.money = a.entity.stats.money.saturating_add(money);
                        a.entity.stats.archery_mastery =
                            a.entity.stats.archery_mastery.saturating_add(skill);
                    }
                }
            }
            CombatTarget::Prop(_) => {}
        }
    }

    fn wake_group(&mut self, group_id: usize, target: CombatTarget) {
        if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
            for a in pop.actors.iter() {
                if a.source_group_id != group_id {
                    continue;
                }
                if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                    if !m.dead {
                        m.combat_target = Some(target);
                        m.path.clear();
                        m.target = None;
                        m.walking = false;
                        m.render_cell = m.cell;
                    }
                }
            }
        }
    }

    fn handle_prop_hit(&mut self, prop_index: usize) {
        let loc = self.current_map;
        let Some(sidx) = self.settlements.iter().position(|s| s.loc_id as u32 == loc) else {
            return;
        };
        let roll = self.combat_rng.roll();
        let mut changed = false;
        let mut wake = None;
        {
            let s = &mut self.settlements[sidx];
            for t in s.targets.iter_mut() {
                if t.prop_index as usize == prop_index {
                    if roll < 21 {
                        t.state = 3;
                        t.timer = 1;
                    }
                    changed = true;
                    break;
                }
            }
            if changed {
                s.flags_4b2 |= 0x15;
                wake = Some(s.group_id as usize);
            }
        }
        if let Some(g) = wake {
            self.wake_group(g, CombatTarget::Player);
        }
    }

    fn update_projectiles(&mut self) {
        if self.projectiles.is_empty() {
            return;
        }
        let loc = self.current_map;
        let player_cell = (self.player_x.round() as i32, self.player_y.round() as i32);
        let mut occupants: Vec<(CombatTarget, (i32, i32), bool)> = Vec::new();
        occupants.push((
            CombatTarget::Player,
            player_cell,
            self.player.dead || self.player.hp_internal == 0,
        ));
        for (i, m) in self.party.iter().enumerate() {
            occupants.push((CombatTarget::Party(i), m.motion.cell, m.motion.dead));
        }
        if let Some(pop) = self.population_cache.get(&loc) {
            for a in &pop.actors {
                if let Some(m) = pop.motion.get(&a.entity.index) {
                    occupants.push((CombatTarget::World(a.entity.index), m.cell, m.dead));
                }
            }
        }
        let mut i = 0;
        while i < self.projectiles.len() {
            let mut remove = false;
            {
                let p = &mut self.projectiles[i];
                p.life = p.life.saturating_sub(1);
                if p.life == 0 {
                    remove = true;
                }
            }
            if remove {
                self.projectiles.remove(i);
                continue;
            }
            for _ in 0..crate::combat::projectile::SUBSTEPS {
                let (px, py, vx, vy, owner, prop_target, bow) = {
                    let p = &self.projectiles[i];
                    (p.x, p.y, p.vx, p.vy, p.owner, p.prop_target, p.bow)
                };
                let cx = px.round() as i32;
                let cy = py.round() as i32;
                let cell = crate::coords::local_px_to_entity(cx, cy);
                let blocked = {
                    let collision = self
                        .world
                        .loc(loc)
                        .map(|l| l.collision.as_slice())
                        .unwrap_or(&[]);
                    let idx = cell.0 * 160 + cell.1;
                    collision
                        .get(idx as usize)
                        .map(|&v| (v >> 8) & 0xC0 != 0)
                        .unwrap_or(true)
                };
                if blocked {
                    if let Some(prop) = prop_target {
                        if self.world.prop_opaque_at(loc, prop, cx, cy) {
                            self.handle_prop_hit(prop);
                        }
                    }
                    remove = true;
                    break;
                }
                let hit = occupants.iter().find(|(t, c, dead)| {
                    *c == cell && *t != owner && !*dead
                });
                if let Some((target, _, _)) = hit.copied() {
                    self.resolve_projectile_hit(owner, target, bow);
                    remove = true;
                    break;
                }
                if let Some(p) = self.projectiles.get_mut(i) {
                    p.x += vx;
                    p.y += vy;
                }
            }
            if remove {
                self.projectiles.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn combat_tick(&mut self) {
        self.update_projectiles();
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

    /// Diagnostic staging for the ranged smoke: equip a short bow + 30 arrows,
    /// put one weak object actor several cells east and order the shot.
    pub fn ranged_test_setup(&mut self) -> u32 {
        self.ensure_population(self.current_map);
        let pc = (self.player_x.round() as i32, self.player_y.round() as i32);
        let loc = self.current_map;
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let mut cell = None;
        for dx in 4..10 {
            let c = (pc.0 + dx, pc.1);
            if crate::actors::is_free(collision, c.0, c.1) {
                cell = Some(c);
                break;
            }
        }
        let Some(cell) = cell else {
            return 0;
        };
        let mut chosen = None;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for a in pop.actors.iter_mut() {
                if a.entity.archetype & 0x40 == 0 {
                    continue;
                }
                a.entity.stats.hp_internal = 3000;
                a.entity.stats.strength_base = 100;
                a.entity.stats.strength_current = 100;
                a.entity.stats.dexterity_base = 0;
                a.entity.stats.dexterity_current = 0;
                a.entity.stats.endurance_base = 10;
                a.entity.stats.endurance_current = 10;
                a.entity.stats.base_armor = 0;
                a.entity.stats.periodic_damage = 0;
                a.entity.stats.raw_0x52 = 0;
                a.entity.equipment = [0; 6];
                a.entity.inventory = [0; 32];
                if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                    m.cell = cell;
                    m.render_cell = cell;
                    m.dead = false;
                    m.combat_target = None;
                    m.path.clear();
                    m.walking = false;
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
        self.combat_test_prop = None;
        self.player.dead = false;
        self.player.hp_internal = self.player.hp_internal.max(1);
        self.player.equipment[1] = 23;
        self.player.raw_0x12 = 1;
        self.player.special_equipment = 27;
        self.world.res.set_item_quantity(27, 30);
        self.world.res.set_item_durability(23, 56.0);
        self.player.flags_b |= crate::anim::FLAG_BATTLE;
        self.party_selected[0] = true;
        if self.issue_attack_order(id) {
            (id as u32).saturating_add(1)
        } else {
            0
        }
    }

    /// Diagnostic: give the staged target the shipped 30-arrow stack (item 102).
    pub fn ranged_target_arrows(&mut self) -> u32 {
        let Some(id) = self.combat_test_target else {
            return 0;
        };
        if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
            if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                a.entity.inventory = [0; 32];
                a.entity.inventory[0] = 102;
                return 1;
            }
        }
        0
    }

    /// Diagnostic: make the staged target dodge every projectile.
    pub fn ranged_make_dodge(&mut self) -> u32 {
        let Some(id) = self.combat_test_target else {
            return 0;
        };
        if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
            if let Some(a) = pop.actors.iter_mut().find(|a| a.entity.index == id) {
                a.entity.stats.dexterity_base = 2000;
                a.entity.stats.dexterity_current = 2000;
                return 1;
            }
        }
        0
    }

    /// Diagnostic staging for the prop branch: use blocked prop `prop_index`
    /// (defaults to loc-4 prop 33, which is also a settlement target), give the
    /// player plain ammo (`+0x02 == 0`) and issue the prop order.
    pub fn prop_test_setup(&mut self, prop_index: usize) -> u32 {
        self.ensure_population(self.current_map);
        let loc = self.current_map;
        let Some((gx, gy)) = self.prop_cell(prop_index) else {
            return 0;
        };
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let mut player_cell = None;
        'search: for radius in 1..10i32 {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let c = (gx + dx, gy + dy);
                    if crate::actors::is_free(collision, c.0, c.1) {
                        player_cell = Some(c);
                        break 'search;
                    }
                }
            }
        }
        let Some(pc) = player_cell else {
            return 0;
        };
        self.player_x = pc.0 as f64;
        self.player_y = pc.1 as f64;
        self.player_render_x = pc.0 as f64;
        self.player_render_y = pc.1 as f64;
        self.player.equipment[1] = 23;
        self.player.raw_0x12 = 1;
        self.player.special_equipment = 27;
        self.world.res.set_item_quantity(27, 30);
        self.world.res.set_item_modifier(27, 0);
        self.world.res.set_item_durability(23, 56.0);
        self.player.flags_b |= crate::anim::FLAG_BATTLE;
        self.party_selected[0] = true;
        self.combat_test_prop = Some(prop_index);
        if self.issue_prop_order(prop_index) {
            (prop_index as u32).saturating_add(1)
        } else {
            0
        }
    }

    /// Diagnostic ranged/prop report.
    pub fn ranged_debug_report(&self) -> String {
        let ammo = self.player.special_equipment;
        let qty = self.world.res.item_quantity(ammo).unwrap_or(0);
        let bow = self.player.equipment[1];
        let bow_dur = self
            .world
            .res
            .item_durability(bow)
            .map(|(c, _)| c)
            .unwrap_or(0.0);
        let (prop_index, prop_state, prop_flags) = self
            .combat_test_prop
            .map(|idx| {
                let (state, flags) = self
                    .settlements
                    .iter()
                    .find(|s| s.loc_id as u32 == self.current_map)
                    .map(|s| {
                        (
                            s.targets
                                .iter()
                                .find(|t| t.prop_index as usize == idx)
                                .map(|t| t.state)
                                .unwrap_or(0),
                            s.flags_4b2,
                        )
                    })
                    .unwrap_or((0, 0));
                (idx as i64, state, flags)
            })
            .unwrap_or((-1, 0, 0));
        let target = self.combat_test_target;
        let target_hp = target
            .and_then(|id| {
                self.population_cache
                    .get(&self.current_map)
                    .and_then(|p| p.actors.iter().find(|a| a.entity.index == id))
                    .map(|a| a.entity.stats.hp_internal)
            })
            .unwrap_or(0);
        let target_dead = target
            .and_then(|id| {
                self.population_cache
                    .get(&self.current_map)
                    .and_then(|p| p.motion.get(&id))
                    .map(|m| m.dead as u8)
            })
            .unwrap_or(0);
        let inv_arrow_qty: u32 = self
            .player
            .inventory
            .iter()
            .filter(|&&id| id != 0)
            .filter(|&&id| {
                self.world
                    .res
                    .item_instance(id)
                    .map(|i| i.class == 12)
                    .unwrap_or(false)
            })
            .map(|&id| self.world.res.item_quantity(id).unwrap_or(0))
            .sum();
        let (p0x, p0y) = self
            .projectiles
            .first()
            .map(|p| (p.x, p.y))
            .unwrap_or((0.0, 0.0));
        format!(
            "projectiles={} proj0_x={:.2} proj0_y={:.2} ammo={} ammo_qty={} bow={} bow_durability={:.4} player_money={} player_archery={} inv_arrow_qty={} ranged_order={} ranged_frame={} ranged_frames={} target_id={} target_hp={} target_dead={} prop_index={} prop_state={} prop_flags={}",
            self.projectiles.len(),
            p0x,
            p0y,
            ammo,
            qty,
            bow,
            bow_dur,
            self.player.money,
            self.player.archery_mastery,
            inv_arrow_qty,
            self.player_attack_ranged as u8,
            self.player_ranged_frame,
            self.player_ranged_frames,
            target.map(|v| v as i64).unwrap_or(-1),
            target_hp,
            target_dead,
            prop_index,
            prop_state,
            prop_flags
        )
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
