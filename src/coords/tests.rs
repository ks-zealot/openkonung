//! Unit tests for the [`crate::coords`] module.

use super::*;

/// `DIR_EVEN`/`DIR_ODD` are the `0x480110`/`0x48010C` tables verbatim.
#[test]
fn direction_tables_match_original_bytes() {
    assert_eq!(
        DIR_EVEN,
        [
            (0, -1),
            (-1, 0),
            (-2, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (2, 0),
            (1, 0)
        ]
    );
    assert_eq!(
        DIR_ODD,
        [
            (0, -1),
            (-1, -1),
            (-2, 0),
            (-1, 0),
            (0, 1),
            (1, 0),
            (2, 0),
            (1, -1)
        ]
    );
}

/// Direction 1 steps left on an even `x` and up-left on an odd `x`.
#[test]
fn step_by_direction_is_parity_dependent() {
    // Even x: dir 1 steps straight left; odd x: dir 1 steps up-left.
    assert_eq!(step_by_direction(4, 10, 1), (3, 10));
    assert_eq!(step_by_direction(5, 10, 1), (4, 9));
    // +y / −y are parity-independent.
    assert_eq!(step_by_direction(4, 10, 4), (4, 11));
    assert_eq!(step_by_direction(5, 10, 0), (5, 9));
}

/// Odd direction codes cost 10, even codes cost 14.
#[test]
fn direction_cost_matches_astar_families() {
    assert_eq!(direction_cost(1), 10);
    assert_eq!(direction_cost(3), 10);
    assert_eq!(direction_cost(5), 10);
    assert_eq!(direction_cost(7), 10);
    assert_eq!(direction_cost(0), 14);
    assert_eq!(direction_cost(2), 14);
    assert_eq!(direction_cost(4), 14);
    assert_eq!(direction_cost(6), 14);
}

/// `direction_from_cells` maps the four cardinals to 0/2/4/6 and the
/// staggered diagonals to 3/5/7.
#[test]
fn direction_from_cells_cardinal_and_diagonal() {
    assert_eq!(direction_from_cells(0, 0, 0, 5), 0); // target −y
    assert_eq!(direction_from_cells(0, 10, 0, 5), 4); // target +y
    assert_eq!(direction_from_cells(20, 5, 5, 5), 6); // target +x
    assert_eq!(direction_from_cells(0, 5, 20, 5), 2); // target −x
    assert_eq!(direction_from_cells(20, 20, 0, 0), 5); // +x,+y
    assert_eq!(direction_from_cells(20, 0, 0, 20), 7); // +x,−y
}

/// `entity_to_local_px` reproduces `FUN_00438A78` exact cases.
#[test]
fn entity_projection_exact_cases() {
    assert_eq!(entity_to_local_px(0, 0), (58, 16));
    assert_eq!(entity_to_local_px(1, 0), (29, 32));
    assert_eq!(entity_to_local_px(2, 0), (58, 48));
    assert_eq!(entity_to_local_px(5, 7), (435, 96));
    // Even x uses the full 58px offset, odd x uses the 29px half offset.
    assert_eq!(entity_to_local_px(6, 9), (9 * 58 + 58, 6 * 16 + 16));
}

/// The f64 projection equals the integer one for every integer input.
#[test]
fn continuous_matches_integer_projection() {
    for x in -3..330 {
        for y in -3..165 {
            let (ix, iy) = entity_to_local_px(x, y);
            let (fx, fy) = entity_to_local_px_f(x as f64, y as f64);
            assert_eq!((fx, fy), (ix as f64, iy as f64), "mismatch at ({x},{y})");
        }
    }
}

/// Projecting every one of the 320x160 cells and inverting gives the cell back.
#[test]
fn exhaustive_round_trip_all_cells() {
    for x in 0..RUNTIME_W {
        for y in 0..RUNTIME_H {
            let (px, py) = entity_to_local_px(x, y);
            let cell = local_px_to_entity(px, py);
            assert_eq!(
                cell,
                (x, y),
                "round-trip failed for cell ({x},{y}) -> pixel ({px},{py})"
            );
        }
    }
}

/// The continuous inverse round-trips to within `1e-6`.
#[test]
fn continuous_round_trip() {
    for &x in &[0.0f64, 0.5, 1.0, 1.25, 12.75, 100.0, 268.0] {
        for &y in &[0.0f64, 3.5, 60.0, 159.0] {
            let (px, py) = entity_to_local_px_f(x, y);
            let (rx, ry) = local_px_to_entity_f(px, py);
            assert!((rx - x).abs() < 1e-6, "x {x} -> {rx}");
            assert!((ry - y).abs() < 1e-6, "y {y} -> {ry}");
        }
    }
}

/// `local_px_to_entity` clamps far off-grid pixels to `0..=319` / `0..=159`.
#[test]
fn inverse_clamps_to_runtime_bounds() {
    assert_eq!(local_px_to_entity(-10_000, -10_000), (0, 0));
    assert_eq!(
        local_px_to_entity(i32::MAX / 2, i32::MAX / 2),
        (RUNTIME_MAX_X, RUNTIME_MAX_Y)
    );
}

/// `FUN_00438640`: the step-list probe, including its `1` fallback.
#[test]
fn face_direction_uses_the_original_step_order() {
    // Same column: above -> 0, below/equal -> 4.
    assert_eq!(face_direction(10, 5, 10, 6), 0);
    assert_eq!(face_direction(10, 7, 10, 6), 4);
    assert_eq!(face_direction(10, 6, 10, 6), 4);
    // Adjacent cells resolve to a step that actually reaches them.
    for dir in 0u8..8 {
        let (tx, ty) = step_by_direction(10, 6, dir);
        if tx == 10 {
            continue; // covered by the same-column rule
        }
        let got = face_direction(tx, ty, 10, 6);
        let (rx, ry) = step_by_direction(10, 6, got);
        assert_eq!((rx, ry), (tx, ty), "dir {dir} -> {got}");
    }
    // Non-adjacent targets fall back to 1.
    assert_eq!(face_direction(40, 40, 10, 6), 1);
}

/// `FUN_00438090`: one 45-degree step per call, clockwise for `diff < 5` and
/// counter-clockwise otherwise, wrapping at 0/7.
#[test]
fn turn_toward_steps_one_direction_per_call() {
    let mut f = 0u8;
    assert!(!turn_toward(&mut f, 2));
    assert_eq!(f, 1);
    assert!(!turn_toward(&mut f, 2));
    assert_eq!(f, 2);
    assert!(turn_toward(&mut f, 2), "already facing the target");
    assert_eq!(f, 2);

    // `diff < 5` turns clockwise: 2 -> 3 -> 4 -> 5 -> 6.
    let mut g = 2u8;
    for expect in [3u8, 4, 5, 6] {
        assert!(!turn_toward(&mut g, 6));
        assert_eq!(g, expect);
    }
    assert!(turn_toward(&mut g, 6));

    // `diff >= 5` turns counter-clockwise: 2 -> 1 -> 0.
    let mut h = 2u8;
    assert!(!turn_toward(&mut h, 0));
    assert_eq!(h, 1);
    assert!(!turn_toward(&mut h, 0));
    assert_eq!(h, 0);
    assert!(turn_toward(&mut h, 0));

    // Every start/target pair is reached within four steps.
    for start in 0u8..8 {
        for target in 0u8..8 {
            let mut f = start;
            let mut steps = 0;
            while !turn_toward(&mut f, target) {
                steps += 1;
                assert!(steps <= 4, "{start} -> {target}");
            }
            assert_eq!(f, target);
        }
    }
}
