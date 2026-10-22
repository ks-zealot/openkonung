//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use super::super::*;
use crate::combat::{self, CombatStats, CombatTarget};

impl PlayableApp {
    pub(super) fn combatant_stats(&self, t: CombatTarget) -> Option<CombatStats> {
        match t {
            CombatTarget::Player => Some(CombatStats::from_player(&self.player)),
            CombatTarget::Party(i) => self
                .party
                .get(i)
                .map(|m| CombatStats::from_entity(&m.entity)),
            CombatTarget::World(id) => self
                .combatant_entity(id)
                .map(|e| CombatStats::from_entity(&e)),
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
            CombatTarget::Player => {
                Some((self.player_x.round() as i32, self.player_y.round() as i32))
            }
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

    pub(super) fn set_combat_facing(&mut self, t: CombatTarget, dir: u8) {
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

    pub(super) fn set_combat_hp(&mut self, t: CombatTarget, hp: u16) {
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

    pub(super) fn add_combat_poison(&mut self, t: CombatTarget, amount: u16) {
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

    pub(super) fn combatant_special(&self, t: CombatTarget) -> u16 {
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

    pub(super) fn clear_combatant_special(&mut self, t: CombatTarget) {
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

    pub(super) fn clear_equipment_slot(&mut self, t: CombatTarget, slot: usize) {
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

    pub(super) fn set_combat_message(&mut self, message: impl Into<String>) {
        self.combat_message = Some(message.into());
        self.combat_message_t = 2.5;
    }

    /// Apply the `FUN_0041c3cc` armor durability writes for one landed hit.
    pub(super) fn apply_armor_wear(&mut self, t: CombatTarget, incoming: i32, armor: i32) {
        let Some(ids) = self.combatant_equipment(t) else {
            return;
        };
        for slot in 3..6 {
            let id = ids[slot];
            if id == 0 {
                continue;
            }
            let rating = self.world.res.item_info(id).map(|i| i.rating).unwrap_or(0);
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
    pub(super) fn apply_weapon_wear(&mut self, t: CombatTarget, incoming: i32) {
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

    pub(super) fn combatant_render_cell(&self, t: CombatTarget) -> (f32, f32) {
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
}
