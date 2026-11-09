//! App-level trade integration tests against the shipped `Game.1` data.

use super::super::*;
use super::common::*;
use crate::inventory::INVENTORY_SLOTS;
use crate::trade::{ROW_PARTNER, ROW_PLAYER};

fn trade_app() -> Option<PlayableApp> {
    let mut app = build_party_app()?;
    let save = std::fs::read(install_dir().join("Game.1")).ok()?;
    app.world.res.set_save_game(&save);
    app.settlement_trade = crate::population::parse_settlement_trade(&save).unwrap_or_default();
    app.player.money = 100_000;
    app.player.inventory = [0; INVENTORY_SLOTS];
    Some(app)
}

fn stock_slot_mut(app: &mut PlayableApp, role: u8) -> Option<&mut u16> {
    let ti = app
        .settlement_trade
        .iter()
        .position(|t| t.loc_id as u32 == app.current_map)?;
    let trade = app.settlement_trade.get_mut(ti)?;
    trade.stock_mut(role)?.first_mut()
}

#[test]
fn role_resolution_matches_settlement_waypoints() {
    let Some(app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    assert_eq!(app.trade_role(278), 2);
    assert_eq!(app.trade_role(279), 3);
    assert_eq!(app.trade_role(280), 4);
    assert_eq!(app.trade_role(420), 0);
}

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
#[test]
fn role2_and_role4_restock_produce_stock() {
    let Some(mut app) = trade_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.tick_settlement_trade(1);
    let t = app
        .settlement_trade
        .iter()
        .find(|t| t.loc_id as u32 == app.current_map)
        .expect("settlement trade");
    assert!(
        t.stock2.iter().filter(|&&id| id != 0).count() >= 3,
        "role 2 should seed herbs/potions"
    );

    app.tick_settlement_trade(4);
    let t = app
        .settlement_trade
        .iter()
        .find(|t| t.loc_id as u32 == app.current_map)
        .expect("settlement trade");
    assert!(
        t.stock4.iter().any(|&id| id != 0),
        "role 4 should craft equipment"
    );
}

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
