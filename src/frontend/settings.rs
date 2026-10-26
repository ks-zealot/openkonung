//! Original Settings screen state (`MENU{width}R.RES` image #1).
//!
//! `FUN_00436214` snapshots the seven live globals when the screen opens, each
//! control toggles the live value immediately, **Ok** writes `KONUNG.CFG`, and
//! **Отмена** restores the snapshot. The 10 control rectangles are the exact
//! `0x46257C` table (inclusive edges) in [`crate::frontend::layout`].

use super::layout::FrontendRes;
use crate::config::{self, GameConfig};

pub const CONTROL_VIDEO_640: usize = 0;
pub const CONTROL_VIDEO_800: usize = 1;
pub const CONTROL_TRANSPARENCY: usize = 2;
pub const CONTROL_SHADOWS: usize = 3;
pub const CONTROL_DAY_NIGHT: usize = 4;
pub const CONTROL_LIGHT_EFFECTS: usize = 5;
pub const CONTROL_MUSIC: usize = 6;
pub const CONTROL_SOUND: usize = 7;
pub const CONTROL_OK: usize = 8;
pub const CONTROL_CANCEL: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsState {
    pub live: GameConfig,
    pub snapshot: GameConfig,
    pub hovered: Option<usize>,
}

impl Default for SettingsState {
    fn default() -> Self {
        let live = config::config();
        SettingsState {
            live,
            snapshot: live,
            hovered: None,
        }
    }
}

impl SettingsState {
    /// Snapshot the live globals when the screen is entered.
    pub fn enter() -> SettingsState {
        let live = config::config();
        SettingsState {
            live,
            snapshot: live,
            hovered: None,
        }
    }

    pub fn hit_test(res: FrontendRes, x: i32, y: i32) -> Option<usize> {
        res.settings_hit_test(x, y)
    }

    /// Apply one control, mirroring `FUN_00436214` case 0..7. Returns true when
    /// the configuration changed.
    pub fn activate(&mut self, control: usize) -> bool {
        match control {
            CONTROL_VIDEO_640 => self.live.video_mode = config::VIDEO_640,
            CONTROL_VIDEO_800 => self.live.video_mode = config::VIDEO_800,
            CONTROL_TRANSPARENCY => self.live.transparency = toggle(self.live.transparency),
            CONTROL_SHADOWS => self.live.shadows = toggle(self.live.shadows),
            CONTROL_DAY_NIGHT => self.live.day_night = toggle(self.live.day_night),
            CONTROL_LIGHT_EFFECTS => self.live.light_effects = toggle(self.live.light_effects),
            CONTROL_MUSIC => self.live.music = toggle(self.live.music),
            CONTROL_SOUND => self.live.sound = toggle(self.live.sound),
            _ => return false,
        }
        config::set_config(self.live);
        true
    }

    /// Ok: persist the live values (already installed globally).
    pub fn apply(&mut self) {
        config::set_config(self.live);
        self.snapshot = self.live;
    }

    /// Cancel: restore the values captured on entry.
    pub fn cancel(&mut self) {
        self.live = self.snapshot;
        config::set_config(self.live);
    }

    /// Marker visible in this control? Video is a radio pair; the six option
    /// rows show the marker while enabled. Ok/Cancel have no marker.
    pub fn marker(&self, control: usize) -> bool {
        match control {
            CONTROL_VIDEO_640 => self.live.video_mode == config::VIDEO_640,
            CONTROL_VIDEO_800 => self.live.video_mode == config::VIDEO_800,
            CONTROL_TRANSPARENCY => self.live.transparency != 0,
            CONTROL_SHADOWS => self.live.shadows != 0,
            CONTROL_DAY_NIGHT => self.live.day_night != 0,
            CONTROL_LIGHT_EFFECTS => self.live.light_effects != 0,
            CONTROL_MUSIC => self.live.music != 0,
            CONTROL_SOUND => self.live.sound != 0,
            _ => false,
        }
    }
}

fn toggle(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_are_exact_zero_one() {
        let mut s = SettingsState {
            live: GameConfig::DEFAULT,
            snapshot: GameConfig::DEFAULT,
            hovered: None,
        };
        s.activate(CONTROL_TRANSPARENCY);
        assert_eq!(s.live.transparency, 0);
        s.activate(CONTROL_TRANSPARENCY);
        assert_eq!(s.live.transparency, 1);
    }

    #[test]
    fn cancel_restores_the_entry_snapshot() {
        let mut s = SettingsState::enter();
        let before = s.snapshot;
        s.activate(CONTROL_SOUND);
        s.activate(CONTROL_VIDEO_640);
        s.cancel();
        assert_eq!(s.live, before);
    }

    #[test]
    fn video_pair_is_mutually_exclusive() {
        let mut s = SettingsState::default();
        s.activate(CONTROL_VIDEO_640);
        assert!(s.marker(CONTROL_VIDEO_640));
        assert!(!s.marker(CONTROL_VIDEO_800));
        s.activate(CONTROL_VIDEO_800);
        assert!(!s.marker(CONTROL_VIDEO_640));
        assert!(s.marker(CONTROL_VIDEO_800));
    }
}
