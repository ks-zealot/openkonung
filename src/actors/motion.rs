//! `motion` subsystem of [`crate::actors`] (split from `actors.rs`).

use super::*;

/// Live movement state for one runtime actor (original Entity grid).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ActorMotion {
    /// Current **committed** runtime/Entity cell (`Entity+0x08/+0x0A`). Advanced
    /// at the START of each walk segment (`FUN_0041435C`), so while walking it is
    /// the cell being stepped INTO.
    pub cell: (i32, i32),
    /// Draw-only cell: the pre-step (source) cell of the animating segment
    /// (`Entity+0xE8`). The sprite is drawn here; the anchor sweeps it onto
    /// `cell` over the cycle. Equals `cell` when idle.
    pub render_cell: (i32, i32),
    /// True while a walk segment is animating (the final segment's direction is
    /// consumed at segment START, so `path` is already empty yet the sprite is
    /// still walking).
    pub walking: bool,
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
    /// (`Res::action_cycle_frames` at [`crate::anim::idle_action`]). Retained for
    /// the eventual `flags_a & 0x40 == 0` idle loop; the shipped active
    /// population holds the idle frame instead (see [`update_population`]).
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
    /// Whether this actor belongs to a **citizen** group. The original
    /// per-frame dispatcher `FUN_00421B50` selects the AI by group class
    /// (`Group+0x1E`): `class & 0x80 != 0` (and `& 0x40 == 0`) uses the
    /// waypoint/wander AI `FUN_004168A4`, while `class & 0x40` (object/enemy
    /// groups) uses `FUN_00412E00`, which **idles in place** while the group
    /// activity byte `Group+0x1D == 0`. Non-citizens therefore never wander.
    pub citizen: bool,
    /// Current melee target. `None` for ordinary citizens/wanderers.
    pub combat_target: Option<crate::combat::CombatTarget>,
    /// Attack-cycle length (`Res::action_cycle_frames` for the attack row).
    pub attack_frames: u8,
    /// Death-cycle length (`Res::action_cycle_frames` for the death row).
    pub death_frames: u8,
    /// Death state: the death row has been entered and the final frame is held.
    pub dead: bool,
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


/// Begin the next walk segment exactly like the original `FUN_0041435C`: take
/// the draw cell (`render_cell`) from the CURRENT cell, face and step one
/// collision cell, then reset the walk frame. The stored `cell` therefore leads
/// the drawn sprite by one step.
pub(crate) fn begin_actor_segment(
    m: &mut ActorMotion,
    occupied: &mut HashSet<(i32, i32)>,
    collision: &[u16],
) {
    let Some(&dir) = m.path.first() else {
        return;
    };
    m.render_cell = m.cell;
    m.facing = dir;
    m.frame = 0;
    m.walking = true;
    m.path.remove(0);
    let (nx, ny) = step_by_direction(m.cell.0, m.cell.1, dir);
    if is_free(collision, nx, ny) && !occupied.contains(&(nx, ny)) {
        occupied.remove(&m.cell);
        m.cell = (nx, ny);
        occupied.insert((nx, ny));
    }
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
                build.motion.get(&a.source_entity_id()).is_some_and(|m| m.citizen),
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
            // Only citizen groups (group class `& 0x80`, `& 0x40 == 0`) run the
            // waypoint/wander AI. Object/enemy groups (`class & 0x40`) idle in
            // place (`FUN_00412E00`, `+0x1D == 0`) and must not wander.
            if !citizen
                || flags & FLAG_WANDER == 0
                || arch & 0x80 != 0
                || matches!(kind, 3 | 9)
            {
                continue;
            }
            let battle = crate::anim::battle_from_flags(flags_b);
            let Some(m) = build.motion.get_mut(&eid) else {
                continue;
            };
            let rect = m.rect_a;
            let cell = m.cell;

            // Route issuing runs only while no segment is in flight, so a route
            // is never re-issued mid-step (the committed cell leads the sprite).
            if !m.walking {
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
                    } else {
                        // (Re)issue a route to the home waypoint. A route longer
                        // than `MAX_PATH_STEPS` is re-issued once its prefix is
                        // consumed.
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
                            m.wait = 30 + (build.rng.next_u32() % 60) as u16;
                        }
                    }
                }
            } else {
                m.frame = 0;
            }
        }
    }
}


/// Follow-formation distance threshold (cells): `FUN_0041f3c8` re-targets the
/// leader once `FUN_0043881c(leader, member) >= 10`.
pub const FOLLOW_DISTANCE: i32 = 10;

/// `FUN_0043881c`: grid distance between two runtime cells (Chebyshev, with a
/// `+1` when the smaller axis exceeds `1`).
pub fn follow_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    if dx < dy {
        if dx > 1 {
            dy + 1
        } else {
            dy
        }
    } else if dy > 1 {
        dx + 1
    } else {
        dx
    }
}

/// A free cell in the ring around `leader` (`FUN_0041f3c8` scans a formation
/// offset table for a free anchor next to the group's first entity), else the
/// leader's own cell when free.
fn follow_goal(
    leader: (i32, i32),
    collision: &[u16],
    occupied: &HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    const RING: [(i32, i32); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];
    for &(dx, dy) in &RING {
        let c = (leader.0 + dx, leader.1 + dy);
        if is_free(collision, c.0, c.1) && !occupied.contains(&c) {
            return Some(c);
        }
    }
    is_free(collision, leader.0, leader.1).then_some(leader)
}

/// Advance recruited companions toward the leader, one collision cell per
/// completed walk cycle (original party **group 0**, class `0x20` -> the
/// `FUN_00421b50` inline AI -> `FUN_0041ecc8` -> `FUN_0041f3c8`). A companion
/// re-targets a free cell beside the leader only when it is at least
/// [`FOLLOW_DISTANCE`] cells away; it then walks there and holds position.
pub fn update_followers(
    members: &mut [PartyMember],
    collision: &[u16],
    leader: (i32, i32),
    ticks: i64,
) {
    if ticks <= 0 || members.is_empty() {
        return;
    }
    let ticks = ticks.min(10);
    let mut occupied: HashSet<(i32, i32)> = members
        .iter()
        .filter(|m| !m.motion.dead)
        .map(|m| m.motion.cell)
        .collect();
    occupied.insert(leader);
    for _ in 0..ticks {
        for m in members.iter_mut() {
            if m.motion.dead || m.motion.combat_target.is_some() {
                continue;
            }
            let battle = crate::anim::battle_from_flags(m.entity.flags_b);
            if !m.motion.walking {
                if follow_distance(m.motion.cell, leader) >= FOLLOW_DISTANCE {
                    if let Some(goal) = follow_goal(leader, collision, &occupied) {
                        if goal != m.motion.cell {
                            let bounds = (0, 0, RUNTIME_W - 1, RUNTIME_H - 1);
                            if let Some(p) =
                                find_path(collision, &occupied, m.motion.cell, goal, bounds)
                            {
                                if !p.is_empty() {
                                    m.motion.path = p;
                                    m.motion.target = Some(goal);
                                }
                            }
                        }
                    }
                } else {
                    m.motion.path.clear();
                    m.motion.target = None;
                }
                if !m.motion.path.is_empty() {
                    begin_actor_segment(&mut m.motion, &mut occupied, collision);
                }
            }
            let next_action = if m.motion.walking {
                crate::anim::walk_action(battle) as u8
            } else {
                crate::anim::idle_action(battle) as u8
            };
            if m.motion.action != next_action {
                m.motion.action = next_action;
                m.motion.frame = 0;
            }
            if m.motion.walking {
                let wf = if m.motion.walk_frames == 0 {
                    FALLBACK_WALK_FRAMES
                } else {
                    m.motion.walk_frames
                };
                m.motion.frame = m.motion.frame.saturating_add(1);
                if m.motion.frame >= wf {
                    m.motion.frame = 0;
                    if !m.motion.path.is_empty() {
                        begin_actor_segment(&mut m.motion, &mut occupied, collision);
                    } else {
                        m.motion.walking = false;
                        m.motion.render_cell = m.motion.cell;
                        m.motion.target = None;
                    }
                }
            } else {
                m.motion.frame = 0;
            }
        }
    }
}


/// Issue a move command to one companion toward `goal`, mirroring the original
/// left-click command 0 (`FUN_0042260C`), which writes the clicked cell to
/// `Entity+0x0C/+0x0E` on **every** unit in the selection array `0x81A814`.
///
/// The destination is stored even when the route fails (`FUN_004142D4` records
/// the target before pathfinding and simply does not move); on success the first
/// step is committed immediately (`FUN_0041435C`) so the companion starts
/// walking and the straggler AI does not clear the commanded route.
pub fn command_follower(
    member: &mut PartyMember,
    collision: &[u16],
    occupied: &mut HashSet<(i32, i32)>,
    goal: (i32, i32),
) {
    member.motion.target = Some(goal);
    if goal == member.motion.cell {
        member.motion.path.clear();
        member.motion.walking = false;
        member.motion.render_cell = member.motion.cell;
        return;
    }
    let bounds = (0, 0, RUNTIME_W - 1, RUNTIME_H - 1);
    match find_path(collision, occupied, member.motion.cell, goal, bounds) {
        Some(p) if !p.is_empty() => {
            member.motion.path = p;
            begin_actor_segment(&mut member.motion, occupied, collision);
        }
        _ => {
            // Destination recorded but unreachable/blocked: no walk (original
            // keeps the stored target and does not start moving).
            member.motion.path.clear();
            member.motion.walking = false;
            member.motion.render_cell = member.motion.cell;
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
