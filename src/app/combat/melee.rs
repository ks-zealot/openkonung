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
use crate::combat::{self, CombatTarget, MeleeRoll};
use crate::coords::neighbors;

impl PlayableApp {
    /// Resolve one melee attack and apply HP, poison, equipment wear and death.
    pub(super) fn resolve_attack(
        &mut self,
        attacker: CombatTarget,
        defender: CombatTarget,
    ) -> Option<MeleeRoll> {
        if self.combatant_dead(attacker) || self.combatant_dead(defender) {
            return None;
        }
        // `FUN_00421B50` case 5: entering the melee row wakes the target's
        // group (`FUN_004245B4`) before damage is resolved.
        self.wake_combatant_group(defender, attacker);
        self.combat_attacks = self.combat_attacks.saturating_add(1);
        let a = self.combatant_stats(attacker)?;
        let d = self.combatant_stats(defender)?;
        let af = self.combatant_facing(attacker);
        let df = self.combatant_facing(defender);
        let (roll, armor) = {
            let res = &self.world.res;
            let db = res.item_db.as_ref()?;
            let roll =
                combat::resolve_melee(db, &res.save_game, &a, af, &d, df, &mut self.combat_rng);
            let armor = combat::armor_total(db, &res.save_game, &d);
            (roll, armor)
        };
        if !roll.hit {
            if roll.dodged {
                self.play_dodge(self.combatant_cell(attacker));
                self.set_combat_message("D�D�D�D_D�D�D�D,D�");
            }
            return Some(roll);
        }
        self.combat_hits = self.combat_hits.saturating_add(1);
        self.play_melee_impact(
            a.archetype,
            a.weapon_slot,
            self.combatant_cell(attacker),
        );
        let hp_before = d.hp;
        if roll.lethal {
            // `FUN_004123BC` case 5 calls `FUN_00418480(defender, 0, 0)`:
            // melee kills never generate a random item (killer level 0).
            self.kill_combatant(defender, 0, 0);
        } else {
            self.set_combat_hp(defender, hp_before.saturating_sub(roll.damage));
            self.add_combat_poison(defender, roll.poison);
        }
        self.apply_armor_wear(defender, roll.incoming as i32, armor);
        self.apply_weapon_wear(attacker, roll.incoming as i32);
        Some(roll)
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

    pub(super) fn is_adjacent(a: (i32, i32), b: (i32, i32)) -> bool {
        a != b && neighbors(a.0, a.1).iter().any(|&(_, x, y)| (x, y) == b)
    }
}
