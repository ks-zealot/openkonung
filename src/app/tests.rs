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
fn cursor_class_mapping_matches_original_jump_table() {
    use super::cursor::cursor_for_class;
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
fn build_movement_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::loc::Location;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let res = Res::new(game, graph, objects, exe);
    let loc = Location {
        loc_id: 1,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let world = World::new(res, vec![loc]);
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
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
        100.0,
        80.0,
        0,
        None,
        Vec::new(),
    ))
}

/// Screen coordinate of runtime cell `(cx, cy)` under the app's current pan.
fn cell_screen(app: &PlayableApp, cx: i32, cy: i32) -> (f64, f64) {
    let (lx, ly) = crate::coords::entity_to_local_px_f(cx as f64, cy as f64);
    (app.detail_pan[0] + lx, app.detail_pan[1] + ly)
}

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
fn build_party_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let konung = std::fs::read(dir.join("konung.res")).ok()?;
    let game1 = std::fs::read(dir.join("Game.1")).ok()?;
    let quest = ["Quest_r.res", "QUEST_R.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok())?;
    let tables = crate::population::parse_tables(&game1).ok()?;
    let settlements = crate::population::parse_settlements(&game1).unwrap_or_default();
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    let _ = res.load_quest(&quest);
    let world = World::new(res, Vec::new());
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
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
        4,
        0.0,
        0.0,
        5,
        Some(tables),
        settlements,
    ))
}

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

/// A party app with the real `Game.1` tables + `QUEST_R.RES` and one synthetic,
/// fully-free location (id `1`), so a world click issues real routes to the
/// leader and its selected companions. Player starts at `(100, 80)`.
fn build_party_move_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::loc::Location;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let konung = std::fs::read(dir.join("konung.res")).ok()?;
    let game1 = std::fs::read(dir.join("Game.1")).ok()?;
    let quest = ["Quest_r.res", "QUEST_R.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok())?;
    let tables = crate::population::parse_tables(&game1).ok()?;
    let settlements = crate::population::parse_settlements(&game1).unwrap_or_default();
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    let _ = res.load_quest(&quest);
    let loc = Location {
        loc_id: 1,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let world = World::new(res, vec![loc]);
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
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
        100.0,
        80.0,
        0,
        Some(tables),
        settlements,
    ))
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

