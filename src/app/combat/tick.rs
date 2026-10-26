//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use super::super::*;
use crate::constants::combat::*;

impl PlayableApp {
    fn combat_tick(&mut self) {
        self.update_projectiles();
        self.combat_frame = self.combat_frame.wrapping_add(1);
        if self.combat_frame & DOT_INTERVAL_MASK == 0 {
            // `FUN_0041D048` calls `FUN_0041BD3C` for every active object group
            // in the table, not only the current location's groups.
            if let Some(tables) = self.tables.as_mut() {
                for g in tables.groups.iter_mut() {
                    if g.class_1e & 0x40 != 0 {
                        crate::actors::update_group_growth(g);
                    }
                }
            }
            self.update_group_relations();
            self.apply_dot();
        }
        self.advance_all_death_frames();
        self.update_world_ai();
        self.update_player_combat();
        self.update_party_combat();
        self.update_world_combat();
    }

    /// Fixed-rate combat update (60 ms logic ticks, capped at 10 per frame).
    pub(in crate::app) fn update_combat(&mut self, dt: f64) {
        self.combat_step_t += dt.max(0.0) * LOGIC_TICK_HZ;
        let mut ticks = self.combat_step_t.floor() as i64;
        self.combat_step_t -= ticks as f64;
        if ticks <= 0 {
            return;
        }
        ticks = ticks.min(MAX_COMBAT_TICKS_PER_FRAME);
        if self.combat_message_t > 0.0 {
            self.combat_message_t -= ticks as f64 / LOGIC_TICK_HZ;
            if self.combat_message_t <= 0.0 {
                self.combat_message = None;
            }
        }
        for _ in 0..ticks {
            self.combat_tick();
        }
    }
}
