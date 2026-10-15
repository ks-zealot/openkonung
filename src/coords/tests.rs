use super::*;

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

#[test]
fn step_by_direction_is_parity_dependent() {
    // Even x: dir 1 steps straight left; odd x: dir 1 steps up-left.
    assert_eq!(step_by_direction(4, 10, 1), (3, 10));
    assert_eq!(step_by_direction(5, 10, 1), (4, 9));
    // +y / −y are parity-independent.
    assert_eq!(step_by_direction(4, 10, 4), (4, 11));
    assert_eq!(step_by_direction(5, 10, 0), (5, 9));
}

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

#[test]
fn direction_from_cells_cardinal_and_diagonal() {
    assert_eq!(direction_from_cells(0, 0, 0, 5), 0); // target −y
    assert_eq!(direction_from_cells(0, 10, 0, 5), 4); // target +y
    assert_eq!(direction_from_cells(20, 5, 5, 5), 6); // target +x
    assert_eq!(direction_from_cells(0, 5, 20, 5), 2); // target −x
    assert_eq!(direction_from_cells(20, 20, 0, 0), 5); // +x,+y
    assert_eq!(direction_from_cells(20, 0, 0, 20), 7); // +x,−y
}

#[test]
fn entity_projection_exact_cases() {
    assert_eq!(entity_to_local_px(0, 0), (58, 16));
    assert_eq!(entity_to_local_px(1, 0), (29, 32));
    assert_eq!(entity_to_local_px(2, 0), (58, 48));
    assert_eq!(entity_to_local_px(5, 7), (435, 96));
    // Even x uses the full 58px offset, odd x uses the 29px half offset.
    assert_eq!(entity_to_local_px(6, 9), (9 * 58 + 58, 6 * 16 + 16));
}

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

#[test]
fn inverse_clamps_to_runtime_bounds() {
    assert_eq!(local_px_to_entity(-10_000, -10_000), (0, 0));
    assert_eq!(
        local_px_to_entity(i32::MAX / 2, i32::MAX / 2),
        (RUNTIME_MAX_X, RUNTIME_MAX_Y)
    );
}
