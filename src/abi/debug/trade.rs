//! Trade diagnostics (headless smoke tests / screenshots).

use super::super::*;

#[no_mangle]
pub extern "C" fn debug_trade_active() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.trade.is_some() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn debug_trade_open(npc: u32) -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.start_trade(npc as usize);
    g.trade.is_some() as u32
}

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

#[no_mangle]
pub extern "C" fn debug_trade_confirm() -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.trade_confirm();
    g.trade.is_some() as u32
}

#[no_mangle]
pub extern "C" fn debug_trade_cancel() -> u32 {
    let rt = runtime_mut();
    let Some(g) = rt.game.as_mut() else {
        return 0;
    };
    g.trade_cancel();
    g.trade.is_none() as u32
}

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

#[no_mangle]
pub extern "C" fn debug_trade_cell(row: u32, col: u32) -> u32 {
    let rt = runtime_mut();
    rt.game
        .as_deref()
        .and_then(|g| g.trade.as_ref())
        .map(|t| t.visible_cell(row as usize, col as usize) as u32)
        .unwrap_or(0)
}

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
