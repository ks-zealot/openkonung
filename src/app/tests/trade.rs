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
