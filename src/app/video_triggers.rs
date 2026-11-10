//! Movie triggers that live in the gameplay loop.
//!
//! - Game over: `FUN_00436214` case 0 checks `leader[+4] & 0x80` and plays
//!   `GAMEOVER.FLM`, then menu page 7.
//! - Settlement defeat: the full `FUN_0041D048` settlement block
//!   (`app/settlement_defense.rs`) compares the `FUN_0041B658` party power sum
//!   against the settlement defenders and, at ratio `>= _DAT_004501A8` (=2.0),
//!   sets `Settlement+0x4B2 |= 0x80` and plays `DEFEAT<location>.FLM`,
//!   returning to gameplay afterwards. The live pass runs from
//!   `advance_world_frame`; [`PlayableApp::check_defeat_video`] is kept as a
//!   direct wrapper for tests/diagnostics.
//! - Victory is handled by the WorldObject source-0 pickup in `world_items.rs`.

use super::*;
use crate::video::{self, VideoAfter, VideoClip};

impl PlayableApp {
    pub(super) fn check_game_over_video(&mut self) {
        if self.game_over || !self.player.dead {
            return;
        }
        if video::request(VideoClip::GameOver, VideoAfter::Menu { game_active: false }) {
            self.game_over = true;
        }
    }

    /// Direct defeat-check wrapper. The live game runs the same check inside
    /// [`PlayableApp::tick_settlement_pass`].
    #[allow(dead_code)]
    pub(super) fn check_defeat_video(&mut self) {
        if self.world_frame % 16 != 9 {
            return;
        }
        for si in 0..self.settlements.len() {
            self.settlement_defeat_check(si);
        }
    }
}
