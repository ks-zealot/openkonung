//! Sound-queue placement tests (`sound/events.rs` + app hooks).

use super::super::*;

use super::common::*;

/// Count the UI-click events currently queued for the next frame.
fn ui_clicks(app: &PlayableApp) -> usize {
    app.sounds
        .iter()
        .filter(|e| e.id == crate::sound::events::UI_CLICK)
        .count()
}

/// Every HUD button code queues exactly one UI click.
#[test]
fn hud_commands_queue_exactly_one_click() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    for code in [
        crate::hud::CODE_WEAPON,
        crate::hud::CODE_STANCE,
        crate::hud::CODE_HALT,
        crate::hud::CODE_ENGAGE,
        crate::hud::CODE_MAP,
        crate::hud::CODE_QUEST,
        crate::hud::CODE_CHARACTER,
    ] {
        app.sounds.clear();
        assert!(app.press_hud_button(code));
        assert_eq!(ui_clicks(&app), 1, "HUD code {code} click count");
    }
}

/// `C` opens the character screen and queues one UI click; Escape closes it.
#[test]
fn character_screen_key_queues_one_click() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.sounds.clear();
    app.key_down(KEY_C);
    assert!(app.character_screen);
    assert_eq!(ui_clicks(&app), 1);
    app.sounds.clear();
    app.key_down(KEY_ESCAPE);
    assert!(!app.character_screen);
}

/// The stance key still plays its click when no unit is selected.
#[test]
fn stance_tab_queues_one_click_even_without_selection() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.party_selected = [false; crate::hud::PARTY_CELLS];
    app.sounds.clear();
    app.key_down(KEY_TAB);
    assert_eq!(ui_clicks(&app), 1);
}

/// Ambient picks stay inside the 8-clip bank of the current location and never fall silent.
#[test]
fn ambient_ids_stay_inside_the_location_banks() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.current_map = 4;
    app.sounds.clear();
    for _ in 0..20_000 {
        app.tick_ambient_sound();
    }
    let base = crate::sound::events::ambient(4, 0) as u32;
    assert!(app
        .sounds
        .iter()
        .all(|e| (base..base + 8).contains(&(e.id as u32))));
    assert!(!app.sounds.is_empty(), "ambient bank never queued a clip");
}

/// Pan is 62.5 per projected cell row and volume -100 per 16 px of distance.
///
/// Two rows away (32 px) therefore give volume -200 and pan 0; one row of row
/// offset gives pan 62 after rounding.
#[test]
fn positioned_sound_uses_the_original_pan_and_volume() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let player = (app.player_x.round() as i32, app.player_y.round() as i32);
    app.sounds.clear();
    // Same column, two rows away: projection distance 32 -> volume -200, pan 0.
    app.queue_sound_at(crate::sound::events::DODGE, (player.0 + 2, player.1));
    assert_eq!(app.sounds.len(), 1);
    let e = app.sounds[0];
    assert_eq!(e.id, crate::sound::events::DODGE);
    assert_eq!(e.volume, -200);
    assert_eq!(e.pan, 0);
    assert!(crate::sound::SAMPLE_RATES.contains(&e.rate));

    // A row offset produces the 62.5-per-cell pan.
    app.sounds.clear();
    app.queue_sound_at(crate::sound::events::DODGE, (player.0, player.1 + 1));
    assert_eq!(app.sounds[0].pan, 62);
}

/// A request whose volume would fall below the `-4000` cut-off is not queued.
#[test]
fn inaudible_requests_are_dropped() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Projected distance for a 200x100 cell gap far exceeds 1600 px.
    app.sounds.clear();
    let player = (app.player_x.round() as i32, app.player_y.round() as i32);
    app.queue_sound_at(
        crate::sound::events::DODGE,
        (player.0 + 200, player.1 + 100),
    );
    assert!(app.sounds.is_empty());
}

/// A centred, non-positional sound keeps volume 0 and pan 0.
#[test]
fn centred_sounds_keep_full_volume() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.sounds.clear();
    app.queue_sound(crate::sound::events::UI_CLICK);
    assert_eq!(app.sounds.len(), 1);
    assert_eq!(app.sounds[0].volume, 0);
    assert_eq!(app.sounds[0].pan, 0);
}
