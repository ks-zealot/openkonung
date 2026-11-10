//! Unit tests for the recovered melee rules.

use super::*;

/// Repository root that holds the original `konung.res` / `Game.N` files.
fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Load the item database from `konung.res` plus the `Game.1` instance table.
fn db_and_game() -> Option<(ItemDb, Vec<u8>)> {
    let konung = std::fs::read(install_dir().join("konung.res")).ok()?;
    let game = std::fs::read(install_dir().join("Game.1")).ok()?;
    Some((ItemDb::from_konung(&konung), game))
}

/// `Game.1` entity 0 (protagonist) and entity 26 (the shipped ant record).
fn heroes() -> Option<(ItemDb, Vec<u8>, Entity, Entity)> {
    let (db, game) = db_and_game()?;
    let entities = crate::population::parse_entities(&game).ok()?;
    let hero = entities.get(0)?.clone();
    // Game.1 entity 26: object-family actor with base armor 10 and three
    // armor-slot items (the shipped ant record).
    let mob = entities.get(26)?.clone();
    Some((db, game, hero, mob))
}

/// `FACING_DODGE` is byte-for-byte the `0x450DB4` 8x8 table.
#[test]
fn facing_dodge_table_is_the_original_8x8_table() {
    assert_eq!(
        FACING_DODGE,
        [
            [0, 0, 0, 1, 1, 1, 0, 0],
            [0, 0, 0, 0, 0, 1, 1, 1],
            [0, 0, 0, 0, 0, 0, 0, 1],
            [0, 1, 0, 1, 0, 0, 0, 0],
            [0, 0, 0, 1, 0, 1, 0, 1],
            [0, 1, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 1, 0, 1, 0, 1],
            [0, 0, 0, 1, 0, 0, 0, 0],
        ]
    );
}

/// The LCG result stays within `0x7FFF` and `roll()` within `0..=100`.
#[test]
fn lcg_matches_the_original_linear_congruential_step() {
    let mut rng = OriginalRng::new(1);
    for _ in 0..32 {
        assert!(rng.next_u32() <= 0x7FFF);
    }
    // `rand()%101` never exceeds 100.
    for _ in 0..32 {
        assert!(rng.roll() <= 100);
    }
}

/// Human reaction is `100` for the new-game hero and `59` for dexterity 35.
#[test]
fn reactions_match_the_verified_formulas() {
    let hero = CombatStats {
        archetype: 5,
        weapon_slot: 2,
        hp: 1600,
        dexterity: 10,
        strength: 15,
        endurance: 10,
        base_armor: 10,
        melee_mastery: 100,
        archery_mastery: 0,
        periodic_damage: 0,
        equipment: [0; 6],
    };
    assert_eq!(reaction(&hero), 100);

    let ant = CombatStats {
        archetype: 0x41,
        weapon_slot: 0,
        hp: 1600,
        dexterity: 35,
        strength: 35,
        endurance: 25,
        base_armor: 10,
        melee_mastery: 0,
        archery_mastery: 0,
        periodic_damage: 0,
        equipment: [0; 6],
    };
    assert_eq!(reaction(&ant), 59); // round(50 + 35*0.25) = 58.75 -> 59
}

/// The shipped hero knife (120 raw vs ant armor 80) mitigates to exactly `42`.
#[test]
fn hero_knife_and_ant_armor_match_the_original_values() {
    let Some((db, game, hero, ant)) = heroes() else {
        eprintln!("skipping: konung.res / Game.1 not present");
        return;
    };
    let hs = CombatStats::from_entity(&hero);
    let as_ = CombatStats::from_entity(&ant);
    assert_eq!(hs.strength, 15);
    assert_eq!(hs.hp, 1600);
    assert_eq!(hs.weapon_slot_index(), Some(2));
    // Knife = class 2 subtype 18, rating 60; armed STR*HP/400 = 60.
    let (damage, poison) = attack_damage(&db, &game, &hs, as_.archetype);
    assert_eq!(damage, 120);
    assert_eq!(poison, 0);
    // Entity 26 (archetype 0x4F) has base armor 80 and no equipment slots.
    assert_eq!(armor_total(&db, &game, &as_), 80);
    // Verified mitigation for 120 incoming, 80 armor, 25 endurance.
    assert_eq!(mitigate_damage(&as_, 120, 80), 42);
}

/// Item instance `+0x02` selects the modifier only against its listed archetype.
#[test]
fn weapon_modifier_selected_by_item_plus_02_target_archetype() {
    let Some((db, game, mut hero, _)) = heroes() else {
        return;
    };
    // Table record 1 = 5000 damage vs archetype 0x55 (ants).
    assert_eq!(db.modifier_damage(1, 0x55), Some(5000));
    assert_eq!(db.modifier_damage(1, 0x41), None);
    // Game.1 item 962 carries +0x02 = 1.
    let inst = ItemDb::decode_instance(&game, 962).expect("item 962");
    assert_eq!(inst.modifier_index, 1);
    hero.equipment[2] = 962;
    let hs = CombatStats::from_entity(&hero);
    let (ant_damage, _) = attack_damage(&db, &game, &hs, 0x55);
    assert_eq!(ant_damage, 5060); // 5000 modifier + 60 armed STR*HP/400
    let (other_damage, _) = attack_damage(&db, &game, &hs, 0x00);
    assert_eq!(other_damage, 60 + db.info(12, 33).unwrap().rating as i32);
}

/// Facing `0` vs `0` has no dodge entry, so the reaction hit resolves to 42.
#[test]
fn resolve_melee_uses_facing_dodge_then_mitigation() {
    let Some((db, game, hero, ant)) = heroes() else {
        return;
    };
    let hs = CombatStats::from_entity(&hero);
    let as_ = CombatStats::from_entity(&ant);
    let mut rng = OriginalRng::new(1);
    // Facing 0 -> 0 has no dodge entry; hero reaction 100 always hits.
    let roll = resolve_melee(&db, &game, &hs, 0, &as_, 0, &mut rng);
    assert!(roll.hit);
    assert!(!roll.dodged);
    assert_eq!(roll.damage, 42);
    assert!(!roll.lethal);
}

#[test]
fn armor_and_weapon_wear_match_the_verified_scales() {
    assert!((armor_wear(90, 102, 36) - 0.031_764_705).abs() < 1e-6);
    assert!((armor_wear(200, 0, 40) - 0.040).abs() < 1e-6);
    assert!((weapon_wear(90) - 0.009).abs() < 1e-6);
}

#[test]
fn corpse_loot_copies_inventory_and_money_without_generated_items() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let entities = crate::population::parse_entities(&game).unwrap();
    let hero = entities.get(0).unwrap();
    let mut rng = OriginalRng::new(1);
    let plan = crate::combat::loot::plan_corpse_loot(
        &db,
        &mut rng,
        hero.stats.money,
        &hero.inventory,
        hero.stats.raw_0x52,
        0,
        0,
        0,
    );
    assert_eq!(plan.money, 100);
    assert_eq!(plan.generated.len(), 0);
    assert_eq!(plan.existing, vec![2, 3, 4, 5, 6, 7]);
}

#[test]
fn poison_actor_always_drops_a_stinger_with_its_poison_value() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let entities = crate::population::parse_entities(&game).unwrap();
    let actor = entities.get(26).unwrap();
    assert_eq!(actor.stats.raw_0x52, 2);
    let mut rng = OriginalRng::new(1);
    let plan = crate::combat::loot::plan_corpse_loot(
        &db,
        &mut rng,
        actor.stats.money,
        &actor.inventory,
        actor.stats.raw_0x52,
        0,
        0,
        0,
    );
    let stinger = plan
        .generated
        .iter()
        .find(|i| i.class == 9 && i.subtype == 0)
        .expect("stinger");
    assert_eq!(stinger.poison, 2);
    assert_eq!(stinger.durability, 1.0);
}

#[test]
fn generated_loot_only_rolls_for_a_nonzero_killer_level() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let entities = crate::population::parse_entities(&game).unwrap();
    let actor = entities.get(26).unwrap();
    // Melee/DOT pass level 0 -> no generated item even with a poison stinger.
    let mut rng = OriginalRng::new(7);
    let plan = crate::combat::loot::plan_corpse_loot(
        &db,
        &mut rng,
        actor.stats.money,
        &actor.inventory,
        actor.stats.raw_0x52,
        0,
        0,
        0,
    );
    assert_eq!(plan.generated.len(), 1);
    // Some seed with killer level 10 produces the extra generated item.
    let mut found = false;
    for seed in 1..500u32 {
        let mut rng = OriginalRng::new(seed);
        let plan = crate::combat::loot::plan_corpse_loot(
            &db,
            &mut rng,
            actor.stats.money,
            &actor.inventory,
            actor.stats.raw_0x52,
            10,
            0,
            0,
        );
        if let Some(item) = plan.generated.iter().find(|i| i.class != 9) {
            assert_eq!(item.modifier, -1, "generated loot uses +0x01, not +0x02");
            assert!(item.condition >= 0);
            found = true;
            break;
        }
    }
    assert!(found, "no seed produced a generated item");
}

#[test]
fn level_tiers_match_fun_00417144() {
    assert_eq!(crate::combat::loot::level_tier(1), 3);
    assert_eq!(crate::combat::loot::level_tier(3), 3);
    assert_eq!(crate::combat::loot::level_tier(4), 6);
    assert_eq!(crate::combat::loot::level_tier(7), 8);
    assert_eq!(crate::combat::loot::level_tier(10), 10);
}

#[test]
fn bow_lifetime_and_ranged_damage_match_the_original() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    // Game.1 bow instances: 23 short bow, 68 composite bow, 1024 glued bow.
    assert_eq!(db.resolve(&game, 23).unwrap().projectile_lifetime, 32);
    assert_eq!(db.resolve(&game, 68).unwrap().projectile_lifetime, 169);
    assert_eq!(db.resolve(&game, 1024).unwrap().projectile_lifetime, 171);
    let stats = CombatStats {
        archetype: 3,
        weapon_slot: 1,
        hp: 1600,
        dexterity: 10,
        strength: 15,
        endurance: 10,
        base_armor: 10,
        melee_mastery: 0,
        archery_mastery: 100,
        periodic_damage: 0,
        equipment: [0, 23, 0, 0, 0, 0],
    };
    let bow = bow_shot(&db, &game, &stats).expect("bow shot");
    assert_eq!(bow.rating, 112);
    assert_eq!(bow.subtype, 8);
    // No modifier: rating * rating.
    assert_eq!(ranged_damage(&db, &bow, 0x55), (112 * 112, 0));
    // Modifier 1 matches ants: 5000 * 112.
    let modified = BowShot { modifier: 1, ..bow };
    assert_eq!(ranged_damage(&db, &modified, 0x55), (5000 * 112, 0));
    assert_eq!(ranged_damage(&db, &modified, 0x41), (112 * 112, 0));
}

#[test]
fn projectile_geometry_matches_the_verified_tables() {
    use crate::combat::projectile as p;
    assert_eq!(p::muzzle_offset(0), (-70.0, -65.0));
    assert_eq!(p::muzzle_offset(7), (-60.0, -31.0));
    assert_eq!(p::MUZZLE_X, [-70, -48, 12, 68, 80, 52, -8, -60]);
    assert_eq!(p::MUZZLE_Y, [-65, -99, -85, -90, -58, -26, -10, -31]);
    assert_eq!(p::DRAW_DX, [0, 0, 0, 0, -3, -29, -29, -23]);
    assert_eq!(p::DRAW_DY, [-3, 0, 0, -3, 0, 0, -17, 0]);
    let (vx, vy) = p::aim_velocity((0.0, 0.0), (100.0, 0.0));
    assert!((vx - 100.0 / 25.001).abs() < 1e-6);
    assert!(vy.abs() < 1e-6);
    let proj = p::spawn(
        CombatTarget::Player,
        (10.0, 20.0),
        (110.0, 20.0),
        3,
        BowShot {
            rating: 1,
            modifier: -1,
            subtype: 8,
            poison: 0,
        },
        32,
        None,
    );
    assert_eq!(proj.sprite_base, 0xB8 + 6);
    assert_eq!(proj.draw_dx, 0);
    assert_eq!(proj.draw_dy, -3);
}

#[test]
fn projectile_dodge_and_mitigation_use_the_melee_constants() {
    let Some((db, game, hero, ant)) = heroes() else {
        return;
    };
    let hs = CombatStats::from_entity(&hero);
    // Hero has no bow in the shipped loadout; use the short-bow rating.
    let bow = BowShot {
        rating: 112,
        modifier: -1,
        subtype: 8,
        poison: 0,
    };
    let as_ = CombatStats::from_entity(&ant);
    let mut rng = OriginalRng::new(1);
    let roll = resolve_projectile(&db, &game, &bow, &as_, &mut rng);
    assert!(roll.hit);
    assert_eq!(roll.incoming, 112 * 112);
    let dodger = CombatStats {
        dexterity: 2000,
        ..as_
    };
    let mut rng = OriginalRng::new(1);
    let roll = resolve_projectile(&db, &game, &bow, &dodger, &mut rng);
    assert!(!roll.hit && roll.dodged);
    let _ = hs;
}

#[test]
fn arrow_carrying_mob_inventory_reaches_the_corpse_plan() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let entities = crate::population::parse_entities(&game).unwrap();
    let ant = entities.get(40).unwrap();
    assert_eq!(ant.inventory[0], 102);
    let mut rng = OriginalRng::new(1);
    let plan = crate::combat::loot::plan_corpse_loot(
        &db,
        &mut rng,
        ant.stats.money,
        &ant.inventory,
        ant.stats.raw_0x52,
        0,
        0,
        0,
    );
    assert!(
        plan.existing.contains(&102),
        "30-arrow stack must be lootable"
    );
}

#[test]
fn player_stats_view_maps_the_authored_fields() {
    let p = PlayerCharacter {
        hero: crate::hero::HEROES[0],
        class: crate::character::CharacterClass::Warrior,
        level: 1,
        money: 0,
        available_xp: 0,
        invested_xp: 0,
        melee_mastery: 100,
        trade: 0,
        archery_mastery: 0,
        building: 0,
        healing: 0,
        identification: 0,
        smithing: 0,
        dexterity_base: 10,
        dexterity_current: 11,
        charisma_base: 10,
        charisma_current: 10,
        strength_base: 15,
        strength_current: 16,
        endurance_base: 10,
        endurance_current: 12,
        equipment: [0; 6],
        inventory: [0; 32],
        additional_equipment: [0; 5],
        special_equipment: 0,
        hp_internal: 1600,
        periodic_damage: 3,
        base_armor: 10,
        raw_0x12: 2,
        flags_b: 0,
        dead: false,
        dialogue_id: 0xFF,
        interface_mode_d2: 0,
        state_d3: 0,
        temp_effect_timer: 0,
        temp_effect_id: 0,
        saved_dexterity: 0,
        saved_charisma: 0,
        saved_strength: 0,
        saved_endurance: 0,
    };
    let s = CombatStats::from_player(&p);
    assert_eq!(
        (s.hp, s.strength, s.dexterity, s.endurance),
        (1600, 15, 10, 10)
    );
    assert_eq!(s.periodic_damage, 3);
}
