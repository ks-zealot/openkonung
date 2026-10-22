use super::*;

fn value(id: u16) -> i32 {
    id as i32 * 10
}

fn weight(_id: u16) -> u32 {
    1
}

fn state(player_inv: [u16; SLOTS], partner_inv: [u16; SLOTS]) -> TradeState {
    TradeState::open(0, 0, 0, player_inv, 100, 10, partner_inv, 100, 10)
}

#[test]
fn buy_moves_partner_item_and_charges_the_player() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 1;
    let mut partner = [0u16; SLOTS];
    partner[0] = 2;
    let mut t = state(inv, partner);
    assert!(t.move_cell(ROW_PARTNER, 0));
    t.rebalance(value);
    assert_eq!(t.rows[ROW_PARTNER_OFFER][0], 2);
    assert_eq!(t.rows[ROW_PARTNER][0], 0);
    assert_eq!(t.table_money[0], 20);
    assert_eq!(t.player_money, 80);
    let out = t.confirm(value, weight, 0, 100).unwrap();
    assert_eq!(out.bought, vec![2]);
    assert_eq!(out.player_money_delta, -20);
    assert_eq!(out.player_inventory[0], 2);
    assert_eq!(out.player_inventory[1], 1);
}

#[test]
fn sell_moves_player_item_and_pays_the_player() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 1;
    let mut t = state(inv, [0u16; SLOTS]);
    assert!(t.move_cell(ROW_PLAYER, 0));
    t.rebalance(value);
    assert_eq!(t.table_money[1], 5);
    assert_eq!(t.partner_money, 95);
    let out = t.confirm(value, weight, 0, 100).unwrap();
    assert_eq!(out.sold, vec![1]);
    assert_eq!(out.player_money_delta, 5);
    assert_eq!(out.partner_inventory[0], 1);
}

#[test]
fn equal_exchange_needs_no_money() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 2; // 20 base, sell 10
    let mut partner = [0u16; SLOTS];
    partner[0] = 1; // buy 10
    let mut t = state(inv, partner);
    assert!(t.move_cell(ROW_PLAYER, 0));
    assert!(t.move_cell(ROW_PARTNER, 0));
    t.rebalance(value);
    assert_eq!(t.table_money, [0, 0]);
    assert!(t.confirm(value, weight, 0, 100).is_ok());
}

#[test]
fn exchange_with_additional_pay_balances_the_difference() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 1; // 10 base, sell 5
    let mut partner = [0u16; SLOTS];
    partner[0] = 3; // 30 base, buy 30
    let mut t = state(inv, partner);
    t.player_money = 50;
    t.partner_money = 50;
    assert!(t.move_cell(ROW_PLAYER, 0));
    assert!(t.move_cell(ROW_PARTNER, 0));
    t.rebalance(value);
    assert_eq!(t.table_money[0], 25);
    assert_eq!(t.player_money, 25);
    let out = t.confirm(value, weight, 0, 100).unwrap();
    assert_eq!(out.player_money_delta, -25);
}

#[test]
fn not_enough_money_is_rejected() {
    let mut partner = [0u16; SLOTS];
    partner[0] = 9; // buy 90
    let mut t = state([0u16; SLOTS], partner);
    t.player_money = 10;
    assert!(t.move_cell(ROW_PARTNER, 0));
    t.rebalance(value);
    assert_eq!(t.table_money[0], 10);
    assert_eq!(
        t.confirm(value, weight, 0, 100),
        Err(TradeError::NotEnoughMoney)
    );
}

#[test]
fn cancel_returns_player_offers() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 1;
    inv[1] = 2;
    let mut t = state(inv, [0u16; SLOTS]);
    assert!(t.move_cell(ROW_PLAYER, 0));
    let out = t.cancel();
    assert_eq!(&out[..2], &[1, 2]);
}

#[test]
fn overweight_and_full_bag_are_rejected() {
    let mut partner = [0u16; SLOTS];
    partner[0] = 1;
    let mut t = state([0u16; SLOTS], partner);
    assert!(t.move_cell(ROW_PARTNER, 0));
    assert_eq!(t.confirm(value, |_| 200, 0, 100), Err(TradeError::TooHeavy));

    let bag = [1u16; SLOTS];
    let mut t = state(bag, partner);
    assert!(t.move_cell(ROW_PARTNER, 0));
    assert_eq!(
        t.confirm(value, weight, 0, 100_000),
        Err(TradeError::TooManyItems)
    );
}

#[test]
fn paging_requires_more_items() {
    let mut inv = [0u16; SLOTS];
    inv[0] = 1;
    inv[VISIBLE] = 2;
    let mut t = state(inv, [0u16; SLOTS]);
    assert!(t.page_row(ROW_PLAYER, 1));
    assert_eq!(t.page[ROW_PLAYER], 1);
    assert!(!t.page_row(ROW_PLAYER, 1));
    assert!(t.page_row(ROW_PLAYER, -1));
    assert_eq!(t.page[ROW_PLAYER], 0);
}

#[test]
fn role_sign_applies_the_settlement_price_multiplier() {
    assert_eq!(sell_value(100, 10, 10, 0), 50);
    assert_eq!(sell_value(100, 10, 10, 1), 33);
    assert_eq!(buy_value(100, 10, 10, 0), 100);
    assert_eq!(buy_value(100, 10, 10, 1), 150);
}
