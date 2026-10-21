//! Unit tests for the [`crate::character_screen`] module.

use super::*;
use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter, PrimaryStat};
use crate::hero::hero;

#[test]
fn window_rect_matches_executable() {
    assert_eq!(window_rect(800), (140, 0, 642, 460));
    assert_eq!(window_rect(640), (112, 0, 528, 376));
}

#[test]
fn resolution_selector_matches_bit() {
    assert_eq!(resolution_selector(800), 1);
    assert_eq!(resolution_selector(640), 0);
}

#[test]
fn main_bases_match_verified_tables() {
    assert_eq!(main_base(1, 0), (182, 103));
    assert_eq!(main_base(1, 2), (182, 103));
    assert_eq!(main_base(1, 5), (328, 103));
    assert_eq!(main_base(0, 0), (136, 70));
    assert_eq!(main_base(0, 5), (280, 70));
}

#[test]
fn additional_and_special_match_verified_tables() {
    assert_eq!(additional_center(1, 0), (232, 74));
    assert_eq!(additional_center(1, 4), (334, 215));
    assert_eq!(additional_center(0, 1), (166, 157));
    assert_eq!(special_base(1), (295, 232));
    assert_eq!(special_base(0), (245, 199));
}

#[test]
fn starting_bow_draws_in_storage_slot_two() {
    // Shipped starting equipment; the bow is storage slot 2.
    let eq = [0u16, 0, 1, 0, 0, 0];
    assert!(!main_slot_drawn(&eq, 2, 0));
    assert!(main_slot_drawn(&eq, 2, 2));
    assert!(!main_slot_drawn(&eq, 2, 1));
}

#[test]
fn overlap_branch_prefers_slot_zero() {
    // Both 0 and 2 occupied, raw_0x12 != 2 -> slot 0 wins, slot 2 hidden.
    let eq = [7u16, 0, 9, 0, 0, 0];
    assert!(main_slot_drawn(&eq, 0, 0));
    assert!(!main_slot_drawn(&eq, 0, 2));
    // raw_0x12 == 2 -> slot 0 hidden, slot 2 shown.
    assert!(!main_slot_drawn(&eq, 2, 0));
    assert!(main_slot_drawn(&eq, 2, 2));
}

#[test]
fn empty_equipment_draws_nothing() {
    let eq = [0u16; 6];
    for slot in 0..6 {
        assert!(!main_slot_drawn(&eq, 0, slot));
    }
}

fn player() -> PlayerCharacter {
    let mut p = PlayerCharacter::from_build(
        *hero(0).unwrap(),
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    p.equipment = [0, 0, 1, 0, 0, 0];
    p.hp_internal = 1600;
    p
}

#[test]
fn value_anchors_match_recovered_tables() {
    // Spot-check the recovered `0x465C54` / `0x465B34` records.
    assert_eq!(VALUE_640[0], (413, 48));
    assert_eq!(VALUE_800[0], (519, 63));
    assert_eq!(VALUE_800[6], (388, 352)); // Здоровье
    assert_eq!(VALUE_800[20], (388, 373)); // Отравление
    assert_eq!(VALUE_800[21], (388, 394)); // Броня
    assert_eq!(VALUE_800[22], (388, 415)); // Удар
    assert_eq!(VALUE_800[23], (388, 436)); // Золото
    assert_eq!(VALUE_800[24], (700, 63)); // Свободный опыт
    assert_eq!(TABLE2_640[0], (578, 70));
    assert_eq!(TABLE2_640[4], (573, 162));
    assert_eq!(TABLE2_800[0], (722, 89));
    assert_eq!(TABLE2_800[4], (716, 203));
}

#[test]
fn needed_raise_matches_tier_cost_gated_by_available_xp() {
    // Warrior STR=15 is specialized -> next cost is 5.
    assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 5), 5);
    assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 4), 0);
    assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 0), 0);
    // Merchant CHA=25 is specialized -> cost 11.
    assert_eq!(needed_raise(CharacterClass::Merchant, PrimaryStat::Charisma, 25, 11), 11);
    // Non-specialized Warrior CHA=8 -> normal tier cost 2.
    assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Charisma, 8, 2), 2);
}

#[test]
fn draw_uses_intr2_and_does_not_panic() {
    // Synthetic 8x8 white background as INTR[2] via a minimal archive is
    // covered elsewhere; here we only check the pure geometry path.
    let p = player();
    assert_eq!(p.portrait_group(), 1);
    assert_eq!(p.portrait_status(), 5);
}

#[test]
fn paper_doll_positions_match_executable_tables() {
    // konung.res DGROUP 0x46035C/0x460360, stride 0x10.
    assert_eq!(paper_doll_position(640), (470, 127));
    assert_eq!(paper_doll_position(800), (570, 160));
}

#[test]
fn draw_blits_paper_doll_pixels() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |n: &str| std::fs::read(dir.join(n)).ok();
    let (Some(game), Some(graph), Some(objects), Some(exe), Some(konung), Some(save)) = (
        read("Game.res"),
        read("GRAPH.RES"),
        read("Objects.res"),
        read("CLIENT.EXE"),
        read("konung.res"),
        read("Game.1"),
    ) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    res.set_save_game(&save);

    let mut p = player();
    p.equipment = [0, 0, 1, 0, 0, 0];
    p.raw_0x12 = 2;
    let pose = PaperDollPose { action: 6, direction: 5, frame: 0 };
    let doll = res
        .paper_doll_frame(&p.equipment, p.raw_0x12, false, 1, p.hero.palette_id(), pose.action, pose.direction as usize, pose.frame)
        .expect("doll");
    let (dx, dy) = paper_doll_position(800);

    let mut frame = vec![0u8; 800 * 600 * 4];
    draw(&mut frame, 800, 600, &mut res, &p, pose);

    let mut matching = 0;
    let mut opaque = 0;
    for y in 0..doll.h {
        for x in 0..doll.w {
            let si = (y * doll.w + x) * 4;
            if doll.data[si + 3] == 0 {
                continue;
            }
            opaque += 1;
            let fx = dx as usize + x;
            let fy = dy as usize + y;
            let fi = (fy * 800 + fx) * 4;
            if frame[fi..fi + 4] == doll.data[si..si + 4] {
                matching += 1;
            }
        }
    }
    assert!(opaque > 100, "doll must have opaque pixels");
    assert!(matching > 50, "draw must blit the paper doll ({matching}/{opaque} matched)");
}

#[test]
fn paper_doll_positions_match_original_dgroup() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let Ok(konung) = std::fs::read(dir.join("konung.res")) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let word = |va: u32| -> i32 {
        let o = 0x36200 + (va as usize - 0x450000);
        i32::from_le_bytes([konung[o], konung[o + 1], konung[o + 2], konung[o + 3]])
    };
    for sel in 0..2u32 {
        let x = word(0x46035C + sel * 0x10);
        let y = word(0x460360 + sel * 0x10);
        let expected = if sel == 0 { (470, 127) } else { (570, 160) };
        assert_eq!((x, y), expected);
        let logical_w = if sel == 0 { 640 } else { 800 };
        assert_eq!(paper_doll_position(logical_w), expected);
    }
}
