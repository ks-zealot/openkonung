//! Player movement/route-commit tests.


use super::common::*;

#[test]
fn click_commits_the_first_step_at_segment_start() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.camera_follow = false;
    assert!(!app.player_active);
    let start = (app.player_x as i32, app.player_y as i32);

    // Walk one step in +x (dir 6, `(2,0)`).
    let goal = (start.0 + 2, start.1);
    let (sx, sy) = cell_screen(&app, goal.0, goal.1);
    app.pointer_down(1, sx, sy);

    // `FUN_0041435C` commits the segment's cell at its START: the logical cell is
    // already the destination, the draw cell is the pre-step (source) cell.
    assert!(app.player_active, "walk segment in flight");
    assert_eq!((app.player_x as i32, app.player_y as i32), goal, "committed cell");
    assert!(app.player_path.is_empty(), "single-step queue consumed at start");
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        start,
        "sprite draws from the pre-step cell"
    );
}

/// A playable app with the real `Game.1` tables and `QUEST_R.RES` (Dragomir's
/// start map 4), so dialogue condition/action execution and recruitment can be
/// exercised against the shipped data.

#[test]
fn mid_walk_click_replans_from_the_committed_cell() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.camera_follow = false;
    let start = (app.player_x as i32, app.player_y as i32);

    // Start walking +x one step, then advance halfway through the cycle.
    let first = (start.0 + 2, start.1);
    let (fx, fy) = cell_screen(&app, first.0, first.1);
    app.pointer_down(1, fx, fy);
    let committed = (app.player_x as i32, app.player_y as i32);
    assert_eq!(committed, first);
    let period = app.player_cell_period();
    app.update_player(period * 0.5);
    assert_eq!(
        (app.player_x as i32, app.player_y as i32),
        committed,
        "logical cell does not move mid-cycle"
    );

    // Re-target back to the original cell. The origin must be the committed cell
    // (the cell being walked INTO), so the committed cell is not rewound, and the
    // sprite's draw cell becomes that committed cell (no backward jump).
    let (bx, by) = cell_screen(&app, start.0, start.1);
    app.pointer_down(1, bx, by);
    assert!(app.player_active);
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        committed,
        "new segment draws from the committed cell"
    );
    assert_eq!((app.player_x as i32, app.player_y as i32), start, "continues to goal");

    // The final segment completes to idle parked on the committed cell.
    app.update_player(period);
    assert!(!app.player_active);
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        (app.player_x as i32, app.player_y as i32),
        "idle parks the sprite on the committed cell"
    );
}
