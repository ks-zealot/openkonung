//! Frontend/input/text tests.

use super::super::*;

use super::common::*;

/// The cursor is the neutral frame 4 over the HUD panel and over free ground.
#[test]
fn cursor_is_neutral_over_hud_and_free_ground() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Party panel (left HUD, outside the world viewport): neutral pointer.
    app.cursor_virtual = (20.0, 20.0);
    assert_eq!(app.debug_cursor_frame(), 4);
    // World viewport: put runtime cell (150, 100) under the pointer. It is
    // free and far from the leader (0, 0), so the neutral pointer is selected.
    let (lx, ly) = crate::coords::entity_to_local_px_f(150.0, 100.0);
    app.detail_pan = [200.0 - lx.round(), 300.0 - ly.round()];
    app.cursor_virtual = (200.0, 300.0);
    assert_eq!(app.debug_cursor_frame(), 4);
}

/// Keys 1/2/3 select weapon slots 2/0/1, `I` toggles the sheet, `A` engages and `F` halts.
#[test]
fn original_hotkeys_are_bound() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.key_down(KEY_1);
    assert_eq!(app.player.raw_0x12, 2);
    app.key_down(KEY_2);
    assert_eq!(app.player.raw_0x12, 0);
    app.key_down(KEY_3);
    assert_eq!(app.player.raw_0x12, 1);

    app.key_down(KEY_I);
    assert!(app.character_screen);
    app.key_down(KEY_ESCAPE);
    assert!(!app.character_screen);

    app.key_down(KEY_A);
    assert!(crate::anim::battle_from_flags(app.player.flags_b));
    app.key_down(KEY_F);
    assert!(!app.debug_player_active());
}

/// `C`/Escape toggle the character screen, which swallows world clicks and never mutates gameplay data.
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
    assert!(
        !app.character_screen,
        "portrait click must not open the screen"
    );
    assert!(
        app.target.is_none(),
        "portrait click must not move the world"
    );

    // `C` opens the character screen.
    app.key_down(KEY_C);
    assert!(app.character_screen);
    assert!(app.target.is_none(), "opening must not issue a world move");

    // While open, a world click is consumed.
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.character_screen);
    assert!(app.target.is_none(), "no click-through to the world");
    assert!(app.held_item.is_none());

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

// Real Game.1 + Konung.3: the 19 `Земляной орех` nodes on location 3 load,
//
// collect, deactivate for 0x1518 frames and respawn with a fresh instance.

/// `cursor_for_class` reproduces the `0x424E78` jump table, including default frame 9.
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

/// A right-click on empty ground issues a move and never toggles the graph view.
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

/// `TAB` flips the leader's `Entity+0x03` battle bit and is exactly reversible.
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

/// Wrapped lines never exceed the requested pixel width, and a long header still wraps.
#[test]
fn wrap_to_px_keeps_every_line_within_width() {
    let s = "OpenKonung v15 | Старый лагерь | hero:Dragomir Game.1 | 800x600 native camera | pal:33 | portals:4 | C follow | H graph | D ids | P pop | L layers | T portals | cell=(72,34)";
    let lines = wrap_to_px(s, 1.0, VIRTUAL_W - 16);
    assert!(lines.len() > 1, "long header must wrap");
    for l in &lines {
        assert!(text_px(l, 1.0) <= VIRTUAL_W - 16, "line too wide: {l:?}");
    }
}

/// A single oversized word is hard-split without losing any character.
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

/// `text_px` is 0 for the empty string and grows with text length and scale.
#[test]
fn text_clamped_width_estimate_is_positive_and_monotonic() {
    assert!(text_px("", 1.0) == 0);
    assert!(text_px("ab", 1.0) < text_px("abcd", 1.0));
    assert!(text_px("abcd", 1.0) < text_px("abcd", 2.0));
}
