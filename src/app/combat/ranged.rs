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
    /// True when the unit can fire: battle stance, bow in `Equipment[1]`, ammo
    /// in `+0xCC` with quantity > 0. `require_plain_ammo` mirrors the original
    /// prop-order gate (`ammo instance +0x02 == 0`).
    pub(super) fn ranged_ready(&self, t: CombatTarget, require_plain_ammo: bool) -> bool {
        let Some(stats) = self.combatant_stats(t) else {
            return false;
        };
        if stats.weapon_slot != 1 || stats.equipment[1] == 0 {
            return false;
        }
        if !crate::anim::battle_from_flags(match t {
            CombatTarget::Player => self.player.flags_b,
            CombatTarget::Party(i) => self.party.get(i).map(|m| m.entity.flags_b).unwrap_or(0),
            CombatTarget::World(id) => self.combatant_entity(id).map(|e| e.flags_b).unwrap_or(0),
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

    /// Spawn one arrow (`FUN_0041BF44`) and consume one ammo charge plus 0.1 bow
    /// durability. Returns false when the pool/bow/ammo is unavailable.
    pub(super) fn spawn_projectile(
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
            let (mx, my) =
                crate::combat::projectile::muzzle_offset((self.combatant_facing(shooter)) & 7);
            (
                bow,
                bow_info.projectile_lifetime,
                (lx as f32 + mx, ly as f32 + my),
                self.combatant_facing(shooter) & 7,
            )
        };
        let (tx, ty) = entity_to_local_px(target_cell.0, target_cell.1);
        let target = (
            tx as f32,
            ty as f32 - crate::combat::projectile::AIM_Y_OFFSET,
        );
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
            if roll.dodged {
                self.play_dodge();
            }
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
        let money = power / RANGED_KILL_MONEY_DIVISOR;
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
                    if roll < SETTLEMENT_PROP_HIT_CHANCE {
                        t.state = SETTLEMENT_PROP_HIT_STATE;
                        t.timer = SETTLEMENT_PROP_HIT_TIMER;
                    }
                    changed = true;
                    break;
                }
            }
            if changed {
                s.flags_4b2 |= SETTLEMENT_ALERT_FLAGS;
                wake = Some(s.group_id as usize);
            }
        }
        if let Some(g) = wake {
            self.wake_group(g, CombatTarget::Player);
        }
    }
    pub(super) fn update_projectiles(&mut self) {
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
                    let idx = cell.0 * COLLISION_ROW_STRIDE + cell.1;
                    collision
                        .get(idx as usize)
                        .map(|&v| (v >> 8) & COLLISION_SOLID_MASK != 0)
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
                let hit = occupants
                    .iter()
                    .find(|(t, c, dead)| *c == cell && *t != owner && !*dead);
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
}
