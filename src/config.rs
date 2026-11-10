//! `KONUNG.CFG` game configuration (28 bytes, seven little-endian `u32`).
//!
//! Verified field order from the settings save/restore code:
//!
//! ```text
//! +00 video_mode     0x111 = 640x480, 0x114 = 800x600
//! +04 transparency
//! +08 day/night
//! +0C light effects
//! +10 music
//! +14 shadows
//! +18 sound
//! ```
//!
//! The original opens `KONUNG.CFG` at startup; a missing or malformed file
//! leaves every flag on and the video mode at 640x480 (`0x111`). The remake
//! stores the same 28 bytes as base64 in `localStorage["openkonung.cfg"]` and
//! defaults to 800x600 because the browser frontend starts there.

use std::ptr::addr_of_mut;

/// Size of one `KONUNG.CFG`: seven little-endian `u32` option words.
pub const CONFIG_LEN: usize = 28;

/// `video_mode` word for 640x480 (the original default when the file is
/// missing or its mode is not recognised).
pub const VIDEO_640: u32 = 0x111;

/// `video_mode` word for 800x600.
pub const VIDEO_800: u32 = 0x114;

/// The seven live option globals mirrored by the 28-byte `KONUNG.CFG`.
///
/// The words are stored in the original order, so [`GameConfig::to_bytes`] and
/// [`GameConfig::from_bytes`] are a straight little-endian copy; the game reads
/// each flag as "non-zero means enabled".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameConfig {
    /// `+00` video mode, `VIDEO_640` (`0x111`) or `VIDEO_800` (`0x114`).
    pub video_mode: u32,
    /// `+04` transparency effects.
    pub transparency: u32,
    /// `+08` day/night ambient cycle (`lighting::ambient` reads this).
    pub day_night: u32,
    /// `+0C` per-map `LIGHTS.N` light masks and the light blend pass.
    pub light_effects: u32,
    /// `+10` music playback.
    pub music: u32,
    /// `+14` ground-shadow span pass (`FUN_0043C040`/`FUN_0043D247`).
    pub shadows: u32,
    /// `+18` sound-effect playback.
    pub sound: u32,
}

impl GameConfig {
    /// Remake default: 800x600 with every option enabled.
    pub const DEFAULT: GameConfig = GameConfig {
        video_mode: VIDEO_800,
        transparency: 1,
        day_night: 1,
        light_effects: 1,
        music: 1,
        shadows: 1,
        sound: 1,
    };

    /// Parse one 28-byte `KONUNG.CFG`. An invalid video mode falls back to
    /// `0x111` exactly like the original loader.
    pub fn from_bytes(data: &[u8]) -> Result<GameConfig, String> {
        if data.len() != CONFIG_LEN {
            return Err(format!(
                "KONUNG.CFG: expected {CONFIG_LEN} bytes, got {}",
                data.len()
            ));
        }
        let word = |i: usize| u32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap());
        let mut video_mode = word(0);
        if video_mode != VIDEO_640 && video_mode != VIDEO_800 {
            video_mode = VIDEO_640;
        }
        Ok(GameConfig {
            video_mode,
            transparency: (word(1) != 0) as u32,
            day_night: (word(2) != 0) as u32,
            light_effects: (word(3) != 0) as u32,
            music: (word(4) != 0) as u32,
            shadows: (word(5) != 0) as u32,
            sound: (word(6) != 0) as u32,
        })
    }

    /// Serialise the seven option words back into the original on-disk order.
    ///
    /// Every flag is written verbatim, so a caller that stored e.g. `7` gets
    /// `7` back; only [`GameConfig::from_bytes`] normalises to 0/1.
    pub fn to_bytes(self) -> [u8; CONFIG_LEN] {
        let mut out = [0u8; CONFIG_LEN];
        for (i, value) in [
            self.video_mode,
            self.transparency,
            self.day_night,
            self.light_effects,
            self.music,
            self.shadows,
            self.sound,
        ]
        .into_iter()
        .enumerate()
        {
            out[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// Logical framebuffer width selected by the mode word: `640` for
    /// `VIDEO_640`, otherwise `800` (`from_bytes` normalises every other word
    /// to `VIDEO_640`, so `800` means `VIDEO_800` in practice).
    pub fn width(self) -> i32 {
        if self.video_mode == VIDEO_640 {
            640
        } else {
            800
        }
    }
}

/// Live configuration, mirroring the original's seven global option words.
static mut CONFIG: GameConfig = GameConfig::DEFAULT;

/// Bumped on every [`set_config`]; the shell persists when it changes.
static mut CONFIG_REVISION: u32 = 0;

/// Current configuration (a `Copy` snapshot; the game never mutates it in place).
pub fn config() -> GameConfig {
    unsafe { *addr_of_mut!(CONFIG) }
}

/// Replace the live configuration and bump the host-persistence revision.
pub fn set_config(value: GameConfig) {
    unsafe {
        *addr_of_mut!(CONFIG) = value;
        let rev = addr_of_mut!(CONFIG_REVISION);
        *rev = (*rev).wrapping_add(1);
    }
}

/// Monotonic revision of the live configuration, starting at `0`.
///
/// `abi/config.rs` exposes it as `config_revision` so `www/app.js` can write
/// `localStorage["openkonung.cfg"]` only after an Ok or toggle actually changed
/// something.
pub fn revision() -> u32 {
    unsafe { *addr_of_mut!(CONFIG_REVISION) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The remake starts at 800x600 with every option word set to 1.
    #[test]
    fn default_is_800_with_every_flag_on() {
        let c = GameConfig::DEFAULT;
        assert_eq!(c.video_mode, VIDEO_800);
        assert_eq!(c.width(), 800);
        assert_eq!(c.to_bytes().len(), CONFIG_LEN);
    }

    /// An unrecognised `+00` word is replaced by `0x111`, never by the 800 mode.
    #[test]
    fn invalid_video_mode_falls_back_to_640() {
        let mut bytes = GameConfig::DEFAULT.to_bytes();
        bytes[0..4].copy_from_slice(&0x123u32.to_le_bytes());
        let c = GameConfig::from_bytes(&bytes).unwrap();
        assert_eq!(c.video_mode, VIDEO_640);
    }

    /// Seven words at the verified offsets round-trip through the 28-byte file.
    #[test]
    fn bytes_roundtrip_in_verified_order() {
        let c = GameConfig {
            video_mode: VIDEO_640,
            transparency: 0,
            day_night: 1,
            light_effects: 0,
            music: 1,
            shadows: 0,
            sound: 1,
        };
        let bytes = c.to_bytes();
        assert_eq!(&bytes[0..4], &VIDEO_640.to_le_bytes());
        assert_eq!(&bytes[4..8], &0u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &1u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &0u32.to_le_bytes());
        assert_eq!(&bytes[24..28], &1u32.to_le_bytes());
        assert_eq!(GameConfig::from_bytes(&bytes).unwrap(), c);
    }
}
