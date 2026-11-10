//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).
//!
//! This file holds the [`CombatTarget`] accessor layer: one uniform read/write
//! for the protagonist, a party slot, a world actor or a prop, so the rules and
//! the unit passes never have to know where the live state is stored.

use super::super::*;
use crate::combat::{self, CombatStats, CombatTarget};

impl PlayableApp {
    /// Snapshot the combat-rule inputs for one [`CombatTarget`].
    ///
    /// The rules in [`crate::combat`] work on plain values, so each live
    /// combatant is converted through [`CombatStats::from_player`] (protagonist)
    /// or [`CombatStats::from_entity`] (party slot / world actor). Props are
    /// static scenery and have no stats.
    ///
    /// # Returns
    ///
    /// `None` for props and for a world id that is not in the current map's
    /// population cache.
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

    /// Clone the authored `Game.N` entity record of a world actor.
    ///
    /// The population cache holds one canonical `Entity` per placed actor; the
    /// combat passes read it through this accessor so a modifier never has to
    /// borrow the whole population build.
    ///
    /// # Returns
    ///
    /// `None` when `id` is not part of the current map's built population.
    pub(super) fn combatant_entity(&self, id: usize) -> Option<crate::population::Entity> {
        self.population_cache
            .get(&self.current_map)?
            .actors
            .iter()
            .find(|a| a.entity.index == id)
            .map(|a| a.entity.clone())
    }

    /// Runtime grid cell of the prop whose source index is `index`.
    ///
    /// A `Konung.N` prop record keeps its own id, which is what the entity and
    /// trigger tables reference; the cell comes from the prop placement and is a
    /// 320x160 runtime cell, never a pixel projection.
    pub(super) fn prop_cell(&self, index: usize) -> Option<(i32, i32)> {
        let loc = self.world.loc(self.current_map)?;
        loc.props
            .iter()
            .find(|p| p.ri == index)
            .map(|p| (p.grid_x, p.grid_y))
    }

    /// Authored level of a combatant, used as the ranged kill's `killer_level`
    /// (which drives the corpse-loot item roll).
    ///
    /// A missing party slot or world actor reports level 1, as do props.
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

    /// Current runtime/Entity cell of a combatant.
    ///
    /// The protagonist uses the committed cell (`player_x/player_y`), party
    /// members and world actors use their live `ActorMotion::cell`, and a prop
    /// uses its `Konung.N` placement. This is the 320x160 runtime grid
    /// (`Entity+0x08/+0x0A`), never the screen projection.
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

    /// Facing direction `0..=7` of a combatant (`Entity+0x01`).
    ///
    /// Drives the death action row and the arrow muzzle offset; a combatant
    /// without live motion reports `0`.
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

    /// True when a combatant is in a death state.
    ///
    /// The protagonist mirrors `dead`/`hp_internal == 0`, party and world actors
    /// use `ActorMotion::dead`. A world id with no motion entry counts as dead so
    /// callers never act on a phantom actor. Props are never dead.
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

    /// Write a combatant's facing direction, masked to the three-bit range.
    ///
    /// The original turns a unit toward its target with `dir & 7`
    /// (`FUN_0043897C`); an out-of-range value must not leak into the animation
    /// row lookup.
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

    /// Store a combatant's raw internal health.
    ///
    /// `hp_internal` is the 16x-scaled value (the interface bar shows
    /// `(hp_internal + 15) / 16`). Saturation and the lethal branch
    /// ([`Self::kill_combatant`]) are the caller's responsibility.
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

    /// Add poison damage to a combatant's periodic-damage accumulator.
    ///
    /// `periodic_damage` (`Entity+0x50` in the original record) is subtracted
    /// every 16 logic frames by [`Self::apply_dot`]. A zero amount is a no-op so
    /// an unpoisoned hit cannot touch the field; the add saturates.
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

    /// Six equipped item-instance ids (`Entity+0x66`, slots 0..=5).
    ///
    /// Slot 0 and 2 hold the two melee families, 1 the bow and 3..=5 the armor;
    /// an id of 0 means the slot is empty. Props have no equipment.
    fn combatant_equipment(&self, t: CombatTarget) -> Option<[u16; 6]> {
        match t {
            CombatTarget::Player => Some(self.player.equipment),
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.equipment),
            CombatTarget::World(id) => self.combatant_entity(id).map(|e| e.equipment),
            CombatTarget::Prop(_) => None,
        }
    }

    /// Instance id in the special slot (`Entity+0xCC`), the ammo cell.
    ///
    /// Ranged combat reads it as the loaded ammo stack (class 12); 0 means
    /// nothing is loaded.
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

    /// Empty the special slot after the last ammo unit was fired.
    ///
    /// The original zeroes `Entity+0xCC` when the decremented quantity reaches
    /// zero, which also disarms the ranged gate in [`Self::ranged_ready`].
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

    /// Empty one equipment slot (0..=5), e.g. when a weapon or armor breaks.
    ///
    /// Out-of-range slot indices are ignored rather than panicking.
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

    /// Queue a transient combat message for the world overlay.
    ///
    /// The string lives for 2.5 logic-rate seconds (`combat_message_t`); the
    /// countdown runs in [`Self::update_combat`]. A new message replaces the
    /// pending one.
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

    /// Pre-step render cell used for the sprite anchor.
    ///
    /// A walk segment commits its step at the START of the cycle, so the stored
    /// cell leads the drawn sprite by one step; the render cell is the cell the
    /// sprite is still leaving. Arrows also start from it. Props report
    /// `(0.0, 0.0)`.
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
