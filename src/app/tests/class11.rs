//! Class-11 item-use dispatcher (`FUN_00414DE4`), C1/C2.

use super::super::*;
use super::common::*;

fn find_item(app: &PlayableApp, class: u8, subtype: u8) -> Option<u16> {
    (1..8192u16).find(|&id| app.world.res.item_class_subtype(id) == Some((class, subtype)))
}

fn alloc(app: &mut PlayableApp, class: u8, subtype: u8, flags: u16) -> u16 {
    app.world
        .res
        .alloc_item_instance(class, subtype, 0, -1, 1.0, 0, flags)
        .expect("free item instance")
}

fn put_in_first_slot(app: &mut PlayableApp, id: u16) {
    app.inventory_owner = crate::inventory::InventoryOwner::Protagonist;
    app.inventory_scroll = 0;
    app.held_item = None;
    app.player.inventory.fill(0);
    app.player.inventory[0] = id;
}

fn use_first_slot(app: &mut PlayableApp) -> bool {
    app.right_click_inventory_slot(0)
}

#[test]
fn c1_routes_lost_map_item_through_the_dispatcher() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(map) = find_item(&app, 11, 0) else {
        eprintln!("skipping: Game.1 has no lost-map item");
        return;
    };
    put_in_first_slot(&mut app, map);
    assert!(use_first_slot(&mut app), "subtype 0 must be used");
    assert_eq!(app.world.res.item_class(map), Some(0xFF));
}

#[test]
fn c1_leaves_unimplemented_subtypes_inert() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Quest tokens with no `FUN_00414DE4` case (subtype 29 is only obtainable
    // from dialogue actions, so allocate one instead of searching the save).
    for subtype in [29u8, 32, 34, 45] {
        let id = find_item(&app, 11, subtype).unwrap_or_else(|| alloc(&mut app, 11, subtype, 0));
        put_in_first_slot(&mut app, id);
        assert!(
            !use_first_slot(&mut app),
            "subtype {subtype} is not implemented yet"
        );
        assert_eq!(
            app.world.res.item_class_subtype(id),
            Some((11, subtype)),
            "an inert subtype must not consume the item"
        );
        assert!(app.held_item.is_none());
    }
}

#[test]
fn e1_shovel_arms_the_dig_cursor_and_is_not_consumed() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let shovel = find_item(&app, 11, 24).unwrap_or_else(|| alloc(&mut app, 11, 24, 0));
    put_in_first_slot(&mut app, shovel);
    assert!(use_first_slot(&mut app), "the shovel must arm dig mode");
    assert!(app.dig_armed);
    assert_eq!(
        app.world.res.item_class(shovel),
        Some(11),
        "the shovel is never consumed"
    );
    // `FUN_0042260C` resets `_DAT_0081D660` from 7 to 4 on the next world click.
    app.world_move(600.0, 300.0);
    assert!(!app.dig_armed);
}

#[test]
fn c1_rejects_non_class11_items() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(id) = (1..8192u16).find(|&id| {
        app.world
            .res
            .item_class(id)
            .map(|c| c != 11 && c != 0xFF)
            .unwrap_or(false)
    }) else {
        eprintln!("skipping: no non-class-11 item");
        return;
    };
    put_in_first_slot(&mut app, id);
    assert!(!app.use_class11_item(crate::inventory::InventoryOwner::Protagonist, id, None));
    assert!(app.world.res.item_class(id).is_some());
}

#[test]
fn c1_lunar_son_is_passive_and_reduces_carried_weight() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let moon = alloc(&mut app, 11, 11, 0);
    let info = app.world.res.item_info(moon).expect("lunar son template");
    assert_eq!(info.name, "Сын Луны");
    // `0xD8F0` at the class-11 icon/weight table (`0x460996`, stride 0x80) is
    // −10000 as `i16`; `FUN_0041B7E4` sign-extends it.
    assert_eq!(info.weight, -10_000);

    app.player.inventory.fill(0);
    let before = {
        let slots = crate::inventory::SlotsRef {
            equipment: &app.player.equipment,
            inventory: &app.player.inventory,
            additional: &app.player.additional_equipment,
            special: app.player.special_equipment,
        };
        crate::inventory::carried_weight(&app.world.res, &slots)
    };
    app.player.inventory[0] = moon;
    let after = {
        let slots = crate::inventory::SlotsRef {
            equipment: &app.player.equipment,
            inventory: &app.player.inventory,
            additional: &app.player.additional_equipment,
            special: app.player.special_equipment,
        };
        crate::inventory::carried_weight(&app.world.res, &slots)
    };
    assert_eq!(after, before - 10_000, "Сын Луны must subtract 10 kg");

    // No `FUN_00414DE4` case exists: using it does nothing and never consumes.
    app.inventory_owner = crate::inventory::InventoryOwner::Protagonist;
    app.inventory_scroll = 0;
    app.held_item = None;
    assert!(!use_first_slot(&mut app));
    assert_eq!(app.world.res.item_class(moon), Some(11));
    assert_eq!(app.player.inventory[0], moon);
}

#[test]
fn c2_skill_books_apply_their_caps_and_messages() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // (subtype, field before, expected after, message)
    let cases: [(u8, fn(&mut PlayableApp, u16), u16, u16, &str); 5] = [
        (
            1,
            |a, v| a.player.healing = v,
            450,
            500,
            "Ты открыл для себя секреты древних целителей",
        ),
        (
            2,
            |a, v| a.player.building = v,
            0,
            5,
            "Ты познал секреты строительного искусства",
        ),
        (
            3,
            |a, v| a.player.smithing = v,
            0,
            4,
            "Ты познал секреты кузнечного дела",
        ),
        (
            4,
            |a, v| a.player.trade = v,
            95,
            100,
            "Ты узнал много нового об искусстве торговли",
        ),
        (
            5,
            |a, v| a.player.identification = v,
            90,
            100,
            "Ты узнал много интересного о неведомых вещах",
        ),
    ];
    for (subtype, set, before, want, message) in cases {
        let Some(id) = find_item(&app, 11, subtype) else {
            eprintln!("skipping: no class-11 subtype {subtype}");
            continue;
        };
        set(&mut app, before);
        put_in_first_slot(&mut app, id);
        assert!(use_first_slot(&mut app), "subtype {subtype}");
        let got = match subtype {
            1 => app.player.healing,
            2 => app.player.building,
            3 => app.player.smithing,
            4 => app.player.trade,
            _ => app.player.identification,
        };
        assert_eq!(got, want, "subtype {subtype}");
        assert_eq!(app.world.res.item_class(id), Some(0xFF), "consumed");
        assert_eq!(app.pickup_message.as_deref(), Some(message));
    }
}

#[test]
fn c2_strength_xp_charisma_and_relics() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };

    // 8 — Соколиная лапка: +3 strength current.
    if let Some(id) = find_item(&app, 11, 8) {
        let before = app.player.strength_current;
        put_in_first_slot(&mut app, id);
        assert!(use_first_slot(&mut app));
        assert_eq!(app.player.strength_current, before + 3);
        assert_eq!(app.world.res.item_class(id), Some(0xFF));
    }

    // 10 — Яблоко познания: +200 XP (AUTO off so it is not spent).
    app.player.flags_b &= !0x02;
    if let Some(id) = find_item(&app, 11, 10) {
        let before = app.player.available_xp;
        put_in_first_slot(&mut app, id);
        assert!(use_first_slot(&mut app));
        assert_eq!(app.player.available_xp, before + 200);
    }

    // 36 — Чаша воеводы: +10 charisma; 39 — Детские игрушки: +5.
    for (subtype, delta) in [(36u8, 10u16), (39, 5)] {
        let Some(id) = find_item(&app, 11, subtype) else {
            continue;
        };
        let before = app.player.charisma_current;
        put_in_first_slot(&mut app, id);
        assert!(use_first_slot(&mut app));
        assert_eq!(app.player.charisma_current, before + delta);
    }

    // 38 — Святые мощи: protagonist +0xD3 = 0x46.
    if let Some(id) = find_item(&app, 11, 38) {
        put_in_first_slot(&mut app, id);
        assert!(use_first_slot(&mut app));
        assert_eq!(app.player.state_d3, 0x46);
        assert_eq!(app.world.res.item_class(id), Some(0xFF));
    }
}

#[test]
fn c2_identify_scroll_clears_flags_and_raises_leader_skill() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let scroll = alloc(&mut app, 11, 9, 0);
    let dummy = alloc(&mut app, 2, 0, 1); // unidentified target item
    put_in_first_slot(&mut app, scroll);
    app.player.inventory[1] = dummy;
    let before = app.player.identification;
    assert!(use_first_slot(&mut app));
    assert_eq!(
        app.world.res.item_instance(dummy).map(|i| i.flags & 1),
        Some(0),
        "the unidentified bit is cleared"
    );
    assert_eq!(app.player.identification, (before + 1).min(100));
    assert_eq!(app.world.res.item_class(scroll), Some(0xFF));
}

#[test]
fn c2_wine_applies_the_temporary_buff() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let wine = alloc(&mut app, 11, 12, 0);
    app.player.dexterity_base = 30;
    app.player.dexterity_current = 30;
    app.player.charisma_current = 10;
    app.player.strength_current = 10;
    app.player.endurance_current = 10;
    app.player.temp_effect_timer = 0;
    put_in_first_slot(&mut app, wine);
    assert!(use_first_slot(&mut app));
    assert_eq!(app.player.temp_effect_timer, 150); // 30 * 5
    assert_eq!(app.player.temp_effect_id, 0);
    assert_eq!(app.player.dexterity_current, 15);
    assert_eq!(app.player.charisma_current, 15);
    assert_eq!(app.player.strength_current, 15);
    assert_eq!(app.player.endurance_current, 15);
    assert_eq!(app.player.saved_dexterity, 30);
    assert_eq!(app.world.res.item_class(wine), Some(0xFF));
}

#[test]
fn c2_birch_bark_letter_activates_quest_slot_zero() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let letter = alloc(&mut app, 11, 28, 0);
    app.quest_flags[0] &= !0x80;
    put_in_first_slot(&mut app, letter);
    assert!(use_first_slot(&mut app));
    assert_ne!(app.quest_flags[0] & 0x80, 0);
    assert_eq!(
        app.pickup_message.as_deref(),
        Some("Ты прочитал Берестяную Грамоту")
    );
    assert_eq!(app.world.res.item_class(letter), Some(0xFF));
}

#[test]
fn c2_torch_is_reusable_and_resets_on_map_entry() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(torch) = find_item(&app, 11, 31) else {
        eprintln!("skipping: no torch item");
        return;
    };
    put_in_first_slot(&mut app, torch);
    assert!(use_first_slot(&mut app));
    assert!(app.torch_reveal);
    assert_eq!(
        app.world.res.item_class(torch),
        Some(11),
        "the torch is reusable"
    );
    // `FUN_0043ADF0` clears the highlight on map entry.
    assert!(app.travel_to_map(4, 4, 0, 0, 0, 0));
    assert!(!app.torch_reveal);
}

fn hold(app: &mut PlayableApp, id: u16) {
    app.held_item = Some(crate::inventory::HeldItem {
        id,
        owner: crate::inventory::InventoryOwner::Protagonist,
        source: None,
    });
}

#[test]
fn c3_duplicator_copies_the_target_onto_the_held_item() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(duplicator) = find_item(&app, 11, 7) else {
        eprintln!("skipping: no duplicator item");
        return;
    };
    let Some(target) = (1..8192u16).find(|&id| {
        app.world
            .res
            .item_class(id)
            .map(|c| c != 11 && c != 0xFF)
            .unwrap_or(false)
    }) else {
        eprintln!("skipping: no target item");
        return;
    };
    let target_before = app.world.res.item_instance(target).unwrap();
    app.player.inventory.fill(0);
    app.player.inventory[0] = target;
    hold(&mut app, duplicator);
    assert!(app.try_craft_held_at(
        crate::inventory::InventoryOwner::Protagonist,
        crate::inventory::ItemLocation::Inventory(0)
    ));
    assert_eq!(app.world.res.item_instance(duplicator), Some(target_before));
    assert_eq!(app.world.res.item_instance(target), Some(target_before));
    assert!(app.held_item.is_none());
    assert!(
        app.player.inventory.contains(&duplicator),
        "the transformed duplicator returns to the bag"
    );
}

#[test]
fn c3_ancient_sword_parts_forge_the_class2_sword() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(hilt) = find_item(&app, 11, 22) else {
        eprintln!("skipping: no sword hilt");
        return;
    };
    let Some(blade) = find_item(&app, 11, 23) else {
        eprintln!("skipping: no sword blade");
        return;
    };
    app.player.flags_b &= !0x02;
    app.player.inventory.fill(0);
    app.player.inventory[0] = blade;
    hold(&mut app, hilt);
    let xp = app.player.available_xp;
    assert!(app.try_craft_held_at(
        crate::inventory::InventoryOwner::Protagonist,
        crate::inventory::ItemLocation::Inventory(0)
    ));
    let forged = app.world.res.item_instance(blade).unwrap();
    assert_eq!(forged.class, 2);
    assert_eq!(forged.subtype, 0x1B);
    assert_eq!(forged.condition, -1);
    assert_eq!(forged.modifier_index, 0x0E);
    assert_eq!(forged.current_durability, 120.0);
    assert_eq!(forged.max_durability, 120.0);
    assert_eq!(app.world.res.item_class(hilt), Some(0xFF));
    assert!(!app.player.inventory.contains(&hilt));
    assert_eq!(app.player.available_xp, xp + 30);
    assert!(app.held_item.is_none());
}

#[test]
fn c3_amulet_parts_combine_and_consumed_part_disappears() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(right) = find_item(&app, 11, 25) else {
        eprintln!("skipping: no right amulet part");
        return;
    };
    let Some(left) = find_item(&app, 11, 26) else {
        eprintln!("skipping: no left amulet part");
        return;
    };
    app.player.flags_b &= !0x02;
    app.player.inventory.fill(0);
    app.player.inventory[0] = left;
    hold(&mut app, right);
    let xp = app.player.available_xp;
    assert!(app.try_craft_held_at(
        crate::inventory::InventoryOwner::Protagonist,
        crate::inventory::ItemLocation::Inventory(0)
    ));
    assert_eq!(
        app.world.res.item_class_subtype(left),
        Some((11, 41)),
        "25 + 26 -> combined left/right part"
    );
    assert_eq!(app.world.res.item_class(right), Some(0xFF));
    assert_eq!(app.player.available_xp, xp + 100);
}

#[test]
fn c3_invalid_amulet_pair_is_rejected_without_consuming() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(right) = find_item(&app, 11, 25) else {
        eprintln!("skipping: no right amulet part");
        return;
    };
    // 40 is a result subtype and usually absent from the shipped saves.
    let full = match find_item(&app, 11, 40) {
        Some(id) => id,
        None => alloc(&mut app, 11, 40, 0),
    };
    app.player.inventory.fill(0);
    app.player.inventory[0] = full;
    hold(&mut app, right);
    assert!(!app.try_craft_held_at(
        crate::inventory::InventoryOwner::Protagonist,
        crate::inventory::ItemLocation::Inventory(0)
    ));
    assert_eq!(app.world.res.item_class_subtype(right), Some((11, 25)));
    assert_eq!(app.world.res.item_class_subtype(full), Some((11, 40)));
    assert!(app.held_item.is_some());
}

fn add_free_location(app: &mut PlayableApp, id: u32) {
    use crate::loc::Location;
    let loc = Location {
        loc_id: id,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let index = app.world.locs.len();
    app.world.locs.push(loc);
    app.world.loc_index.insert(id, index);
}

#[test]
fn c4_resurrection_revives_a_dead_world_actor() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.ensure_population(4);
    let Some(eid) = app.population_cache.get(&4).and_then(|pop| {
        pop.actors
            .iter()
            .find(|a| a.entity.group_id != 0)
            .map(|a| a.source_entity_id())
    }) else {
        eprintln!("skipping: no world actor on loc 4");
        return;
    };
    if let Some(pop) = app.population_cache.get_mut(&4) {
        if let Some(m) = pop.motion.get_mut(&eid) {
            m.dead = true;
            m.action = 3;
        }
        if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == eid) {
            a.entity.stats.hp_internal = 0;
            a.entity.archetype |= 0x80;
        }
    }
    let Some(revive) = find_item(&app, 11, 6) else {
        eprintln!("skipping: no resurrection item");
        return;
    };
    put_in_first_slot(&mut app, revive);
    assert!(use_first_slot(&mut app));
    let pop = app.population_cache.get(&4).unwrap();
    assert!(!pop.motion.get(&eid).unwrap().dead);
    let a = pop
        .actors
        .iter()
        .find(|a| a.source_entity_id() == eid)
        .unwrap();
    assert_eq!(a.entity.stats.hp_internal, 1600);
    assert_eq!(a.entity.archetype & 0x80, 0);
    assert_eq!(app.world.res.item_class(revive), Some(0xFF));
}

#[test]
fn c4_magnet_moves_other_group_money_and_items() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.ensure_population(4);
    let Some((eid, item)) = app.population_cache.get(&4).and_then(|pop| {
        pop.actors
            .iter()
            .find(|a| a.entity.group_id != 0)
            .map(|a| (a.source_entity_id(), 1007u16))
    }) else {
        eprintln!("skipping: no world actor on loc 4");
        return;
    };
    if let Some(pop) = app.population_cache.get_mut(&4) {
        if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == eid) {
            a.entity.stats.money = 50;
            a.entity.inventory.fill(0);
            a.entity.inventory[0] = item;
        }
    }
    let Some(magnet) = find_item(&app, 11, 13) else {
        eprintln!("skipping: no magnet item");
        return;
    };
    put_in_first_slot(&mut app, magnet);
    let money = app.player.money;
    assert!(use_first_slot(&mut app));
    assert_eq!(app.player.money, money + 50);
    assert!(app.player.inventory.contains(&item));
    let pop = app.population_cache.get(&4).unwrap();
    let a = pop
        .actors
        .iter()
        .find(|a| a.source_entity_id() == eid)
        .unwrap();
    assert_eq!(a.entity.stats.money, 0);
    assert!(a.entity.inventory.iter().all(|&id| id == 0));
    assert_eq!(app.world.res.item_class(magnet), Some(0xFF));
}

#[test]
fn c4_orb_teleports_and_is_reusable() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 28);
    let Some(orb) = find_item(&app, 11, 14) else {
        eprintln!("skipping: no orb item");
        return;
    };
    put_in_first_slot(&mut app, orb);
    assert!(use_first_slot(&mut app));
    assert_eq!(app.current_map, 28);
    assert_eq!(app.player_x.round() as i32, 184);
    assert_eq!(app.player_y.round() as i32, 18);
    assert_eq!(app.player_dir, 1);
    assert_eq!(app.world.res.item_class(orb), Some(11));
}

#[test]
fn c4_map_orb_opens_the_world_map() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some(orb) = find_item(&app, 11, 20) else {
        eprintln!("skipping: no map orb");
        return;
    };
    put_in_first_slot(&mut app, orb);
    assert!(use_first_slot(&mut app));
    assert!(app.map_open);
    assert_eq!(app.world.res.item_class(orb), Some(11));
}

#[test]
fn c4_conditional_key_teleports_only_at_its_cell() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 50);
    add_free_location(&mut app, 46);
    let key = alloc(&mut app, 11, 44, 0);
    put_in_first_slot(&mut app, key);
    app.current_map = 50;
    app.player_x = 34.0;
    app.player_y = 11.0;
    assert!(use_first_slot(&mut app));
    assert_eq!(app.current_map, 46);
    assert_eq!(
        (app.player_x.round() as i32, app.player_y.round() as i32),
        (68, 13)
    );
    assert_eq!(app.player_dir, 2);
    assert_eq!(app.world.res.item_class(key), Some(11));
}

#[test]
fn c4_skeleton_lamp_kills_other_groups() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.ensure_population(4);
    let Some(eid) = app.population_cache.get(&4).and_then(|pop| {
        pop.actors
            .iter()
            .find(|a| a.entity.group_id != 0)
            .map(|a| a.source_entity_id())
    }) else {
        eprintln!("skipping: no world actor on loc 4");
        return;
    };
    let lamp = alloc(&mut app, 11, 30, 0);
    put_in_first_slot(&mut app, lamp);
    assert!(use_first_slot(&mut app));
    let pop = app.population_cache.get(&4).unwrap();
    assert!(pop.motion.get(&eid).map(|m| m.dead).unwrap_or(false));
    assert_eq!(app.world.res.item_class(lamp), Some(0xFF));
}

#[test]
fn c4_mirror_reactivates_negative_ground_records() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let index = app.world_object_sources.len();
    app.world_object_sources
        .push(crate::world_objects::WorldObjectSource {
            index,
            t0: 0,
            t4: 0,
            x: 10,
            y: 10,
            money: -5,
            loc: 4,
            items: [0; crate::world_objects::WORLD_OBJECT_ITEM_SLOTS],
        });
    app.rebuild_world_objects();
    // `FUN_0043ADF0` keeps buried records in the active pool; `FUN_0042532C`
    // simply never draws them (money < 0).
    assert!(app
        .active_world_objects
        .iter()
        .any(|o| o.cell() == (10, 10) && o.money < 0));
    let mirror = alloc(&mut app, 11, 48, 0);
    put_in_first_slot(&mut app, mirror);
    assert!(use_first_slot(&mut app));
    assert_eq!(app.world_object_sources[index].money, 5);
    assert!(app
        .active_world_objects
        .iter()
        .any(|o| o.cell() == (10, 10) && o.money > 0));
    assert_eq!(app.world.res.item_class(mirror), Some(0xFF));
}

#[test]
fn c4_whistle_scatters_object_actors() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.ensure_population(4);
    let Some(eid) = app.population_cache.get(&4).and_then(|pop| {
        pop.actors
            .iter()
            .find(|a| a.entity.archetype & 0x40 != 0)
            .map(|a| a.source_entity_id())
    }) else {
        eprintln!("skipping: no object-family actor on loc 4");
        return;
    };
    let Some(whistle) = find_item(&app, 11, 35) else {
        eprintln!("skipping: no whistle");
        return;
    };
    if let Some(pop) = app.population_cache.get_mut(&4) {
        if let Some(m) = pop.motion.get_mut(&eid) {
            m.cell = (100, 80);
            m.render_cell = (100, 80);
        }
    }
    put_in_first_slot(&mut app, whistle);
    assert!(use_first_slot(&mut app));
    let m = app
        .population_cache
        .get(&4)
        .and_then(|p| p.motion.get(&eid))
        .unwrap();
    assert!(
        m.target.is_some() || m.walking || !m.path.is_empty(),
        "the whistle must issue a move"
    );
    assert_eq!(app.world.res.item_class(whistle), Some(11));
}

#[test]
fn c1_doll_sets_npc_state_and_is_consumed() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.ensure_population(4);
    let ids: Vec<usize> = app
        .population_cache
        .get(&4)
        .map(|p| {
            p.actors
                .iter()
                .filter(|a| a.entity.group_id != 0)
                .map(|a| a.source_entity_id())
                .take(2)
                .collect()
        })
        .unwrap_or_default();
    if ids.len() < 2 {
        eprintln!("skipping: loc 4 needs two world actors");
        return;
    }
    let (alive, dead) = (ids[0], ids[1]);
    for id in [alive, dead] {
        if let Some(tables) = app.tables.as_mut() {
            if let Some(e) = tables.entities.get_mut(id) {
                e.state_d2 = 0;
                e.archetype &= 0x7F;
            }
        }
        if let Some(pop) = app.population_cache.get_mut(&4) {
            if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == id) {
                a.entity.state_d2 = 0;
                a.entity.archetype &= 0x7F;
            }
            if let Some(m) = pop.motion.get_mut(&id) {
                m.dead = false;
                m.action = 0;
            }
        }
    }
    // Second actor is a corpse: row 3, archetype death bit.
    if let Some(tables) = app.tables.as_mut() {
        if let Some(e) = tables.entities.get_mut(dead) {
            e.archetype |= 0x80;
        }
    }
    if let Some(pop) = app.population_cache.get_mut(&4) {
        if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == dead) {
            a.entity.archetype |= 0x80;
        }
        if let Some(m) = pop.motion.get_mut(&dead) {
            m.dead = true;
            m.action = 3;
        }
    }
    for m in app.party.iter_mut() {
        m.entity.state_d2 = 0;
    }

    let doll = find_item(&app, 11, 33).unwrap_or_else(|| alloc(&mut app, 11, 33, 0));
    put_in_first_slot(&mut app, doll);
    assert!(use_first_slot(&mut app), "the doll must be used");

    let pop = app.population_cache.get(&4).unwrap();
    let alive_actor = pop
        .actors
        .iter()
        .find(|a| a.source_entity_id() == alive)
        .unwrap();
    assert_eq!(alive_actor.entity.state_d2, 0x13);
    let dead_actor = pop
        .actors
        .iter()
        .find(|a| a.source_entity_id() == dead)
        .unwrap();
    assert_eq!(dead_actor.entity.state_d2, 0, "death rows are skipped");
    assert!(
        app.party.iter().all(|m| m.entity.state_d2 == 0),
        "the user's own group is skipped"
    );
    assert_eq!(app.tables.as_ref().unwrap().entities[alive].state_d2, 0x13);
    assert_eq!(app.world.res.item_class(doll), Some(0xFF));
}
