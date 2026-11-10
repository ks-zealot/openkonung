//! Citizen waypoint/wander AI.
//!
//! One fixed-step pass over the location's actors. Stationary anchors (home and
//! role waypoints) are walked to once and then held; unanchored citizens pick
//! random targets strictly inside the group's spawn rectangle A. One cell is
//! stepped per completed walk-animation cycle, keyed off the original 60 ms
//! frame so the cadence matches the 1999 build.

use super::super::*;

use crate::constants::motion::*;
use crate::coords::step_by_direction;

/// Fixed-step citizen update (`FUN_004168a4` + `FUN_00413a84` + `FUN_004142d4`/
/// `FUN_0041435c`).
///
/// Only [`wanders`]-eligible actors move. An actor that **owns** a settlement
/// home waypoint (`FUN_0041182C`: `waypoint.owner == Entity index`) walks to
/// that waypoint and then stays idle there (stationary). Only actors without an
/// owned waypoint random-wander within the group's rectangle A. One cell is
/// stepped per completed walk-animation cycle. `settlement` supplies the current
/// location's waypoints.
pub fn update_population(
    build: &mut PopulationBuild,
    collision: &[u16],
    settlement: Option<&Settlement>,
    dt: f64,
) {
    build.tick_accum += dt.max(0.0) * LOGIC_TICK_HZ;
    let mut ticks = build.tick_accum.floor() as i64;
    build.tick_accum -= ticks as f64;
    if ticks <= 0 {
        return;
    }
    if ticks > MAX_MOTION_TICKS_PER_FRAME {
        ticks = MAX_MOTION_TICKS_PER_FRAME;
    }

    // Owning Entity index -> home waypoint for the current settlement. The
    // original `FUN_0041182C` scans only the first five waypoint slots.
    let homes: HashMap<u16, Waypoint> = settlement
        .map(|s| {
            s.waypoints
                .iter()
                .take(5)
                .filter(|w| w.owner != 0)
                .map(|w| (w.owner, *w))
                .collect()
        })
        .unwrap_or_default();

    // Snapshot static per-actor metadata (stable iteration order).
    let meta: Vec<(usize, u8, u8, u8, u8, bool)> = build
        .actors
        .iter()
        .map(|a| {
            (
                a.source_entity_id(),
                a.entity.archetype,
                a.entity.flags_a,
                a.entity.kind,
                a.entity.flags_b,
                build
                    .motion
                    .get(&a.source_entity_id())
                    .is_some_and(|m| m.citizen),
            )
        })
        .collect();

    let mut occupied: HashSet<(i32, i32)> = build
        .motion
        .values()
        .filter(|m| !m.dead)
        .map(|m| m.cell)
        .collect();

    for _ in 0..ticks {
        for &(eid, arch, flags, kind, flags_b, citizen) in &meta {
            if build.motion.get(&eid).is_some_and(|m| m.dead) {
                continue;
            }
            // A citizen that has a live combat target is driven by the combat
            // action path; do not let the wander pass reset its attack frame.
            if build
                .motion
                .get(&eid)
                .is_some_and(|m| m.combat_target.is_some())
            {
                continue;
            }
            // Only citizen groups (group class `& 0x80`, `& 0x40 == 0`) run the
            // waypoint/wander AI. Object/enemy groups (`class & 0x40`) idle in
            // place (`FUN_00412E00`, `+0x1D == 0`) and must not wander.
            if !citizen
                || flags & FLAG_WANDER == 0
                || arch & CITIZEN_GROUP_CLASS_BIT != 0
                || matches!(kind, 3 | 9)
            {
                continue;
            }
            let battle = crate::anim::battle_from_flags(flags_b);
            // `FUN_004168A4` role-station branch: role bits select waypoint
            // slot 1..=3 (the master's workstation). If the slot has an owner
            // the actor walks there; otherwise the original clears the role.
            let role = ((arch & 0x38) >> 3) as usize;
            let station = if role > 0 {
                settlement
                    .and_then(|s| s.waypoints.get(role))
                    .filter(|w| w.owner != 0)
                    .copied()
            } else {
                None
            };
            if role > 0 && station.is_none() {
                if let Some(a) = build.actors.iter_mut().find(|a| a.entity.index == eid) {
                    a.entity.archetype &= !0x38;
                }
            }
            let Some(m) = build.motion.get_mut(&eid) else {
                continue;
            };
            let rect = m.rect_a;
            let cell = m.cell;

            // Route issuing runs only while no segment is in flight, so a route
            // is never re-issued mid-step (the committed cell leads the sprite).
            if !m.walking {
                // Role workstation first (`FUN_004168A4`): the actor never
                // stands on the master's own cell. If it is already on one of
                // the 8 neighbours (`FUN_0041165C`) it faces the waypoint's
                // facing; otherwise it moves to the first free neighbour
                // (`FUN_00410FFC`).
                if let Some(w) = station {
                    let station_cell = (w.x as i32, w.y as i32);
                    let adjacent = (0..8u8)
                        .any(|d| step_by_direction(station_cell.0, station_cell.1, d) == cell);
                    if adjacent {
                        m.path.clear();
                        m.target = None;
                        if m.facing != w.facing {
                            m.facing = w.facing;
                        }
                    } else if let Some(goal) = (0..8u8)
                        .map(|d| step_by_direction(station_cell.0, station_cell.1, d))
                        .find(|&(nx, ny)| is_free(collision, nx, ny))
                    {
                        m.target = Some(goal);
                        let bounds = path_bounds(&rect, cell, goal);
                        match find_path(collision, &occupied, cell, goal, bounds) {
                            Some(p) if !p.is_empty() => m.path = p,
                            _ => {
                                m.wait = WANDER_WAIT_MIN;
                                m.target = None;
                            }
                        }
                    } else {
                        m.path.clear();
                        m.target = None;
                    }
                } else if let Some(w) = homes.get(&(eid as u16)).copied() {
                    // Settlement home waypoint owned by this entity
                    // (`FUN_0041182C`): the owner walks onto the exact cell.
                    let home_cell = (w.x as i32, w.y as i32);
                    if cell == home_cell {
                        m.path.clear();
                        m.target = None;
                        if m.facing != w.facing {
                            m.facing = w.facing;
                        }
                    } else {
                        // (Re)issue a route to the home waypoint. A route longer
                        // than `MAX_PATH_STEPS` is re-issued once its prefix is
                        // consumed.
                        m.target = Some(home_cell);
                        let bounds = path_bounds(&rect, cell, home_cell);
                        match find_path(collision, &occupied, cell, home_cell, bounds) {
                            Some(p) if !p.is_empty() => m.path = p,
                            _ => {
                                m.wait = WANDER_WAIT_MIN;
                                m.target = None;
                            }
                        }
                    }
                } else if let Some(t) = m.target {
                    // Continue/refresh an in-flight wander route.
                    if cell == t {
                        m.target = None;
                        m.wait = WANDER_WAIT_MIN + (build.rng.next_u32() % WANDER_WAIT_SPAN) as u16;
                    } else {
                        let bounds = path_bounds(&rect, cell, t);
                        match find_path(collision, &occupied, cell, t, bounds) {
                            Some(p) if !p.is_empty() => m.path = p,
                            _ => {
                                m.target = None;
                                m.wait = WANDER_WAIT_MIN;
                            }
                        }
                    }
                } else {
                    // Think when idle: 50 % skip, else pick a random rect-A target.
                    if m.wait > 0 {
                        m.wait -= 1;
                    } else if build.rng.next_u32() % WANDER_THINK_SKIP_DIVISOR == 0 {
                        if let Some(t) = wander_target(&rect, cell, &mut build.rng, collision) {
                            if t != cell {
                                let bounds = path_bounds(&rect, cell, t);
                                match find_path(collision, &occupied, cell, t, bounds) {
                                    Some(p) if !p.is_empty() => {
                                        m.path = p;
                                        m.target = Some(t);
                                    }
                                    _ => m.wait = WANDER_WAIT_MIN,
                                }
                            }
                        }
                    }
                }

                // Commit the first cell of a freshly issued route at its START
                // (`FUN_0041435C`): draw from the pre-step cell, then advance.
                if !m.path.is_empty() {
                    begin_actor_segment(m, &mut occupied, collision);
                }
            }

            // Animate the current stance action (`FUN_00414A34` idle 0/6,
            // `FUN_004142D4` walk 1/7). Walking steps one cell per completed walk
            // cycle; the idle frame is **held**, not looped (the shipped active
            // population idles with `FUN_00412E00 == 0`, so it holds frame 0).
            let next_action = if m.walking {
                crate::anim::walk_action(battle) as u8
            } else {
                crate::anim::idle_action(battle) as u8
            };
            if m.action != next_action {
                m.action = next_action;
                m.frame = 0;
            }
            if m.walking {
                let walk_frames = if m.walk_frames == 0 {
                    FALLBACK_WALK_FRAMES
                } else {
                    m.walk_frames
                };
                m.frame = m.frame.saturating_add(1);
                if m.frame >= walk_frames {
                    m.frame = 0;
                    if !m.path.is_empty() {
                        // Commit the next cell at cycle start.
                        begin_actor_segment(m, &mut occupied, collision);
                    } else {
                        m.walking = false;
                        m.render_cell = m.cell;
                        if m.target == Some(m.cell) {
                            m.target = None;
                            m.wait =
                                WANDER_WAIT_MIN + (build.rng.next_u32() % WANDER_WAIT_SPAN) as u16;
                        }
                    }
                }
            } else {
                m.frame = 0;
            }
        }
    }
}
