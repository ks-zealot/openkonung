//! Frontend/input/text tests.

use super::super::*;

use super::common::*;

#[test]
fn character_screen_open_close_and_no_world_clickthrough() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let inv0 = app.player.inventory;
    let eq0 = app.player.equipment;
    let hp0 = app.player.hp_internal;

    // A party-portrait click selects/consumes only: it never opens the
    // screen (original semantics) and never issues a world move.
    assert!(!app.character_screen);
    app.pointer_down(1, 36.0, 36.0);
    assert!(!app.character_screen, "portrait click must not open the screen");
    assert!(app.target.is_none(), "portrait click must not move the world");

    // `C` opens the character screen.
    app.key_down(KEY_C);
    assert!(app.character_screen);
    assert!(app.target.is_none(), "opening must not issue a world move");

    // While open, a world click and an equipment click are consumed.
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.character_screen);
    assert!(app.target.is_none(), "no click-through to the world");
    app.pointer_down(1, 183.0, 103.0);
    assert!(app.character_screen);
    assert!(app.target.is_none());

    // `C` toggles closed; Escape also closes.
    app.key_down(KEY_C);
    assert!(!app.character_screen);
    app.key_down(KEY_C);
    assert!(app.character_screen);
    app.key_down(KEY_ESCAPE);
    assert!(!app.character_screen);

    // World clicks work again after closing.
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.target.is_some(), "world targeting restored after close");

    // `V` toggles camera follow (moved off `C`).
    let cf = app.camera_follow;
    app.key_down(KEY_V);
    assert_eq!(app.camera_follow, !cf);

    // Open/close cycles do not mutate gameplay data.
    assert_eq!(app.player.inventory, inv0);
    assert_eq!(app.player.equipment, eq0);
    assert_eq!(app.player.hp_internal, hp0);
}

/// Real Game.1 + Konung.3: the 19 `Земляной орех` nodes on location 3 load,
///
/// collect, deactivate for 0x1518 frames and respawn with a fresh instance.

#[test]
fn cursor_class_mapping_matches_original_jump_table() {
    use super::super::cursor::cursor_for_class;
    // FUN_00424E0C jump table 0x424E78 (cases 0..=0xC, default 9).
    assert_eq!(cursor_for_class(0), 10);
    assert_eq!(cursor_for_class(1), 2);
    assert_eq!(cursor_for_class(2), 10);
    for c in 3..=5 {
        assert_eq!(cursor_for_class(c), 0);
    }
    for c in [6, 7, 8, 10, 11, 13, 255] {
        assert_eq!(cursor_for_class(c), 9);
    }
    assert_eq!(cursor_for_class(9), 8);
    assert_eq!(cursor_for_class(12), 2);
}

#[test]
fn right_click_moves_and_no_longer_toggles_graph() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Left-click moves (unchanged).
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.target.is_some(), "left-click moves");
    // Right-click also moves when no actor is under the cursor (original
    // right-button branch, command code 0) and must NOT toggle the graph view.
    app.target = None;
    assert!(!app.mode_graph);
    app.pointer_down(3, 500.0, 300.0);
    assert!(!app.mode_graph, "right-click no longer opens the graph");
    assert!(
        app.target.is_some(),
        "right-click moves when no actor is targeted"
    );
}

#[test]
fn tab_toggles_protagonist_battle_stance() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let before = app.player.flags_b & crate::anim::FLAG_BATTLE;
    app.key_down(KEY_TAB);
    assert_ne!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        before,
        "Tab must flip the battle stance bit"
    );
    app.key_down(KEY_TAB);
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        before,
        "Tab must be a toggle"
    );
}


#[test]
fn wrap_to_px_keeps_every_line_within_width() {
    let s = "OpenKonung v15 | Старый лагерь | hero:Dragomir Game.1 | 800x600 native camera | pal:33 | portals:4 | C follow | H graph | D ids | P pop | L layers | T portals | cell=(72,34)";
    let lines = wrap_to_px(s, 1.0, VIRTUAL_W - 16);
    assert!(lines.len() > 1, "long header must wrap");
    for l in &lines {
        assert!(text_px(l, 1.0) <= VIRTUAL_W - 16, "line too wide: {l:?}");
    }
}

#[test]
fn wrap_to_px_hard_splits_a_single_long_word() {
    let s = "x".repeat(400);
    let lines = wrap_to_px(&s, 1.0, 200);
    assert!(lines.len() > 1);
    for l in &lines {
        assert!(text_px(l, 1.0) <= 200);
    }
    assert_eq!(lines.concat().len(), 400);
}

#[test]
fn text_clamped_width_estimate_is_positive_and_monotonic() {
    assert!(text_px("", 1.0) == 0);
    assert!(text_px("ab", 1.0) < text_px("abcd", 1.0));
    assert!(text_px("abcd", 1.0) < text_px("abcd", 2.0));
}

