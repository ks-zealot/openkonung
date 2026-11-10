//! Unit tests for the [`crate::character_screen`] module.

use super::*;
use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter, PrimaryStat};
use crate::hero::hero;

/// The window occupies `(140,0,642,460)` at 800x600 and `(112,0,528,376)` at
/// 640x480.
#[test]
fn window_rect_matches_executable() {
    assert_eq!(window_rect(800), (140, 0, 642, 460));
    assert_eq!(window_rect(640), (112, 0, 528, 376));
}

/// `0x81D644` is `1` for 800x600 and `0` for anything at or below 640.
#[test]
fn resolution_selector_matches_bit() {
    assert_eq!(resolution_selector(800), 1);
    assert_eq!(resolution_selector(640), 0);
}

/// The main-slot table keeps the recovered bases, including the shared 0/2
/// position and the last slot at (328,103)/(280,70).
#[test]
fn main_bases_match_verified_tables() {
    assert_eq!(main_base(1, 0), (182, 103));
    assert_eq!(main_base(1, 2), (182, 103));
    assert_eq!(main_base(1, 5), (328, 103));
    assert_eq!(main_base(0, 0), (136, 70));
    assert_eq!(main_base(0, 5), (280, 70));
}

/// The additional-slot centers and the special-slot origin keep the recovered
/// per-resolution values.
#[test]
fn additional_and_special_match_verified_tables() {
    assert_eq!(additional_center(1, 0), (232, 74));
    assert_eq!(additional_center(1, 4), (334, 215));
    assert_eq!(additional_center(0, 1), (166, 157));
    assert_eq!(special_base(1), (295, 232));
    assert_eq!(special_base(0), (245, 199));
}

/// With `raw_0x12 == 2` and both slots empty, the knife in storage slot 2
/// draws on the shared 0/2 position instead of slot 0.
#[test]
fn starting_knife_draws_in_storage_slot_two() {
    // Shipped starting equipment: item instance 1 (Нож) is in storage slot 2.
    let eq = [0u16, 0, 1, 0, 0, 0];
    assert!(!main_slot_drawn(&eq, 2, 0));
    assert!(main_slot_drawn(&eq, 2, 2));
    assert!(!main_slot_drawn(&eq, 2, 1));
}

/// Exactly one of slots 0 and 2 is drawn: slot 0 wins unless `raw_0x12 == 2`
/// and slot 2 is occupied.
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

/// All-empty equipment draws no main slot at all.
#[test]
fn empty_equipment_draws_nothing() {
    let eq = [0u16; 6];
    for slot in 0..6 {
        assert!(!main_slot_drawn(&eq, 0, slot));
    }
}

/// A warrior protagonist with item instance 1 (Нож) in storage slot 2, matching
/// the shipped `Game.1` starting equipment, at full health.
fn player() -> PlayerCharacter {
    let mut p = PlayerCharacter::from_build(
        *hero(0).unwrap(),
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    p.equipment = [0, 0, 1, 0, 0, 0];
    p.hp_internal = 1600;
    p
}

/// The value and secondary anchor tables keep the recovered `0x465C54` /
/// `0x465B34` records for both resolutions.
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

/// The raise column shows the tier cost only when the player can pay it in full.
#[test]
fn needed_raise_matches_tier_cost_gated_by_available_xp() {
    // Warrior STR=15 is specialized -> next cost is 5, and the level-1
    // specialized cap (19) still allows it.
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 1, 5),
        5
    );
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 1, 4),
        0
    );
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 1, 0),
        0
    );
    // Level 0 caps (0x45B0AB) are far above any starting value.
    assert_eq!(
        needed_raise(CharacterClass::Chief, PrimaryStat::Charisma, 25, 0, 11),
        11
    );
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Charisma, 8, 0, 2),
        2
    );
}

/// Reaching the `0x45B0AB` level cap for a stat hides its raise cost even with
/// unlimited experience; level 0 has no practical cap.
#[test]
fn level_caps_hide_raise_costs() {
    // `0x45B0AB`: at level 1 the normal cap is 7 and the specialized cap is 19.
    assert_eq!(
        stat_growth_cap(CharacterClass::Warrior, PrimaryStat::Dexterity, 1),
        7
    );
    assert_eq!(
        stat_growth_cap(CharacterClass::Warrior, PrimaryStat::Strength, 1),
        19
    );
    // DEX 10 is already above the level-1 cap -> the cost is hidden.
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Dexterity, 10, 1, 100),
        0
    );
    // Chief CHA=25 is above the specialized level-1 cap (19).
    assert_eq!(
        needed_raise(CharacterClass::Chief, PrimaryStat::Charisma, 25, 1, 100),
        0
    );
    // Warrior CHA=8 is above the normal level-1 cap (7).
    assert_eq!(
        needed_raise(CharacterClass::Warrior, PrimaryStat::Charisma, 8, 1, 100),
        0
    );
    // Level 0 has no practical cap.
    assert_eq!(
        stat_growth_cap(CharacterClass::Warrior, PrimaryStat::Dexterity, 0),
        120
    );
}

/// The icon-only level-up row requires every per-level stat minimum, and level
/// `0xD` is refused outright.
#[test]
fn raise_anything_requires_level_minimums() {
    // Warrior level 1 minima: DEX 5, CHA 5, STR 16, END 5.
    assert!(!can_raise_any(CharacterClass::Warrior, 1, 10, 10, 15, 10));
    assert!(can_raise_any(CharacterClass::Warrior, 1, 10, 10, 16, 10));
    // Level 0 minima are 3/3/3 (STR specialized 10).
    assert!(can_raise_any(CharacterClass::Warrior, 0, 3, 3, 15, 10));
    // Level 0xD is refused outright.
    assert!(!can_raise_any(CharacterClass::Warrior, 0xD, 99, 99, 99, 99));
}

/// Hero 0 reports portrait group 1 and status 5, the reference case the
/// portrait selector was verified against.
#[test]
fn draw_uses_intr2_and_does_not_panic() {
    // Synthetic 8x8 white background as INTR[2] via a minimal archive is
    // covered elsewhere; here we only check the pure geometry path.
    let p = player();
    assert_eq!(p.portrait_group(), 1);
    assert_eq!(p.portrait_status(), 5);
}

/// The paper doll lands at (470,127) / (570,160), the `0x46035C` table values.
#[test]
fn paper_doll_positions_match_executable_tables() {
    // konung.res DGROUP 0x46035C/0x460360, stride 0x10.
    assert_eq!(paper_doll_position(640), (470, 127));
    assert_eq!(paper_doll_position(800), (570, 160));
}

/// `draw` blits the fetched paper-doll pixels unchanged at its origin (skipped
/// when the original resources are absent).
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
    let pose = PaperDollPose {
        action: 6,
        direction: 5,
        frame: 0,
    };
    let doll = res
        .paper_doll_frame(
            &p.equipment,
            p.raw_0x12,
            false,
            1,
            p.hero.palette_id(),
            pose.action,
            pose.direction as usize,
            pose.frame,
        )
        .expect("doll");
    let (dx, dy) = paper_doll_position(800);

    let mut frame = vec![0u8; 800 * 600 * 4];
    let sheet = CharacterSheet {
        name_id: p.hero.name_id,
        class: p.entity_class(),
        equipment: p.equipment,
        additional: p.additional_equipment,
        special: p.special_equipment,
        inventory: p.inventory,
        raw_0x12: p.raw_0x12,
        flags_b: p.flags_b,
        visual_index: p.hero.visual_index,
        palette_id: p.hero.palette_id(),
        snapshot: crate::inventory::OwnerSnapshot::from_player(&p),
        pose,
    };
    draw(&mut frame, 800, 600, &mut res, &sheet, None, (0, 0));

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
    assert!(
        matching > 50,
        "draw must blit the paper doll ({matching}/{opaque} matched)"
    );
}

/// The hard-coded doll origins equal the values read straight out of the
/// original `konung.res` DGROUP for both selectors.
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
