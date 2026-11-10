//! App-level inventory/craft/drag/companion tests.

use super::super::*;
use super::common::*;
use crate::inventory::{InventoryOwner, ItemLocation, StatOp, INVENTORY_SLOTS, MAIN_SLOTS};

fn inventory_app() -> Option<PlayableApp> {
    let mut app = build_party_app()?;
    let save = std::fs::read(install_dir().join("Game.1")).ok()?;
    app.world.res.set_save_game(&save);
    Some(app)
}

#[test]
fn lift_place_and_deposit_equipment() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(1, 1, 0, -1, 1.0, 0, 0)
        .expect("item");
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.equipment = [0; MAIN_SLOTS];
    app.player.inventory[3] = item;
    app.set_inventory_owner(InventoryOwner::Protagonist);

    assert!(app.lift_item(ItemLocation::Inventory(3)));
    assert_eq!(app.held_item.map(|h| h.id), Some(item));
    assert_eq!(app.player.inventory[3], 0);

    assert!(app.place_held(ItemLocation::Main(1)));
    assert_eq!(app.player.equipment[1], item);
    assert!(app.held_item.is_none());

    assert!(app.lift_item(ItemLocation::Main(1)));
    assert_eq!(app.player.equipment[1], 0);
    assert!(app.deposit_held());
    assert_eq!(app.player.inventory[0], item);
    assert!(app.held_item.is_none());
}

#[test]
fn craft_through_cursor_produces_potion_and_raises_healing() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let a = app
        .world
        .res
        .alloc_item_instance(9, 3, 0, -1, 1.0, 0, 0)
        .expect("jar");
    let b = app
        .world
        .res
        .alloc_item_instance(9, 0, 0, -1, 1.0, 0, 0)
        .expect("stinger");
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.inventory[0] = a;
    app.player.inventory[1] = b;
    let healing = app.player.healing;

    assert!(app.lift_item(ItemLocation::Inventory(0)));
    // Lifting shifts the remaining inventory left, so the target is slot 0.
    assert!(app.try_craft_held_at(InventoryOwner::Protagonist, ItemLocation::Inventory(0)));
    assert_eq!(app.player.inventory[0], 0);
    assert_eq!(app.held_item.map(|h| h.id), Some(a));
    assert_eq!(app.world.res.item_class_subtype(a), Some((9, 6)));
    assert_eq!(app.player.healing, healing + 3);
    assert!(app.deposit_held());
    assert_eq!(app.player.inventory[0], a);
}

#[test]
fn right_click_potion_heals_and_becomes_an_empty_jar() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let potion = app
        .world
        .res
        .alloc_item_instance(9, 5, 0, -1, 10.0, 0, 0)
        .expect("potion");
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.inventory[0] = potion;
    app.player.hp_internal = 1000;
    assert!(app.use_inventory_slot(0));
    assert_eq!(app.player.hp_internal, 1600);
    assert_eq!(app.world.res.item_class_subtype(potion), Some((9, 3)));
}

#[test]
fn right_click_equips_by_class() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(1, 1, 0, -1, 1.0, 0, 0)
        .expect("item");
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.equipment = [0; MAIN_SLOTS];
    app.player.inventory[0] = item;
    assert!(app.right_click_inventory_slot(0));
    assert_eq!(app.player.equipment[1], item);
    assert!(app.held_item.is_none());
}

#[test]
fn depositing_ammo_stacks_into_the_special_slot() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let existing = app
        .world
        .res
        .alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0)
        .expect("stack");
    let incoming = app
        .world
        .res
        .alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0)
        .expect("incoming");
    app.world.res.set_item_quantity(existing, 30);
    app.world.res.set_item_quantity(incoming, 5);
    app.player.special_equipment = existing;
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.held_item = Some(crate::inventory::HeldItem {
        id: incoming,
        owner: InventoryOwner::Protagonist,
        source: None,
    });
    assert!(app.deposit_held());
    assert_eq!(app.world.res.item_quantity(existing), Some(35));
    assert_eq!(app.world.res.item_class(incoming), Some(0xFF));
    assert!(app.held_item.is_none());
}

#[test]
fn companion_owner_switch_and_transfer() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.recruit_companion(539);
    assert_eq!(app.party.len(), 1);
    let item = app
        .world
        .res
        .alloc_item_instance(9, 1, 0, -1, 1.0, 0, 0)
        .expect("herb");
    app.party[0].entity.inventory[0] = item;
    app.set_inventory_owner(InventoryOwner::Party(0));

    assert!(app.lift_item(ItemLocation::Inventory(0)));
    assert_eq!(
        app.held_item.map(|h| h.owner),
        Some(InventoryOwner::Party(0))
    );
    assert!(app.give_held_to_member(0));
    assert!(app.held_item.is_none());
    assert!(app.player.inventory.contains(&item));
}

#[test]
fn temporary_effect_restores_saved_stats() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.player.dexterity_current = 30;
    app.player.charisma_current = 10;
    app.player.strength_current = 10;
    app.player.endurance_current = 10;
    let ops = [
        StatOp::SaveStats,
        StatOp::SetTempTimer(1),
        StatOp::AddDexterityCurrent(-6),
        StatOp::AddCharismaCurrent(2),
    ];
    crate::inventory::apply_player_ops(&mut app.player, &ops);
    assert_eq!(app.player.dexterity_current, 24);
    assert_eq!(app.player.charisma_current, 12);
    app.tick_temp_effects(1.0);
    assert_eq!(app.player.dexterity_current, 30);
    assert_eq!(app.player.charisma_current, 10);
}

#[test]
fn raise_stat_moves_xp_and_increments_current_only() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.set_inventory_owner(InventoryOwner::Protagonist);
    // Level 0 (0x45B0AB caps are open) so the cost table decides.
    app.player.level = 0;
    app.player.available_xp = 1000;
    app.player.invested_xp = 0;
    let stat = crate::character::PrimaryStat::Strength;
    let cost = {
        let sheet = app
            .growth_sheet(InventoryOwner::Protagonist)
            .expect("sheet");
        crate::character_screen::stat_raise_cost(&sheet, stat)
    };
    assert!(cost > 0, "fresh level-0 warrior can raise strength");
    let base = app.player.strength_base;
    let current = app.player.strength_current;
    assert!(app.raise_owner_stat(InventoryOwner::Protagonist, stat));
    assert_eq!(app.player.strength_current, current + 1);
    assert_eq!(app.player.strength_base, base, "base half untouched");
    assert_eq!(app.player.available_xp, 1000 - cost);
    assert_eq!(app.player.invested_xp, cost);
}

#[test]
fn level_up_requires_the_min_gate() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.set_inventory_owner(InventoryOwner::Protagonist);
    app.player.level = 1;
    app.player.dexterity_current = 10;
    app.player.charisma_current = 10;
    app.player.strength_current = 15;
    app.player.endurance_current = 10;
    assert!(!app.level_up_owner(InventoryOwner::Protagonist));
    assert_eq!(app.player.level, 1);
    app.player.strength_current = 16;
    assert!(app.level_up_owner(InventoryOwner::Protagonist));
    assert_eq!(app.player.level, 2);
}
