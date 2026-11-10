//! Unit tests for the [`crate::combat::los`] line-of-fire probe.

use super::*;

/// `Konung.N` collision grid: `u16` per cell, `index = x*160 + y`.
fn grid() -> Vec<u16> {
    vec![0u16; 320 * 160]
}

/// Write one collision cell by runtime coordinates.
fn set_cell(grid: &mut [u16], x: i32, y: i32, value: u16) {
    grid[x as usize * 160 + y as usize] = value;
}

/// Block every `y` (i.e. a whole screen-vertical band) at runtime row `x`.
fn wall(grid: &mut [u16], x: i32) {
    for y in 0..160 {
        set_cell(grid, x, y, 0xC000);
    }
}

/// Targets closer than `MIN_DISTANCE` cells report `-1`.
#[test]
fn too_close_targets_return_minus_one() {
    let g = grid();
    assert_eq!(probe(&g, (10, 10), (10, 12), 40), LineOfFire::TooClose);
    assert_eq!(probe(&g, (10, 10), (10, 12), 40).as_code(), -1);
    // Exactly three cells is not "too close".
    assert_ne!(probe(&g, (10, 10), (10, 13), 40), LineOfFire::TooClose);
}

/// An unobstructed DDA walk reaches the target cell and returns `1`.
#[test]
fn an_open_field_line_reaches_the_target_cell() {
    let g = grid();
    // 4 cells along the row: ~152 projected pixels, covered by `8 * 20`.
    assert_eq!(probe(&g, (10, 10), (10, 14), 20), LineOfFire::Clear);
    assert_eq!(probe(&g, (10, 10), (10, 14), 20).as_code(), 1);
}

/// A `0xC000` wall between the actors stops the walk (`0`).
#[test]
fn a_wall_between_the_actors_blocks_the_line() {
    let mut g = grid();
    wall(&mut g, 9);
    assert_eq!(probe(&g, (10, 10), (10, 14), 20), LineOfFire::Blocked);
    assert_eq!(probe(&g, (10, 10), (10, 14), 20).as_code(), 0);
}

/// A real occupant id blocks, while the reserved `0xFFF` marker stays walkable.
#[test]
fn an_occupied_cell_blocks_the_line() {
    let mut g = grid();
    for y in 0..160 {
        set_cell(&mut g, 9, y, 5);
    }
    assert_eq!(probe(&g, (10, 10), (10, 14), 20), LineOfFire::Blocked);
    // `0xFFF` is the "reserved" occupant marker and stays walkable for the
    // probe, exactly like `FUN_0041130C`.
    let mut g2 = grid();
    for y in 0..160 {
        set_cell(&mut g2, 9, y, 0x0FFF);
    }
    assert_eq!(probe(&g2, (10, 10), (10, 14), 20), LineOfFire::Clear);
}

/// The sample budget is exactly `8 * range`, so range `0` and `1` cannot reach.
#[test]
fn the_walk_is_bounded_by_the_weapon_range() {
    let g = grid();
    // The melee range value is 1 -> only 8 one-pixel steps: no reach.
    assert_eq!(probe(&g, (10, 10), (10, 14), 1), LineOfFire::Blocked);
    // A ranged value reaches the same target.
    assert_eq!(probe(&g, (10, 10), (10, 14), 40), LineOfFire::Clear);
    // Range 0 performs no steps at all.
    assert_eq!(probe(&g, (10, 10), (10, 14), 0), LineOfFire::Blocked);
}

/// The probe uses a fixed DDA (no RNG), so equal inputs give equal results.
#[test]
fn the_probe_is_deterministic() {
    let g = grid();
    let a = probe(&g, (40, 30), (46, 34), 60);
    let b = probe(&g, (40, 30), (46, 34), 60);
    assert_eq!(a, b);
}

/// `LineOfFire::as_code`/`is_clear` reproduce the original `-1 / 0 / 1` returns.
#[test]
fn result_codes_match_the_original_returns() {
    assert_eq!(LineOfFire::TooClose.as_code(), -1);
    assert_eq!(LineOfFire::Blocked.as_code(), 0);
    assert_eq!(LineOfFire::Clear.as_code(), 1);
    assert!(LineOfFire::Clear.is_clear());
    assert!(!LineOfFire::Blocked.is_clear());
}

/// `0x45ACD0` holds one signed `(dx, dy)` nudge per facing code.
#[test]
fn the_facing_start_nudge_covers_eight_directions() {
    assert_eq!(DIR_START_OFFSET.len(), 8);
    // `0x45ACD0` is read as signed bytes.
    assert_eq!(DIR_START_OFFSET[0], (-70, -65));
    assert_eq!(DIR_START_OFFSET[4], (80, -58));
}
