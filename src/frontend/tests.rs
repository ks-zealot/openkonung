//! Unit tests for the [`crate::frontend`] module.

use super::*;
use layout::{MENU_IMAGE_A, NEWH_HIT_640};

/// Build a 640x480 frontend from minimal synthetic MENU/NEWH resources.
fn synthetic_frontend() -> Frontend {
    // Minimal 15-image MENU and 13-image NEWH resources, 1x1 each.
    let mut menu_bytes = Vec::new();
    for _ in 0..15 {
        menu_bytes.extend_from_slice(&rec(1, 1, &[[0x00]]));
    }
    let mut newh_bytes = Vec::new();
    for _ in 0..13 {
        newh_bytes.extend_from_slice(&rec(1, 1, &[[0x00]]));
    }
    Frontend::new(
        FrontendRes::R640,
        "MENU640R.RES",
        &menu_bytes,
        "NEWH640R.RES",
        &newh_bytes,
        None,
        &[],
    )
    .unwrap()
}

/// Click the main-menu action with the given index at the 640 layout.
fn click_action(f: &mut Frontend, action: MainMenuAction) -> Option<FrontendCommand> {
    f.on_resize(640, 480);
    let r = MAIN_MENU[action.index()].rect_640;
    f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0)
}

/// Encode one opaque 1-pixel-per-row menu image record the decoder accepts.
fn rec(w: u16, h: u16, rows: &[[u8; 1]]) -> Vec<u8> {
    let mut out = vec![0x00, 0x00, 0x00, 0x05];
    let sum = rows.len();
    let payload = 4 + rows.len() * 2 + sum;
    out.extend_from_slice(&(payload as u32).to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    for _ in rows {
        out.extend_from_slice(&1u16.to_le_bytes());
    }
    for r in rows {
        out.extend_from_slice(r);
    }
    out
}

/// A fresh startup menu exposes only Load, New Game, Settings and Exit.
#[test]
fn startup_menu_omits_resume_and_save() {
    let visible = main_menu::visible_indices(false);
    assert_eq!(visible, vec![2, 3, 4, 5]);
    assert!(!visible.contains(&MainMenuAction::Resume.index()));
    assert!(!visible.contains(&MainMenuAction::Save.index()));
}

/// Clicking New Game moves the frontend to the New-Hero screen without a command.
#[test]
fn new_game_switches_to_new_hero() {
    let mut f = synthetic_frontend();
    assert_eq!(f.screen, FrontendScreen::MainMenu);
    // Click the NewGame rect.
    let r = MAIN_MENU[MainMenuAction::NewGame.index()].rect_640;
    f.on_resize(640, 480);
    let cmd = f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0);
    assert_eq!(cmd, None);
    assert_eq!(f.screen, FrontendScreen::NewHero);
}

/// The New-Hero default draft is Dragomir the Warrior with 3/15/3/10 and 100 fencing.
#[test]
fn new_hero_defaults_match_original_init() {
    use crate::character::CharacterClass;
    let s = NewHeroState::default();
    assert_eq!(s.selected_hero, Some(0)); // Dragomir
    assert_eq!(s.selected_class, Some(CharacterClass::Warrior));
    let b = s.build.unwrap();
    assert_eq!(
        (b.charisma, b.strength, b.dexterity, b.endurance),
        (3, 15, 3, 10)
    );
    assert_eq!(b.melee_mastery, 100);
    assert_eq!(b.archery_mastery, 0);
    assert_eq!(b.identification, 20);
}

/// Confirmed edits travel verbatim in the StartGame command.
#[test]
fn hero_and_class_selection_then_confirm_starts() {
    use crate::character::CharacterClass;
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::NewHero;
    f.new_hero = NewHeroState::default();
    f.on_resize(640, 480);
    let ok = layout::NEWH_HIT_640[layout::BOTTOM_OK_REGION];
    let click = |f: &mut Frontend, r: crate::frontend::layout::Rect| {
        f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0)
    };

    // Original defaults are already confirmable.
    assert!(f.new_hero.can_confirm());

    // Select hero 1 (Mikhail).
    assert_eq!(click(&mut f, NEWH_HIT_640[1]), None);
    assert_eq!(f.new_hero.selected_hero, Some(1));

    // Select Merchant (region 3) -> preset applied.
    assert_eq!(
        click(&mut f, NEWH_HIT_640[CharacterClass::Merchant.region()]),
        None
    );
    assert_eq!(f.new_hero.selected_class, Some(CharacterClass::Merchant));

    // Modify the draft (stat/skill editing is future work; this exercises the
    // copy path that Start Game must honour, §10).
    {
        let b = f.new_hero.build.as_mut().unwrap();
        b.charisma = 24;
        b.trade = 11;
    }
    match click(&mut f, ok) {
        Some(FrontendCommand::StartGame { hero_index, build }) => {
            assert_eq!(hero_index, 1);
            assert_eq!(build.class, CharacterClass::Merchant);
            assert_eq!(build.charisma, 24);
            assert_eq!(build.trade, 11);
        }
        other => panic!("expected StartGame, got {other:?}"),
    }
}

/// The Cancel control returns to the main menu and issues no command.
#[test]
fn cancel_returns_to_main_menu() {
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::NewHero;
    f.new_hero = NewHeroState::default();
    f.on_resize(640, 480);
    let cancel = layout::NEWH_HIT_640[layout::BOTTOM_CANCEL_REGION];
    let cmd = f.pointer_down(1, cancel.x as f64 + 1.0, cancel.y as f64 + 1.0);
    assert_eq!(cmd, None);
    assert_eq!(f.screen, FrontendScreen::MainMenu);
}

/// Region 20 is Ok on the left and region 21 is Cancel on the right, with their image pairs.
#[test]
fn bottom_controls_are_ok_left_cancel_right() {
    // Original `FUN_0042EA6C`: region 20 (left) = Ok, region 21 (right) = Cancel.
    assert_eq!(layout::BOTTOM_OK_REGION, 20);
    assert_eq!(layout::BOTTOM_CANCEL_REGION, 21);
    assert_eq!(NEWH_HIT_640[layout::BOTTOM_OK_REGION].x, 65);
    assert_eq!(NEWH_HIT_640[layout::BOTTOM_CANCEL_REGION].x, 175);
    assert_eq!(layout::BOTTOM_IMAGE_PAIRS[0], (11, 12)); // Ok
    assert_eq!(layout::BOTTOM_IMAGE_PAIRS[1], (9, 10)); // Отмена
}

/// Portrait artwork follows selection only; hovering never swaps the image.
#[test]
fn hero_regions_pick_distinct_images_without_hover() {
    let mut s = NewHeroState::default();
    // Default selection = Dragomir.
    assert_eq!(s.hero_image(0, 0), 4);
    assert_eq!(s.hero_image(1, 1), 5);
    s.hovered_region = Some(1);
    assert_eq!(s.hero_image(1, 1), 5); // hover does not change the artwork
    s.selected_hero = Some(1);
    assert_eq!(s.hero_image(0, 0), 3);
    assert_eq!(s.hero_image(1, 1), 6);
}

/// Hover sets the description and an unrelated region keeps the last one shown.
#[test]
fn description_hover_normalises_and_persists() {
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::NewHero;
    f.new_hero = NewHeroState::default();
    f.on_resize(640, 480);
    let center = |r: crate::frontend::layout::Rect| {
        (r.x as f64 + r.w as f64 / 2.0, r.y as f64 + r.h as f64 / 2.0)
    };

    // Hero 0 description.
    let (x, y) = center(NEWH_HIT_640[0]);
    f.pointer_move(x, y);
    assert_eq!(f.new_hero.description_region, Some(0));

    // Stepper 25/26 -> Charisma row 7.
    let (x, y) = center(NEWH_HIT_640[25]);
    f.pointer_move(x, y);
    assert_eq!(f.new_hero.description_region, Some(7));

    // Moving over the big panel (region 23) keeps the last description.
    let (x, y) = center(NEWH_HIT_640[23]);
    f.pointer_move(x, y);
    assert_eq!(f.new_hero.description_region, Some(7));

    // Skill selector 15.
    let (x, y) = center(NEWH_HIT_640[15]);
    f.pointer_move(x, y);
    assert_eq!(f.new_hero.description_region, Some(15));
}

/// Combat/civil markers follow the skill thresholds, not the class preset.
#[test]
fn skill_markers_follow_skill_values() {
    use crate::character::CharacterClass;
    let mut s = NewHeroState::default();
    // Default Fencing + Identification: boxes 15 and 17.
    assert_eq!(s.skill_marker_regions().unwrap(), vec![15, 17]);

    s.select_class(CharacterClass::Hunter); // class does not change skills
    assert_eq!(s.skill_marker_regions().unwrap(), vec![15, 17]);

    s.select_combat(crate::character::CombatSpecialization::Marksmanship);
    assert_eq!(s.skill_marker_regions().unwrap(), vec![16, 17]);

    s.build.as_mut().unwrap().healing = 100;
    s.build.as_mut().unwrap().trade = 20;
    s.build.as_mut().unwrap().identification = 10;
    assert_eq!(s.skill_marker_regions().unwrap(), vec![16, 18, 19]);
}

/// Hero and specialization handlers set `Entity+0x12` exactly as the original does.
#[test]
fn preview_weapon_slot_matches_original_handlers() {
    use crate::character::CombatSpecialization;
    let mut s = NewHeroState::default();
    assert_eq!(s.preview_weapon_slot, 0);
    s.select_hero(1);
    assert_eq!(s.preview_weapon_slot, 2);
    s.select_hero(0);
    assert_eq!(s.preview_weapon_slot, 0);
    s.select_combat(CombatSpecialization::Marksmanship);
    assert_eq!(s.preview_weapon_slot, 1);
    s.select_combat(CombatSpecialization::Fencing);
    assert_eq!(s.preview_weapon_slot, 2);
    s.select_hero(1);
    assert_eq!(s.preview_weapon_slot, 2);
    s.select_combat(CombatSpecialization::Fencing);
    assert_eq!(s.preview_weapon_slot, 0);
}

/// A menu frame fills the RGBA buffer and the debug overlay adds labels.
#[test]
fn menu_draws_without_panicking() {
    let mut f = synthetic_frontend();
    f.step();
    assert_eq!(f.frame.len(), 640 * 480 * 4);
    f.debug_regions = true;
    f.step();
    assert!(!f.texts.is_empty());
}

/// `MENU#0` is the main-menu background image.
#[test]
fn background_image_is_index_zero() {
    assert_eq!(MENU_IMAGE_A, 0);
}

/// The host size follows `on_resize` and clamps degenerate sizes to 1x1.
#[test]
fn host_size_reflects_on_resize() {
    let mut f = synthetic_frontend();
    // Defaults to the logical resolution until JS reports the real host size.
    assert_eq!(f.host_size(), (640, 480));
    f.on_resize(1920, 1080);
    assert_eq!(f.host_size(), (1920, 1080));
    // Degenerate sizes are clamped to at least 1x1.
    f.on_resize(0, -5);
    assert_eq!(f.host_size(), (1, 1));
}

/// With a live game all six buttons, including Resume and Save, are visible.
#[test]
fn in_game_menu_exposes_all_six() {
    assert_eq!(main_menu::visible_indices(true), vec![0, 1, 2, 3, 4, 5]);
}

/// Escape from Settings returns to the menu with the live game untouched.
#[test]
fn settings_entry_and_back_preserves_game_active() {
    let mut f = synthetic_frontend();
    f.game_active = true;
    assert_eq!(click_action(&mut f, MainMenuAction::Settings), None);
    assert_eq!(f.screen, FrontendScreen::Settings);
    assert_eq!(f.key_down(KEY_ESCAPE), None);
    assert_eq!(f.screen, FrontendScreen::MainMenu);
    assert!(f.game_active);
}

/// Save and Load are routed states, and Escape returns to the main menu from both.
#[test]
fn save_and_load_entry_and_back() {
    let mut f = synthetic_frontend();
    f.game_active = true;
    assert_eq!(click_action(&mut f, MainMenuAction::Save), None);
    assert_eq!(f.screen, FrontendScreen::SaveGame);
    assert_eq!(f.key_down(KEY_ESCAPE), None);
    assert_eq!(f.screen, FrontendScreen::MainMenu);

    assert_eq!(click_action(&mut f, MainMenuAction::Load), None);
    assert_eq!(f.screen, FrontendScreen::LoadGame);
    assert_eq!(f.key_down(KEY_ESCAPE), None);
    assert_eq!(f.screen, FrontendScreen::MainMenu);
}

/// Resume is dispatched only when a live game exists; otherwise its rect is inert.
#[test]
fn resume_request_only_when_game_active() {
    let mut f = synthetic_frontend();
    f.game_active = true;
    assert_eq!(
        click_action(&mut f, MainMenuAction::Resume),
        Some(FrontendCommand::Resume)
    );
    // Fresh menu: Resume is not visible, so no action is hit.
    let mut g = synthetic_frontend();
    g.on_resize(640, 480);
    let r = MAIN_MENU[MainMenuAction::Resume.index()].rect_640;
    assert_eq!(g.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0), None);
}

/// Escape resumes a preserved game and quits only on the fresh startup menu.
#[test]
fn escape_resumes_with_game_and_quits_without() {
    let mut f = synthetic_frontend();
    f.game_active = true;
    assert_eq!(f.key_down(KEY_ESCAPE), Some(FrontendCommand::Resume));
    let mut g = synthetic_frontend();
    assert_eq!(g.key_down(KEY_ESCAPE), Some(FrontendCommand::Quit));
}

/// Entering and cancelling New Hero never clears the preserved game.
#[test]
fn new_hero_cancel_preserves_game_active() {
    let mut f = synthetic_frontend();
    f.game_active = true;
    assert_eq!(click_action(&mut f, MainMenuAction::NewGame), None);
    assert_eq!(f.screen, FrontendScreen::NewHero);
    assert!(
        f.game_active,
        "entering New Hero must not clear the live game"
    );
    f.on_resize(640, 480);
    let cancel = NEWH_HIT_640[layout::BOTTOM_CANCEL_REGION];
    assert_eq!(
        f.pointer_down(1, cancel.x as f64 + 1.0, cancel.y as f64 + 1.0),
        None
    );
    assert_eq!(f.screen, FrontendScreen::MainMenu);
    assert!(f.game_active);
}

/// The Settings and SAVE screens paint a full buffer at 640x480.
#[test]
fn settings_and_save_load_render_without_panicking() {
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::Settings;
    f.step();
    assert_eq!(f.frame.len(), 640 * 480 * 4);
    f.screen = FrontendScreen::SaveGame;
    f.step();
    f.screen = FrontendScreen::LoadGame;
    f.step();
    assert_eq!(f.res.width(), 640);
}

/// Diagnostic screen codes stay 0..4 for the five frontend screens.
#[test]
fn screen_codes_are_stable() {
    let mut f = synthetic_frontend();
    assert_eq!(f.screen_code(), 0);
    f.screen = FrontendScreen::NewHero;
    assert_eq!(f.screen_code(), 1);
    f.screen = FrontendScreen::Settings;
    assert_eq!(f.screen_code(), 2);
    f.screen = FrontendScreen::LoadGame;
    assert_eq!(f.screen_code(), 3);
    f.screen = FrontendScreen::SaveGame;
    assert_eq!(f.screen_code(), 4);
}

/// Regions 15..19 select the specialization and rewrite only its own skill block.
#[test]
fn skill_selector_regions_route_to_specializations() {
    use crate::character::{CivilSpecialization, CombatSpecialization};
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::NewHero;
    f.on_resize(640, 480);
    let click = |f: &mut Frontend, region: usize| {
        let r = NEWH_HIT_640[region];
        f.pointer_down(
            1,
            r.x as f64 + r.w as f64 / 2.0,
            r.y as f64 + r.h as f64 / 2.0,
        )
    };
    click(&mut f, 6); // Warrior
    let b = f.new_hero.build.unwrap();
    assert_eq!(b.combat_specialization, CombatSpecialization::Fencing);
    assert_eq!(b.civil_specialization, CivilSpecialization::Identification);

    click(&mut f, 16); // Marksmanship
    let b = f.new_hero.build.unwrap();
    assert_eq!(b.combat_specialization, CombatSpecialization::Marksmanship);
    assert_eq!(
        (b.melee_mastery, b.archery_mastery, b.building, b.smithing),
        (0, 100, 2, 5)
    );

    click(&mut f, 18); // Trade
    let b = f.new_hero.build.unwrap();
    assert_eq!(b.civil_specialization, CivilSpecialization::Trade);
    assert_eq!((b.trade, b.healing, b.identification), (20, 50, 10));
    // Combat fields untouched by the civil selection.
    assert_eq!(
        (b.melee_mastery, b.archery_mastery, b.building, b.smithing),
        (0, 100, 2, 5)
    );
}

/// Regions 13/14 are original no-op handlers and change no skill value.
#[test]
fn smithing_and_building_regions_are_noops() {
    let mut f = synthetic_frontend();
    f.screen = FrontendScreen::NewHero;
    f.on_resize(640, 480);
    let click = |f: &mut Frontend, region: usize| {
        let r = NEWH_HIT_640[region];
        f.pointer_down(
            1,
            r.x as f64 + r.w as f64 / 2.0,
            r.y as f64 + r.h as f64 / 2.0,
        )
    };
    click(&mut f, 6); // class
    let before = f.new_hero.build.unwrap().skills();
    click(&mut f, 13); // Smithing (no-op)
    click(&mut f, 14); // Building (no-op)
    assert_eq!(f.new_hero.build.unwrap().skills(), before);
}
