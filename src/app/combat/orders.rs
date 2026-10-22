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
use crate::constants::combat::*;

impl PlayableApp {
    /// Right-click attack order (`FUN_0042260C`): only selected units already in
    /// battle stance receive it. Returns false when nothing was ordered.
    pub(in crate::app) fn issue_attack_order(&mut self, entity_id: usize) -> bool {
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
    pub(in crate::app) fn issue_prop_order(&mut self, prop_index: usize) -> bool {
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
            .map(|a| {
                (
                    a.source_group_id,
                    a.entity.archetype & OBJECT_ARCHETYPE_BIT != 0,
                )
            })
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
            if a.source_group_id != group || a.entity.archetype & OBJECT_ARCHETYPE_BIT == 0 {
                continue;
            }
            if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                if !m.dead {
                    m.combat_target = Some(target);
                }
            }
        }
    }

    pub(super) fn wake_group(&mut self, group_id: usize, target: CombatTarget) {
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
}
