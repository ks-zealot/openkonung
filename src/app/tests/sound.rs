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

/// `0x4365d4` rolls `0x62 < rand()%100` (1%, roll 99 only), not 2%.
#[test]
fn ambient_rate_is_one_percent_per_game_frame() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.current_map = 4;
    app.sound_rng = crate::combat::OriginalRng::new(0x1234_5678);
    app.sounds.clear();
    for _ in 0..20_000 {
        app.tick_ambient_sound();
    }
    let queued = app.sounds.len();
    assert!(
        (120..=280).contains(&queued),
        "expected about 200 ambient events in 20k frames (1%), got {queued}"
    );
    let base = crate::sound::events::ambient(4, 0) as u32;
    assert!(app
        .sounds
        .iter()
        .all(|e| (base..base + 8).contains(&(e.id as u32))));
}

/// The ambient call is inside `FUN_00436214` case 0: no interface window plays it.
#[test]
fn ambient_is_silent_outside_world_mode() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.current_map = 4;
    app.sound_rng = crate::combat::OriginalRng::new(7);
    for window in ["character", "map", "quest"] {
        app.character_screen = window == "character";
        app.map_open = window == "map";
        app.quest_log_open = window == "quest";
        app.sounds.clear();
        for _ in 0..5_000 {
            app.tick_ambient_sound();
        }
        assert!(
            app.sounds.is_empty(),
            "ambient played while {window} window was open"
        );
    }
}

/// The ambient roll runs once per 60 ms original frame, not per rendered step.
#[test]
fn ambient_ticks_only_on_original_frames() {
    let Some(mut app) = build_test_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.current_map = 4;
    app.sound_rng = crate::combat::OriginalRng::new(0x2B2B_2B2B);
    let base = crate::sound::events::ambient(4, 0) as u32;
    let mut ambient = 0;
    // 500 × 1 ms = 0.5 s ≈ 8 original frames; at 1% that is almost always 0.
    // A per-render-step tick would produce ~5 events here.
    for _ in 0..500 {
        app.sounds.clear();
        app.step(0.001);
        ambient += app
            .sounds
            .iter()
            .filter(|e| (base..base + 8).contains(&(e.id as u32)))
            .count();
    }
    assert!(
        ambient <= 2,
        "ambient ran per rendered step instead of per 60 ms frame ({ambient} events)"
    );
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
