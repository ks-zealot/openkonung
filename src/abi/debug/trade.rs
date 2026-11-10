//! Trade diagnostics (headless smoke tests / screenshots).
//!
//! Drives the interface-mode-7 trade window from outside: open it on an NPC,
//! click cells, page rows, confirm/cancel, and read back money, visible cells
//! and the settlement merchant stocks (`Settlement+0x412/+0x444/+0x484` for
//! roles 2/3/4). All functions return a small status code rather than panicking,
//! and 0 when no game is live.

use super::super::*;

/// 1 while the trade window (interface mode 7) is open, else 0.
#[no_mangle]
pub extern "C" fn debug_trade_active() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.trade.is_some() as u32)
        .unwrap_or(0)
}

/// Open the trade window on actor `npc` (source Entity id) exactly like
/// dialogue opcode `0x113` (`FUN_00431E2C`). Returns 1 when a trade session
/// exists afterwards.
#[no_mangle]
pub extern "C" fn debug_trade_open(npc: u32) -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.start_trade(npc as usize);
    g.trade.is_some() as u32
}

/// Click the trade cell at `row`/`col` and report whether the row changed.
///
/// Rows are the original four 32-slot arrays (`0x81CB24`): 0 player bag, 1
/// player offer, 2 partner stock, 3 partner offer. Returns 1 when the click moved
/// an item, else 0 (including out-of-range rows).
#[no_mangle]
pub extern "C" fn debug_trade_move(row: u32, col: u32) -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    let before = g
        .trade
        .as_ref()
        .map(|t| t.rows[row as usize])
        .unwrap_or([0; crate::trade::SLOTS]);
    g.trade_click_cell(row as usize, col as usize);
    let after = g
        .trade
        .as_ref()
        .map(|t| t.rows[row as usize])
        .unwrap_or([0; crate::trade::SLOTS]);
    (before != after) as u32
}

/// Page trade `row` by `direction` (-1 previous, +1 next) and report whether the
/// page index changed. Five cells are visible per row.
#[no_mangle]
pub extern "C" fn debug_trade_page(row: u32, direction: i32) -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    let before = g.trade.as_ref().map(|t| t.page[row as usize]).unwrap_or(0);
    g.trade_page(row as usize, direction);
    let after = g.trade.as_ref().map(|t| t.page[row as usize]).unwrap_or(0);
    (before != after) as u32
}

/// Confirm the deal (`FUN_004203F4`) and return 1 while a trade window exists.
///
/// Both outcomes keep mode 7: a successful deal refreshes and reopens the
/// exchange, a rejected one only sets the status message. Compare
/// [`debug_trade_cell`]/[`debug_trade_money`] before and after to tell them
/// apart.
#[no_mangle]
pub extern "C" fn debug_trade_confirm() -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.trade_confirm();
    g.trade.is_some() as u32
}

/// Cancel the trade window (`INTR[180]`) and return 1 when it is now closed.
#[no_mangle]
pub extern "C" fn debug_trade_cancel() -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.trade_cancel();
    g.trade.is_none() as u32
}

/// Money word of the trade session: `side` 0 player, 1 partner, 2 first table
/// word, anything else the second table word. 0 when no trade is open.
#[no_mangle]
pub extern "C" fn debug_trade_money(side: u32) -> i32 {
    let rt = runtime_mut();
    rt.game
        .as_deref()
        .and_then(|g| g.trade.as_ref())
        .map(|t| match side {
            0 => t.player_money,
            1 => t.partner_money,
            2 => t.table_money[0],
            _ => t.table_money[1],
        })
        .unwrap_or(0)
}

/// Item instance id in the visible trade grid at `row`/`col`, or 0 when empty,
/// out of range or no trade is open.
#[no_mangle]
pub extern "C" fn debug_trade_cell(row: u32, col: u32) -> u32 {
    let rt = runtime_mut();
    rt.game
        .as_deref()
        .and_then(|g| g.trade.as_ref())
        .map(|t| t.visible_cell(row as usize, col as usize) as u32)
        .unwrap_or(0)
}

/// Settlement merchant stock slot: role 2 (herbs/potions, 24 slots), role 3
/// (32) or role 4 (equipment, 23). Returns the stored item id, or 0 when the
/// role/slot/settlement is absent.
#[no_mangle]
pub extern "C" fn debug_trade_stock(role: u32, slot: u32) -> u32 {
    let rt = runtime_mut();
    rt.game
        .as_deref()
        .and_then(|g| {
            let si = g
                .settlements
                .iter()
                .position(|s| s.loc_id as u32 == g.current_map)?;
            let t = g.settlement_trade.iter().find(|t| t.index == si)?;
            t.stock(role as u8)?.get(slot as usize).copied()
        })
        .map(|id| id as u32)
        .unwrap_or(0)
}

/// Seed one merchant stock slot (diagnostic-only; never called by gameplay).
///
/// Returns 1 when the slot was written, 0 when no game/settlement/trade record
/// exists or the role/slot is out of range.
#[no_mangle]
pub extern "C" fn debug_trade_set_stock(role: u32, slot: u32, item: u32) -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    let Some(si) = g
        .settlements
        .iter()
        .position(|s| s.loc_id as u32 == g.current_map)
    else {
        return 0;
    };
    let Some(t) = g.settlement_trade.iter_mut().find(|t| t.index == si) else {
        return 0;
    };
    let Some(stock) = t.stock_mut(role as u8) else {
        return 0;
    };
    let Some(dst) = stock.get_mut(slot as usize) else {
        return 0;
    };
    *dst = item as u16;
    1
}
