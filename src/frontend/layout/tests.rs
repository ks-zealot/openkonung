//! Unit tests for the [`crate::frontend::layout`] module.

use super::*;

#[test]
fn main_menu_ordering_and_new_game_index() {
    assert_eq!(MainMenuAction::NewGame.index(), 3);
    assert_eq!(MainMenuAction::from_index(3), Some(MainMenuAction::NewGame));
    for (i, a) in MainMenuAction::ALL.iter().enumerate() {
        assert_eq!(a.index(), i);
    }
}

#[test]
fn main_menu_pairs_match_task_table() {
    let expect = [
        (3, 4, MainMenuAction::Resume),
        (5, 6, MainMenuAction::Save),
        (7, 8, MainMenuAction::Load),
        (9, 10, MainMenuAction::NewGame),
        (11, 12, MainMenuAction::Settings),
        (13, 14, MainMenuAction::Exit),
    ];
    for (i, (n, h, act)) in expect.iter().enumerate() {
        let b = &MAIN_MENU[i];
        assert_eq!((b.normal_image, b.hover_image, b.action), (*n, *h, *act));
    }
}

#[test]
fn main_menu_640_rects_match_recovered_table() {
    let expect = [
        (233, 130, 192, 31),
        (231, 164, 173, 35),
        (212, 200, 178, 40),
        (224, 241, 128, 40),
        (215, 282, 118, 39),
        (222, 327, 78, 44),
    ];
    for (i, (x, y, w, h)) in expect.iter().enumerate() {
        assert_eq!(
            MAIN_MENU[i].rect_640,
            Rect::new(*x, *y, *w, *h),
            "button {i}"
        );
    }
}

#[test]
fn hero_hit_rects_match_task_section_16() {
    // task §16 #0/#1/#2 (640).
    assert_eq!(NEWH_HIT_640[0], Rect::new(44, 94, 57, 96));
    assert_eq!(NEWH_HIT_640[1], Rect::new(137, 94, 58, 96));
    assert_eq!(NEWH_HIT_640[2], Rect::new(229, 94, 58, 96));
}

#[test]
fn bottom_regions_match_task_section_20() {
    assert_eq!(NEWH_HIT_640[BOTTOM_REGIONS[0]], Rect::new(65, 427, 100, 33));
    assert_eq!(NEWH_HIT_640[BOTTOM_REGIONS[1]], Rect::new(175, 427, 100, 33));
}

#[test]
fn newh_tables_have_33_regions() {
    assert_eq!(NEWH_HIT_640.len(), 33);
    assert_eq!(NEWH_HIT_800.len(), 33);
    assert_eq!(NEWH_DRAW_640.len(), 33);
    assert_eq!(NEWH_DRAW_800.len(), 33);
}

#[test]
fn points_box_and_steppers_match_recovered_tables() {
    // Region 24 (top ПАРАМЕТРЫ points box).
    assert_eq!(NEWH_DRAW_640[NEWH_POINTS_REGION], Rect::new(463, 134, 30, 15));
    assert_eq!(NEWH_DRAW_800[NEWH_POINTS_REGION], Rect::new(543, 164, 34, 15));
    // Steppers: #25/26 CHA, #27/28 STR, #29/30 DEX, #31/32 END.
    assert_eq!(FrontendRes::R800.newh_stepper(0), (Rect::new(515, 195, 30, 15), Rect::new(575, 195, 30, 15)));
    assert_eq!(FrontendRes::R800.newh_stepper(1), (Rect::new(515, 218, 30, 15), Rect::new(575, 218, 30, 15)));
    assert_eq!(FrontendRes::R800.newh_stepper(2), (Rect::new(515, 239, 30, 15), Rect::new(575, 239, 30, 15)));
    assert_eq!(FrontendRes::R800.newh_stepper(3), (Rect::new(515, 259, 30, 15), Rect::new(575, 259, 30, 15)));
    assert_eq!(FrontendRes::R640.newh_stepper(0), (Rect::new(435, 160, 30, 15), Rect::new(485, 160, 40, 15)));
    assert_eq!(FrontendRes::R640.newh_stepper(3), (Rect::new(435, 214, 30, 15), Rect::new(485, 214, 40, 15)));
}

#[test]
fn skill_selector_hitboxes_match_verified_rects() {
    let r640 = [
        (356, 333, 98, 20),
        (356, 350, 98, 20),
        (356, 387, 125, 20),
        (356, 404, 125, 20),
        (361, 421, 120, 20),
    ];
    for (i, (x, y, w, h)) in r640.iter().enumerate() {
        assert_eq!(NEWH_HIT_640[15 + i], Rect::new(*x, *y, *w, *h), "640 #{i}");
    }
    let r800 = [
        (428, 413, 134, 20),
        (428, 434, 134, 20),
        (428, 466, 134, 20),
        (428, 487, 134, 20),
        (428, 509, 134, 20),
    ];
    for (i, (x, y, w, h)) in r800.iter().enumerate() {
        assert_eq!(NEWH_HIT_800[15 + i], Rect::new(*x, *y, *w, *h), "800 #{i}");
    }
}

#[test]
fn rect_inclusive_contains() {
    let r = Rect::new(10, 20, 5, 4);
    assert!(r.contains(10, 20));
    assert!(r.contains(14, 23));
    assert!(!r.contains(15, 23));
    assert!(!r.contains(14, 24));
    assert!(!r.contains(9, 20));
}
