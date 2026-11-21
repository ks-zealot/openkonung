//! Player movement/route-commit tests.

use super::common::*;

/// A click commits the destination cell at the start of the walk segment while the sprite draws from the source cell.
///
/// This is the `FUN_0041435C` step-commit timing: the stored cell `+0x08/+0x0A`
/// leads the drawn sprite, and a single-step route is already consumed.
#[test]
fn click_commits_the_first_step_at_segment_start() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.camera_follow = false;
    assert!(!app.player_active);
    let start = (app.player_x as i32, app.player_y as i32);

    // Walk one step in +x (dir 6, `(2,0)`). Pre-align the facing: the turn gate
    // itself is covered by `the_player_turns_before_the_first_step`.
    app.player_dir = 6;
    let goal = (start.0 + 2, start.1);
    let (sx, sy) = cell_screen(&app, goal.0, goal.1);
    app.pointer_down(1, sx, sy);

    // `FUN_0041435C` commits the segment's cell at its START: the logical cell is
    // already the destination, the draw cell is the pre-step (source) cell.
    assert!(app.player_active, "walk segment in flight");
    assert_eq!(
        (app.player_x as i32, app.player_y as i32),
        goal,
        "committed cell"
    );
    assert!(
        app.player_path.is_empty(),
        "single-step queue consumed at start"
    );
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        start,
        "sprite draws from the pre-step cell"
    );
}

// A playable app with the real `Game.1` tables and `QUEST_R.RES` (Dragomir's
// start map 4), so dialogue condition/action execution and recruitment can be
// exercised against the shipped data.

/// A mid-walk click re-plans from the committed (already-entered) cell, so the sprite never jumps backwards.
#[test]
fn mid_walk_click_replans_from_the_committed_cell() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.camera_follow = false;
    let start = (app.player_x as i32, app.player_y as i32);

    // Start walking +x one step, then advance halfway through the cycle.
    app.player_dir = 6;
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
    // sprite's draw cell becomes that committed cell (no backward jump). The
    // westward segment first turns one 45-degree step per logic frame
    // (`FUN_00438090`), so drive the gate until the step commits.
    let (bx, by) = cell_screen(&app, start.0, start.1);
    app.pointer_down(1, bx, by);
    let mut committed_back = false;
    for _ in 0..8 {
        app.update_player(1.0 / crate::actors::LOGIC_TICK_HZ + 1e-9);
        if (app.player_x as i32, app.player_y as i32) == start {
            committed_back = true;
            break;
        }
    }
    assert!(committed_back, "the westward segment commits after the turn");
    assert!(app.player_active);
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        committed,
        "new segment draws from the committed cell"
    );

    // The final segment completes to idle parked on the committed cell.
    app.update_player(period);
    assert!(!app.player_active);
    assert_eq!(
        (app.player_render_x as i32, app.player_render_y as i32),
        (app.player_x as i32, app.player_y as i32),
        "idle parks the sprite on the committed cell"
    );
}

/// `FUN_00438090`: with a queued direction the protagonist turns one 45-degree
/// step per logic frame and consumes no cell until the facing matches.
#[test]
fn the_player_turns_before_the_first_step() {    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.camera_follow = false;
    let start = (app.player_x as i32, app.player_y as i32);
    // Facing north; the click is two cells east (dir 6).
    app.player_dir = 0;
    let goal = (start.0 + 2, start.1);
    let (sx, sy) = cell_screen(&app, goal.0, goal.1);
    app.pointer_down(1, sx, sy);

    assert!(!app.player_active, "a turn precedes the step");
    assert_eq!(
        (app.player_x as i32, app.player_y as i32),
        start,
        "no cell is consumed while turning"
    );

    let mut started = false;
    for _ in 0..10 {
        app.update_player(1.0 / crate::actors::LOGIC_TICK_HZ + 1e-9);
        if app.player_active {
            started = true;
            break;
        }
    }
    assert!(started, "the walk starts once the facing matches");
    assert_eq!(app.player_dir, 6, "turned to the queue head");
    assert_eq!(
        (app.player_x as i32, app.player_y as i32),
        goal,
        "the first step commits after alignment"
    );
}

/// `0x421964`: the walk-on transition activation.
///
/// The record whose table slot equals the current location's settlement index is
/// the arrival door and is suppressed (`Settlement+0x3E != 3`; all shipped
/// states are 0). Other records travel using their `+0x02` arrival facing.
#[test]
fn the_current_settlement_suppresses_its_arrival_door() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Free synthetic locations: 14 (source) and the two destinations.
    for id in [14u32, 15, 16] {
        let loc = crate::loc::Location {
            loc_id: id,
            data: Vec::new(),
            props: Vec::new(),
            gates: Vec::new(),
            graph_placements: Vec::new(),
            collision: vec![0u16; 320 * 160],
            max_x: 319,
            max_y: 159,
        };
        let index = app.world.locs.len();
        app.world.locs.push(loc);
        app.world.loc_index.insert(id, index);
    }
    app.current_map = 14;
    // Location 14's settlement is table index 0 (`Game.1`).
    app.settlements = vec![crate::population::Settlement {
        index: 0,
        loc_id: 14,
        ..Default::default()
    }];
    app.edges = vec![
        crate::edges::Edge {
            rec: 0,
            meta: 3,
            from: 14,
            to: 15,
            src: [40, 40],
            dst: [10, 10],
            dst2: [10, 10],
            side: "?",
            profiles: vec![1],
        },
        crate::edges::Edge {
            rec: 7,
            meta: 5,
            from: 14,
            to: 16,
            src: [50, 50],
            dst: [20, 20],
            dst2: [20, 20],
            side: "?",
            profiles: vec![1],
        },
    ];

    // Record 0 is the arrival door of settlement 0 -> suppressed.
    app.player_x = 10.0;
    app.player_y = 10.0;
    app.teleport_cooldown = 0.0;
    app.update_player(1.0 / crate::actors::LOGIC_TICK_HZ);
    assert_eq!(app.current_map, 14, "the arrival door must not fire");

    // Record 7 belongs to no settlement slot -> travels with its own facing.
    app.player_x = 20.0;
    app.player_y = 20.0;
    app.teleport_cooldown = 0.0;
    app.update_player(1.0 / crate::actors::LOGIC_TICK_HZ);
    assert_eq!(app.current_map, 16, "other records still travel");
    assert_eq!(app.player_dir, 5, "arrival facing is the record's +0x02");
    assert_eq!((app.player_x, app.player_y), (50.0, 50.0));
}
