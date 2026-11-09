//! Dialogue-driven inventory exchange (`QUEST_R.RES` opcode `0x113`).
//!
//! Recovered from `konung.res`:
//!
//! * `FUN_00431E2C` builds the four trade rows and opens interface mode 7.
//! * `FUN_00430D6C` renders the window and auto-balances money on every redraw.
//! * `FUN_004203F4(0/1)` cancels/commits the exchange.
//! * `FUN_0041AE00`/`FUN_0041B5A8` apply trade skill and settlement price
//!   multipliers; `FUN_0041B2A8` is the per-instance base value.
//!
//! Row layout (`0x81CB24`):
//!
//! ```text
//! 0 player bag       1 player offer
//! 2 partner bag/stock 3 partner offer
//! ```
//!
//! The page offsets are `0x81CC24[row]` (five visible cells per row).

/// Number of trade rows.
pub const ROWS: usize = 4;
/// Slots per trade row.
pub const SLOTS: usize = 32;
/// Cells visible in one row (`_DAT_0081D644 + 5`).
pub const VISIBLE: usize = 5;

pub const ROW_PLAYER: usize = 0;
pub const ROW_PLAYER_OFFER: usize = 1;
pub const ROW_PARTNER: usize = 2;
pub const ROW_PARTNER_OFFER: usize = 3;

/// Message shown by `FUN_0042E9CC` at `0x45BD1C`.
pub const MSG_TOO_MANY: &str = "Мешок у тебя не резиновый!";
/// Message at `0x45C51C`.
pub const MSG_TOO_HEAVY: &str = "Превышен вес";
/// Message at `0x45BF1C`.
pub const MSG_NOT_ENOUGH_MONEY: &str = "Слишком мало денег!";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeError {
    TooManyItems,
    TooHeavy,
    NotEnoughMoney,
}

impl TradeError {
    pub fn message(self) -> &'static str {
        match self {
            TradeError::TooManyItems => MSG_TOO_MANY,
            TradeError::TooHeavy => MSG_TOO_HEAVY,
            TradeError::NotEnoughMoney => MSG_NOT_ENOUGH_MONEY,
        }
    }
}

/// What the app must apply after a successful confirm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOutcome {
    /// Player inventory after the deal (received partner offers, then the
    /// player's remaining bag; compacted exactly like `FUN_004203F4`).
    pub player_inventory: [u16; SLOTS],
    /// Real player-money delta (`partner_table - player_table`).
    pub player_money_delta: i32,
    /// Partner inventory after the deal (normal NPC only).
    pub partner_inventory: [u16; SLOTS],
    /// Real partner-money delta (`player_table - partner_table`).
    pub partner_money_delta: i32,
    /// Items received from the partner offer (removed from settlement stock).
    pub bought: Vec<u16>,
    /// Items the player offered (destroyed for settlement merchants).
    pub sold: Vec<u16>,
    /// `(player_table_money + sell_value) >> 10`.
    pub trade_gain: u16,
}

/// One open exchange.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeState {
    pub role: u8,
    /// `_DAT_0081D63C`: positive selects the settlement price multiplier.
    pub role_sign: i32,
    pub npc_entity: usize,
    /// Displayed/available player money (`_DAT_0081D694`).
    pub player_money: i32,
    /// Real `Entity+0x34` (used only for the final transfer/clamp).
    pub player_real_money: i32,
    pub partner_money: i32,
    pub player_trade: u16,
    pub partner_trade: u16,
    pub rows: [[u16; SLOTS]; ROWS],
    pub page: [usize; ROWS],
    /// Player/partner money currently on the table (`0x81D6F4/0x81D6F8`).
    pub table_money: [i32; 2],
}

/// `FUN_0041AE00`: player sell value.
pub fn sell_value(total: i32, player_trade: u16, partner_trade: u16, role_sign: i32) -> i32 {
    if total == 0 {
        return 0;
    }
    let diff = player_trade as f64 - partner_trade as f64;
    let mut v = total as f64 * 0.5 * (1.0 + diff * 0.008);
    if role_sign > 0 {
        v *= 2.0 / 3.0;
    }
    v.round() as i32
}

/// `FUN_0041B5A8`: player buy value.
pub fn buy_value(total: i32, player_trade: u16, partner_trade: u16, role_sign: i32) -> i32 {
    if total == 0 {
        return 0;
    }
    let diff = player_trade as f64 - partner_trade as f64;
    let mut v = total as f64 * (1.0 - diff * 0.008);
    if role_sign > 0 {
        v *= 1.5;
    }
    v.round() as i32
}

impl TradeState {
    /// Build the state exactly like `FUN_00431E2C`, including the class-`0x0B`
    /// subtype-`0x20` +10 money scan on the player inventory.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        npc_entity: usize,
        role: u8,
        role_sign: i32,
        player_inventory: [u16; SLOTS],
        player_money: i32,
        player_trade: u16,
        partner_inventory: [u16; SLOTS],
        partner_money: i32,
        partner_trade: u16,
    ) -> TradeState {
        let mut rows = [[0u16; SLOTS]; ROWS];
        rows[ROW_PLAYER] = player_inventory;
        rows[ROW_PARTNER] = partner_inventory;
        TradeState {
            role,
            role_sign,
            npc_entity,
            player_money,
            player_real_money: player_money,
            partner_money,
            player_trade,
            partner_trade,
            rows,
            page: [0; ROWS],
            table_money: [0; 2],
        }
    }

    /// Player offer total (`_DAT_0081D714`) before the sell multiplier.
    pub fn player_offer_base(&self, value: impl Fn(u16) -> i32) -> i32 {
        self.rows[ROW_PLAYER_OFFER]
            .iter()
            .copied()
            .filter(|&id| id != 0)
            .map(&value)
            .sum()
    }

    /// Partner offer total (`_DAT_0081D718`) before the buy multiplier.
    pub fn partner_offer_base(&self, value: impl Fn(u16) -> i32) -> i32 {
        self.rows[ROW_PARTNER_OFFER]
            .iter()
            .copied()
            .filter(|&id| id != 0)
            .map(&value)
            .sum()
    }

    pub fn player_offer_value(&self, value: impl Fn(u16) -> i32) -> i32 {
        sell_value(
            self.player_offer_base(value),
            self.player_trade,
            self.partner_trade,
            self.role_sign,
        )
    }

    pub fn partner_offer_value(&self, value: impl Fn(u16) -> i32) -> i32 {
        buy_value(
            self.partner_offer_base(value),
            self.player_trade,
            self.partner_trade,
            self.role_sign,
        )
    }

    /// `FUN_00430D6C` money auto-balance. Implements all four cases:
    /// buy (player pays), sell (partner pays), equal exchange (no cash), and
    /// exchange-with-additional-pay (the difference is moved automatically).
    pub fn rebalance(&mut self, value: impl Fn(u16) -> i32) {
        let sell = self.player_offer_value(&value);
        let buy = self.partner_offer_value(&value);
        let mut player_side = sell + self.table_money[0];
        let mut partner_side = buy + self.table_money[1];
        if partner_side < player_side {
            // Player offered too much: refund table money first, then take
            // partner money onto the table.
            while self.table_money[0] != 0 && partner_side < player_side {
                self.player_money += 1;
                self.table_money[0] -= 1;
                player_side -= 1;
            }
            while self.partner_money != 0 && partner_side < player_side {
                self.partner_money -= 1;
                self.table_money[1] += 1;
                partner_side += 1;
            }
        } else {
            // Player offered too little: refund partner table money first, then
            // put player money on the table.
            while self.table_money[1] != 0 && player_side < partner_side {
                self.partner_money += 1;
                self.table_money[1] -= 1;
                partner_side -= 1;
            }
            while self.player_money != 0 && player_side < partner_side {
                self.player_money -= 1;
                self.table_money[0] += 1;
                player_side += 1;
            }
        }
    }

    /// Move a visible cell between its bag and offer row.
    ///
    /// `row` is the physical trade row, `visible_col` is `0..VISIBLE`; the
    /// original add the row's page offset and masks with `0x1F`.
    pub fn move_cell(&mut self, row: usize, visible_col: usize) -> bool {
        if row >= ROWS || visible_col >= VISIBLE {
            return false;
        }
        let col = (self.page[row] + visible_col) & 0x1F;
        let source = self.rows[row][col];
        if source == 0 {
            return false;
        }
        let target_row = if row < 2 { 1 - row } else { 5 - row };
        let Some(free) = self.rows[target_row].iter().position(|&id| id == 0) else {
            return false;
        };
        self.rows[target_row][free] = source;
        for i in col..SLOTS - 1 {
            self.rows[row][i] = self.rows[row][i + 1];
        }
        self.rows[row][SLOTS - 1] = 0;
        true
    }

    /// Page a row left (`direction < 0`) or right (`direction > 0`).
    pub fn page_row(&mut self, row: usize, direction: i32) -> bool {
        if row >= ROWS || direction == 0 {
            return false;
        }
        if direction > 0 {
            if self.page[row] + VISIBLE >= SLOTS || self.rows[row][self.page[row] + VISIBLE] == 0 {
                return false;
            }
            self.page[row] += 1;
        } else if self.page[row] > 0 {
            self.page[row] -= 1;
        } else {
            return false;
        }
        true
    }

    pub fn visible_cell(&self, row: usize, visible_col: usize) -> u16 {
        if row >= ROWS || visible_col >= VISIBLE {
            return 0;
        }
        let col = (self.page[row] + visible_col) & 0x1F;
        self.rows[row][col]
    }

    fn count(values: &[u16; SLOTS]) -> usize {
        values.iter().filter(|&&id| id != 0).count()
    }

    fn collected(values: &[u16; SLOTS]) -> Vec<u16> {
        values.iter().copied().filter(|&id| id != 0).collect()
    }

    /// `FUN_004203F4(1)` validation and result. `base_weight` excludes the
    /// player's inventory (equipment/additional/special only); `capacity` is
    /// `(endurance_base*1000)/3 + 10000`.
    pub fn confirm(
        &self,
        value: impl Fn(u16) -> i32,
        weight: impl Fn(u16) -> i32,
        base_weight: i32,
        capacity: i32,
    ) -> Result<TradeOutcome, TradeError> {
        let bag = &self.rows[ROW_PLAYER];
        let received = &self.rows[ROW_PARTNER_OFFER];
        let sold = &self.rows[ROW_PLAYER_OFFER];

        let count = Self::count(bag) + Self::count(received);
        if count >= SLOTS + 1 {
            return Err(TradeError::TooManyItems);
        }
        let mut total_weight = base_weight;
        for &id in bag.iter().chain(received.iter()) {
            if id != 0 {
                total_weight = total_weight.saturating_add(weight(id));
            }
        }
        if total_weight > capacity {
            return Err(TradeError::TooHeavy);
        }

        let sell = self.player_offer_value(&value);
        let buy = self.partner_offer_value(&value);
        if sell + self.table_money[0] < buy + self.table_money[1] {
            return Err(TradeError::NotEnoughMoney);
        }

        let mut player_inventory = [0u16; SLOTS];
        let mut i = 0;
        for &id in received.iter().chain(bag.iter()) {
            if id != 0 && i < SLOTS {
                player_inventory[i] = id;
                i += 1;
            }
        }
        let mut partner_inventory = [0u16; SLOTS];
        i = 0;
        for &id in sold.iter().chain(self.rows[ROW_PARTNER].iter()) {
            if id != 0 && i < SLOTS {
                partner_inventory[i] = id;
                i += 1;
            }
        }

        let trade_gain = ((self.table_money[0] + sell).max(0) / 1024).min(u16::MAX as i32) as u16;
        Ok(TradeOutcome {
            player_inventory,
            player_money_delta: self.table_money[1] - self.table_money[0],
            partner_inventory,
            partner_money_delta: self.table_money[0] - self.table_money[1],
            bought: Self::collected(received),
            sold: Self::collected(sold),
            trade_gain,
        })
    }

    /// `FUN_004203F4(0)`: return the player's offers to the bag. The partner is
    /// unchanged.
    pub fn cancel(&self) -> [u16; SLOTS] {
        let mut out = [0u16; SLOTS];
        let mut i = 0;
        for &id in self.rows[ROW_PLAYER_OFFER]
            .iter()
            .chain(self.rows[ROW_PLAYER].iter())
        {
            if id != 0 && i < SLOTS {
                out[i] = id;
                i += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
