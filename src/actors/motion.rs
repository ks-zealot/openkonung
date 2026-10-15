//! `motion` subsystem of [`crate::actors`] (split from `actors.rs`).

use super::*;

/// Live movement state for one runtime actor (original Entity grid).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ActorMotion {
    /// Current runtime/Entity cell.
    pub cell: (i32, i32),
    /// Destination cell, when walking.
    pub target: Option<(i32, i32)>,
    /// Remaining movement-direction codes (0..=7) to the target
    /// (`FUN_0043DA1F` queue; each entry is consumed by one walk cycle).
    pub path: Vec<u8>,
    /// Current facing (0..7).
    pub facing: u8,
    /// Walk-cycle length in frames for this actor (`Res::walk_cycle_frames`,
    /// equals the original `Entity+0xD7`). `0` = unresolved -> [`FALLBACK_WALK_FRAMES`].
    pub walk_frames: u8,
    /// Idle-action cycle length in frames for the current stance
    /// (`Res::action_cycle_frames` at [`crate::anim::idle_action`]).
    pub idle_frames: u8,
    /// Walk-animation frame counter; steps a cell every `walk_frames` ticks.
    pub frame: u8,
    /// Current original action row (`Entity+0x00`): idle/walk/attack/death for
    /// the current stance ([`crate::anim`]).
    pub action: u8,
    /// Idle cooldown in logic ticks before the next think.
    pub wait: u16,
    /// Enumerating group's spawn rectangle A (wander bounds).
    pub rect_a: Rect,
}


/// How a [`RuntimeActor`] gets its draw position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorPlacement {
    /// Persisted Entity x/y (+0x08/+0x0A) are the draw coordinates.
    Persisted,
    /// Runtime-assigned position; the original Entity x/y remain `(0,0)`.
    Runtime {
        x: i32,
        y: i32,
        attempts: u8,
        spawn_rect: Rect,
    },
}


/// A renderable non-player entity.
///
/// Holds the **one canonical parsed [`Entity`]** (graphics + authored gameplay
/// state: stats, equipment, inventory, ids) plus the enumerating group id and
/// the runtime placement. Runtime placement only adds a position; it never
/// writes back into `entity`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeActor {
    /// Canonical parsed Entity — the source of truth for all authored state.
    pub entity: Entity,
    /// The group index that enumerated this entity.
    pub source_group_id: usize,
    pub placement: ActorPlacement,
}


/// Projected draw key: local vertical position, then local horizontal position
/// as a stable tie-break. Uses the protagonist's persisted-coordinate
/// projection, not the collision grid.
pub fn projected_sort_key(a: &RuntimeActor) -> (i64, i64) {
    let (x, y) = a.display_raw();
    let (lx, ly) = entity_to_local_px(x.round() as i32, y.round() as i32);
    (ly as i64, lx as i64)
}


/// Back-to-front sort for painter's-algorithm drawing.
pub fn sort_actors(actors: &mut [RuntimeActor]) {
    actors.sort_by_key(projected_sort_key);
}


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
    if ticks > 10 {
        ticks = 10;
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
    let meta: Vec<(usize, u8, u8, u8, u8)> = build
        .actors
        .iter()
        .map(|a| {
            (
                a.source_entity_id(),
                a.entity.archetype,
                a.entity.flags_a,
                a.entity.kind,
                a.entity.flags_b,
            )
        })
        .collect();

    let mut occupied: HashSet<(i32, i32)> = build.motion.values().map(|m| m.cell).collect();

    for _ in 0..ticks {
        for &(eid, arch, flags, kind, flags_b) in &meta {
            if flags & FLAG_WANDER == 0 || arch & 0x80 != 0 || matches!(kind, 3 | 9) {
                continue;
            }
            let battle = crate::anim::battle_from_flags(flags_b);
            let Some(m) = build.motion.get_mut(&eid) else {
                continue;
            };
            let rect = m.rect_a;
            let cell = m.cell;

            // Settlement home waypoint owned by this entity (`FUN_0041182C`):
            // its owner walks there and then stays put (`FUN_004168A4` keeps it
            // idle, facing the authored direction); only entities WITHOUT a home
            // waypoint random-wander (`FUN_00413A84`).
            if let Some(w) = homes.get(&(eid as u16)).copied() {
                let home_cell = (w.x as i32, w.y as i32);
                if cell == home_cell {
                    m.path.clear();
                    m.target = None;
                    if m.facing != w.facing {
                        m.facing = w.facing;
                    }
                } else if m.path.is_empty() {
                    // (Re)issue a route to the home waypoint. A route longer than
                    // `MAX_PATH_STEPS` is re-issued once its prefix is consumed.
                    m.target = Some(home_cell);
                    let bounds = path_bounds(&rect, cell, home_cell);
                    match find_path(collision, &occupied, cell, home_cell, bounds) {
                        Some(p) if !p.is_empty() => m.path = p,
                        _ => {
                            m.wait = 30;
                            m.target = None;
                        }
                    }
                }
            } else if let Some(t) = m.target {
                // Continue/refresh an in-flight wander route.
                if m.path.is_empty() {
                    if cell == t {
                        m.target = None;
                        m.wait = 30 + (build.rng.next_u32() % 60) as u16;
                    } else {
                        let bounds = path_bounds(&rect, cell, t);
                        match find_path(collision, &occupied, cell, t, bounds) {
                            Some(p) if !p.is_empty() => m.path = p,
                            _ => {
                                m.target = None;
                                m.wait = 30;
                            }
                        }
                    }
                }
            } else {
                // Think when idle: 50 % skip, else pick a random rect-A target.
                if m.wait > 0 {
                    m.wait -= 1;
                } else if build.rng.next_u32() % 2 == 0 {
                    if let Some(t) = wander_target(&rect, cell, &mut build.rng, collision) {
                        if t != cell {
                            let bounds = path_bounds(&rect, cell, t);
                            match find_path(collision, &occupied, cell, t, bounds) {
                                Some(p) if !p.is_empty() => {
                                    m.path = p;
                                    m.target = Some(t);
                                }
                                _ => m.wait = 30,
                            }
                        }
                    }
                }
            }

            // Animate the current stance action (`FUN_00414A34` idle 0/6,
            // `FUN_004142D4` walk 1/7). Walking faces the pending direction and
            // steps one cell per completed walk cycle (`FUN_004123BC` gate:
            // facing == path[0]); idle loops its own cycle length.
            let next_action = if m.path.is_empty() {
                crate::anim::idle_action(battle) as u8
            } else {
                crate::anim::walk_action(battle) as u8
            };
            if m.action != next_action {
                m.action = next_action;
                m.frame = 0;
            }
            if !m.path.is_empty() {
                let walk_frames = if m.walk_frames == 0 {
                    FALLBACK_WALK_FRAMES
                } else {
                    m.walk_frames
                };
                m.facing = m.path[0];
                m.frame = m.frame.saturating_add(1);
                if m.frame >= walk_frames {
                    m.frame = 0;
                    let dir = m.path.remove(0);
                    let (nx, ny) = step_by_direction(m.cell.0, m.cell.1, dir);
                    if is_free(collision, nx, ny) && !occupied.contains(&(nx, ny)) {
                        occupied.remove(&m.cell);
                        m.cell = (nx, ny);
                        occupied.insert((nx, ny));
                    }
                    if m.path.is_empty() && m.target == Some(m.cell) {
                        m.target = None;
                        m.wait = 30 + (build.rng.next_u32() % 60) as u16;
                    }
                }
            } else {
                let idle_frames = if m.idle_frames == 0 {
                    FALLBACK_WALK_FRAMES
                } else {
                    m.idle_frames
                };
                m.frame = (m.frame + 1) % idle_frames.max(1);
            }
        }
    }
}


impl RuntimeActor {
    /// Source Entity id (`Entity` table index).
    pub fn source_entity_id(&self) -> usize {
        self.entity.index
    }


    pub fn is_runtime(&self) -> bool {
        matches!(self.placement, ActorPlacement::Runtime { .. })
    }


    pub fn attempts(&self) -> Option<u8> {
        match self.placement {
            ActorPlacement::Persisted => None,
            ActorPlacement::Runtime { attempts, .. } => Some(attempts),
        }
    }


    /// Spawn rectangle A used for a runtime placement, if this actor is runtime.
    pub fn spawn_rect(&self) -> Option<Rect> {
        match self.placement {
            ActorPlacement::Persisted => None,
            ActorPlacement::Runtime { spawn_rect, .. } => Some(spawn_rect),
        }
    }


    /// Entity/transition-domain coordinates to project for drawing. Runtime
    /// placement overrides the authored `(0,0)` position; the entity is unchanged.
    pub fn display_raw(&self) -> (f64, f64) {
        match self.placement {
            ActorPlacement::Persisted => (self.entity.x as f64, self.entity.y as f64),
            ActorPlacement::Runtime { x, y, .. } => (x as f64, y as f64),
        }
    }

}
