//! Original Settings screen state (`MENU{width}R.RES` image #1).
//!
//! `FUN_00436214` snapshots the seven live globals when the screen opens, each
//! control toggles the live value immediately, **Ok** writes `KONUNG.CFG`, and
//! **Отмена** restores the snapshot. The 10 control rectangles are the exact
//! `0x46257C` table (inclusive edges) in [`crate::frontend::layout`].
//!
//! Control order is fixed by that table; every constant here is the index into
//! [`crate::frontend::layout::SETTINGS_HIT_640`] / `_800` and into the
//! `0x46257C` records. Controls `0..=7` edit the seven `KONUNG.CFG` words,
//! `8` is Ok and `9` is the Cancel button.

use super::layout::FrontendRes;
use crate::config::{self, GameConfig};

/// Control `0`: set the video mode to 640x480 (`video_mode = 0`).
pub const CONTROL_VIDEO_640: usize = 0;
/// Control `1`: set the video mode to 800x600 (`video_mode = 1`).
pub const CONTROL_VIDEO_800: usize = 1;
/// Control `2`: toggle the transparency option word.
pub const CONTROL_TRANSPARENCY: usize = 2;
/// Control `3`: toggle the ground-shadow option word.
pub const CONTROL_SHADOWS: usize = 3;
/// Control `4`: toggle the day/night ambient option word.
pub const CONTROL_DAY_NIGHT: usize = 4;
/// Control `5`: toggle the light-effects option word.
pub const CONTROL_LIGHT_EFFECTS: usize = 5;
/// Control `6`: toggle the music option word.
pub const CONTROL_MUSIC: usize = 6;
/// Control `7`: toggle the sound-effects option word.
pub const CONTROL_SOUND: usize = 7;
/// Control `8`: **Ok**, persist the live values (and rebuild on a mode change).
pub const CONTROL_OK: usize = 8;
/// Control `9`: the Cancel button, restore the values captured on entry.
pub const CONTROL_CANCEL: usize = 9;

/// Live Settings-screen values plus the entry snapshot used by the Cancel button.
///
/// `live` is written into the global configuration on every control activation,
/// so the game/runtime already sees the new value before Ok; `snapshot` is the
/// only thing that makes the change reversible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsState {
    /// Values currently shown and installed globally (the seven `KONUNG.CFG` words).
    pub live: GameConfig,
    /// Values captured when the screen was entered (`SettingsState::enter`).
    pub snapshot: GameConfig,
    /// Hovered control index (`0..=9`), used for the debug overlay only.
    pub hovered: Option<usize>,
}

impl Default for SettingsState {
    /// Start from the currently installed global configuration, with the
    /// snapshot equal to it (nothing to undo yet).
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

    /// Control index under a logical point, or `None` outside every control.
    ///
    /// The rectangles come from the exact `0x46257C` table for the active
    /// resolution; the first match in table order wins.
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

/// Flip one `KONUNG.CFG` option word: any non-zero value becomes `0`, and `0`
/// becomes `1`.
///
/// The original only ever stores `0`/`1`; this keeps that invariant instead of
/// incrementing or negating the word.
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

    /// Option toggles stay binary: `1 -> 0 -> 1`, never another value.
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

    /// Cancel restores every word captured on entry, discarding later edits.
    #[test]
    fn cancel_restores_the_entry_snapshot() {
        let mut s = SettingsState::enter();
        let before = s.snapshot;
        s.activate(CONTROL_SOUND);
        s.activate(CONTROL_VIDEO_640);
        s.cancel();
        assert_eq!(s.live, before);
    }

    /// The two video controls are a radio pair: exactly one marker is set.
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
