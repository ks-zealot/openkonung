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

fn rect_contains(rect: &crate::population::Rect, x: i32, y: i32) -> bool {
    x >= rect.x1 as i32 && x <= rect.x2 as i32 && y >= rect.y1 as i32 && y <= rect.y2 as i32
}

impl PlayableApp {
    /// `FUN_00412C30`: refresh object/enemy group activity and target group.
    pub(in crate::app) fn update_group_relations(&mut self) {
        let loc = self.current_map;
        let mut party_cells = Vec::new();
        if !self.player.dead {
            party_cells.push((
                self.player_x.round() as i32,
                self.player_y.round() as i32,
            ));
        }
        party_cells.extend(
            self.party
                .iter()
                .filter(|m| !m.motion.dead)
                .map(|m| m.motion.cell),
        );

        let Some(tables) = self.tables.as_mut() else {
            return;
        };
        let GameTables { groups, entities } = tables;
        for g in groups.iter_mut() {
            if g.loc_id as u32 != loc || g.class_1e & OBJECT_ARCHETYPE_BIT == 0 {
                continue;
            }
            if party_cells
                .iter()
                .any(|&(x, y)| rect_contains(&g.rect_a, x, y))
            {
                // `FUN_00412C30`: target group = leader[+5] (party group 0).
                g.target_group = 0;
                g.activity_1d = 1;
                continue;
            }
            let outside = g.entity_range().ok().is_some_and(|range| {
                entities[range]
                    .iter()
                    .any(|e| !rect_contains(&g.rect_a, e.x as i32, e.y as i32))
            });
            g.activity_1d = if outside { 0x20 } else { 0 };
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
        let groups: Vec<(usize, u8, u8, Rect)> = self
            .tables
            .as_ref()
            .map(|t| {
                t.groups
                    .iter()
                    .filter(|g| {
                        g.loc_id as u32 == loc
                            && (g.activity_1d == 1 || g.activity_1d == 0x20)
                    })
                    .map(|g| (g.index, g.target_group, g.activity_1d, g.rect_a))
                    .collect()
            })
            .unwrap_or_default();
        if groups.is_empty() {
            return;
        }

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

        for (group_id, target_group, activity, rect) in groups {
            if activity == 0x20 {
                Self::return_group_to_rect(pop, collision, group_id, &rect);
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
                .filter(|a| a.source_group_id == group_id && !pop.motion.get(&a.entity.index).is_some_and(|m| m.dead))
                .map(|a| a.entity.index)
                .collect();
            if members.is_empty() {
                continue;
            }
            let mut engaged: HashSet<CombatTarget> = members
                .iter()
                .filter_map(|id| pop.motion.get(id).and_then(|m| m.combat_target))
                .collect();

            for id in members {
                let Some(ai) = pop.actors.iter().position(|a| a.entity.index == id) else {
                    continue;
                };
                if pop.motion.get(&id).is_some_and(|m| m.combat_target.is_some()) {
                    continue;
                }
                let cell = pop.motion.get(&id).map(|m| m.cell).unwrap_or((0, 0));
                let arch = pop.actors[ai].entity.archetype;
                let target = if matches!(arch, 0x44 | 0x57) {
                    Self::adjacent_enemy(cell, &enemies)
                } else if matches!(arch, 0x41 | 0x4D | 0x4E | 0x4F) {
                    Self::shared_target(&engaged, &enemies)
                        .or_else(|| Self::nearest_unengaged(cell, &enemies, &engaged))
                        .or_else(|| Self::nearest_enemy(cell, &enemies))
                } else {
                    Self::nearest_unengaged(cell, &enemies, &engaged)
                        .or_else(|| Self::nearest_enemy(cell, &enemies))
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

    fn shared_target(engaged: &HashSet<CombatTarget>, enemies: &[EnemyRef]) -> Option<CombatTarget> {
        enemies
            .iter()
            .find(|e| engaged.contains(&e.target))
            .map(|e| e.target)
    }

    fn nearest_enemy(cell: (i32, i32), enemies: &[EnemyRef]) -> Option<CombatTarget> {
        enemies
            .iter()
            .min_by_key(|e| crate::actors::follow_distance(cell, e.cell))
            .map(|e| e.target)
    }

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

    fn adjacent_enemy(cell: (i32, i32), enemies: &[EnemyRef]) -> Option<CombatTarget> {
        for (_, nx, ny) in crate::coords::neighbors(cell.0, cell.1) {
            if let Some(e) = enemies.iter().find(|e| e.cell == (nx, ny)) {
                return Some(e.target);
            }
        }
        None
    }

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
