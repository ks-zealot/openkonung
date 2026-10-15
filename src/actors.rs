//! Phase C1/C2 runtime population layer.
//!
//! Builds renderable actors from the Phase B [`crate::population`] Group/Entity
//! tables. It performs **no** population growth, combat or dialogue. Movement
//! and animation **state** (action/frame) advance for wander-eligible actors via
//! [`update_population`]; sprite decoding is done by the render layer.
//!
//! Source selection (`KONUNG_SOURCE_OF_TRUTH.md` §4.1/§4.2):
//! - group `location_id (+0x1B) == current_location_id`;
//! - reserved group 0 is skipped (never an ordinary runtime actor);
//! - `Group +0x1E != 0` (active class) is required;
//! - exactly `first_entity .. first_entity + entity_count (+0x1C)` is
//!   enumerated — never `capacity (+0x19)`, never one actor per group.
//!
//! C1 (persisted): entities with `x != 0 || y != 0` are rendered at their saved
//! coordinates. C2 (runtime): entities with `x == 0 && y == 0` are assigned a
//! position from the group's first spawn rectangle (A). The parsed `Entity`
//! records are never mutated.
//!
//! Coordinate domain (`KONUNG_SOURCE_OF_TRUTH.md` §1.1/§1.2/§15): persisted and
//! runtime positions are the 320×160 runtime/Entity grid (the same grid the
//! collision table indexes as `x*160 + y`) and are projected at draw time by
//! `coords::entity_to_local_px` — the same helper the protagonist uses. They are
//! never clamped to the rendered terrain surface.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;

use crate::coords::{entity_to_local_px, neighbors, step_by_direction};
use crate::population::{Entity, GameTables, Group, Rect, Settlement, Waypoint};

/// Maximum path length in cells. The original stores the direction queue in the
/// 32-byte `Entity+0x13` field (terminator at `+0x32`), so at most 31 steps are
/// kept (`FUN_0043DA1F`); longer routes are re-issued from the new position.
pub const MAX_PATH_STEPS: usize = 0x1f;

/// Maximum placement attempts in the original routine.
pub const MAX_ATTEMPTS: u8 = 100;

/// Original movement is discrete: one collision cell per completed walk-animation
/// cycle (`FUN_0041435c` state 1/7 + `FUN_00413de4`). The world logic runs on the
/// original fixed frame budget: `timeGetTime()` deltas are compared against a
/// **60 ms** period (`konung.res` `0x42b32f` `MOV dword ptr [0x81d5c8],0x3c`;
/// the limiter compare is at `0x42b61a`), so the game advances one animation /
/// logic frame per 60 ms (~16.67 Hz). At default game speed `[0x81d634]` (0) the
/// entity update runs once per frame and the tick counter `_DAT_0081d640`
/// increments once per frame (`0x4366e5`); the periodic population pass keys off
/// `& 0xf` (~0.96 s per 16 frames), corroborating the ~16.7 Hz rate. A walk cycle
/// of `n` frames therefore takes `n * 60 ms` (e.g. 12 frames -> 0.72 s/cell).
pub const ORIGINAL_FRAME_MS: f64 = 60.0;
pub const LOGIC_TICK_HZ: f64 = 1000.0 / ORIGINAL_FRAME_MS;

/// Fallback walk-cycle length (frames) used when an actor's own sprite cycle
/// cannot be resolved (`Res::action_cycle_frames`). 8 frames at ~16.67 Hz
/// ~= 2.1 cells/s.
pub const FALLBACK_WALK_FRAMES: u8 = 8;

/// Wander search radius used by `FUN_00413a84` (`_DAT_0081d6e0`/`_DAT_0081d6a4`).
/// Those are runtime `.bss` values, not statically resolvable here; the accepted
/// candidate must also lie strictly inside the group rectangle, so these only
/// bound the random offset.
pub const WANDER_RADIUS_X: i32 = 8;
pub const WANDER_RADIUS_Y: i32 = 4;

/// `FUN_00413a84` retry count.
pub const WANDER_MAX_ATTEMPTS: u8 = 10;

/// Runtime collision-grid bounds (`KONUNG_SOURCE_OF_TRUTH.md` §3).
pub const RUNTIME_W: i32 = 320;
pub const RUNTIME_H: i32 = 160;

/// `Entity+0x02` bit set on wandering citizens (`FUN_004168a4` dispatch gate).
pub const FLAG_WANDER: u8 = 0x40;

/// Graphics family selected by `archetype & 0x40` (`KONUNG_SOURCE_OF_TRUTH.md`
/// §5.1) — never by `kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualFamily {
    Human,
    Object,
}

/// Graphics family for an entity archetype.
pub fn visual_family(archetype: u8) -> VisualFamily {
    if archetype & 0x40 != 0 {
        VisualFamily::Object
    } else {
        VisualFamily::Human
    }
}

/// A generated candidate in the Group spawn-rectangle domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawSpawnCoord {
    pub x: i32,
    pub y: i32,
}

/// Runtime placement failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    NoCandidateAfterAttempts { attempts: u8 },
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementError::NoCandidateAfterAttempts { attempts } => {
                write!(f, "placement_failed_after_{attempts}_attempts")
            }
        }
    }
}

impl std::error::Error for PlacementError {}

/// Deterministic RNG (xorshift64). No thread/browser randomness is used, so
/// placement is reproducible for a given location.
#[derive(Clone, Debug)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64) -> Self {
        DeterministicRng { state: seed | 1 }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }
}

impl Default for DeterministicRng {
    fn default() -> Self {
        DeterministicRng { state: 1 }
    }
}

/// One deterministic RNG stream per location/session.
pub fn location_rng(loc_id: u8) -> DeterministicRng {
    DeterministicRng::new(0x9E37_79B9_7F4A_7C15 ^ (loc_id as u64).wrapping_mul(0x1000_0000_01B3))
}

/// Center/half-extents of spawn rectangle A (`KONUNG_SOURCE_OF_TRUTH.md` §6.2),
/// using signed truncation:
/// `half=(x2-x1)/2`, `center=(x1+x2+1)/2`.
fn spawn_center_half(rect: &Rect) -> (i32, i32, i32, i32) {
    let x1 = rect.x1 as i32;
    let x2 = rect.x2 as i32;
    let y1 = rect.y1 as i32;
    let y2 = rect.y2 as i32;
    (
        (x1 + x2 + 1) / 2,
        (y1 + y2 + 1) / 2,
        (x2 - x1) / 2,
        (y2 - y1) / 2,
    )
}

fn axis_delta(half: i32, rng: &mut DeterministicRng) -> i32 {
    if half < 2 {
        return 0;
    }
    let magnitude = (rng.next_u32() % half as u32) as i32;
    if rng.next_u32() & 1 == 1 {
        -magnitude
    } else {
        magnitude
    }
}

/// Generate one candidate from the group's first spawn rectangle (A), i.e.
/// `+08/+0C/+10/+14`. Rectangle B (`+0A/+0E/+12/+16`) is **not** used.
pub fn generate_spawn_candidate(group: &Group, rng: &mut DeterministicRng) -> RawSpawnCoord {
    let (cx, cy, hw, hh) = spawn_center_half(&group.rect_a);
    RawSpawnCoord {
        x: cx + axis_delta(hw, rng),
        y: cy + axis_delta(hh, rng),
    }
}

/// Original placement acceptance test (`KONUNG_SOURCE_OF_TRUTH.md` §3/§16/§23.4):
/// the candidate is a runtime/entity/collision coordinate; it is accepted only
/// when in bounds and the collision cell's low 12 bits are clear.
pub fn validate_spawn_candidate(x: i32, y: i32, collision: &[u16]) -> bool {
    if x < 0 || x >= 320 || y < 0 || y >= 160 {
        return false;
    }
    let idx = x as usize * 160 + y as usize;
    idx < collision.len() && (collision[idx] & 0x0FFF) == 0
}

/// Placement loop with an injected acceptance predicate. Keeps the original
/// maximum of 100 attempts and never falls back to the rectangle center.
pub fn place_with<F: FnMut(RawSpawnCoord) -> bool>(
    group: &Group,
    rng: &mut DeterministicRng,
    mut valid: F,
) -> Result<(RawSpawnCoord, u8), PlacementError> {
    for attempt in 1..=MAX_ATTEMPTS {
        let candidate = generate_spawn_candidate(group, rng);
        if valid(candidate) {
            return Ok((candidate, attempt));
        }
    }
    Err(PlacementError::NoCandidateAfterAttempts {
        attempts: MAX_ATTEMPTS,
    })
}

/// Place one entity using the original collision acceptance test.
pub fn place_entity(
    group: &Group,
    rng: &mut DeterministicRng,
    collision: &[u16],
) -> Result<(RawSpawnCoord, u8), PlacementError> {
    place_with(group, rng, |raw| {
        validate_spawn_candidate(raw.x, raw.y, collision)
    })
}

// ---------------------------------------------------------------------------
// Citizen AI / movement (`FUN_004168a4`, `FUN_00413a84`, `FUN_004142d4`, ...)
// ---------------------------------------------------------------------------

/// True when this entity is a wandering civilian, mirroring the original
/// dispatch: `flags_a(+0x02) & 0x40` set, `archetype & 0x80 == 0`, kind not
/// `3`/`9` (`FUN_00415fc8` -> `FUN_004168a4`).
///
/// Movement additionally honors settlement ownership: an entity that owns a
/// home waypoint (`FUN_0041182C`, `waypoint.owner == Entity index`) walks to
/// that waypoint and then stays put, so it is wander-eligible by flag but
/// anchored in practice ([`update_population`]).
pub fn wanders(e: &Entity) -> bool {
    e.flags_a & FLAG_WANDER != 0 && e.archetype & 0x80 == 0 && !matches!(e.kind, 3 | 9)
}

/// Runtime collision-grid passability: in bounds and low-12 collision bits clear.
pub fn is_free(collision: &[u16], x: i32, y: i32) -> bool {
    if x < 0 || x >= RUNTIME_W || y < 0 || y >= RUNTIME_H {
        return false;
    }
    let i = x as usize * 160 + y as usize;
    i < collision.len() && (collision[i] & 0x0FFF) == 0
}

/// Inclusive rectangle containment in the runtime/Entity grid. Used by tests;
/// the production wander path relies on `wander_target`'s strict-interior rule.
#[cfg(test)]
fn rect_contains(rect: &Rect, c: (i32, i32)) -> bool {
    let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
    let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
    c.0 >= x1 && c.0 <= x2 && c.1 >= y1 && c.1 <= y2
}

/// Port of `FUN_00413a84`: pick a random collision-free point strictly inside the
/// group's spawn rectangle A, near the current position (or the rectangle centre
/// when the actor is outside it). Up to [`WANDER_MAX_ATTEMPTS`] tries; `None` on
/// failure (the original never falls back to a fixed point).
pub fn wander_target(
    rect: &Rect,
    from: (i32, i32),
    rng: &mut DeterministicRng,
    collision: &[u16],
) -> Option<(i32, i32)> {
    let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
    let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
    // Base = current cell when inside rect A, else rect A centre (`FUN_00413a84`).
    let outside = from.0 < x1 || x2 < from.0 || from.1 < y1 || y2 < from.1;
    let (bx, by) = if outside {
        ((x1 + x2) / 2, (y1 + y2) / 2)
    } else {
        (from.0, from.1)
    };
    for _ in 0..WANDER_MAX_ATTEMPTS {
        let dir = rng.next_u32() % 4;
        let ox = if WANDER_RADIUS_X < 2 {
            0
        } else {
            (rng.next_u32() % (WANDER_RADIUS_X as u32 * 2)) as i32
        };
        let oy = if WANDER_RADIUS_Y < 2 {
            0
        } else {
            (rng.next_u32() % (WANDER_RADIUS_Y as u32 * 2)) as i32
        };
        let (tx, ty) = match dir {
            0 => (bx - ox, by - oy),
            1 => (bx - ox, by + oy),
            2 => (bx + ox, by - oy),
            _ => (bx + ox, by + oy),
        };
        if x1 < tx && tx < x2 && y1 < ty && ty < y2 && is_free(collision, tx, ty) {
            return Some((tx, ty));
        }
    }
    None
}

/// Original route builder (`FUN_0043DA1F`): a weighted search over the staggered
/// movement grid that returns the list of direction codes (0..=7) to walk from
/// `start` to `goal`, or `None` when the goal is blocked ("not an acceptable
/// point") or unreachable.
///
/// Neighbours are the parity-selected [`crate::coords::neighbors`] of each cell
/// (`FUN_0043D922`). `occupied` blocks actor cells; the goal itself is allowed
/// even when occupied. Edge weights match the original: dirs 1/3/5/7 cost 10,
/// dirs 0/2/4/6 cost 14. The result is capped at [`MAX_PATH_STEPS`].
pub fn find_path(
    collision: &[u16],
    occupied: &HashSet<(i32, i32)>,
    start: (i32, i32),
    goal: (i32, i32),
    bounds: (i32, i32, i32, i32),
) -> Option<Vec<u8>> {
    let (x0, y0, x1, y1) = bounds;
    let w = (x1 - x0 + 1).max(1) as usize;
    let h = (y1 - y0 + 1).max(1) as usize;
    if w * h > 320 * 160 {
        return None;
    }
    let in_bounds = |x: i32, y: i32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    if !in_bounds(start.0, start.1) || !in_bounds(goal.0, goal.1) {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }
    // The destination must be an "acceptable" (passable) point; the original
    // aborts the command otherwise.
    if !is_free(collision, goal.0, goal.1) {
        return None;
    }
    let idx = |x: i32, y: i32| ((y - y0) as usize) * w + ((x - x0) as usize);
    let si = idx(start.0, start.1);
    let gi = idx(goal.0, goal.1);
    let mut dist = vec![u32::MAX; w * h];
    let mut prev = vec![usize::MAX; w * h];
    let mut pdir = vec![0u8; w * h];
    dist[si] = 0;
    let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
    heap.push(Reverse((0, si)));
    while let Some(Reverse((d, u))) = heap.pop() {
        if d > dist[u] {
            continue;
        }
        if u == gi {
            break;
        }
        let cx = x0 + (u % w) as i32;
        let cy = y0 + (u / w) as i32;
        for (dir, nx, ny) in neighbors(cx, cy) {
            if !in_bounds(nx, ny) || !is_free(collision, nx, ny) {
                continue;
            }
            let ni = idx(nx, ny);
            if ni != gi && occupied.contains(&(nx, ny)) {
                continue;
            }
            let nd = d + crate::coords::direction_cost(dir);
            if nd < dist[ni] {
                dist[ni] = nd;
                prev[ni] = u;
                pdir[ni] = dir;
                heap.push(Reverse((nd, ni)));
            }
        }
    }
    if dist[gi] == u32::MAX {
        return None;
    }
    let mut dirs = Vec::new();
    let mut c = gi;
    while c != si {
        dirs.push(pdir[c]);
        c = prev[c];
        if c == usize::MAX {
            return None;
        }
    }
    dirs.reverse();
    dirs.truncate(MAX_PATH_STEPS);
    Some(dirs)
}

/// Bounds covering a rectangle plus start/goal, clamped to the runtime grid.
fn path_bounds(rect: &Rect, a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    let xs = [rect.x1 as i32, rect.x2 as i32, a.0, b.0];
    let ys = [rect.y1 as i32, rect.y2 as i32, a.1, b.1];
    let x0 = xs.iter().copied().min().unwrap_or(0) - 2;
    let x1 = xs.iter().copied().max().unwrap_or(0) + 2;
    let y0 = ys.iter().copied().min().unwrap_or(0) - 2;
    let y1 = ys.iter().copied().max().unwrap_or(0) + 2;
    (x0.max(0), y0.max(0), x1.min(RUNTIME_W - 1), y1.min(RUNTIME_H - 1))
}

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

/// A `(0,0)` entity that could not be placed after [`MAX_ATTEMPTS`] attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailedPlacement {
    pub source_group_id: usize,
    pub source_entity_id: usize,
    pub spawn_rect: Rect,
    pub attempts: u8,
}

/// Result of building one location's population.
#[derive(Clone, Debug, Default)]
pub struct PopulationBuild {
    /// Persisted and successfully runtime-positioned actors, unsorted.
    pub actors: Vec<RuntimeActor>,
    /// `(0,0)` entities that failed placement.
    pub failed: Vec<FailedPlacement>,
    /// Every group whose `location_id` matches, including group 0 / inactive.
    pub matching_groups: usize,
    pub reserved_group_zero: usize,
    pub inactive_groups: usize,
    pub invalid_ranges: usize,
    /// Entities iterated in valid `first_entity..+entity_count` ranges.
    pub enumerated_entities: usize,
    /// `(0,0)` records attempted (== placed + failed).
    pub runtime_candidates: usize,
    pub runtime_placed: usize,
    pub runtime_failed: usize,
    /// Live movement state keyed by source Entity id.
    pub motion: HashMap<usize, ActorMotion>,
    /// Per-location deterministic RNG stream (persists across frames).
    pub rng: DeterministicRng,
    /// Sub-tick accumulator for the [`LOGIC_TICK_HZ`] fixed step.
    tick_accum: f64,
}

impl PopulationBuild {
    pub fn persisted_position_actors(&self) -> usize {
        self.actors.iter().filter(|a| !a.is_runtime()).count()
    }

    pub fn runtime_positioned(&self) -> usize {
        self.actors.iter().filter(|a| a.is_runtime()).count()
    }
}

/// Select, and (for `(0,0)` records) place, the current location's population.
/// Pure: `tables` is never mutated. Deterministic per location. Runtime
/// placement validates against the location's collision table (source §16).
pub fn build_population(tables: &GameTables, loc_id: u8, collision: &[u16]) -> PopulationBuild {
    let mut rng = location_rng(loc_id);
    let mut out = PopulationBuild::default();
    out.rng = location_rng(loc_id);

    for g in tables.groups.iter().filter(|g| g.loc_id == loc_id) {
        out.matching_groups += 1;
        if g.index == 0 {
            out.reserved_group_zero += 1;
            continue;
        }
        if !g.is_active_class() {
            out.inactive_groups += 1;
            continue;
        }
        let range = match g.entity_range() {
            Ok(r) => r,
            Err(_) => {
                out.invalid_ranges += 1;
                continue;
            }
        };
        for e in &tables.entities[range] {
            out.enumerated_entities += 1;
            let persisted = e.x != 0 || e.y != 0;
            if persisted {
                out.actors.push(RuntimeActor {
                    entity: e.clone(),
                    source_group_id: g.index,
                    placement: ActorPlacement::Persisted,
                });
                out.motion.insert(
                    e.index,
                    ActorMotion {
                        cell: (e.x as i32, e.y as i32),
                        facing: e.facing,
                        rect_a: g.rect_a,
                        ..Default::default()
                    },
                );
            } else {
                out.runtime_candidates += 1;
                match place_entity(g, &mut rng, collision) {
                    Ok((raw, attempts)) => {
                        out.runtime_placed += 1;
                        out.actors.push(RuntimeActor {
                            entity: e.clone(),
                            source_group_id: g.index,
                            placement: ActorPlacement::Runtime {
                                x: raw.x,
                                y: raw.y,
                                attempts,
                                spawn_rect: g.rect_a,
                            },
                        });
                        out.motion.insert(
                            e.index,
                            ActorMotion {
                                cell: (raw.x, raw.y),
                                facing: e.facing,
                                rect_a: g.rect_a,
                                ..Default::default()
                            },
                        );
                    }
                    Err(_) => {
                        out.runtime_failed += 1;
                        out.failed.push(FailedPlacement {
                            source_group_id: g.index,
                            source_entity_id: e.index,
                            spawn_rect: g.rect_a,
                            attempts: MAX_ATTEMPTS,
                        });
                    }
                }
            }
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::{Entity, EntityStats, Group};

    fn group(index: usize, first: u16, count: u8, loc: u8, class: u8, rect_a: Rect) -> Group {
        Group {
            index,
            first_entity: first,
            rect_a,
            rect_b: Rect::default(),
            count_18: count,
            capacity: count,
            loc_id: loc,
            entity_count: count,
            class_1e: class,
        }
    }

    fn entity(index: usize, kind: u8, archetype: u8, x: u16, y: u16) -> Entity {
        Entity {
            index,
            kind,
            facing: 3,
            flags_a: 0,
            flags_b: 0,
            archetype,
            group_id: 0,
            raw_0x12: 0,
            x,
            y,
            stats: EntityStats::default(),
            equipment: [0; 6],
            inventory: [0; 32],
            dialogue_id: 255,
            name_id: 0,
            nickname_id: 0,
            visual_index: 0,
            palette_offset: 0,
            additional_equipment: [0; 5],
            special_equipment: 0,
        }
    }

    fn tables(groups: Vec<Group>, entities: Vec<Entity>) -> GameTables {
        GameTables { groups, entities }
    }

    fn free_collision() -> Vec<u16> {
        vec![0u16; 320 * 160]
    }

    #[test]
    fn spawn_center_half_matches_original_formula() {
        // Real g27 rect A.
        let g = group(1, 0, 0, 1, 65, Rect { x1: 36, x2: 136, y1: 24, y2: 50 });
        assert_eq!(spawn_center_half(&g.rect_a), (86, 37, 50, 13));
        // Degenerate rect: half < 2 => candidate is exactly the center.
        let g2 = group(1, 0, 0, 1, 65, Rect { x1: 134, x2: 134, y1: 21, y2: 21 });
        assert_eq!(spawn_center_half(&g2.rect_a), (134, 21, 0, 0));
        let mut rng = location_rng(1);
        assert_eq!(generate_spawn_candidate(&g2, &mut rng), RawSpawnCoord { x: 134, y: 21 });
    }

    #[test]
    fn candidates_stay_within_center_and_half() {
        let g = group(1, 0, 0, 1, 65, Rect { x1: 36, x2: 136, y1: 24, y2: 50 });
        let (cx, cy, hw, hh) = spawn_center_half(&g.rect_a);
        let mut rng = location_rng(7);
        for _ in 0..1000 {
            let c = generate_spawn_candidate(&g, &mut rng);
            assert!(c.x >= cx - hw && c.x <= cx + hw, "x {} out of range", c.x);
            assert!(c.y >= cy - hh && c.y <= cy + hh, "y {} out of range", c.y);
        }
    }

    #[test]
    fn collision_validator_accepts_first_free_candidate() {
        let g = group(1, 0, 0, 1, 65, Rect { x1: 10, x2: 30, y1: 4, y2: 8 });
        let free = free_collision();
        let mut rng = location_rng(1);
        let (raw, attempts) = place_entity(&g, &mut rng, &free).unwrap();
        assert_eq!(attempts, 1);
        assert_eq!(raw, generate_spawn_candidate(&g, &mut location_rng(1)));
    }

    #[test]
    fn collision_validator_bounds_and_index() {
        let mut c = free_collision();
        // x=268 is a valid runtime X (source §3/§23.4).
        assert!(validate_spawn_candidate(268, 5, &c));
        assert!(validate_spawn_candidate(319, 159, &c));
        assert!(!validate_spawn_candidate(320, 0, &c));
        assert!(!validate_spawn_candidate(0, 160, &c));
        assert!(!validate_spawn_candidate(-1, 0, &c));
        assert!(!validate_spawn_candidate(0, -1, &c));
        // index = x*160 + y.
        c[268 * 160 + 5] = 0x0001; // low 12 bits non-zero => blocked
        assert!(!validate_spawn_candidate(268, 5, &c));
        c[268 * 160 + 5] = 0x1000; // high bits only => free
        assert!(validate_spawn_candidate(268, 5, &c));
        c[319 * 160 + 159] = 0xFF;
        assert!(!validate_spawn_candidate(319, 159, &c));
    }

    #[test]
    fn placement_fails_when_rect_area_is_blocked() {
        let g = group(1, 0, 0, 1, 65, Rect { x1: 0, x2: 40, y1: 0, y2: 40 });
        let mut blocked = free_collision();
        for cell in blocked.iter_mut() {
            *cell = 0x0001;
        }
        let mut rng = location_rng(3);
        assert!(place_entity(&g, &mut rng, &blocked).is_err());
    }

    #[test]
    fn place_with_fails_after_exactly_100_attempts() {
        let g = group(1, 0, 0, 1, 65, Rect { x1: 0, x2: 40, y1: 0, y2: 40 });
        let mut rng = location_rng(3);
        let err = place_with(&g, &mut rng, |_| false).unwrap_err();
        assert_eq!(err, PlacementError::NoCandidateAfterAttempts { attempts: 100 });
        assert_eq!(err.to_string(), "placement_failed_after_100_attempts");
    }

    #[test]
    fn skips_group_zero_and_inactive_groups() {
        let groups = vec![
            group(0, 0, 1, 1, 32, Rect::default()), // reserved
            group(1, 1, 1, 1, 0, Rect::default()),  // inactive
            group(2, 2, 1, 1, 65, Rect::default()), // active
        ];
        let entities = vec![
            entity(0, 6, 0x00, 100, 60), // player start: never an actor
            entity(1, 6, 0x00, 5, 6),
            entity(2, 6, 0x00, 7, 8),
        ];
        let t = tables(groups, entities);
        let b = build_population(&t, 1, &free_collision());
        assert_eq!(b.matching_groups, 3);
        assert_eq!(b.reserved_group_zero, 1);
        assert_eq!(b.inactive_groups, 1);
        assert_eq!(b.enumerated_entities, 1);
        assert_eq!(b.actors.len(), 1);
        assert_eq!(b.actors[0].source_entity_id(), 2);
    }

    #[test]
    fn enumerates_first_plus_count_not_capacity_nor_one_per_group() {
        let mut g = group(1, 10, 3, 1, 65, Rect { x1: 0, x2: 8, y1: 0, y2: 8 });
        g.capacity = 99;
        g.count_18 = 99;
        let mut entities: Vec<Entity> = (0..14).map(|i| entity(i, 0, 0, 1, 1)).collect();
        entities[13] = entity(13, 0, 0, 4, 4);
        let t = tables(vec![g], entities);
        let b = build_population(&t, 1, &free_collision());
        assert_eq!(b.enumerated_entities, 3);
        assert_eq!(b.persisted_position_actors(), 3);
        let ids: Vec<usize> = b.actors.iter().map(|a| a.source_entity_id()).collect();
        assert_eq!(ids, vec![10, 11, 12]);
    }

    #[test]
    fn zero_position_is_runtime_placed_and_source_unchanged() {
        let rect = Rect { x1: 10, x2: 30, y1: 4, y2: 8 };
        let groups = vec![group(1, 0, 2, 1, 65, rect)];
        let entities = vec![entity(0, 0, 0x4F, 0, 0), entity(1, 0, 0x4F, 0, 0)];
        let t = tables(groups, entities);

        let b = build_population(&t, 1, &free_collision());
        assert_eq!(b.runtime_candidates, 2);
        assert_eq!(b.runtime_placed, 2);
        assert_eq!(b.runtime_failed, 0);
        assert!(b.failed.is_empty());
        assert_eq!(b.enumerated_entities, 2);
        assert_eq!(b.runtime_positioned(), 2);

        for a in &b.actors {
            assert!(a.is_runtime());
            assert_eq!(a.attempts(), Some(1));
            // Original parsed fields are preserved untouched.
            assert_eq!((a.entity.x, a.entity.y), (0, 0));
            let (rx, ry) = a.display_raw();
            let (cx, cy, hw, hh) = (20, 6, 10, 2);
            assert!(rx >= (cx - hw) as f64 && rx <= (cx + hw) as f64);
            assert!(ry >= (cy - hh) as f64 && ry <= (cy + hh) as f64);
        }
    }

    #[test]
    fn deterministic_repeatability_per_location() {
        let rect = Rect { x1: 10, x2: 60, y1: 5, y2: 25 };
        let groups = vec![group(1, 0, 6, 1, 65, rect)];
        let entities: Vec<Entity> = (0..6).map(|i| entity(i, 0, 0x4F, 0, 0)).collect();
        let t = tables(groups, entities);

        let a = build_population(&t, 1, &free_collision());
        let b = build_population(&t, 1, &free_collision());
        let coords = |p: &PopulationBuild| {
            p.actors.iter().map(|x| x.display_raw()).collect::<Vec<_>>()
        };
        assert_eq!(coords(&a), coords(&b));
    }

    #[test]
    fn family_is_selected_by_archetype_bit_not_kind() {
        assert_eq!(visual_family(0x52), VisualFamily::Object);
        assert_eq!(visual_family(0x01), VisualFamily::Human);
        assert_eq!(visual_family(0x40), VisualFamily::Object);
        assert_eq!(visual_family(0x3F), VisualFamily::Human);
    }

    #[test]
    fn projected_sort_key_uses_entity_projection() {
        let a = RuntimeActor {
            entity: entity(1, 0, 0, 5, 10),
            source_group_id: 1,
            placement: ActorPlacement::Persisted,
        };
        let b = RuntimeActor {
            entity: entity(1, 0, 0, 200, 1),
            ..a.clone()
        };
        // local_x = y*58 + (2-(x&1))*29, local_y = x*16+16.
        assert_eq!(
            projected_sort_key(&a),
            (a.entity.x as i64 * 16 + 16, 10 * 58 + 29)
        );
        assert_eq!(
            projected_sort_key(&b),
            (b.entity.x as i64 * 16 + 16, 1 * 58 + 58)
        );

        let mut actors = vec![b.clone(), a.clone()];
        sort_actors(&mut actors);
        assert_eq!(actors[0].entity.x, 5);
        assert_eq!(actors[1].entity.x, 200);
    }

    #[test]
    fn invalid_range_is_counted_not_silently_dropped() {
        let groups = vec![group(1, 1999, 5, 1, 65, Rect::default())];
        let entities: Vec<Entity> = (0..2000).map(|i| entity(i, 0, 0, 0, 0)).collect();
        let t = tables(groups, entities);
        let b = build_population(&t, 1, &free_collision());
        assert_eq!(b.invalid_ranges, 1);
        assert_eq!(b.enumerated_entities, 0);
    }

    #[test]
    fn wanders_classification_matches_dispatch_gate() {
        let mut e = entity(0, 6, 0x05, 10, 10);
        e.flags_a = FLAG_WANDER;
        assert!(wanders(&e));
        e.flags_a = 0x00;
        assert!(!wanders(&e));
        e.flags_a = FLAG_WANDER;
        e.archetype = 0x85; // bit 0x80 set -> not civilian AI
        assert!(!wanders(&e));
        e.archetype = 0x05;
        e.kind = 3;
        assert!(!wanders(&e));
        e.kind = 6;
        assert!(wanders(&e));
    }

    #[test]
    fn wander_target_stays_strictly_inside_rect_and_free() {
        let rect = Rect { x1: 10, x2: 60, y1: 5, y2: 25 };
        let free = free_collision();
        let mut rng = location_rng(1);
        for _ in 0..500 {
            let t = wander_target(&rect, (30, 15), &mut rng, &free).expect("candidate");
            assert!(t.0 > rect.x1 as i32 && t.0 < rect.x2 as i32, "x {t:?}");
            assert!(t.1 > rect.y1 as i32 && t.1 < rect.y2 as i32, "y {t:?}");
            assert!(is_free(&free, t.0, t.1));
        }
    }

    #[test]
    fn wander_target_none_when_rect_blocked() {
        let rect = Rect { x1: 0, x2: 40, y1: 0, y2: 40 };
        let mut blocked = free_collision();
        for c in blocked.iter_mut() {
            *c = 0x0001;
        }
        let mut rng = location_rng(3);
        assert!(wander_target(&rect, (20, 20), &mut rng, &blocked).is_none());
    }

    #[test]
    fn find_path_routes_around_obstacles() {
        let mut c = free_collision();
        // Vertical wall at x=5 for y 0..=9, gap at y>=10.
        for y in 0..10 {
            c[5 * 160 + y] = 0x0001;
        }
        let occ = std::collections::HashSet::new();
        let start = (0, 0);
        let goal = (10, 0);
        let dirs = find_path(&c, &occ, start, goal, (0, 0, 20, 20)).expect("path");
        assert!(!dirs.is_empty());
        // Replay the direction codes and verify the goal is reached without
        // crossing the wall or stepping onto a blocked cell.
        let mut cell = start;
        for &d in &dirs {
            let next = step_by_direction(cell.0, cell.1, d);
            assert!(!(next.0 == 5 && next.1 < 10), "path crosses the wall at {next:?}");
            assert!(is_free(&c, next.0, next.1), "path steps onto blocked {next:?}");
            cell = next;
        }
        assert_eq!(cell, goal);
    }

    #[test]
    fn find_path_rejects_blocked_goal() {
        // The destination must be an "acceptable" (passable) point; the original
        // aborts the command for a blocked click.
        let mut c = free_collision();
        c[10 * 160 + 0] = 0x0001;
        let occ = std::collections::HashSet::new();
        assert!(find_path(&c, &occ, (0, 0), (10, 0), (0, 0, 20, 20)).is_none());
    }

    #[test]
    fn dup_column_and_row_walks_keep_constant_direction() {
        let c = free_collision();
        let occ = std::collections::HashSet::new();
        // Column move (vary x) is a straight screen line: one constant code.
        let col = find_path(&c, &occ, (0, 0), (6, 0), (0, 0, 20, 20)).expect("col");
        assert!(!col.is_empty());
        assert!(col.iter().all(|&d| d == col[0]), "column zigzags: {col:?}");
        // Row move (vary y) likewise.
        let row = find_path(&c, &occ, (0, 0), (0, 4), (0, 0, 20, 20)).expect("row");
        assert!(!row.is_empty());
        assert!(row.iter().all(|&d| d == row[0]), "row zigzags: {row:?}");
    }

    #[test]
    fn update_population_moves_only_wanderers() {
        let rect = Rect { x1: 10, x2: 60, y1: 5, y2: 25 };
        let groups = vec![group(1, 0, 2, 1, 0x81, rect)];
        let mut citizen = entity(0, 6, 0x05, 30, 15);
        citizen.flags_a = FLAG_WANDER;
        let mob = entity(1, 6, 0x05, 40, 15); // flags_a 0 -> stays put
        let t = tables(groups, vec![citizen, mob]);
        let collision = free_collision();
        let mut b = build_population(&t, 1, &collision);

        let start_c = b.motion[&0].cell;
        let start_m = b.motion[&1].cell;
        for _ in 0..600 {
            update_population(&mut b, &collision, None, 1.0 / LOGIC_TICK_HZ);
        }
        assert_ne!(b.motion[&0].cell, start_c, "citizen must wander");
        assert_eq!(b.motion[&1].cell, start_m, "mob must stay at spawn");
        assert!(rect_contains(&rect, b.motion[&0].cell));
    }

    #[test]
    fn real_game1_group23_owners_stay_non_owners_wander() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (Ok(game), Ok(k4)) = (
            std::fs::read(dir.join("Game.1")),
            std::fs::read(dir.join("Konung.4")),
        ) else {
            eprintln!("skipping: Game.1 / Konung.4 not present");
            return;
        };
        let tables = crate::population::parse_tables(&game).unwrap();
        let settlements = crate::population::parse_settlements(&game).unwrap();
        let collision = crate::loc::parse_collision(&k4).unwrap();
        let settlement = settlements
            .iter()
            .find(|s| s.loc_id == 4)
            .expect("loc 4 settlement");

        // Home waypoint owners (`FUN_0041182C`, first five slots).
        let owner_home: HashMap<usize, (i32, i32)> = settlement
            .waypoints
            .iter()
            .take(5)
            .filter(|w| w.owner != 0)
            .map(|w| (w.owner as usize, (w.x as i32, w.y as i32)))
            .collect();
        assert!(
            owner_home.contains_key(&277),
            "Гилли (277) must own a loc-4 home waypoint"
        );

        let mut b = build_population(&tables, 4, &collision);
        let movers: Vec<usize> = b
            .actors
            .iter()
            .filter(|a| wanders(&a.entity) && !owner_home.contains_key(&a.source_entity_id()))
            .map(|a| a.source_entity_id())
            .collect();
        assert!(!movers.is_empty(), "loc 4 must have wandering non-owners");
        let mover_starts: Vec<(i32, i32)> = movers.iter().map(|id| b.motion[id].cell).collect();

        for _ in 0..3600 {
            update_population(&mut b, &collision, Some(settlement), 1.0 / LOGIC_TICK_HZ);
        }

        // Waypoint owners stay exactly at their home waypoint.
        for (id, home) in &owner_home {
            if let Some(m) = b.motion.get(id) {
                assert_eq!(m.cell, *home, "home-waypoint owner {id} must stay put");
            }
        }
        // Non-owners wander within rect A.
        let moved = movers
            .iter()
            .zip(&mover_starts)
            .filter(|(id, s)| b.motion[id].cell != **s)
            .count();
        assert!(moved > 0, "non-owner citizens must wander");
        for id in &movers {
            let m = &b.motion[id];
            assert!(rect_contains(&m.rect_a, m.cell), "left rect A: {id} {:?}", m.cell);
        }
    }
}
