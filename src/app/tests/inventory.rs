//! App-level inventory/craft/drag/companion tests.

use super::super::*;
use super::common::*;
use crate::inventory::{
    HeldItem, InventoryOwner, ItemLocation, StatOp, INVENTORY_SLOTS, MAIN_SLOTS,
};

/// Build a party app with the real `Game.1` item-instance table attached.
///
/// `build_party_app` alone leaves `Res` without a save game; re-attaching `Game.1`
/// makes `alloc_item_instance`, `item_info` and the durability/quantity setters work.
fn inventory_app() -> Option<PlayableApp> {
    let mut app = build_party_app()?;
    let save = std::fs::read(install_dir().join("Game.1")).ok()?;
    app.world.res.set_save_game(&save);
    Some(app)
}

/// Lifting clears its source slot, placing fills the equipment slot, and
/// depositing returns it to the source slot (`FUN_0041E8A8`).
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

    // The Spata prototype requires strength 55 (`0x41B1A4` stat 5).
    app.player.strength_base = 100;
    app.player.strength_current = 100;
    assert!(app.lift_item(ItemLocation::Inventory(3)));
    assert_eq!(app.held_item.map(|h| h.id), Some(item));
    assert_eq!(app.player.inventory[3], 0);

    assert!(app.place_held(ItemLocation::Main(1)));
    assert_eq!(app.player.equipment[1], item);
    assert!(app.held_item.is_none());

    assert!(app.lift_item(ItemLocation::Main(1)));
    assert_eq!(app.player.equipment[1], 0);
    assert!(app.deposit_held());
    assert_eq!(
        app.player.equipment[1], item,
        "put-down restores the lift-time container slot"
    );
    assert!(app.held_item.is_none());
    assert!(
        !app.player.inventory.contains(&item),
        "the item is not moved into the bag"
    );
}

/// A held jar on the stinger crafts the potion, raises healing by the table delta and stays in hand.
///
/// Lifting slot 0 shifts the remaining bag left, so the target of the craft is slot 0
/// and the crafted item is the held one.
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

/// A right-clicked healing potion restores full health and turns into an empty jar (subtype 3).
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

/// Right-click equips a class-1 item into equipment slot 1 and leaves the hand empty.
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
    // The Spata prototype requires strength 55 (`0x41B1A4` stat 5).
    app.player.strength_base = 100;
    app.player.strength_current = 100;
    app.player.inventory[0] = item;
    assert!(app.right_click_inventory_slot(0));
    assert_eq!(app.player.equipment[1], item);
    assert!(app.held_item.is_none());
}

/// Ammo on the cursor stacks into the class-12 special slot (30 + 5) and the source instance is freed.
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

/// An item lifted from a companion keeps that owner, and `give_held_to_member` transfers it to the leader.
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

/// A temporary effect snapshots the stats, applies its deltas, and the timer tick restores the snapshot.
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
    // The countdown is one unit per 16-frame pass (`FUN_0041D048` phase A).
    app.tick_temp_effect_pass();
    assert_eq!(app.player.dexterity_current, 30);
    assert_eq!(app.player.charisma_current, 10);
}

/// Raising a stat at level 0 pays the cost table, books the XP as invested and increments only the current half.
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
    let current = app.player.strength_current;
    assert!(app.raise_owner_stat(InventoryOwner::Protagonist, stat));
    assert_eq!(app.player.strength_current, current + 1);
    // `FUN_0041C69C` recomputes the derived half from the stored one.
    assert_eq!(app.player.strength_base, current + 1, "derived half follows");
    assert_eq!(app.player.available_xp, 1000 - cost);
    assert_eq!(app.player.invested_xp, cost);
}

/// A level-up is refused until every current stat meets the per-level minimum.
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

/// `0x41E450` param 12 (`+0xCC` ammo): the class must be 12, unlike the other
/// slot families which are gated per slot.
#[test]
fn only_ammo_fits_the_special_slot() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let sword = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("sword");
    app.player.special_equipment = 0;
    // The arrow prototype (subtype 0) requires strength 28 (`0x41B1A4` stat 5).
    app.player.strength_base = 100;
    app.player.strength_current = 100;
    app.held_item = Some(HeldItem {
        id: sword,
        owner: InventoryOwner::Protagonist,
        source: None,
    });
    assert!(!app.place_held(ItemLocation::Special), "class 0 is not ammo");
    assert!(app.held_item.is_some(), "the sword stays on the cursor");

    app.held_item = None;
    let arrow = app
        .world
        .res
        .alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0)
        .expect("arrow");
    app.held_item = Some(HeldItem {
        id: arrow,
        owner: InventoryOwner::Protagonist,
        source: None,
    });
    assert!(app.place_held(ItemLocation::Special));
    assert_eq!(app.player.special_equipment, arrow);
}

/// `0x41E8A8`: putting a lifted item down restores the container slot it came
/// from instead of front-inserting it into the bag.
#[test]
fn put_down_restores_the_source_slot() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(2, 1, 0, -1, 100.0, 0, 0)
        .expect("weapon");
    app.player.equipment[2] = item;
    app.player.inventory = [0; INVENTORY_SLOTS];
    assert!(app.lift_item(ItemLocation::Main(2)));
    assert_eq!(app.player.equipment[2], 0);
    assert!(app.deposit_held());
    assert_eq!(app.player.equipment[2], item, "back in its slot");
    assert!(
        !app.player.inventory.contains(&item),
        "not front-inserted into the bag"
    );
}

/// `0x422520`: `0x16B` destroys the departing actor's item instances; the
/// `0x14C` settle path keeps them.
#[test]
fn removing_an_actor_releases_its_items() {
    use super::super::dialogue_ops::QuestContext;
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("gear");
    if let Some(t) = app.tables.as_mut() {
        t.entities[280].equipment[0] = item;
    }
    let mut ctx = QuestContext::from_app(&app, 280);
    assert!(ctx.exec_op(0x16B));
    ctx.apply_to_app(&mut app);
    assert_eq!(
        app.world.res.item_class(item),
        Some(0xFF),
        "the departing NPC's gear is destroyed"
    );
}

/// `FUN_0041DAB0` case 11 also sets the ground-marker highlight
/// (`_DAT_0081D628 = 1`), the same flag the torch raises.
#[test]
fn highlight_potion_sets_the_torch_reveal() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let potion = app
        .world
        .res
        .alloc_item_instance(9, 11, 0, -1, 5.0, 0, 0)
        .expect("potion");
    app.inventory_owner = InventoryOwner::Protagonist;
    app.inventory_scroll = 0;
    app.held_item = None;
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.inventory[0] = potion;
    app.torch_reveal = false;
    assert!(app.right_click_inventory_slot(0));
    assert!(app.torch_reveal, "subtype 11 highlights ground markers");
    assert_eq!(app.world.res.item_class_subtype(potion), Some((9, 3)));
}

/// `FUN_0041DAB0` case 8 is an antidote: it reduces the periodic damage by
/// `quality*10` (clamped at 0) and does nothing for an unpoisoned user.
#[test]
fn antidote_reduces_periodic_damage() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let potion = app
        .world
        .res
        .alloc_item_instance(9, 8, 0, -1, 3.0, 0, 0)
        .expect("antidote");
    app.inventory_owner = InventoryOwner::Protagonist;
    app.inventory_scroll = 0;
    app.held_item = None;
    app.player.inventory = [0; INVENTORY_SLOTS];
    app.player.inventory[0] = potion;
    app.player.periodic_damage = 100;
    assert!(app.right_click_inventory_slot(0));
    assert_eq!(app.player.periodic_damage, 70);
    assert_eq!(app.world.res.item_class_subtype(potion), Some((9, 3)));
}

/// `FUN_00410B08` returns 0 while `Entity+0xCE` is running: no raise cost and
/// no level-up.
#[test]
fn temp_effect_blocks_raise_and_level_up() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.set_inventory_owner(InventoryOwner::Protagonist);
    app.player.level = 0;
    app.player.available_xp = 100_000;
    app.player.temp_effect_timer = 30;
    let sheet = app
        .growth_sheet(InventoryOwner::Protagonist)
        .expect("sheet");
    assert_eq!(
        crate::character_screen::stat_raise_cost(&sheet, crate::character::PrimaryStat::Strength),
        0
    );
    assert!(!app.level_up_owner(InventoryOwner::Protagonist));
    app.player.temp_effect_timer = 0;
    let sheet = app
        .growth_sheet(InventoryOwner::Protagonist)
        .expect("sheet");
    assert!(
        crate::character_screen::stat_raise_cost(&sheet, crate::character::PrimaryStat::Strength)
            > 0
    );
}

/// `FUN_0041B0C4`: class 10 has no weight in the original (the selector's
/// `else -> 0` branch); every other table-backed class keeps its prototype
/// weight.
#[test]
fn class_ten_items_weigh_nothing() {
    let Some(mut app) = inventory_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(1, 1, 0, -1, 1.0, 0, 0)
        .expect("item");
    let before = crate::inventory::item_weight(&app.world.res, item);
    assert_ne!(before, 0, "the base item has a prototype weight");
    assert!(app.world.res.set_item_class(item, 10));
    assert_eq!(
        crate::inventory::item_weight(&app.world.res, item),
        0,
        "class 10 is not weighted by FUN_0041B0C4"
    );
}
