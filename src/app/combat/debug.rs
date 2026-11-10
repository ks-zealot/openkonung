//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).
//!
//! This file: the diagnostic staging and report helpers used by the headless
//! combat, ranged and mob-AI smoke tests (`tools/combat_smoke.mjs`,
//! `tools/ranged_smoke.mjs`, `tools/mob_ai_smoke.mjs`).

use super::super::*;
use crate::combat::{self, CombatStats, CombatTarget};
use crate::constants::combat::*;
use crate::coords::neighbors;

impl PlayableApp {
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
                if a.entity.archetype & OBJECT_ARCHETYPE_BIT == 0 {
                    continue;
                }
                if pop.motion.get(&a.entity.index).is_some_and(|m| m.dead) {
                    continue;
                }
                a.entity.stats.hp_internal = COMBAT_TEST_TARGET_HP;
                a.entity.stats.strength_current = 1;
                a.entity.stats.strength_base = 1;
                a.entity.stats.dexterity_current = 0;
                a.entity.stats.dexterity_base = 0;
                a.entity.stats.endurance_current = 1;
                a.entity.stats.endurance_base = 1;
                a.entity.stats.base_armor = 0;
                a.entity.stats.periodic_damage = 0;
                a.entity.stats.raw_0x52 = COMBAT_TEST_VENOM;
                a.entity.stats.money = COMBAT_TEST_TARGET_MONEY;
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

    /// Diagnostic: clear all combat targets and attack cycles (smoke staging).
    pub fn debug_clear_combat_targets(&mut self) {
        self.player_attack_target = None;
        self.player_attack_frame = 0;
        self.player_attack_ranged = false;
        self.player_ranged_frame = 0;
        self.player_ranged_fired = false;
        for m in self.party.iter_mut() {
            m.motion.combat_target = None;
            m.motion.ranged_order = false;
            m.motion.ranged_fired = false;
            m.motion.attack_frames = 0;
            m.motion.frame = 0;
            m.motion.path.clear();
            m.motion.walking = false;
        }
        if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
            for m in pop.motion.values_mut() {
                m.combat_target = None;
                m.ranged_order = false;
                m.ranged_fired = false;
                m.attack_frames = 0;
                m.frame = 0;
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
            }
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
        for dx in RANGED_TEST_MIN_DISTANCE..RANGED_TEST_MAX_DISTANCE {
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
                if a.entity.archetype & OBJECT_ARCHETYPE_BIT == 0 {
                    continue;
                }
                a.entity.stats.hp_internal = RANGED_TEST_TARGET_HP;
                a.entity.stats.strength_base = RANGED_TEST_TARGET_STRENGTH;
                a.entity.stats.strength_current = RANGED_TEST_TARGET_STRENGTH;
                a.entity.stats.dexterity_base = 0;
                a.entity.stats.dexterity_current = 0;
                a.entity.stats.endurance_base = RANGED_TEST_TARGET_ENDURANCE;
                a.entity.stats.endurance_current = RANGED_TEST_TARGET_ENDURANCE;
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
        self.player.equipment[1] = RANGED_TEST_BOW_ITEM;
        self.player.raw_0x12 = 1;
        self.player.special_equipment = RANGED_TEST_AMMO_ITEM;
        self.world
            .res
            .set_item_quantity(RANGED_TEST_AMMO_ITEM, RANGED_TEST_AMMO_QUANTITY);
        self.world
            .res
            .set_item_durability(RANGED_TEST_BOW_ITEM, RANGED_TEST_BOW_DURABILITY);
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
                a.entity.inventory[0] = RANGED_TEST_TARGET_ARROW_ITEM;
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
                a.entity.stats.dexterity_base = RANGED_TEST_DODGE_DEXTERITY;
                a.entity.stats.dexterity_current = RANGED_TEST_DODGE_DEXTERITY;
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
        'search: for radius in 1..PROP_TEST_SEARCH_RADIUS {
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
        self.player.equipment[1] = RANGED_TEST_BOW_ITEM;
        self.player.raw_0x12 = 1;
        self.player.special_equipment = RANGED_TEST_AMMO_ITEM;
        self.world
            .res
            .set_item_quantity(RANGED_TEST_AMMO_ITEM, RANGED_TEST_AMMO_QUANTITY);
        self.world.res.set_item_modifier(RANGED_TEST_AMMO_ITEM, 0);
        self.world
            .res
            .set_item_durability(RANGED_TEST_BOW_ITEM, RANGED_TEST_BOW_DURABILITY);
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

    /// Diagnostic staging for the autonomous-AI smoke: put the party inside an
    /// object group's spawn rectangle, place one live object actor next to the
    /// leader, and set the group activity to combat. Returns actor id + 1.
    pub fn ai_test_setup(&mut self) -> u32 {
        self.ensure_population(self.current_map);
        let loc = self.current_map;
        let Some(tables) = self.tables.as_ref() else {
            return 0;
        };
        let Some((group, rect)) = tables
            .groups
            .iter()
            .find(|g| g.loc_id as u32 == loc && g.class_1e & OBJECT_ARCHETYPE_BIT != 0)
            .map(|g| (g.index, g.rect_a))
        else {
            return 0;
        };
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let center = (
            (rect.x1 as i32 + rect.x2 as i32) / 2,
            (rect.y1 as i32 + rect.y2 as i32) / 2,
        );
        let Some(player_cell) = std::iter::once(center)
            .chain(
                crate::coords::neighbors(center.0, center.1)
                    .iter()
                    .map(|&(_, x, y)| (x, y)),
            )
            .find(|&(x, y)| crate::actors::is_free(collision, x, y))
        else {
            return 0;
        };
        let Some(actor_cell) = crate::coords::neighbors(player_cell.0, player_cell.1)
            .iter()
            .map(|&(_, x, y)| (x, y))
            .find(|&(x, y)| crate::actors::is_free(collision, x, y) && (x, y) != player_cell)
        else {
            return 0;
        };
        self.player_x = player_cell.0 as f64;
        self.player_y = player_cell.1 as f64;
        self.player_render_x = self.player_x;
        self.player_render_y = self.player_y;
        self.player.dead = false;
        self.player.hp_internal = self.player.hp_internal.max(1600);
        let mut chosen = None;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for a in pop.actors.iter_mut() {
                if a.source_group_id != group || a.entity.archetype & OBJECT_ARCHETYPE_BIT == 0 {
                    continue;
                }
                a.entity.stats.hp_internal = 3000;
                a.entity.stats.strength_base = 10;
                a.entity.stats.strength_current = 10;
                a.entity.stats.dexterity_base = 0;
                a.entity.stats.dexterity_current = 0;
                a.entity.stats.endurance_base = 10;
                a.entity.stats.endurance_current = 10;
                a.entity.stats.base_armor = 0;
                a.entity.stats.periodic_damage = 0;
                if let Some(m) = pop.motion.get_mut(&a.entity.index) {
                    m.cell = actor_cell;
                    m.render_cell = actor_cell;
                    m.dead = false;
                    m.combat_target = None;
                    m.path.clear();
                    m.target = None;
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
        if let Some(g) = self.tables.as_mut().and_then(|t| t.groups.get_mut(group)) {
            g.activity_1d = 1;
            g.target_group = 0;
        }
        self.combat_test_target = Some(id);
        (id as u32).saturating_add(1)
    }

    /// Encoded autonomous target: -1 none, 0 leader, 1000+slot party,
    /// 2000+entity world.
    pub fn world_target_code(&self, id: usize) -> i32 {
        let Some(m) = self
            .population_cache
            .get(&self.current_map)
            .and_then(|p| p.motion.get(&id))
        else {
            return -1;
        };
        match m.combat_target {
            None => -1,
            Some(CombatTarget::Player) => 0,
            Some(CombatTarget::Party(i)) => 1000 + i as i32,
            Some(CombatTarget::World(i)) => 2000 + i as i32,
            Some(CombatTarget::Prop(i)) => 3000 + i as i32,
        }
    }

    /// Diagnostic: current `Group+0x1D` activity byte of a group.
    ///
    /// Reports 0 when the group table is not loaded or the index is invalid.
    pub fn group_activity(&self, group_id: usize) -> u32 {
        self.tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .map(|g| g.activity_1d as u32)
            .unwrap_or(0)
    }

    /// Diagnostic: force a group's `+0x1D` activity byte.
    ///
    /// # Returns
    ///
    /// 1 on success, 0 when the group table is not loaded or the index is
    /// invalid.
    pub fn set_group_activity(&mut self, group_id: usize, activity: u8) -> u32 {
        if let Some(g) = self
            .tables
            .as_mut()
            .and_then(|t| t.groups.get_mut(group_id))
        {
            g.activity_1d = activity;
            1
        } else {
            0
        }
    }
    /// Diagnostic: source group id of a world actor, or -1 when the actor is not
    /// in the current map's population.
    pub fn world_group_id(&self, id: usize) -> i32 {
        self.population_cache
            .get(&self.current_map)
            .and_then(|p| p.actors.iter().find(|a| a.entity.index == id))
            .map(|a| a.source_group_id as i32)
            .unwrap_or(-1)
    }
}
