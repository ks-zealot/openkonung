//! HUD button strip, map screen, discovery and quest-log tests.

use super::super::*;

use super::common::*;

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
