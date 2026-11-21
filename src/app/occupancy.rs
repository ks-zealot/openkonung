//! Runtime actor occupancy wiring.
//!
//! The pure marker layer lives in [`crate::actors::occupancy`]; this module
//! owns the per-location rebuild/mark helpers used by map entry, save load,
//! party transitions, death and debug placement.
//!
//! Original mapping (`KONUNG_SOURCE_OF_TRUTH.md` §8.1/§28.2):
//! `collision[cell] |= entity_index + 1` on entering, `&= 0xF000` on leaving.
//! The leader is entity index 0, so its marker is `1`.

use super::*;
use crate::combat::CombatTarget;

impl PlayableApp {
    /// One-based collision marker of a combat target (`Entity+0x06`).
    pub(super) fn combat_target_marker(&self, t: CombatTarget) -> Option<u16> {
        match t {
            CombatTarget::Player => {
                Some(crate::actors::occupancy::marker(self.leader_entity_index))
            }
            CombatTarget::Party(i) => self
                .party
                .get(i)
                .map(|m| crate::actors::occupancy::marker(m.entity.index)),
            CombatTarget::World(id) => Some(crate::actors::occupancy::marker(id)),
            CombatTarget::Prop(_) => None,
        }
    }
    /// Rebuild the current location's runtime collision table from the authored
    /// terrain plus every live, visible actor (population, party, leader).
    pub(super) fn rebuild_occupancy(&mut self, loc: u32) {
        let PlayableApp {
            world,
            population_cache,
            party,
            player,
            player_x,
            player_y,
            leader_entity_index,
            ..
        } = self;
        world.reset_collision(loc);
        let Some(collision) = world.collision_mut(loc) else {
            return;
        };
        if let Some(pop) = population_cache.get(&loc) {
            for a in &pop.actors {
                let Some(m) = pop.motion.get(&a.entity.index) else {
                    continue;
                };
                if crate::actors::should_mark(&a.entity, Some(m)) {
                    crate::actors::occupy(collision, m.cell, a.entity.index);
                }
            }
        }
        for m in party.iter() {
            if crate::actors::should_mark(&m.entity, Some(&m.motion)) {
                crate::actors::occupy(collision, m.motion.cell, m.entity.index);
            }
        }
        if !player.dead {
            let cell = (player_x.round() as i32, player_y.round() as i32);
            crate::actors::occupy(collision, cell, *leader_entity_index);
        }
    }

    /// Clear one cell's occupancy (death / removal).
    pub(super) fn clear_occupancy_cell(&mut self, loc: u32, cell: (i32, i32)) {
        if let Some(collision) = self.world.collision_mut(loc) {
            crate::actors::vacate(collision, cell);
        }
    }

    /// Recompute the `#[serde(skip)]` one-based target markers from the
    /// serialized `combat_target` handles after a save load.
    pub(super) fn derive_target_markers(&mut self) {
        let leader = crate::actors::occupancy::marker(self.leader_entity_index);
        let party_markers: Vec<u16> = self
            .party
            .iter()
            .map(|m| crate::actors::occupancy::marker(m.entity.index))
            .collect();
        let marker_of = |t: Option<CombatTarget>| -> u16 {
            match t {
                Some(CombatTarget::Player) => leader,
                Some(CombatTarget::Party(i)) => party_markers.get(i).copied().unwrap_or(0),
                Some(CombatTarget::World(id)) => crate::actors::occupancy::marker(id),
                Some(CombatTarget::Prop(_)) | None => 0,
            }
        };
        let loc = self.current_map;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for m in pop.motion.values_mut() {
                m.target_marker = marker_of(m.combat_target);
            }
        }
        for m in self.party.iter_mut() {
            m.motion.target_marker = marker_of(m.motion.combat_target);
        }
    }
}
