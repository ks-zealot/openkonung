//! Phase C1/C2 runtime population layer.
//!
//! Builds renderable actors from the Phase B [`crate::population`] Group/Entity
//! tables. It performs **no** population growth, AI, movement, collision,
//! combat, dialogue or animation advancement.
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
//! runtime positions are Entity/transition coordinates and are projected at draw
//! time by `app::transition_xy_to_screen` — the same helper the protagonist uses.
//! They are **not** collision-grid indices, and are never clamped to the
//! rendered terrain surface.

use std::fmt;

use crate::coords::entity_to_local_px;
use crate::population::{Entity, GameTables, Group, Rect};

/// Maximum placement attempts in the original routine.
pub const MAX_ATTEMPTS: u8 = 100;

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
}
