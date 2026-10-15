//! Unit tests for the [`crate::app`] module.

use super::*;

// Konung.4, Game.1: gates at raw averaged 244/245 grid centers, in gate-index order.
const K4_GATES: [(f64, f64); 4] =
    [(123.0, 61.5), (115.5, 5.0), (16.0, 6.5), (66.0, 65.0)];
// Outgoing edges in (rec, to) order: rec10 4->5, rec11 4->15, rec12 4->30, rec13 4->32.
const K4_TRIGGERS: [(i32, i32, i32, i32); 4] =
    [(63, 64, 63, 64), (119, 61, 119, 61), (112, 4, 112, 4), (12, 5, 12, 5)];
const K4_RECS: [usize; 4] = [10, 11, 12, 13];

fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A minimal playable app (real `Game.res` + `CLIENT.EXE`; synthetic
/// GRAPH/Objects) for input-routing tests. Gated on files being present.
fn build_test_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let graph = vec![0u8; 0x30000]; // palettes/terrain unused in this test
    let objects = vec![0u8; 8 + 512 * 8];
    let res = Res::new(game, graph, objects, exe);
    let world = World::new(res, Vec::new());
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
    player.equipment = [0, 0, 1, 0, 0, 0];
    Some(PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        1,
        0.0,
        0.0,
        5,
        None,
        Vec::new(),
    ))
}

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
fn konung4_maps_each_trigger_to_its_gate() {
    let a = match_triggers_to_gates(&K4_TRIGGERS, &K4_RECS, &K4_GATES, MAX_GATE_TRIGGER_DIST);
    // rec10 4->5 -> gate3, rec11 4->15 -> gate0, rec12 4->30 -> gate1, rec13 4->32 -> gate2
    assert_eq!(a, vec![Some(3), Some(0), Some(1), Some(2)]);
}

#[test]
fn global_matching_beats_greedy_rec_order() {
    // g0 near e1; e0 is nearer to g1 than greedy (rec order) would allow by
    // taking g0 first and stranding e1.
    let gates = [(0.0, 0.0), (10.0, 0.0)];
    let triggers = [(4, 0, 4, 0), (1, 0, 1, 0)]; // e0 rec0, e1 rec1
    let recs = [0usize, 1];
    let a = match_triggers_to_gates(&triggers, &recs, &gates, 6.0);
    // Global optimum binds both: e0->g1 (6.0), e1->g0 (1.0).
    assert_eq!(a, vec![Some(1), Some(0)]);
}

#[test]
fn threshold_leaves_distant_trigger_unbound() {
    let gates = [(0.0, 0.0)];
    let triggers = [(50, 50, 50, 50)];
    let recs = [7usize];
    assert_eq!(match_triggers_to_gates(&triggers, &recs, &gates, MAX_GATE_TRIGGER_DIST), vec![None]);
}

#[test]
fn point_and_rectangular_trigger_centers() {
    assert_eq!(trigger_center(12, 5, 12, 5), [12.0, 5.0]);
    assert_eq!(trigger_center(136, 10, 146, 14), [141.0, 12.0]);
    assert_eq!(trigger_center(14, 7, 12, 5), [13.0, 6.0]);
}

#[test]
fn dist_point_to_rect_handles_points_and_ranges() {
    // Point trigger.
    assert_eq!(dist_point_to_rect(12.0, 5.0, 12, 5, 12, 5), 0.0);
    assert_eq!(dist_point_to_rect(15.0, 9.0, 12, 5, 12, 5), 5.0);
    // Inside a normalized range.
    assert_eq!(dist_point_to_rect(12.0, 5.0, 14, 7, 0, 0), 0.0);
    // Nearest corner when outside; unnormalized corners are accepted.
    assert_eq!(dist_point_to_rect(0.0, 0.0, 14, 7, 12, 5), 13.0);
}

#[test]
fn tie_break_is_deterministic_by_rec_then_gate() {
    // Two identical gates, two coincident triggers of equal cost: the winner
    // must be the lexicographically smallest (rec, gate) pairing.
    let gates = [(0.0, 0.0), (0.0, 0.0)];
    let triggers = [(0, 0, 0, 0), (0, 0, 0, 0)];
    let recs = [5usize, 3]; // local 0 = rec5, local 1 = rec3
    let a = match_triggers_to_gates(&triggers, &recs, &gates, 1.0);
    // rec3 -> gate0, rec5 -> gate1.
    assert_eq!(a, vec![Some(1), Some(0)]);
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
