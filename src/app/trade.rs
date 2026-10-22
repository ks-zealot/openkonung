//! Trade integration for [`PlayableApp`]: `0x113` opener, commit/cancel and
//! set-up of the interface-mode-7 exchange state.

use super::*;
use crate::inventory::{carried_weight, item_weight, SlotsRef};
use crate::trade::{TradeError, TradeState};

impl PlayableApp {
    fn settlement_index_for_loc(&self) -> Option<usize> {
        self.settlements
            .iter()
            .position(|s| s.loc_id as u32 == self.current_map)
    }

    fn settlement_trade_index(&self, index: usize) -> Option<usize> {
        self.settlement_trade.iter().position(|t| t.index == index)
    }

    pub(super) fn trade_role(&self, npc_entity: usize) -> u8 {
        self.settlement_index_for_loc()
            .and_then(|i| self.settlements.get(i))
            .map(|s| s.role_of(npc_entity as u16))
            .unwrap_or(0)
    }

    fn partner_snapshot(&self, npc_entity: usize, role: u8) -> Option<([u16; 32], i32, u16)> {
        if matches!(role, 2..=4) {
            let si = self.settlement_index_for_loc()?;
            let ti = self.settlement_trade_index(si)?;
            let t = self.settlement_trade.get(ti)?;
            let stock = t.stock(role)?;
            let mut inv = [0u16; 32];
            let mut n = 0;
            for &id in stock.iter() {
                if id == 0 {
                    continue;
                }
                if t.flags_4b2 & 4 != 0 {
                    let class = self.world.res.item_class(id).unwrap_or(0);
                    if class <= 2 {
                        continue;
                    }
                }
                if n == inv.len() {
                    break;
                }
                inv[n] = id;
                n += 1;
            }
            let trade = self.partner_trade_skill(npc_entity);
            return Some((inv, 99999, trade));
        }
        let e = self.tables.as_ref()?.entities.get(npc_entity)?;
        Some((e.inventory, e.stats.money, e.stats.trade))
    }

    fn partner_trade_skill(&self, npc_entity: usize) -> u16 {
        self.tables
            .as_ref()
            .and_then(|t| t.entities.get(npc_entity))
            .map(|e| e.stats.trade)
            .unwrap_or(0)
    }

    /// Base carried weight excluding the inventory row (equipment, additional,
    /// special), matching `FUN_004203F4`'s temporary inventory clear.
    fn trade_base_weight(&self) -> u32 {
        let empty = [0u16; 32];
        let slots = SlotsRef {
            equipment: &self.player.equipment,
            inventory: &empty,
            additional: &self.player.additional_equipment,
            special: self.player.special_equipment,
        };
        carried_weight(&self.world.res, &slots)
    }

    /// Open the exchange for `npc_entity` (`FUN_00431E2C`).
    pub(crate) fn start_trade(&mut self, npc_entity: usize) {
        self.trade = None;
        let role = self.trade_role(npc_entity);
        let Some((partner_inv, partner_money, partner_trade)) =
            self.partner_snapshot(npc_entity, role)
        else {
            return;
        };
        let role_sign = if matches!(role, 2..=4) {
            let positive = self
                .settlement_index_for_loc()
                .and_then(|si| self.settlement_trade_index(si))
                .and_then(|ti| self.settlement_trade.get(ti))
                .map(|t| t.flags_4b2 & 0x10 != 0)
                .unwrap_or(false);
            if positive {
                role as i32
            } else {
                -(role as i32)
            }
        } else {
            6
        };

        let mut available = self.player.money;
        for &id in self.player.inventory.iter() {
            if id != 0 {
                if let Some((class, subtype)) = self.world.res.item_class_subtype(id) {
                    if class == 0x0B && subtype == 0x20 {
                        available += 10;
                    }
                }
            }
        }

        let mut state = TradeState::open(
            npc_entity,
            role,
            role_sign,
            self.player.inventory,
            available,
            self.player.trade,
            partner_inv,
            partner_money,
            partner_trade,
        );
        state.player_real_money = self.player.money;
        self.trade = Some(state);
        self.trade_message = None;
        self.trade_message_t = 0.0;
        self.dialog = None;
        self.character_screen = false;
        self.hovered_slot = None;
    }

    fn trade_balance(&mut self) {
        let Some(state) = self.trade.as_mut() else {
            return;
        };
        let res = &self.world.res;
        state.rebalance(|id| res.item_trade_value(id).unwrap_or(0));
    }

    /// Click a visible trade cell.
    pub(crate) fn trade_click_cell(&mut self, row: usize, col: usize) {
        let moved = self
            .trade
            .as_mut()
            .map(|t| t.move_cell(row, col))
            .unwrap_or(false);
        if moved {
            self.trade_balance();
        }
    }

    /// Page a trade row left/right.
    pub(crate) fn trade_page(&mut self, row: usize, direction: i32) {
        if let Some(t) = self.trade.as_mut() {
            if t.page_row(row, direction) {
                self.trade_balance();
            }
        }
    }

    /// Cancel the exchange (`FUN_004203F4(0)`).
    pub(crate) fn trade_cancel(&mut self) {
        let Some(state) = self.trade.take() else {
            return;
        };
        self.player.inventory = state.cancel();
        self.trade_message = None;
        self.trade_message_t = 0.0;
    }

    fn trade_reject(&mut self, error: TradeError) {
        self.trade_message = Some(error.message().to_string());
        self.trade_message_t = 2.0;
    }

    /// Confirm the exchange (`FUN_004203F4(1)`), apply the result and reopen the
    /// refreshed trade window for NPC partners.
    pub(crate) fn trade_confirm(&mut self) {
        let Some(state) = self.trade.as_ref() else {
            return;
        };
        let base_weight = self.trade_base_weight();
        let capacity = crate::inventory::capacity(self.player.endurance_base);
        let res = &self.world.res;
        let result = state.confirm(
            |id| res.item_trade_value(id).unwrap_or(0),
            |id| item_weight(res, id),
            base_weight,
            capacity,
        );
        let outcome = match result {
            Ok(out) => out,
            Err(e) => {
                self.trade_reject(e);
                return;
            }
        };
        let role = state.role;
        let npc = state.npc_entity;

        self.player.inventory = outcome.player_inventory;
        self.player.money = (self.player.money + outcome.player_money_delta).max(0);
        self.player.trade = self
            .player
            .trade
            .saturating_add(outcome.trade_gain)
            .min(100);

        if matches!(role, 2..=4) {
            let si = self.settlement_index_for_loc();
            if let Some(ti) = si.and_then(|si| self.settlement_trade_index(si)) {
                if let Some(trade) = self.settlement_trade.get_mut(ti) {
                    if let Some(stock) = trade.stock_mut(role) {
                        for id in &outcome.bought {
                            if let Some(slot) = stock.iter_mut().find(|v| **v == *id) {
                                *slot = 0;
                            }
                        }
                    }
                }
            }
            for id in &outcome.sold {
                self.world.res.set_item_class(*id, 0xFF);
            }
            if let Some(tables) = self.tables.as_mut() {
                if let Some(e) = tables.entities.get_mut(npc) {
                    e.stats.trade = e.stats.trade.saturating_add(outcome.trade_gain).min(100);
                }
            }
        } else if let Some(tables) = self.tables.as_mut() {
            if let Some(e) = tables.entities.get_mut(npc) {
                e.inventory = outcome.partner_inventory;
                e.stats.money = e.stats.money.saturating_add(outcome.partner_money_delta);
                e.stats.trade = e.stats.trade.saturating_add(outcome.trade_gain).min(100);
            }
            let loc = self.current_map;
            if let Some(pop) = self.population_cache.get_mut(&loc) {
                if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == npc) {
                    if let Some(e) = tables.entities.get(npc) {
                        a.entity = e.clone();
                    }
                }
            }
        }

        // Original `0xC9` reloads the exchange after a successful deal.
        self.start_trade(npc);
    }
}
