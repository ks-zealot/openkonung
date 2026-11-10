//! `placement` subsystem of [`crate::actors`] (split from `actors.rs`).
//!
//! Runtime placement of `(0,0)` Entity records and the deterministic RNG that
//! makes it reproducible per location. A candidate is drawn from the group's
//! first spawn rectangle (A) as `center` plus a signed random delta, accepted
//! only when the collision cell's low 12 bits are clear, and re-drawn at most
//! 100 times. There is no centre fallback: after 100 rejections the entity is
//! reported as failed placement and is not rendered.
//!
//! This module also holds [`PopulationBuild`], the per-location result that the
//! renderer and the AI consume.

use super::*;

/// A generated candidate in the Group spawn-rectangle domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawSpawnCoord {
    /// Runtime grid column (`0..=319`) before the collision test.
    pub x: i32,
    /// Runtime grid row (`0..=159`) before the collision test.
    pub y: i32,
}

/// Runtime placement failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// Every one of the original 100 attempts hit a blocked cell.
    NoCandidateAfterAttempts {
        /// Number of attempts performed (always [`MAX_ATTEMPTS`]).
        attempts: u8,
    },
}

/// Deterministic RNG (xorshift64). No thread/browser randomness is used, so
/// placement is reproducible for a given location.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    /// Create a stream from `seed`; the low bit is forced set so the state can
    /// never be zero (xorshift would then stay stuck at zero).
    pub fn new(seed: u64) -> Self {
        DeterministicRng { state: seed | 1 }
    }

    /// Next 32 random bits: one xorshift64 step, high half returned.
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
    /// The fixed state-1 stream, used before a location RNG is installed.
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
pub fn spawn_center_half(rect: &Rect) -> (i32, i32, i32, i32) {
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

/// Signed random offset for one axis: `0` when the half-extent is below 2,
/// otherwise `rand % half` with a random sign, so candidates never reach the
/// rectangle border on that axis.
pub fn axis_delta(half: i32, rng: &mut DeterministicRng) -> i32 {
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
///
/// Returns the accepted candidate plus the 1-based attempt number, or
/// [`PlacementError::NoCandidateAfterAttempts`] after exactly `MAX_ATTEMPTS`
/// rejections.
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

/// A `(0,0)` entity that could not be placed after [`MAX_ATTEMPTS`] attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FailedPlacement {
    /// `Group` index that enumerated the entity.
    pub source_group_id: usize,
    /// Entity table index that could not be placed.
    pub source_entity_id: usize,
    /// Spawn rectangle A the candidates were drawn from.
    pub spawn_rect: Rect,
    /// Attempts performed (always the 100-attempt cap).
    pub attempts: u8,
}

/// Result of building one location's population.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PopulationBuild {
    /// Persisted and successfully runtime-positioned actors, unsorted.
    pub actors: Vec<RuntimeActor>,
    /// `(0,0)` entities that failed placement.
    pub failed: Vec<FailedPlacement>,
    /// Every group whose `location_id` matches, including group 0 / inactive.
    pub matching_groups: usize,
    /// Matching groups skipped because they are the reserved group 0.
    pub reserved_group_zero: usize,
    /// Matching groups skipped because `Group+0x1E == 0` (inactive class).
    pub inactive_groups: usize,
    /// Matching groups skipped because `first_entity + entity_count` fell outside
    /// the Entity table.
    pub invalid_ranges: usize,
    /// Entities iterated in valid `first_entity..+entity_count` ranges.
    pub enumerated_entities: usize,
    /// `(0,0)` records attempted (== placed + failed).
    pub runtime_candidates: usize,
    /// `(0,0)` records that got a valid runtime cell.
    pub runtime_placed: usize,
    /// `(0,0)` records rejected 100 times (see [`PopulationBuild::failed`]).
    pub runtime_failed: usize,
    /// Live movement state keyed by source Entity id.
    pub motion: HashMap<usize, ActorMotion>,
    /// Per-location deterministic RNG stream (persists across frames).
    pub rng: DeterministicRng,
    /// Sub-tick accumulator for the [`LOGIC_TICK_HZ`] fixed step.
    pub(super) tick_accum: f64,
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
        // Original AI selection is by **group class** (`FUN_00421B50`):
        // `class & 0x80 != 0` (with `& 0x40 == 0`) runs the waypoint/wander AI
        // (`FUN_004168A4`); `class & 0x40` (object/enemy groups) runs
        // `FUN_00412E00`, which idles in place while `Group+0x1D == 0`.
        let citizen = g.class_1e & 0x80 != 0 && g.class_1e & 0x40 == 0;
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
                        render_cell: (e.x as i32, e.y as i32),
                        facing: e.facing,
                        rect_a: g.rect_a,
                        citizen,
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
                                render_cell: (raw.x, raw.y),
                                facing: e.facing,
                                rect_a: g.rect_a,
                                citizen,
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

impl PopulationBuild {
    /// Number of actors drawn at their authored (`Game.N`) cell.
    pub fn persisted_position_actors(&self) -> usize {
        self.actors.iter().filter(|a| !a.is_runtime()).count()
    }

    /// Number of actors whose cell was assigned by the placement pass.
    pub fn runtime_positioned(&self) -> usize {
        self.actors.iter().filter(|a| a.is_runtime()).count()
    }
}
