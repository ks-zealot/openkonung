//! Spawn placement/collision tests.

use super::super::*;

use super::common::*;

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

