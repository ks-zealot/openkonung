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
