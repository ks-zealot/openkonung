//! HUD button strip, map screen, discovery and quest-log tests.

use super::super::*;

use super::common::*;

/// A fresh game has no discovered world-map location and both windows closed.
#[test]
fn new_game_starts_with_no_discovered_locations() {
    let Some(app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    assert!(app.discovered_locations.iter().all(|&b| b == 0));
    assert!(!app.map_open);
    assert!(!app.quest_log_open);
}

/// A class-11 subtype-0 map item reveals exactly one non-reserved location and is consumed.
#[test]
fn map_item_reveals_exactly_one_allowed_location_and_is_consumed() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let game1 = std::fs::read(install_dir().join("Game.1")).unwrap();
    app.world.res.set_save_game(&game1);
    // Synthesize the quest-spawned class-11/subtype-0 map item.
    let id = 1u16;
    assert!(app.world.res.set_item_class(id, 11));
    assert!(app.world.res.set_item_subtype(id, 0));
    assert_eq!(app.world.res.item_class_subtype(id), Some((11, 0)));

    assert!(app.use_map_item(id));
    let count = app.discovered_locations.iter().filter(|&&b| b != 0).count();
    assert_eq!(count, 1);
    assert!(app
        .discovered_locations
        .iter()
        .zip(crate::world_map::MAP_RESERVED.iter())
        .all(|(&d, &reserved)| reserved != 0 || d == 0 || d == 1));
    let revealed = app
        .discovered_locations
        .iter()
        .position(|&b| b != 0)
        .unwrap();
    assert_eq!(crate::world_map::MAP_RESERVED[revealed], 0);
    assert_eq!(app.world.res.item_class_subtype(id), Some((0xFF, 0)));
}

/// `hud::button_at` reproduces the `0x457740`/`0x45437C` row/run tables at both widths.
///
/// The sampled points hit codes 1..7 (weapon plaque, stance, map, halt, engage,
/// quest log, character screen) for the 800 and 640 layouts.
#[test]
fn hud_buttons_match_the_original_hit_table() {
    // 800: exact row runs from `0x457740`/`0x45437C`.
    assert_eq!(crate::hud::button_at(60, 355, 800), Some(1)); // weapon plaque
    assert_eq!(crate::hud::button_at(10, 380, 800), Some(2)); // stance
    assert_eq!(crate::hud::button_at(130, 380, 800), Some(3)); // map
    assert_eq!(crate::hud::button_at(20, 460, 800), Some(4)); // halt
    assert_eq!(crate::hud::button_at(120, 460, 800), Some(5)); // engage
    assert_eq!(crate::hud::button_at(30, 560, 800), Some(6)); // quest
    assert_eq!(crate::hud::button_at(100, 560, 800), Some(7)); // character
                                                               // 640 variant.
    assert_eq!(crate::hud::button_at(60, 280, 640), Some(1));
    assert_eq!(crate::hud::button_at(5, 310, 640), Some(2));
    assert_eq!(crate::hud::button_at(105, 310, 640), Some(3));
    assert_eq!(crate::hud::button_at(10, 370, 640), Some(4));
    assert_eq!(crate::hud::button_at(100, 370, 640), Some(5));
    assert_eq!(crate::hud::button_at(10, 470, 640), Some(6));
    assert_eq!(crate::hud::button_at(80, 470, 640), Some(7));
}

/// `M` toggles the map, the compass icon closes it, and the book icon toggles the quest log.
#[test]
fn map_and_quest_toggle_from_keys_and_icons() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.key_down(KEY_M);
    assert!(app.map_open);
    app.key_down(KEY_M);
    assert!(!app.map_open);

    // Compass icon closes the open map (code 3).
    app.key_down(KEY_M);
    assert!(app.map_open);
    app.pointer_down(1, 130.0, 380.0);
    assert!(!app.map_open);

    // Book icon opens/closes the quest log (code 6).
    app.pointer_down(1, 30.0, 560.0);
    assert!(app.quest_log_open);
    app.pointer_down(1, 30.0, 560.0);
    assert!(!app.quest_log_open);
}

/// The weapon plaque cycles to the next occupied slot and an open map swallows world clicks.
#[test]
fn map_consumes_world_clicks_and_weapon_button_cycles_slots() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Equipment [0,0,1,...] and raw slot 0 -> next occupied slot is 2.
    app.player.equipment = [0, 0, 1, 0, 0, 0];
    app.player.raw_0x12 = 0;
    app.pointer_down(1, 60.0, 355.0);
    assert_eq!(app.player.raw_0x12, 2);

    // Clicks inside the map rectangle never issue a world move.
    app.key_down(KEY_M);
    app.pointer_down(1, 500.0, 300.0);
    assert!(app.target.is_none());
    assert!(app.map_open);
    app.key_down(KEY_ESCAPE);
    assert!(!app.map_open);
}

/// Synthetic destination locations for the world-map travel tests.
fn add_free_locations(app: &mut PlayableApp, ids: &[u32]) {
    for &id in ids {
        let loc = crate::loc::Location {
            loc_id: id,
            data: Vec::new(),
            props: Vec::new(),
            gates: Vec::new(),
            graph_placements: Vec::new(),
            collision: vec![0u16; 320 * 160],
            max_x: 319,
            max_y: 159,
            camera_right: 20000,
            camera_bottom: 12000,
        };
        let index = app.world.locs.len();
        app.world.locs.push(loc);
        app.world.loc_index.insert(id, index);
    }
}

fn test_edge(rec: usize, to: u32, arrival: [i32; 2]) -> crate::edges::Edge {
    crate::edges::Edge {
        rec,
        meta: 3,
        from: 4,
        to,
        src: arrival,
        dst: [0, 0],
        dst2: [0, 0],
        side: "?",
        profiles: vec![1],
    }
}

/// Client coordinates of the first map pixel whose run value is `run`.
fn map_click_at(run: u8) -> Option<(i32, i32)> {
    let (mx, my) = crate::world_map::point_for_location(run)?;
    let l = crate::hud::layout(VIRTUAL_W);
    let (dx, dy) = crate::world_map::overlay_offset(VIRTUAL_W);
    Some((l.viewport.x + dx + mx, l.viewport.y + dy + my))
}

/// Give the leader one class-11/subtype-20 jump orb in inventory slot `slot`.
fn give_jump_orb(app: &mut PlayableApp, slot: usize) -> u16 {
    let game1 = std::fs::read(install_dir().join("Game.1")).unwrap();
    app.world.res.set_save_game(&game1);
    let id = 1u16;
    assert!(app.world.res.set_item_class(id, 11));
    assert!(app.world.res.set_item_subtype(id, 20));
    app.player.inventory.fill(0);
    app.player.inventory[slot] = id;
    id
}

/// `0x42260C` code 3 (interface mode 5): a map click travels to the transition
/// record whose `dst (+0x04)` equals **`run + 1`** and consumes one class-11
/// subtype-20 «Шар магического прыжка» from the leader's inventory.
///
/// The edge list deliberately contains a record whose destination equals the
/// 0-based run value, first, so a regression to `edge.to == run` would travel
/// to the wrong location.
#[test]
fn map_travel_matches_run_plus_one_and_consumes_the_jump_orb() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_locations(&mut app, &[14, 15]);
    app.current_map = 4;
    // run 14 is location 15; the 0-based decoy comes first.
    app.edges = vec![test_edge(0, 14, [60, 60]), test_edge(1, 15, [40, 40])];
    app.discovered_locations = [0; 56];
    app.discovered_locations[14] = 1;

    give_jump_orb(&mut app, 2);
    app.player.inventory[3] = 9;
    app.map_open = true;

    let (vx, vy) = map_click_at(14).expect("run 14 has a map region");
    assert!(app.map_click(vx, vy));
    assert_eq!(app.current_map, 15, "destination is dst == run + 1");
    assert_eq!(
        app.player.inventory[2], 9,
        "FUN_00421AD0 shifts the following slots down"
    );
    assert_eq!(app.player.inventory[31], 0);
    assert!(!app.map_open, "a successful travel closes the map");
    assert_eq!((app.player_x, app.player_y), (40.0, 40.0));
    assert_eq!(app.player_dir, 3, "arrival facing is the record meta");
}

/// Without the jump orb in the leader's inventory the click is a no-op
/// (`FUN_00433A8C(0x15)` returns 0): no travel, no consumption, map stays open.
#[test]
fn map_travel_requires_the_jump_orb() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_locations(&mut app, &[14, 15]);
    app.current_map = 4;
    app.edges = vec![test_edge(1, 15, [40, 40])];
    app.discovered_locations = [0; 56];
    app.discovered_locations[14] = 1;
    app.map_open = true;

    let (vx, vy) = map_click_at(14).expect("run 14 has a map region");
    assert!(app.map_click(vx, vy));
    assert_eq!(app.current_map, 4, "no orb, no travel");
    assert!(app.map_open, "the map stays open");
}

/// The orb is spent *before* the record scan, so a discovered location with no
/// matching transition still consumes it (original `FUN_0042260C` order).
#[test]
fn map_travel_spends_the_orb_even_without_a_matching_record() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_locations(&mut app, &[14, 15]);
    app.current_map = 4;
    app.edges = vec![test_edge(0, 13, [10, 10])];
    app.discovered_locations = [0; 56];
    app.discovered_locations[14] = 1;
    give_jump_orb(&mut app, 0);
    app.map_open = true;

    let (vx, vy) = map_click_at(14).expect("run 14 has a map region");
    assert!(app.map_click(vx, vy));
    assert_eq!(app.current_map, 4, "no matching record, no travel");
    assert!(app.map_open);
    assert_eq!(app.player.inventory[0], 0, "the orb is still consumed");
}
