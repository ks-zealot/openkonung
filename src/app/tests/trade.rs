//! App-level trade integration tests against the shipped `Game.1` data.

use super::super::*;
use super::common::*;
use crate::inventory::INVENTORY_SLOTS;
use crate::trade::{ROW_PARTNER, ROW_PLAYER};

/// Build a rich party app with the real `Game.1` trade stocks and an empty bag.
///
/// Attaches `Game.1` to `Res`, parses the settlement trade rows and gives the
/// leader 100000 gold so price checks never fail for lack of money.
fn trade_app() -> Option<PlayableApp> {
    let mut app = build_party_app()?;
    let save = std::fs::read(install_dir().join("Game.1")).ok()?;
    app.world.res.set_save_game(&save);
    app.settlement_trade = crate::population::parse_settlement_trade(&save).unwrap_or_default();
    app.player.money = 100_000;
    app.player.inventory = [0; INVENTORY_SLOTS];
    Some(app)
}

/// Borrow the first stock slot of `role` at the current location.
fn stock_slot_mut(app: &mut PlayableApp, role: u8) -> Option<&mut u16> {
    let ti = app
        .settlement_trade
        .iter()
        .position(|t| t.loc_id as u32 == app.current_map)?;
    let trade = app.settlement_trade.get_mut(ti)?;
    trade.stock_mut(role)?.first_mut()
}

/// `FUN_00417010`: the pile is the partner side; taking its stock on confirm
/// moves the item into the bag and shrinks (or finishes) the pile.
#[test]
fn ground_container_confirm_moves_pile_items_into_the_bag() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(9, 1, 0, -1, 1.0, 0, 0)
        .expect("item");
    let cell = (app.player_x.round() as i32, app.player_y.round() as i32);
    app.active_world_objects.push(crate::world_objects::ActiveWorldObject {
        source: usize::MAX,
        x: cell.0,
        y: cell.1,
        money: 0,
        items: vec![item],
        sprite: 0,
    });
    let index = app.active_world_objects.len() - 1;
    app.start_ground_trade(index);
    assert!(app.trade.is_some(), "ground window opens");
    assert_eq!(app.ground_pile, Some(index));

    // Click the pile stock (row 2) -> partner offer (row 3), then confirm.
    app.trade_click_cell(ROW_PARTNER, 0);
    app.trade_confirm();
    assert!(app.trade.is_none(), "confirm closes the ground window");
    assert_eq!(app.ground_pile, None);
    assert!(
        app.player.inventory.contains(&item),
        "the taken item reached the bag"
    );
    assert!(
        !app.active_world_objects
            .iter()
            .any(|o| o.cell() == cell),
        "an emptied pile is finalized"
    );
}

/// `FUN_004110EC` + `FUN_00417010`: a full bag leaves the remainder in the pile
/// and opens the ground-container window instead of silently stopping.
#[test]
fn a_full_bag_opens_the_ground_container_window() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.ensure_population(app.current_map);
    let item = app
        .world
        .res
        .alloc_item_instance(9, 1, 0, -1, 1.0, 0, 0)
        .expect("item");
    let cell = (app.player_x.round() as i32, app.player_y.round() as i32);
    app.active_world_objects.push(crate::world_objects::ActiveWorldObject {
        source: usize::MAX,
        x: cell.0,
        y: cell.1,
        money: 0,
        items: vec![item],
        sprite: 0,
    });
    // Fill every bag slot so the pickup must refuse the item.
    app.player.inventory = [item; INVENTORY_SLOTS];
    app.try_pickup_here();
    assert!(app.trade.is_some(), "the discard window opens");
    let index = app.ground_pile.expect("pile index");
    assert_eq!(
        app.active_world_objects[index].items,
        vec![item],
        "the item stays in the pile"
    );
    app.trade_cancel();
    assert_eq!(
        app.active_world_objects[index].items,
        vec![item],
        "cancel leaves the pile untouched"
    );
}

/// Entities 278/279/280 resolve to the merchant/smith/voivode roles from the waypoint owners, and anyone else to 0.
#[test]
fn role_resolution_matches_settlement_waypoints() {    let Some(app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    assert_eq!(app.trade_role(278), 2);
    assert_eq!(app.trade_role(279), 3);
    assert_eq!(app.trade_role(280), 4);
    assert_eq!(app.trade_role(420), 0);
}

/// Buying stock moves the item into the bag, charges money, clears the slot and reopens the exchange.
#[test]
fn buy_from_settlement_stock_deducts_money_and_clears_stock() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("item");
    *stock_slot_mut(&mut app, 2).expect("role 2 stock") = item;
    app.start_trade(278);
    assert!(app.trade.is_some());
    let before = app.player.money;
    app.trade_click_cell(ROW_PARTNER, 0);
    app.trade_confirm();
    assert!(app.trade.is_some(), "successful NPC deal reopens trade");
    assert!(app.player.inventory.contains(&item));
    assert!(app.player.money < before);
    let role2 = app
        .settlement_trade
        .iter()
        .find(|t| t.loc_id as u32 == app.current_map)
        .map(|t| t.stock2)
        .unwrap();
    assert!(!role2.contains(&item));
}

/// Selling to a settlement merchant pays the player and destroys the sold instance (`class = 0xFF`).
#[test]
fn selling_destroys_the_item_for_a_settlement_merchant() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("item");
    app.player.inventory[0] = item;
    app.start_trade(278);
    let before = app.player.money;
    app.trade_click_cell(ROW_PLAYER, 0);
    app.trade_confirm();
    assert!(app.player.money > before);
    assert_eq!(app.world.res.item_class(item), Some(0xFF));
}

/// Cancelling returns the offers to their rows and closes the window.
#[test]
fn cancel_returns_offers_and_closes_the_exchange() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("item");
    app.player.inventory[0] = item;
    app.start_trade(278);
    app.trade_click_cell(ROW_PLAYER, 0);
    app.trade_cancel();
    assert!(app.trade.is_none());
    assert_eq!(app.player.inventory[0], item);
}

/// Dialogue record 2788 (op `0x113`) closes the dialogue and opens the trade window.
#[test]
fn npc_dialogue_trade_option_opens_the_exchange() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.open_dialog(420, 117);
    let mut index = app
        .dialog
        .as_ref()
        .and_then(|d| d.options.iter().position(|o| o.record == 2788));
    if index.is_none() {
        // The merchant's first page is a menu; option 1 leads to the shop.
        app.dialog_click_option(1);
        index = app
            .dialog
            .as_ref()
            .and_then(|d| d.options.iter().position(|o| o.record == 2788));
    }
    let index = index.expect("trade option in dialogue 117");
    app.dialog_click_option(index);
    assert!(app.trade.is_some());
    assert!(app.dialog.is_none());
}
/// The settlement pass seeds role-2 potions and role-4 equipment at the
/// original pace: role 4 only *reserves* a slot on the first pass and finishes
/// it after `round(level * 60 / sqrt(smithing))` passes.
#[test]
fn role2_and_role4_restock_produce_stock() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.tick_settlement_trade(1);
    let ti = settlement_ti(&app);
    let t = &app.settlement_trade[ti];
    assert!(
        t.stock2.iter().filter(|&&id| id != 0).count() >= 3,
        "role 2 should seed potions"
    );
    // The loc-1 smith (entity 360) has smithing 9, so the first slot (recipe
    // level 1) completes after round(1 * 60 / sqrt(9)) = 20 passes.
    assert_eq!(t.last_slot, 0, "role 4 reserves the first empty slot");
    assert_eq!(
        t.timer4, 20,
        "delay = round(level*60 / sqrt(smithing)) (`FUN_004179C8`)"
    );

    app.tick_settlement_trade(20);
    let t = &app.settlement_trade[ti];
    assert!(
        t.stock4.iter().any(|&id| id != 0),
        "role 4 should craft equipment once the timer expires"
    );
}

/// The role-2 producer crafts the `(input, 3)` recipe rows into consumables:
/// subtype 5 healing potion, 6 poison, 7 weapon poison, with
/// `quality = healing * 0.01` and the row's healing-skill gain.
#[test]
fn role2_restock_crafts_potions() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.tick_settlement_trade(1);
    let ti = settlement_ti(&app);
    // Elements 5/6/7 live at stock indices 2/3/4 (`+0x40C + 2*i`, row at +0x412).
    let made: Vec<u16> = app.settlement_trade[ti].stock2[2..5].to_vec();
    let kinds: Vec<(u8, u8)> = made
        .iter()
        .map(|&id| app.world.res.item_class_subtype(id).expect("produced item"))
        .collect();
    assert_eq!(kinds, vec![(9, 5), (9, 6), (9, 7)]);
    for (i, &id) in made.iter().enumerate() {
        let q = app.world.res.item_quality(id).expect("quality");
        // The row gains apply immediately: 201 -> 201 (gain 0) -> 204 (gain 3)
        // -> 206 (gain 2), so the quality is the current healing * 0.01.
        let expected = [2.01f32, 2.01, 2.04][i];
        assert!(
            (q - expected).abs() < 0.001,
            "quality of item {id} = {q}, expected {expected}"
        );
    }
    // Row gains 0 + 3 + 2 (`0x450DF4` rows 25/21/23).
    assert_eq!(
        app.tables.as_ref().unwrap().entities[358].stats.healing,
        206
    );
}

/// Return the `settlement_trade` index matching the current location's settlement.
fn settlement_ti(app: &PlayableApp) -> usize {
    let si = app
        .settlements
        .iter()
        .position(|s| s.loc_id as u32 == app.current_map)
        .expect("current settlement");
    app.settlement_trade
        .iter()
        .position(|t| t.index == si)
        .expect("settlement trade")
}

/// Role 3 is never restocked but still buys from the player.
#[test]
fn role3_is_buy_only() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.tick_settlement_trade(200);
    let ti = settlement_ti(&app);
    assert!(
        app.settlement_trade[ti].stock3.iter().all(|&id| id == 0),
        "role 3 must never be restocked"
    );

    let item = app
        .world
        .res
        .alloc_item_instance(0, 1, 0, -1, 100.0, 0, 0)
        .expect("item");
    app.player.inventory[0] = item;
    app.start_trade(279);
    let before = app.player.money;
    app.trade_click_cell(ROW_PLAYER, 0);
    app.trade_confirm();
    assert!(app.player.money > before);
    assert_eq!(app.world.res.item_class(item), Some(0xFF));
    assert!(app.settlement_trade[ti].stock3.iter().all(|&id| id == 0));
}

/// A full role-4 stock moves onto a group member's ammo slot and empties the stock.
#[test]
fn role4_distributes_full_stock_to_a_group_member() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let item = app
        .world
        .res
        .alloc_item_instance(12, 0x1E, 0, -1, 0.0, 0, 0)
        .expect("item");
    app.world.res.set_item_quantity(item, 30);
    let ti = settlement_ti(&app);
    for slot in app.settlement_trade[ti].stock4.iter_mut() {
        *slot = item;
    }
    if let Some(tables) = app.tables.as_mut() {
        tables.entities[277].special_equipment = 0;
    }
    app.tick_settlement_trade(1);
    assert_eq!(app.settlement_trade[ti].stock4[0], 0);
    let special = app.tables.as_ref().unwrap().entities[277].special_equipment;
    assert_eq!(special, item);
}

/// A higher-subtype restock replaces the member's equipment and destroys the old piece.
#[test]
fn role4_distribution_replaces_lower_subtype_gear() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let filler = app
        .world
        .res
        .alloc_item_instance(12, 0x1E, 0, -1, 0.0, 0, 0)
        .expect("filler");
    let new = app
        .world
        .res
        .alloc_item_instance(2, 9, 0, -1, 100.0, 0, 0)
        .expect("new");
    let old = app
        .world
        .res
        .alloc_item_instance(2, 1, 0, -1, 100.0, 0, 0)
        .expect("old");
    let ti = settlement_ti(&app);
    for slot in app.settlement_trade[ti].stock4.iter_mut() {
        *slot = new;
    }
    if let Some(tables) = app.tables.as_mut() {
        for member in 277..289 {
            tables.entities[member].special_equipment = filler;
        }
        // The Кинжал (subtype 9) prototype requires strength 21; the
        // requirement map is now the real `0x41B1A4` one.
        tables.entities[278].stats.strength_current = 30;
        tables.entities[278].stats.strength_base = 30;
        tables.entities[278].equipment[2] = old;
    }
    app.tick_settlement_trade(1);
    assert_eq!(app.settlement_trade[ti].stock4[3], 0);
    assert_eq!(app.tables.as_ref().unwrap().entities[278].equipment[2], new);
    // The old lower-subtype item is destroyed (`class = 0xFF`). The pass now
    // runs for all 20 settlements, so a later settlement's role-2 restock may
    // immediately recycle the freed instance slot; either way the old
    // `(class 2, subtype 1)` gear is gone.
    assert_ne!(
        app.world.res.item_class_subtype(old),
        Some((2, 1)),
        "replaced gear must be destroyed or its slot recycled"
    );
}

/// The map-entry seeder (`FUN_00418E6C`) fills the role-3 ladder and the
/// role-2 jar slots, which no producer ever creates.
#[test]
fn map_entry_seeds_settlement_stock() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let ti = settlement_ti(&app);
    assert!(
        app.settlement_trade[ti].stock3.iter().all(|&id| id == 0),
        "stock starts empty"
    );
    app.seed_settlement_stock(
        app.current_map,
        &mut crate::actors::DeterministicRng::new(7),
    );
    let t = &app.settlement_trade[ti];
    for slot in 14..32 {
        assert_ne!(t.stock3[slot], 0, "slot {slot} is part of the fixed ladder");
    }
    for (slot, class) in [
        (14usize, 7u8),
        (15, 6),
        (16, 8),
        (19, 1),
        (20, 1),
        (21, 3),
        (22, 5),
        (23, 4),
        (24, 3),
        (25, 5),
        (26, 4),
        (28, 1),
        (29, 3),
        (30, 5),
        (31, 4),
    ] {
        assert_eq!(
            app.world.res
                .item_class_subtype(t.stock3[slot])
                .expect("seeded item")
                .0,
            class,
            "slot {slot} class"
        );
    }
    for slot in [17usize, 18, 27] {
        let class = app
            .world
            .res
            .item_class_subtype(t.stock3[slot])
            .expect("seeded weapon")
            .0;
        assert!(class == 0 || class == 2, "slot {slot} weapon class");
    }
    // 1..3 jars across the two role-2 elements (0 and 14).
    let jars: Vec<(u8, u8)> = [0usize, 14]
        .iter()
        .filter(|&&slot| t.stock2[slot] != 0)
        .map(|&slot| app.world.res.item_class_subtype(t.stock2[slot]).unwrap())
        .collect();
    assert!(!jars.is_empty() && jars.iter().all(|&k| k == (9, 3)));
}
