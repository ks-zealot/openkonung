//! Pathfinding tests.

use super::super::*;

use super::common::*;

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

