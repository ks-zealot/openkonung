//! Group-activity and target-acquisition AI (`FUN_00412C30`, `FUN_00412E00`).
//!
//! The original per-frame pass keys all object/enemy AI off `Group+0x1D`:
//!
//! * `0`   - idle; normalize the action to idle `0`/`6`;
//! * `1`   - combat: acquire a target from `Group+0x1A`;
//! * `0x20`- recall: a member strayed outside rectangle A; return to the rect
//!           (and chase an available hostile through `FUN_0041FB64`).
//!
//! `FUN_00412C30` runs every 16 logic frames for current-location groups with
//! `class & 0x40`. It sets activity `1` when any party member is inside the
//! group's rectangle A, otherwise `0x20` when any own member is outside it, and
//! `0` otherwise. `FUN_004245B4` is only the wake helper: activity `1` plus a
//! cleared route terminator for eligible members; it does not assign a target.

use super::super::*;
use crate::combat::CombatTarget;
use crate::constants::combat::OBJECT_ARCHETYPE_BIT;
use crate::population::{GameTables, Rect};

/// Inclusive rectangle-A test over raw runtime cells.
///
/// The original compares the live cell against the group rectangle bounds
/// directly, so a member exactly on `x2`/`y2` is inside.
fn rect_contains(rect: &crate::population::Rect, x: i32, y: i32) -> bool {
    x >= rect.x1 as i32 && x <= rect.x2 as i32 && y >= rect.y1 as i32 && y <= rect.y2 as i32
}

impl PlayableApp {
    /// `FUN_00412C30`: refresh object/enemy group activity and target group.
    pub(in crate::app) fn update_group_relations(&mut self) {
        let loc = self.current_map;
        let mut party_cells = Vec::new();
        if !self.player.dead {
            party_cells.push((self.player_x.round() as i32, self.player_y.round() as i32));
        }
        party_cells.extend(self.party.iter().map(|m| m.motion.cell));

        let Some(tables) = self.tables.as_mut() else {
            return;
        };
        let GameTables { groups, entities } = tables;
        // Snapshot the current-location object groups and their authored
        // visibility. Runtime cells come from the population cache below
        // (the original reads live `Entity+0x08/+0x0A`, which the remake keeps
        // in `ActorMotion::cell`).
        let group_infos: Vec<(usize, Rect, Vec<(usize, (i32, i32), u8, u8)>)> = groups
            .iter()
            .filter(|g| g.loc_id as u32 == loc && g.class_1e & OBJECT_ARCHETYPE_BIT != 0)
            .map(|g| {
                let members = g
                    .entity_range()
                    .map(|range| {
                        entities[range]
                            .iter()
                            .map(|e| (e.index, (e.x as i32, e.y as i32), e.archetype, e.kind))
                            .collect()
                    })
                    .unwrap_or_default();
                (g.index, g.rect_a, members)
            })
            .collect();
        let live_cells: std::collections::HashMap<usize, (i32, i32)> = self
            .population_cache
            .get(&loc)
            .map(|p| p.motion.iter().map(|(&id, m)| (id, m.cell)).collect())
            .unwrap_or_default();
        for (index, rect, members) in group_infos {
            if party_cells.iter().any(|&(x, y)| rect_contains(&rect, x, y)) {
                if let Some(g) = groups.get_mut(index) {
                    // `FUN_00412C30`: target group = leader[+5] (party group 0).
                    g.target_group = 0;
                    g.activity_1d = 1;
                }
                continue;
            }
            let outside = members.iter().any(|&(id, authored, archetype, kind)| {
                let visible = archetype & 0x80 == 0 && kind != 9 && kind != 3;
                if !visible {
                    return false;
                }
                let (x, y) = live_cells.get(&id).copied().unwrap_or(authored);
                !rect_contains(&rect, x, y)
            });
            if let Some(g) = groups.get_mut(index) {
                g.activity_1d = if outside { 0x20 } else { 0 };
            }
        }
    }

    /// Mode-0 map entry (`FUN_0043ADF0`): object/enemy groups start idle.
    pub(in crate::app) fn reset_object_group_activity(&mut self) {
        let loc = self.current_map;
        let Some(tables) = self.tables.as_mut() else {
            return;
        };
        for g in tables.groups.iter_mut() {
            if g.loc_id as u32 == loc && g.class_1e & OBJECT_ARCHETYPE_BIT != 0 {
                g.activity_1d = 0;
            }
        }
    }

    /// `FUN_00412E00`: assign combat targets for active groups. This only
    /// writes `ActorMotion.combat_target`; animation, movement and damage stay
    /// in `update_world_combat`.
    pub(in crate::app) fn update_world_ai(&mut self) {
        let loc = self.current_map;
        let party: Vec<EnemyRef> = {
            let mut v = Vec::new();
            if !self.player.dead {
                v.push(EnemyRef {
                    target: CombatTarget::Player,
                    cell: (self.player_x.round() as i32, self.player_y.round() as i32),
                    group: 0,
                });
            }
            for (i, m) in self.party.iter().enumerate() {
                if !m.motion.dead {
                    v.push(EnemyRef {
                        target: CombatTarget::Party(i),
                        cell: m.motion.cell,
                        group: 0,
                    });
                }
            }
            v
        };

        // Read activity/target-group before taking the mutable population borrow.
        let groups: Vec<(usize, u8, u8, Rect, u8)> = self
            .tables
            .as_ref()
            .map(|t| {
                t.groups
                    .iter()
                    .filter(|g| {
                        g.index != 0
                            && g.loc_id as u32 == loc
                            && g.class_1e & 0xC0 != 0
                            && (g.activity_1d == 1 || g.activity_1d == 0x20)
                    })
                    .map(|g| (g.index, g.target_group, g.activity_1d, g.rect_a, g.class_1e))
                    .collect()
            })
            .unwrap_or_default();
        if groups.is_empty() {
            self.idle_inactive_object_groups();
            return;
        }
        self.idle_inactive_object_groups();

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
        let world_enemies: Vec<EnemyRef> = pop
            .actors
            .iter()
            .filter_map(|a| {
                if a.entity.archetype & 0x80 != 0 || a.entity.kind == 9 || a.entity.kind == 3 {
                    return None;
                }
                let m = pop.motion.get(&a.entity.index)?;
                if m.dead {
                    return None;
                }
                Some(EnemyRef {
                    target: CombatTarget::World(a.entity.index),
                    cell: m.cell,
                    group: a.entity.group_id,
                })
            })
            .collect();

        for (group_id, target_group, activity, rect, class) in groups {
            if activity == 0x20 {
                // `FUN_00412E00` handles recall only for object groups; the
                // citizen dispatcher returns without a recall path.
                if class & 0x40 != 0 {
                    Self::return_group_to_rect(pop, collision, group_id, &rect);
                }
                continue;
            }
            let enemies: Vec<EnemyRef> = if target_group == 0 {
                party.clone()
            } else {
                world_enemies
                    .iter()
                    .copied()
                    .filter(|e| e.group == target_group)
                    .collect()
            };
            if enemies.is_empty() && target_group != 0 {
                // A woken citizen/object group targets the attacker's group; a
                // destroyed attacker group leaves nothing to acquire.
                continue;
            }
            let members: Vec<usize> = pop
                .actors
                .iter()
                .filter(|a| {
                    a.source_group_id == group_id
                        && !pop.motion.get(&a.entity.index).is_some_and(|m| m.dead)
                })
                .map(|a| a.entity.index)
                .collect();
            if members.is_empty() {
                continue;
            }
            let mut engaged: HashSet<CombatTarget> = members
                .iter()
                .filter_map(|id| pop.motion.get(id).and_then(|m| m.combat_target))
                .collect();
            let citizen = class & 0x40 == 0;

            for id in members {
                let Some(ai) = pop.actors.iter().position(|a| a.entity.index == id) else {
                    continue;
                };
                if pop
                    .motion
                    .get(&id)
                    .is_some_and(|m| m.combat_target.is_some())
                {
                    continue;
                }
                let cell = pop.motion.get(&id).map(|m| m.cell).unwrap_or((0, 0));
                let arch = pop.actors[ai].entity.archetype;
                // `FUN_00410E84`: an armed archer (`Entity+0x12 == 1` with a
                // melee weapon in slot 0 and ammo in `+0xCC`) only acquires a
                // target its line-of-fire probe (`FUN_0041130C`) reaches.
                let probe_range = Self::ranged_probe_range(&pop.actors[ai].entity, world);
                let candidates: Vec<EnemyRef> = match probe_range {
                    Some(range) => enemies
                        .iter()
                        .copied()
                        .filter(|e| {
                            crate::combat::los::probe(collision, cell, e.cell, range).is_clear()
                        })
                        .collect(),
                    None => enemies.clone(),
                };
                if candidates.is_empty() {
                    continue;
                }
                let target = if citizen {
                    // `FUN_004168A4` (citizen branch): plain nearest, no
                    // unengaged/focus distinction.
                    Self::nearest_enemy(cell, &candidates)
                } else {
                    match arch {
                        // `FUN_00412E00` adjacent branch (0x44/0x57).
                        0x44 | 0x57 => Self::adjacent_enemy(cell, &candidates),
                        // Shared branch: `FUN_0041FF68`; no unengaged
                        // intermediate in the original.
                        0x41 | 0x4D | 0x4F => Self::shared_target(&engaged, &candidates)
                            .or_else(|| Self::nearest_enemy(cell, &candidates)),
                        // `FUN_00420190` nearest-unengaged, then `FUN_00420098`.
                        0x43 | 0x45 | 0x46 | 0x4C | 0x55 => {
                            Self::nearest_unengaged(cell, &candidates, &engaged)
                                .or_else(|| Self::nearest_enemy(cell, &candidates))
                        }
                        // `FUN_00420098` plain nearest (including 0x4E).
                        _ => Self::nearest_enemy(cell, &candidates),
                    }
                };
                let Some(target) = target else {
                    continue;
                };
                engaged.insert(target);
                if let Some(m) = pop.motion.get_mut(&id) {
                    m.combat_target = Some(target);
                    m.attack_frames = 0;
                    m.ranged_order = false;
                    m.ranged_fired = false;
                    m.frame = 0;
                    m.path.clear();
                    m.target = None;
                    m.walking = false;
                    m.render_cell = m.cell;
                }
                if pop.actors[ai].entity.archetype & OBJECT_ARCHETYPE_BIT == 0 {
                    pop.actors[ai].entity.flags_b |= crate::anim::FLAG_BATTLE;
                }
            }
        }
    }

    /// Activity-0 object groups idle while awaiting the next relation pass.
    fn idle_inactive_object_groups(&mut self) {
        let loc = self.current_map;
        let inactive: Vec<usize> = self
            .tables
            .as_ref()
            .map(|t| {
                t.groups
                    .iter()
                    .filter(|g| {
                        g.index != 0
                            && g.loc_id as u32 == loc
                            && g.class_1e & OBJECT_ARCHETYPE_BIT != 0
                            && g.activity_1d == 0
                    })
                    .map(|g| g.index)
                    .collect()
            })
            .unwrap_or_default();
        if inactive.is_empty() {
            return;
        }
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for a in pop.actors.iter_mut() {
                if !inactive.contains(&a.source_group_id) {
                    continue;
                }
                let Some(m) = pop.motion.get_mut(&a.entity.index) else {
                    continue;
                };
                if m.dead || m.walking || m.combat_target.is_some() {
                    if m.dead || m.walking {
                        continue;
                    }
                    // `FUN_00412E00` activity 0 returns before the action
                    // machine; a stale target must not keep attacking.
                    m.combat_target = None;
                    m.ranged_order = false;
                    m.attack_frames = 0;
                }
                let idle =
                    crate::anim::idle_action(crate::anim::battle_from_flags(a.entity.flags_b))
                        as u8;
                if m.action != idle {
                    m.action = idle;
                    m.frame = 0;
                }
            }
        }
    }

    /// `FUN_004105A0` / `FUN_004104D4`: every non-party human actor refreshes
    /// its equipment and active weapon slot on its AI tick. The original calls
    /// it from `FUN_00412E00` (object/enemy groups) and `FUN_004168A4` (citizen
    /// groups), i.e. for exactly the actors in the loaded population.
    ///
    /// Deviation (documented): object-family actors (`archetype & 0x40`) are
    /// skipped. They can never hold equipment, and for `archetype 0x4D/0x4F`
    /// `Entity+0x12` is the authored M/O walk delay — writing the weapon slot
    /// there would corrupt it.
    pub(in crate::app) fn refresh_npc_equipment(&mut self) {
        let loc = self.current_map;
        let party_ids: HashSet<usize> = self.party.iter().map(|m| m.entity.index).collect();
        let PlayableApp {
            world,
            population_cache,
            ..
        } = self;
        let Some(pop) = population_cache.get_mut(&loc) else {
            return;
        };
        let ids: Vec<usize> = pop
            .actors
            .iter()
            .filter(|a| a.entity.archetype & 0x40 == 0 && !party_ids.contains(&a.entity.index))
            .map(|a| a.entity.index)
            .collect();
        for id in ids {
            if pop.motion.get(&id).is_some_and(|m| m.dead) {
                continue;
            }
            let Some(pos) = pop.actors.iter().position(|a| a.entity.index == id) else {
                continue;
            };
            let entity = &mut pop.actors[pos].entity;
            let snap = crate::inventory::OwnerSnapshot::from_entity(entity);
            let out = {
                let mut slots = crate::inventory::SlotsMut {
                    equipment: &mut entity.equipment,
                    inventory: &mut entity.inventory,
                    additional: &mut entity.additional_equipment,
                    special: &mut entity.special_equipment,
                };
                crate::inventory::refresh_equipment(&snap, &world.res, &mut slots)
            };
            if out.changed || entity.raw_0x12 != out.weapon_slot {
                entity.raw_0x12 = out.weapon_slot;
            }
        }
    }

    /// `FUN_00410E84` gate: active weapon is the bow (`Entity+0x12 == 1`), a
    /// melee weapon is equipped (`equipment[0]`) and ammo is loaded
    /// (`special_equipment`). Returns the line-of-fire range of the **melee**
    /// weapon's subtype (`u16@0x459B84 + subtype*0x40`, read by
    /// `FUN_0041130C`).
    fn ranged_probe_range(
        entity: &crate::population::Entity,
        world: &crate::app::World,
    ) -> Option<u16> {
        if entity.raw_0x12 != 1 || entity.equipment[0] == 0 || entity.special_equipment == 0 {
            return None;
        }
        let subtype = world.res.item_class_subtype(entity.equipment[0])?.1;
        world.res.item_db.as_ref().map(|db| db.probe_range(subtype))
    }

    /// `FUN_0041FF68` shared branch: the first available enemy that some group
    /// member already targets.
    ///
    /// `engaged` holds the targets of the group's own members, so the group
    /// focuses its attacks instead of splitting over every candidate.
    fn shared_target(
        engaged: &HashSet<CombatTarget>,
        enemies: &[EnemyRef],
    ) -> Option<CombatTarget> {
        enemies
            .iter()
            .find(|e| engaged.contains(&e.target))
            .map(|e| e.target)
    }

    /// `FUN_00420098` plain nearest: the enemy at the smallest
    /// `FUN_0043881C` distance from `cell`.
    fn nearest_enemy(cell: (i32, i32), enemies: &[EnemyRef]) -> Option<CombatTarget> {
        enemies
            .iter()
            .min_by_key(|e| crate::actors::follow_distance(cell, e.cell))
            .map(|e| e.target)
    }

    /// `FUN_00420190` nearest-unengaged: the smallest-distance enemy that no
    /// group member has targeted yet.
    fn nearest_unengaged(
        cell: (i32, i32),
        enemies: &[EnemyRef],
        engaged: &HashSet<CombatTarget>,
    ) -> Option<CombatTarget> {
        enemies
            .iter()
            .filter(|e| !engaged.contains(&e.target))
            .min_by_key(|e| crate::actors::follow_distance(cell, e.cell))
            .map(|e| e.target)
    }

    /// `FUN_00412E00` adjacent branch (archetypes `0x44`/`0x57`): the enemy
    /// standing on one of the eight neighbouring cells of `cell`.
    ///
    /// Returns `None` when no adjacent cell holds an enemy, which leaves the
    /// unit without a target for this pass.
    fn adjacent_enemy(cell: (i32, i32), enemies: &[EnemyRef]) -> Option<CombatTarget> {
        for (_, nx, ny) in crate::coords::neighbors(cell.0, cell.1) {
            if let Some(e) = enemies.iter().find(|e| e.cell == (nx, ny)) {
                return Some(e.target);
            }
        }
        None
    }

    /// Activity `0x20` recall: walk every stray group member back into
    /// rectangle A.
    ///
    /// The goal is the member's cell clamped into the rectangle, so it never aims
    /// past the bound. A reversed (invalid) authored rectangle is skipped instead
    /// of being normalized. Walking members finish their current cycle; idle
    /// ones start a route through `FUN_0043DA1F`.
    fn return_group_to_rect(
        pop: &mut PopulationBuild,
        collision: &[u16],
        group_id: usize,
        rect: &crate::population::Rect,
    ) {
        if rect.x1 > rect.x2 || rect.y1 > rect.y2 {
            // Invalid authored rectangle: never clamp with reversed bounds.
            return;
        }
        let mut occupied: HashSet<(i32, i32)> = pop
            .motion
            .values()
            .filter(|m| !m.dead)
            .map(|m| m.cell)
            .collect();
        let ids: Vec<usize> = pop
            .actors
            .iter()
            .filter(|a| a.source_group_id == group_id)
            .map(|a| a.entity.index)
            .collect();
        for id in ids {
            let Some(m) = pop.motion.get_mut(&id) else {
                continue;
            };
            if rect_contains(rect, m.cell.0, m.cell.1) {
                if !m.walking {
                    m.action = crate::anim::idle_action(false) as u8;
                }
                continue;
            }
            if m.walking {
                if Self::advance_motion_walk(m, collision, &mut occupied) {
                    m.action = crate::anim::walk_action(false) as u8;
                } else {
                    m.action = crate::anim::idle_action(false) as u8;
                }
                continue;
            }
            let goal = (
                m.cell.0.clamp(rect.x1 as i32, rect.x2 as i32),
                m.cell.1.clamp(rect.y1 as i32, rect.y2 as i32),
            );
            if goal == m.cell {
                m.action = crate::anim::idle_action(false) as u8;
                continue;
            }
            Self::start_motion_walk(m, collision, &mut occupied, goal);
            if m.walking {
                m.action = crate::anim::walk_action(false) as u8;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct EnemyRef {
    target: CombatTarget,
    cell: (i32, i32),
    group: u8,
}
