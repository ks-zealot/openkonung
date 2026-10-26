//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use super::super::*;
use crate::actors::PopulationBuild;
use crate::combat::CombatTarget;

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
        let group = self
            .population_cache
            .get(&self.current_map)
            .and_then(|pop| {
                pop.actors
                    .iter()
                    .find(|a| a.entity.index == entity_id)
                    .map(|a| a.source_group_id)
            });
        if let Some(group) = group {
            self.wake_group(group, target);
        }
    }

    fn combat_target_group(&self, target: CombatTarget) -> u8 {
        match target {
            CombatTarget::World(id) => self
                .population_cache
                .get(&self.current_map)
                .and_then(|p| p.actors.iter().find(|a| a.entity.index == id))
                .map(|a| a.entity.group_id)
                .unwrap_or(0),
            // Party/leader group is the reserved group 0.
            CombatTarget::Player | CombatTarget::Party(_) | CombatTarget::Prop(_) => 0,
        }
    }

    /// `FUN_004245B4`: set activity `1`, clear the route terminator for members
    /// where `flags_a & 1 == 0` and `archetype != 0x49`. The target is only
    /// recorded as the group's target group; acquisition happens on the next AI
    /// tick. This function does not assign per-actor combat targets.
    pub(in crate::app) fn wake_group(&mut self, group_id: usize, target: CombatTarget) {
        let target_group = self.combat_target_group(target);
        if let Some(tables) = self.tables.as_mut() {
            if let Some(g) = tables.groups.get_mut(group_id) {
                g.target_group = target_group;
                g.activity_1d = 1;
            }
        }
        if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
            let PopulationBuild { actors, motion, .. } = pop;
            for a in actors.iter_mut() {
                if a.source_group_id != group_id {
                    continue;
                }
                if a.entity.flags_a & 1 != 0 || a.entity.archetype == 0x49 {
                    continue;
                }
                if let Some(m) = motion.get_mut(&a.entity.index) {
                    if !m.dead {
                        m.path.clear();
                        m.target = None;
                        m.walking = false;
                        m.render_cell = m.cell;
                    }
                }
            }
        }
    }

    /// Wake the group that owns `defender` after a landed/touching hit
    /// (`FUN_00421B50` case 5 / `FUN_00420F30` -> `FUN_004245B4`).
    pub(super) fn wake_combatant_group(&mut self, defender: CombatTarget, attacker: CombatTarget) {
        let CombatTarget::World(id) = defender else {
            return;
        };
        let group = self
            .population_cache
            .get(&self.current_map)
            .and_then(|p| {
                p.actors
                    .iter()
                    .find(|a| a.entity.index == id)
                    .map(|a| a.source_group_id)
            });
        if let Some(group) = group {
            self.wake_group(group, attacker);
        }
    }
}
