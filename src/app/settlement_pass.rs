//! The full `FUN_0041D048` settlement branch (one pass per 16 original frames).
//!
//! Per settlement, in original order:
//!
//! 1. compute the free/worker counts **before** any side effect;
//! 2. `free == 0` → clear activity/flags/`+0x14` (`settlement_no_worker_clear`);
//! 3. defeat check (`settlement_defeat_check`);
//! 4. current-map waypoint claim (`FUN_00416670`, `settlement_claim`);
//! 5. `+0x14` event counter (`settlement_random_event`);
//! 6. target timer/state pass (`settlement_targets`), producing `local_30`;
//! 7. role-2/role-4 restock (`tick_settlement_trade`);
//! 8. barracks training only when `local_30 == 0` (`tick_settlement_training`).

use super::*;

impl PlayableApp {
    /// Advance the settlement branch by `passes` 16-frame passes.
    pub(super) fn tick_settlement_pass(&mut self, passes: u32) {
        if passes == 0 || self.tables.is_none() {
            return;
        }
        for _ in 0..passes {
            // The original computes `local_24` before the defense/claim block;
            // the target pass must use that value, not a post-claim one.
            let workers: Vec<super::settlement_targets::SettlementWorkers> = (0..self
                .settlements
                .len())
                .map(|si| self.settlement_workers(si))
                .collect();
            for si in 0..self.settlements.len() {
                if workers.get(si).map(|w| w.free).unwrap_or(0) == 0 {
                    self.settlement_no_worker_clear(si);
                }
                self.settlement_defeat_check(si);
                if self
                    .settlements
                    .get(si)
                    .map(|s| s.loc_id as u32 == self.current_map)
                    .unwrap_or(false)
                {
                    self.settlement_claim(si);
                }
                self.settlement_random_event(si);
            }
            let blocked = self.tick_settlement_targets(&workers);
            self.tick_settlement_trade(1);
            self.tick_settlement_training(1, &blocked);
        }
    }
}
