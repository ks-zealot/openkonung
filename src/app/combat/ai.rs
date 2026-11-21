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
                    marker: crate::actors::occupancy::marker(self.leader_entity_index),
                });
            }
            for (i, m) in self.party.iter().enumerate() {
                if !m.motion.dead {
                    v.push(EnemyRef {
                        target: CombatTarget::Party(i),
                        cell: m.motion.cell,
                        group: 0,
                        marker: crate::actors::occupancy::marker(m.entity.index),
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

        let leader_marker = crate::actors::occupancy::marker(self.leader_entity_index);
        let party_markers: Vec<u16> = self
            .party
            .iter()
            .map(|m| crate::actors::occupancy::marker(m.entity.index))
            .collect();

        let PlayableApp {
            world,
            population_cache,
            ai_schedule,
            combat_rng,
            ..
        } = self;
        let Some((collision, res)) = world.collision_and_res_mut(loc) else {
            return;
        };
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
                    marker: crate::actors::occupancy::marker(a.entity.index),
                })
            })
            .collect();

        for (group_id, target_group, activity, rect, class) in groups {
            if activity == 0x20 {
                // `FUN_00412E00` handles recall only for object groups; the
                // citizen dispatcher returns without a recall path.
                if class & 0x40 != 0 {
                    Self::return_group_to_rect(
                        pop,
                        collision,
                        group_id,
                        &rect,
                        &party,
                        combat_rng,
                    );
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
            // `FUN_0041FF68(own_group)`: the target of the **first engaged
            // member** of this group (member order), provided that target is
            // still visible and alive. The shared branch uses it verbatim; it is
            // not a search over the enemy list.
            let shared: Option<CombatTarget> = members.iter().find_map(|id| {
                let m = pop.motion.get(id)?;
                if !m.engaged {
                    return None;
                }
                let t = m.combat_target?;
                Self::target_is_visible(t, &party, &world_enemies).then_some(t)
            });
            let citizen = class & 0x40 == 0;

            for &id in &members {
                let Some(ai) = pop.actors.iter().position(|a| a.entity.index == id) else {
                    continue;
                };
                // `FUN_004116CC`: the unit takes a turn only when the frame
                // budget allows and the cursor is behind it (one-based order).
                let order = id + 1;
                if !ai_schedule.allows(order) {
                    continue;
                }
                ai_schedule.serve(order);
                let arch = pop.actors[ai].entity.archetype;
                // `FUN_00412E00` returns 1 (handled, no AI) for the three
                // prop-like archetypes before any target logic.
                if matches!(arch, 0x51 | 0x52 | 0x53) {
                    continue;
                }
                // `FUN_00412E00` citizen branch / `FUN_004168A4`: a **non-object**
                // member of a woken group that is still at peace refreshes its
                // gear (`FUN_004105A0`, which charges one action), enters battle
                // stance and idles for that frame (`return 1`).
                if arch & OBJECT_ARCHETYPE_BIT == 0
                    && !crate::anim::battle_from_flags(pop.actors[ai].entity.flags_b)
                {
                    {
                        let entity = &mut pop.actors[ai].entity;
                        let snap = crate::inventory::OwnerSnapshot::from_entity(entity);
                        let out = {
                            let mut slots = crate::inventory::SlotsMut {
                                equipment: &mut entity.equipment,
                                inventory: &mut entity.inventory,
                                additional: &mut entity.additional_equipment,
                                special: &mut entity.special_equipment,
                            };
                            crate::inventory::refresh_equipment(&snap, res, &mut slots)
                        };
                        if out.changed || entity.raw_0x12 != out.weapon_slot {
                            entity.raw_0x12 = out.weapon_slot;
                        }
                    }
                    ai_schedule.charge();
                    pop.actors[ai].entity.flags_b |= crate::anim::FLAG_BATTLE;
                    if let Some(m) = pop.motion.get_mut(&id) {
                        m.action = crate::anim::idle_action(true) as u8;
                        m.frame = 0;
                    }
                    continue;
                }
                if pop
                    .motion
                    .get(&id)
                    .is_some_and(|m| m.combat_target.is_some())
                {
                    continue;
                }
                let cell = pop.motion.get(&id).map(|m| m.cell).unwrap_or((0, 0));
                // `FUN_00410E84`: an armed archer (`Entity+0x12 == 1` with a
                // bow in slot 1 and ammo in `+0xCC`) only acquires a
                // target its line-of-fire probe (`FUN_0041130C`) reaches.
                let probe_range = Self::ranged_probe_range(&pop.actors[ai].entity, res);
                let candidates: Vec<EnemyRef> = match probe_range {
                    // `FUN_00410E84` charges one action for the pick and one per
                    // line-of-fire probe. The remake probes every candidate (the
                    // original stops at the first clear one); both are charged.
                    // A failed pick falls through to the ordinary archetype
                    // switch (`FUN_00412E00` `LAB_0041326b`), never to
                    // "no target at all".
                    Some(range) => {
                        ai_schedule.charge();
                        let mut clear: Vec<EnemyRef> = Vec::new();
                        for e in enemies.iter().copied() {
                            ai_schedule.charge();
                            if crate::combat::los::probe(collision, cell, e.cell, range).is_clear() {
                                clear.push(e);
                            }
                        }
                        if clear.is_empty() {
                            enemies.clone()
                        } else {
                            clear
                        }
                    }
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
                        // Shared branch: `FUN_0041FF68` (first engaged member's
                        // target), then `FUN_00420098` plain nearest.
                        0x41 | 0x4D | 0x4F => {
                            shared.or_else(|| Self::nearest_enemy(cell, &candidates))
                        }
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
                let marker = match target {
                    CombatTarget::Player => leader_marker,
                    CombatTarget::Party(i) => party_markers.get(i).copied().unwrap_or(0),
                    CombatTarget::World(tid) => crate::actors::occupancy::marker(tid),
                    CombatTarget::Prop(_) => 0,
                };
                if let Some(m) = pop.motion.get_mut(&id) {
                    m.combat_target = Some(target);
                    m.target_marker = marker;
                    // `FUN_004142D4(entity, target.cell, 1)` writes the whole
                    // order byte (`Entity+0x02 = 1`): the unit now holds the
                    // persistent engage order `FUN_0041EB48` looks for.
                    m.engaged = true;
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
                    m.target_marker = 0;
                    m.engaged = false;
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

    /// `FUN_00410E84` gate: active weapon is the bow (`Entity+0x12 == 1`), a bow
    /// is equipped (`equipment[1]`, `Entity+0x68`) and ammo is loaded
    /// (`Entity+0xCC`). Returns the line-of-fire sample count of the **bow's**
    /// subtype (`u16@0x459B84 + subtype*0x40`, read by `FUN_0041130C`).
    fn ranged_probe_range(
        entity: &crate::population::Entity,
        res: &crate::res::Res,
    ) -> Option<u16> {
        let bow = super::armed_bow(
            entity.raw_0x12,
            &entity.equipment,
            entity.special_equipment,
        )?;
        let subtype = res.item_class_subtype(bow)?.1;
        res.item_db.as_ref().map(|db| db.probe_range(subtype))
    }

    /// `FUN_0041FF68` visibility test: a world target must still be in the live
    /// visible enemy list; leader/party targets are always visible.
    fn target_is_visible(
        target: CombatTarget,
        party: &[EnemyRef],
        world_enemies: &[EnemyRef],
    ) -> bool {
        match target {
            CombatTarget::World(_) => world_enemies.iter().any(|e| e.target == target),
            _ => party.iter().any(|e| e.target == target),
        }
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

    /// `FUN_00412E00` activity `0x20` recall.
    ///
    /// A member that still holds its engage order (`Entity+0x02 & 1`) chases the
    /// nearest hostile group (`FUN_0041FB64` - the party for a world group).
    /// A member without it walks to a **random cell inside rectangle A**:
    /// `rand()%2` picks the direction, then one `rand()%((dim-1)/4)` per axis
    /// when that span is at least two quarters, and a forward free-cell scan
    /// wraps inside `[x1+1, x2-1] × [y1+1, y2-1]` for at most `(w-1)*(h-1)`
    /// cells. `FUN_004142D4(..., 8)` issues the move (mode 8 = no engage bit).
    ///
    /// A member already inside the rectangle only normalizes its idle action.
    /// Walking members finish their current cycle (the remake commits one cell
    /// per cycle; the original re-issues a route every recall pass).
    fn return_group_to_rect(
        pop: &mut PopulationBuild,
        collision: &mut [u16],
        group_id: usize,
        rect: &crate::population::Rect,
        party: &[EnemyRef],
        rng: &mut crate::combat::OriginalRng,
    ) {
        if rect.x1 > rect.x2 || rect.y1 > rect.y2 {
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
            if m.dead {
                continue;
            }
            if m.walking {
                if Self::advance_motion_walk(m, id, collision, &mut occupied) {
                    m.action = crate::anim::walk_action(false) as u8;
                } else {
                    m.action = crate::anim::idle_action(false) as u8;
                }
                continue;
            }
            if m.engaged {
                // `FUN_0041FB64` for an object-group caller scans the **party
                // group inside this group's rectangle A** and returns group 0;
                // `FUN_00420190`/`FUN_00420098` then pick its nearest member.
                // No party member inside the rectangle means no chase.
                if let Some(e) = party
                    .iter()
                    .filter(|e| rect_contains(rect, e.cell.0, e.cell.1))
                    .min_by_key(|e| crate::actors::follow_distance(m.cell, e.cell))
                {
                    let (target, cell, marker) = (e.target, e.cell, e.marker);
                    m.combat_target = Some(target);
                    m.target_marker = marker;
                    Self::start_motion_walk(m, id, collision, &mut occupied, cell);
                    if m.walking {
                        m.action = crate::anim::walk_action(false) as u8;
                    }
                }
                continue;
            }
            if rect_contains(rect, m.cell.0, m.cell.1) {
                let idle = crate::anim::idle_action(false) as u8;
                if m.action != idle {
                    m.action = idle;
                    m.frame = 0;
                }
                continue;
            }
            if let Some(goal) = Self::recall_wander_goal(rect, collision, &occupied, rng) {
                Self::start_motion_walk(m, id, collision, &mut occupied, goal);
                if m.walking {
                    m.action = crate::anim::walk_action(false) as u8;
                }
            }
        }
    }

    /// `LAB_004134b3`: random point inside rectangle A plus the forward
    /// free-cell scan.
    fn recall_wander_goal(
        rect: &crate::population::Rect,
        collision: &[u16],
        occupied: &HashSet<(i32, i32)>,
        rng: &mut crate::combat::OriginalRng,
    ) -> Option<(i32, i32)> {
        let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
        let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
        let w = (x2 - x1) - 1;
        let h = (y2 - y1) - 1;
        if w < 1 || h < 1 {
            return None;
        }
        let mut cx = (x1 + x2) / 2;
        let mut cy = (y1 + y2) / 2;
        let negate = rng.roll() % 2 == 0;
        let kx = if w / 4 < 2 {
            0
        } else {
            (rng.roll() % (w / 4) as u32) as i32
        };
        let ky = if h / 4 < 2 {
            0
        } else {
            (rng.roll() % (h / 4) as u32) as i32
        };
        if negate {
            cx -= kx;
            cy -= ky;
        } else {
            cx += kx;
            cy += ky;
        }
        cx = cx.clamp(x1 + 1, x2 - 1);
        cy = cy.clamp(y1 + 1, y2 - 1);
        let total = w * h;
        let mut i = 0;
        while i < total && (!crate::actors::is_free(collision, cx, cy) || occupied.contains(&(cx, cy)))
        {
            cy += 1;
            if cy >= y2 - 1 {
                cy = y1 + 1;
                cx += 1;
                if cx >= x2 - 1 {
                    cx = x1 + 1;
                }
            }
            i += 1;
        }
        if i == total {
            None
        } else {
            Some((cx, cy))
        }
    }
}

#[derive(Clone, Copy)]
struct EnemyRef {
    target: CombatTarget,
    cell: (i32, i32),
    group: u8,
    /// One-based occupancy marker (`entity_index + 1`).
    marker: u16,
}
