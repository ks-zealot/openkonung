//! Party recruitment/selection tests.

use super::super::*;

use super::common::*;

#[test]
fn party_selection_gates_world_commands() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Leader/party slot 0 is selected at start; a world click issues a move.
    assert!(app.party_selected[0], "leader selected at start");
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.target.is_some(), "selected leader receives the move command");

    // Single-select an empty cell replaces the whole set (`FUN_00424D8C`).
    app.set_shift(false);
    app.select_party_member(5);
    assert!(!app.party_selected[0], "leader deselected");
    assert!(app.party_selected[5]);
    app.target = None;
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.target.is_none(), "deselected leader receives no command");

    // Additive select (Shift) keeps the previous member.
    app.set_shift(true);
    app.select_party_member(0);
    assert!(app.party_selected[0] && app.party_selected[5], "additive set");
}

#[test]
fn party_portrait_click_is_consumed_and_selects() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.set_shift(false);
    app.party_selected = [false; crate::hud::PARTY_CELLS];
    app.pointer_down(1, 36.0, 36.0);
    assert!(app.party_selected[0], "portrait click selects cell 0");
    assert!(app.target.is_none(), "portrait click must not move the world");
}

/// A playable app with one synthetic, fully-free location so world clicks issue
/// real routes (the plain `build_test_app` has no location, so every route is
/// rejected). Player starts at `(100, 80)`.

#[test]
fn sigma_joins_the_party_through_dialogue() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Сигмар: `Game.1` entity 539 at the start location (4), dialogue id 97.
    // Branch node 1767 -> first-meeting page 1771 -> node 1774 -> recruit op 0x123.
    app.open_dialog(539, 97);
    assert!(app.dialog.is_some(), "dialogue 97 opens");
    assert_eq!(
        app.dialog.as_ref().unwrap().page,
        1771,
        "first-meeting page (flag 207 unset)"
    );

    // Option 0 = "А что ты умеешь?" -> node 1774.
    app.dialog_click_option(0);
    assert_eq!(app.dialog.as_ref().unwrap().page, 1774);

    // Option 0 = "В таком случае ты мне пригодишься." (act: op 0x123, set flag 207).
    app.dialog_click_option(0);
    assert_eq!(app.party.len(), 1, "companion recruited");
    assert_eq!(app.party[0].entity.index, 539);
    assert!(app.party_selected[1], "new member auto-selected");
    assert!(app.quest_flags[207] & 0x80 != 0, "flag 207 set by the action");

    // Re-open: the member branch (op 0x15E) now resolves to the party menu 1781.
    app.dialog = None;
    app.open_dialog(539, 97);
    assert_eq!(
        app.dialog.as_ref().unwrap().page,
        1781,
        "party-member dialogue page"
    );

    // The member menu offers a dismissal option ("leave the party", op 0x14C).
    let labels: Vec<String> = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .map(|o| o.label())
        .collect();
    let leave = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("служба"))
        .expect("dismiss option present");
    assert!(!labels.is_empty());
    app.dialog_click_option(leave);
    assert!(app.party.is_empty(), "companion dismissed via op 0x14C");
}


#[test]
fn selected_companion_receives_the_world_click_move() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Recruit Сигмар through the dialogue (Game.1 entity 539, dialogue 97).
    app.open_dialog(539, 97);
    app.dialog_click_option(0); // 1771 -> 1774
    app.dialog_click_option(0); // recruit (op 0x123)
    assert_eq!(app.party.len(), 1);
    assert!(app.party_selected[0] && app.party_selected[1], "leader + recruit selected");

    // Click a cell ~8 east of the leader. The original command 0 stores the same
    // clicked cell on every selected unit, so the companion is commanded too.
    let start = app.party[0].motion.cell;
    let goal = (100 + 8, 80);
    let (sx, sy) = {
        let (lx, ly) = crate::coords::entity_to_local_px_f(goal.0 as f64, goal.1 as f64);
        (app.detail_pan[0] + lx, app.detail_pan[1] + ly)
    };
    app.pointer_down(1, sx, sy);

    assert!(app.player_active, "leader walks to the click");
    assert!(app.party[0].motion.walking, "companion is commanded to the same cell");
    assert!(
        !app.party[0].motion.path.is_empty(),
        "companion has a route to the clicked cell"
    );

    // Advance the companion follow/step logic; its committed cell must move.
    for _ in 0..300 {
        app.update_party(1.0 / LOGIC_TICK_HZ);
    }
    assert_ne!(app.party[0].motion.cell, start, "companion moved on the click command");
}


#[test]
fn tab_toggles_every_selected_party_member() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.open_dialog(539, 97);
    app.dialog_click_option(0); // 1771 -> 1774
    app.dialog_click_option(0); // recruit (op 0x123)
    assert_eq!(app.party.len(), 1);
    assert!(app.party_selected[0] && app.party_selected[1]);

    let leader_before = app.player.flags_b & crate::anim::FLAG_BATTLE;
    let sig_before = app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE;
    app.key_down(KEY_TAB);
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        leader_before ^ crate::anim::FLAG_BATTLE
    );
    assert_eq!(
        app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE,
        sig_before ^ crate::anim::FLAG_BATTLE
    );
    let battle = crate::anim::battle_from_flags(app.party[0].entity.flags_b);
    let expected = if app.party[0].motion.walking {
        crate::anim::walk_action(battle)
    } else {
        crate::anim::idle_action(battle)
    } as u8;
    assert_eq!(app.party[0].motion.action, expected);

    // The toggle is exact and reversible for both selected units.
    app.key_down(KEY_TAB);
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        leader_before
    );
    assert_eq!(
        app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE,
        sig_before
    );
}

#[test]
fn tab_toggles_a_solo_selected_companion_not_the_leader() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.open_dialog(539, 97);
    app.dialog_click_option(0);
    app.dialog_click_option(0);
    assert_eq!(app.party.len(), 1);

    // Clicking Sigmarr's portrait without Shift clears the leader and selects
    // only the companion (`FUN_00424D8C`).
    app.party_selected = [false; crate::hud::PARTY_CELLS];
    app.party_selected[1] = true;

    let leader_before = app.player.flags_b & crate::anim::FLAG_BATTLE;
    let sig_before = app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE;
    app.key_down(KEY_TAB);
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        leader_before,
        "unselected leader must not change"
    );
    assert_eq!(
        app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE,
        sig_before ^ crate::anim::FLAG_BATTLE,
        "selected companion must change"
    );
    let battle = crate::anim::battle_from_flags(app.party[0].entity.flags_b);
    let expected = if app.party[0].motion.walking {
        crate::anim::walk_action(battle)
    } else {
        crate::anim::idle_action(battle)
    } as u8;
    assert_eq!(app.party[0].motion.action, expected);
}

#[test]
fn tab_stance_direction_comes_from_first_selected_unit() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.open_dialog(539, 97);
    app.dialog_click_option(0);
    app.dialog_click_option(0);
    assert_eq!(app.party.len(), 1);

    // First selected (leader) is peaceful; Sigmarr is already in battle.
    app.player.flags_b &= !crate::anim::FLAG_BATTLE;
    app.party[0].entity.flags_b |= crate::anim::FLAG_BATTLE;
    app.key_down(KEY_TAB);
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        crate::anim::FLAG_BATTLE,
        "leader enters battle"
    );
    assert_eq!(
        app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE,
        crate::anim::FLAG_BATTLE,
        "companion already in battle is left unchanged"
    );
}

#[test]
fn tab_skips_dead_companions() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.open_dialog(539, 97);
    app.dialog_click_option(0);
    app.dialog_click_option(0);
    assert_eq!(app.party.len(), 1);

    // Original `FUN_00421554` skips action rows 3/9 and `+0x04 & 0x80`.
    app.party[0].motion.action = crate::anim::ACT_PEACE_DEATH as u8;
    let sig_before = app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE;
    app.key_down(KEY_TAB);
    assert_eq!(
        app.party[0].entity.flags_b & crate::anim::FLAG_BATTLE,
        sig_before,
        "dead companion is skipped"
    );
    assert_eq!(
        app.player.flags_b & crate::anim::FLAG_BATTLE,
        crate::anim::FLAG_BATTLE,
        "living selected leader still toggles"
    );
}
