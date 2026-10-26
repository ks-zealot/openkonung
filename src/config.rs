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

pub const CONFIG_LEN: usize = 28;
pub const VIDEO_640: u32 = 0x111;
pub const VIDEO_800: u32 = 0x114;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameConfig {
    pub video_mode: u32,
    pub transparency: u32,
    pub day_night: u32,
    pub light_effects: u32,
    pub music: u32,
    pub shadows: u32,
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

    pub fn width(self) -> i32 {
        if self.video_mode == VIDEO_640 {
            640
        } else {
            800
        }
    }
}

static mut CONFIG: GameConfig = GameConfig::DEFAULT;
static mut CONFIG_REVISION: u32 = 0;

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

pub fn revision() -> u32 {
    unsafe { *addr_of_mut!(CONFIG_REVISION) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_800_with_every_flag_on() {
        let c = GameConfig::DEFAULT;
        assert_eq!(c.video_mode, VIDEO_800);
        assert_eq!(c.width(), 800);
        assert_eq!(c.to_bytes().len(), CONFIG_LEN);
    }

    #[test]
    fn invalid_video_mode_falls_back_to_640() {
        let mut bytes = GameConfig::DEFAULT.to_bytes();
        bytes[0..4].copy_from_slice(&0x123u32.to_le_bytes());
        let c = GameConfig::from_bytes(&bytes).unwrap();
        assert_eq!(c.video_mode, VIDEO_640);
    }

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
