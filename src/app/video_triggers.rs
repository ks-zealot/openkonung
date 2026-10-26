//! Movie triggers that live in the gameplay loop.
//!
//! - Game over: `FUN_00436214` case 0 checks `leader[+4] & 0x80` and plays
//!   `GAMEOVER.FLM`, then menu page 7.
//! - Settlement defeat: `FUN_0041D048` runs every 16 frames
//!   (`(world_frame + 7) & 0xF == 0`), compares party health against the
//!   settlement group's defenders, and on ratio >= `_DAT_004501A8` (=2.0) sets
//!   `Settlement+0x4B2 |= 0x80` and plays `DEFEAT<location>.FLM`, returning to
//!   gameplay afterwards.
//! - Victory is handled by the WorldObject source-0 pickup in `world_items.rs`.

use super::*;
use crate::video::{self, VideoAfter, VideoClip};

impl PlayableApp {
    pub(super) fn check_game_over_video(&mut self) {
        if self.game_over || !self.player.dead {
            return;
        }
        if video::request(
            VideoClip::GameOver,
            VideoAfter::Menu { game_active: false },
        ) {
            self.game_over = true;
        }
    }

    pub(super) fn check_defeat_video(&mut self) {
        // `(frame + 7) & 0xF == 0` -> frame % 16 == 9.
        if self.world_frame % 16 != 9 {
            return;
        }
        let Some(tables) = self.tables.as_ref() else {
            return;
        };
        let Some(pop) = self.population_cache.get(&self.current_map) else {
            return;
        };

        let mut party_hp = self.player.hp_internal as i64;
        for m in &self.party {
            if !m.motion.dead {
                party_hp += m.entity.stats.hp_internal as i64;
            }
        }
        if party_hp <= 0 {
            return;
        }

        let loc = self.current_map as u8;
        let mut defeat_index = None;
        for settlement in self.settlements.iter().filter(|s| s.loc_id == loc) {
            if settlement.flags_4b2 & 0x80 != 0 {
                continue;
            }
            let Some(group) = tables
                .groups
                .iter()
                .find(|g| g.index == settlement.group_id as usize && g.loc_id == loc)
            else {
                continue;
            };
            // `FUN_0041D048` tests the high byte of the dword at Group+0x1A,
            // i.e. activity byte `Group+0x1D == 1` (party inside rectangle A).
            if group.activity_1d != 1 {
                continue;
            }
            let mut defender_hp = 0i64;
            for actor in pop
                .actors
                .iter()
                .filter(|a| a.source_group_id == group.index)
            {
                if actor.entity.stats.level < 3
                    && !pop
                        .motion
                        .get(&actor.entity.index)
                        .map(|m| m.dead)
                        .unwrap_or(false)
                {
                    defender_hp += actor.entity.stats.hp_internal as i64;
                }
            }
            if defender_hp <= 0 {
                continue;
            }
            if party_hp * 2 >= defender_hp {
                defeat_index = Some(settlement.index);
                break;
            }
        }

        let Some(index) = defeat_index else {
            return;
        };
        if video::request(
            VideoClip::Defeat(loc),
            VideoAfter::Gameplay,
        ) {
            if let Some(s) = self.settlements.iter_mut().find(|s| s.index == index) {
                s.flags_4b2 |= 0x80;
            }
            for s in self.settlement_trade.iter_mut().filter(|s| s.index == index) {
                s.flags_4b2 |= 0x80;
            }
            if let Some(pop) = self.population_cache.get_mut(&self.current_map) {
                for actor in pop.actors.iter_mut() {
                    if let Some(m) = pop.motion.get_mut(&actor.entity.index) {
                        m.combat_target = None;
                    }
                    if let Some(g) = self
                        .tables
                        .as_mut()
                        .and_then(|t| t.groups.get_mut(actor.source_group_id))
                    {
                        g.activity_1d = 0;
                    }
                }
            }
        }
    }
}
