use super::*;
use crate::hero::hero;

#[test]
fn starting_loadout_is_the_loaded_game_entity_for_each_hero() {
    // The menu character-creation equipment arrays are NOT the in-game
    // loadout: the original loads Game.N and the field-restore does not copy
    // equipment/inventory. The shipped Game.N Entity[0] records are the
    // default (identical) loadout, so this verifies identity differs per
    // hero while the items come from the correct Game.N file.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for (idx, suffix) in [(0u32, 1u32), (1, 2), (2, 0)] {
        let h = hero(idx).unwrap();
        assert_eq!(h.game_suffix, suffix, "hero {idx} maps to Game.{suffix}");
        let path = dir.join(format!("Game.{suffix}"));
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("skipping: {} not present", path.display());
            return;
        };
        let t = crate::population::parse_tables(&data).unwrap();
        let mut pc = PlayerCharacter::from_build(
            *h,
            CharacterBuild::from_class(CharacterClass::Warrior),
        );
        pc.attach_starting_items(&t.entities[0]);
        assert_eq!(pc.equipment, [0, 0, 1, 0, 0, 0], "hero {idx} equipment");
        assert_eq!(&pc.inventory[0..7], &[2, 3, 4, 5, 6, 7, 0], "hero {idx} inventory");
        assert_eq!(pc.hp_internal, 1600, "hero {idx} hp");
        assert_eq!(pc.money, 100, "hero {idx} money");
        assert_eq!(pc.raw_0x12, 2, "hero {idx} +0x12");
        assert!(!pc.auto_enabled(), "hero {idx} AUTO off (flags_b & 2 == 0)");
        assert_eq!(pc.portrait_group(), h.visual_index, "hero {idx} portrait group");
    }
}

#[test]
fn attach_starting_items_copies_and_keeps_equipment_separate() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Game.1");
    let Ok(data) = std::fs::read(&p) else {
        eprintln!("skipping: Game.1 not present");
        return;
    };
    let t = crate::population::parse_tables(&data).unwrap();
    let mut pc = PlayerCharacter::from_build(
        *hero(0).unwrap(),
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    // A fresh build has no items.
    assert_eq!(pc.equipment, [0; 6]);
    assert_eq!(pc.inventory, [0; 32]);
    pc.attach_starting_items(&t.entities[0]);
    assert_eq!(pc.equipment, [0, 0, 1, 0, 0, 0]);
    assert_eq!(&pc.inventory[0..7], &[2, 3, 4, 5, 6, 7, 0]);
    assert_eq!(pc.additional_equipment, [0; 5]);
    assert_eq!(pc.special_equipment, 0);
    // The equipped item (1) is not duplicated into the inventory.
    assert!(!pc.inventory.contains(&1));
}

#[test]
fn exact_class_presets() {
    assert_eq!(CharacterClass::Warrior.entity_class(), 5);
    assert_eq!(CharacterClass::Hunter.entity_class(), 3);
    assert_eq!(CharacterClass::Merchant.entity_class(), 4);
    assert_eq!(CharacterClass::Chief.entity_class(), 2);

    let w = CharacterClass::Warrior.preset();
    assert_eq!((w.dexterity, w.charisma, w.strength, w.endurance), (3, 3, 15, 10));
    let h = CharacterClass::Hunter.preset();
    assert_eq!((h.dexterity, h.charisma, h.strength, h.endurance), (15, 3, 3, 10));
    let m = CharacterClass::Merchant.preset();
    assert_eq!((m.dexterity, m.charisma, m.strength, m.endurance), (3, 25, 7, 7));
    let c = CharacterClass::Chief.preset();
    assert_eq!((c.dexterity, c.charisma, c.strength, c.endurance), (3, 3, 10, 12));
}

#[test]
fn from_class_sets_base_equal_current_for_every_stat() {
    for class in CharacterClass::ALL {
        let pc = PlayerCharacter::from_build(*hero(0).unwrap(), CharacterBuild::from_class(class));
        assert_eq!(pc.dexterity_base, pc.dexterity_current, "{class:?} dex");
        assert_eq!(pc.charisma_base, pc.charisma_current, "{class:?} cha");
        assert_eq!(pc.strength_base, pc.strength_current, "{class:?} str");
        assert_eq!(pc.endurance_base, pc.endurance_current, "{class:?} end");
    }
}

#[test]
fn initial_skill_values() {
    let pc = PlayerCharacter::from_build(
        *hero(0).unwrap(),
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    assert_eq!(pc.melee_mastery, 100);
    assert_eq!(pc.trade, 10);
    assert_eq!(pc.archery_mastery, 0);
    assert_eq!(pc.building, 1);
    assert_eq!(pc.healing, 50);
    assert_eq!(pc.identification, 20);
    assert_eq!(pc.smithing, 10);
}

#[test]
fn modified_build_survives_without_preset_regeneration() {
    // §10: choose Merchant, then edit charisma 25 -> 24 and trade 10 -> 11.
    let mut build = CharacterBuild::from_class(CharacterClass::Merchant);
    build.charisma = 24;
    build.trade = 11;
    let pc = PlayerCharacter::from_build(*hero(1).unwrap(), build);
    assert_eq!(pc.class, CharacterClass::Merchant);
    assert_eq!(pc.entity_class(), 4);
    assert_eq!(pc.charisma_base, 24);
    assert_eq!(pc.charisma_current, 24);
    assert_eq!(pc.trade, 11);
    // The class preset (charisma 25) must not have been regenerated.
    assert_ne!(pc.charisma_current, CharacterClass::Merchant.preset().charisma);
}

#[test]
fn class_region_mapping() {
    // Original: region = entity_class + 1.
    assert_eq!(CharacterClass::from_region(3), Some(CharacterClass::Chief));
    assert_eq!(CharacterClass::from_region(4), Some(CharacterClass::Hunter));
    assert_eq!(CharacterClass::from_region(5), Some(CharacterClass::Merchant));
    assert_eq!(CharacterClass::from_region(6), Some(CharacterClass::Warrior));
    assert_eq!(CharacterClass::from_region(0), None);
    assert_eq!(CharacterClass::from_region(7), None);
    assert_eq!(CharacterClass::Chief.region(), 3);
    assert_eq!(CharacterClass::Hunter.region(), 4);
    assert_eq!(CharacterClass::Merchant.region(), 5);
    assert_eq!(CharacterClass::Warrior.region(), 6);
    for class in CharacterClass::ALL {
        assert_eq!(class.region(), class.entity_class() as usize + 1, "{class:?}");
        assert_eq!(CharacterClass::from_region(class.region()), Some(class));
    }
}

#[test]
fn default_hero_build_copies_identity_and_level() {
    let h = *hero(2).unwrap();
    let pc = PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
    assert_eq!(pc.hero.name, "Wolf");
    assert_eq!(pc.level, 1);
    assert_eq!(pc.money, 0);
    assert_eq!(pc.palette_id(), h.palette_id());
}

#[test]
fn debug_block_contains_expected_fields() {
    let pc = PlayerCharacter::from_build(
        *hero(0).unwrap(),
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    let b = pc.debug_block();
    assert!(b.contains("HERO Dragomir"), "{b}");
    assert!(b.contains("CLASS Warrior (entity +04 = 5)"), "{b}");
    assert!(b.contains("STR base=15 current=15"), "{b}");
    assert!(b.contains("DEX base=3 current=3"), "{b}");
    assert!(b.contains("CHA base=3 current=3"), "{b}");
    assert!(b.contains("END base=10 current=10"), "{b}");
    assert!(b.contains("melee=100"), "{b}");
}

#[test]
fn specialization_mapping() {
    assert_eq!(CharacterClass::Chief.specialization(), PrimaryStat::Endurance);
    assert_eq!(CharacterClass::Hunter.specialization(), PrimaryStat::Dexterity);
    assert_eq!(CharacterClass::Merchant.specialization(), PrimaryStat::Charisma);
    assert_eq!(CharacterClass::Warrior.specialization(), PrimaryStat::Strength);
}

#[test]
fn stat_cost_worked_examples() {
    // The four documented refund/cost examples.
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 14), 5);
    assert_eq!(stat_cost(CharacterClass::Hunter, PrimaryStat::Dexterity, 14), 5);
    assert_eq!(stat_cost(CharacterClass::Merchant, PrimaryStat::Charisma, 24), 11);
    assert_eq!(stat_cost(CharacterClass::Chief, PrimaryStat::Endurance, 11), 2);
}

#[test]
fn stat_cost_tier_boundaries() {
    // Specialized (Warrior/STR): threshold row applies while spec_thr > v.
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 0), 2);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 12), 2);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 13), 5);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 18), 5);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 19), 11);
    // Non-specialized (Warrior/CHA uses the normal thresholds).
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 8), 2);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 9), 5);
    // Past the last finite tier -> 999.
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 100), 999);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 80), 999);
    // The last finite specialized tier (150/95/75).
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 94), 150);
    assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 95), 200);
}

#[test]
fn max_load_examples() {
    assert_eq!(format!("{:4.1}", max_load(7)), "12.3");
    assert_eq!(format!("{:4.1}", max_load(10)), "13.3");
    assert_eq!(format!("{:4.1}", max_load(12)), "14.0");
}

#[test]
fn max_party_examples_and_edges() {
    assert_eq!(max_party(CharacterClass::Chief, 3), 1);
    assert_eq!(max_party(CharacterClass::Hunter, 3), 1);
    assert_eq!(max_party(CharacterClass::Merchant, 25), 3);
    assert_eq!(max_party(CharacterClass::Warrior, 3), 1);
    // Threshold edges.
    assert_eq!(max_party(CharacterClass::Chief, 14), 1);
    assert_eq!(max_party(CharacterClass::Chief, 15), 2);
    assert_eq!(max_party(CharacterClass::Chief, 94), 10);
    assert_eq!(max_party(CharacterClass::Chief, 95), 10);
    assert_eq!(max_party(CharacterClass::Warrior, 19), 1);
    assert_eq!(max_party(CharacterClass::Warrior, 20), 2);
    assert_eq!(max_party(CharacterClass::Warrior, 110), 10);
}

#[test]
fn stat_stepper_is_exactly_reversible() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior); // STR 15, points 0
    assert_eq!(b.available_xp, 0);
    b.decrease(PrimaryStat::Strength);
    assert_eq!(b.strength, 14);
    assert_eq!(b.available_xp, 5);
    assert!(b.try_increase(PrimaryStat::Strength));
    assert_eq!(b.strength, 15);
    assert_eq!(b.available_xp, 0);
}

#[test]
fn insufficient_points_is_a_noop() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior); // points 0
    assert!(!b.try_increase(PrimaryStat::Charisma)); // cost 2 > 0
    assert_eq!(b.charisma, 3);
    assert_eq!(b.available_xp, 0);
}

#[test]
fn decrease_at_zero_is_a_noop() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
    b.charisma = 0;
    b.decrease(PrimaryStat::Charisma);
    assert_eq!(b.charisma, 0);
    assert_eq!(b.available_xp, 0);
}

#[test]
fn available_xp_persists_into_the_runtime_player() {
    let mut b = CharacterBuild::from_class(CharacterClass::Merchant);
    b.decrease(PrimaryStat::Charisma); // 25 -> 24, refund 11
    assert_eq!((b.charisma, b.available_xp), (24, 11));
    let pc = PlayerCharacter::from_build(*hero(0).unwrap(), b);
    assert_eq!(pc.available_xp, 11);
    assert_eq!(pc.charisma_base, 24);
    assert_eq!(pc.charisma_current, 24);
}

#[test]
fn secondary_skill_defaults_match_initial_skills() {
    let b = CharacterBuild::from_class(CharacterClass::Warrior);
    assert_eq!(b.combat_specialization, CombatSpecialization::Fencing);
    assert_eq!(b.civil_specialization, CivilSpecialization::Identification);
    assert_eq!(b.skills(), INITIAL_SKILLS);
}

fn combat4(b: &CharacterBuild) -> (u16, u16, u16, u16) {
    (b.melee_mastery, b.archery_mastery, b.building, b.smithing)
}

fn civil3(b: &CharacterBuild) -> (u16, u16, u16) {
    (b.trade, b.healing, b.identification)
}

#[test]
fn combat_specialization_switch() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
    b.select_combat(CombatSpecialization::Marksmanship);
    assert_eq!(combat4(&b), (0, 100, 2, 5));
    assert_eq!(civil3(&b), (10, 50, 20), "civil fields must be untouched");
    b.select_combat(CombatSpecialization::Fencing);
    assert_eq!(combat4(&b), (100, 0, 1, 10));
    assert_eq!(civil3(&b), (10, 50, 20));
}

#[test]
fn civil_specialization_switch() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
    b.select_civil(CivilSpecialization::Trade);
    assert_eq!(civil3(&b), (20, 50, 10));
    assert_eq!(combat4(&b), (100, 0, 1, 10), "combat fields must be untouched");
    b.select_civil(CivilSpecialization::Healing);
    assert_eq!(civil3(&b), (10, 100, 10));
    assert_eq!(combat4(&b), (100, 0, 1, 10));
    b.select_civil(CivilSpecialization::Identification);
    assert_eq!(civil3(&b), (10, 50, 20));
    assert_eq!(combat4(&b), (100, 0, 1, 10));
}

#[test]
fn class_change_preserves_secondary_skills_and_specializations() {
    let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
    b.select_combat(CombatSpecialization::Marksmanship);
    b.select_civil(CivilSpecialization::Trade);
    let skills = b.skills();
    b.apply_class_preset(CharacterClass::Merchant);
    assert_eq!(b.class, CharacterClass::Merchant);
    assert_eq!(b.combat_specialization, CombatSpecialization::Marksmanship);
    assert_eq!(b.civil_specialization, CivilSpecialization::Trade);
    assert_eq!(b.skills(), skills, "class change must not touch the skills");
    assert_eq!(b.available_xp, 0, "class change resets points");
}
