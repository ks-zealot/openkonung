//! Day/night ambient palette deltas, ported from `FUN_00428FE0`.
//!
//! The original returns one 32-bit palette delta (one signed byte per BGR
//! channel, consumed by `FUN_0043D971`). The world frame counter wraps at
//! `0x5460` (21 600 frames ≈ 21.6 minutes at 60 ms/frame). The boundaries and
//! phase colors below are the recovered `0x45AE0E` threshold table.
//!
//! `FUN_0042532C` only shades when the result is non-zero **and** the
//! light-effects option is enabled; the per-map `LIGHTS.N` low map then paints
//! the light masks back over terrain, while the high map marks lit entity
//! cells that use the unshaded palette.

/// Length of the ambient cycle in 60 ms frames (`0x5460` = 21 600 frames,
/// about 21.6 minutes of play); the world frame counter wraps modulo this.
pub const AMBIENT_PERIOD: u64 = 0x5460;

/// Recovered phase boundaries from the `0x45AE0E` table, in 60 ms frames.
///
/// Entry 0 is the cycle start (`0`) and is never tested directly; entries
/// `1..=12` are the boundaries [`ambient`] compares the wrapped frame against.
pub const AMBIENT_TABLE: [u16; 13] = [
    0, 1800, 1830, 3570, 3600, 5400, 8100, 11700, 11880, 11910, 16155, 16200, 18900,
];

/// Signed per-channel ambient palette delta (blue, green, red order as the
/// original packs it: byte 0 = blue, byte 1 = green, byte 2 = red).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Ambient {
    /// Red channel delta, `-128..=127` (byte 2 of the packed word).
    pub r: i16,
    /// Green channel delta, `-128..=127` (byte 1 of the packed word).
    pub g: i16,
    /// Blue channel delta, `-128..=127` (byte 0 of the packed word).
    pub b: i16,
}

impl Ambient {
    /// Whether this delta leaves every palette entry unchanged.
    ///
    /// `FUN_0042532C` skips the whole shading pass on a zero delta, so the
    /// caller can avoid a full-frame sweep per tick.
    pub fn is_zero(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }

    /// Unpack the original 32-bit word: lowest byte is blue, then green, then
    /// red; each byte is a two's-complement signed delta.
    fn from_packed(v: u32) -> Ambient {
        Ambient {
            b: v as u8 as i8 as i16,
            g: (v >> 8) as u8 as i8 as i16,
            r: (v >> 16) as u8 as i8 as i16,
        }
    }
}

/// Ambient palette delta for a world frame. `day_night == 0` disables the
/// whole cycle (the original returns zero immediately).
pub fn ambient(world_frame: u64, day_night: u32) -> Ambient {
    if day_night == 0 {
        return Ambient::default();
    }
    let p = (world_frame % AMBIENT_PERIOD) as i32;
    let [_, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12] = AMBIENT_TABLE;
    let t = |a: u16, b: u16, v: i32| -> i32 {
        let a = a as i32;
        let b = b as i32;
        if v <= a {
            0
        } else if v >= b {
            b - a
        } else {
            v - a
        }
    };
    let rgb = |r: i32, g: i32, b: i32| -> Ambient { Ambient::from_packed(pack(r, g, b)) };

    if p < t1 as i32 {
        return Ambient::default();
    }
    if p < t2 as i32 {
        let d = t(t1, t2, p) / 2;
        return rgb(0, d / 2, d);
    }
    if p < t3 as i32 {
        return rgb(0x0C, 0x06, 0);
    }
    if p < t4 as i32 {
        let d = (24 - t(t3, t4, p)) / 2;
        return rgb(0, d / 2, d);
    }
    if p < t5 as i32 {
        return Ambient::default();
    }
    if p < t6 as i32 {
        // Dusk: transition through the recovered warm ramp.
        let d = t(t5, t6, p) / 2;
        return rgb(d / 4, 0xEC - d / 3, 0xCE - d / 4);
    }
    if p < t7 as i32 {
        return rgb(0, 0xCE, 0xBA);
    }
    if p < t8 as i32 {
        let d = t(t7, t8, p) / 2;
        return rgb(0, 0xBA, 0xBA - d / 8);
    }
    if p < t9 as i32 {
        return rgb(0, 0xBA, 0xBA);
    }
    if p < t10 as i32 {
        return rgb(0xC4, 0xBA, 0xBA);
    }
    if p < t11 as i32 {
        return rgb(0xC4, 0xBA, 0xBA);
    }
    if p < t12 as i32 {
        // Sunrise.
        let d = t(t11, t12, p) / 2;
        return rgb(0xC4, 0xBA + d / 2, 0xBA - d / 3);
    }
    let d = (p - t12 as i32) / 2;
    if d < 0x79 {
        return rgb(
            (d / 2 - 0x1E).max(0),
            (d - 0x3C).max(0),
            (d / 3 - 0x14).max(0),
        );
    }
    Ambient::default()
}

/// Pack a channel triple into the original word order (byte 0 blue, byte 1
/// green, byte 2 red), masking each channel to 8 bits so the round trip through
/// [`Ambient::from_packed`] sign-extends the same value.
fn pack(r: i32, g: i32, b: i32) -> u32 {
    (((r & 0xff) << 16) | ((g & 0xff) << 8) | (b & 0xff)) as u32
}

/// Apply one ambient delta to an RGBA byte triple.
#[inline]
pub fn shade_channel(value: u8, delta: i16) -> u8 {
    (value as i16 + delta).clamp(0, 255) as u8
}

/// Blend one pixel toward a warm light color by `alpha` in `0..=31`.
#[inline]
pub fn light_pixel(dst: [u8; 4], alpha: u8) -> [u8; 4] {
    if alpha == 0 || dst[3] == 0 {
        return dst;
    }
    let k = (alpha.min(31) as u32 * 80) / 31;
    let mix = |c: u8, target: u8| -> u8 {
        let c = c as u32;
        let t = target as u32;
        (c + (t.saturating_sub(c) * k) / 100).min(255) as u8
    };
    [mix(dst[0], 255), mix(dst[1], 242), mix(dst[2], 206), dst[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recovered period and boundary entries are `0x5460`, `1800` and
    /// `18900`.
    #[test]
    fn thresholds_are_the_original_table() {
        assert_eq!(AMBIENT_PERIOD, 21600);
        assert_eq!(AMBIENT_TABLE[1], 1800);
        assert_eq!(AMBIENT_TABLE[12], 18900);
    }

    /// `day_night == 0` returns the zero delta at every phase, so the shading
    /// pass is skipped entirely.
    #[test]
    fn day_night_off_is_always_zero() {
        for f in [0, 9000, 12000, 20000] {
            assert!(ambient(f, 0).is_zero());
        }
    }

    /// The frame counter is taken modulo the period, so frame `0x5460` shades
    /// exactly like frame 0.
    #[test]
    fn phase_wraps_and_stays_shaded_at_night() {
        assert_eq!(ambient(AMBIENT_PERIOD, 1), ambient(0, 1));
        assert!(!ambient(12000, 1).is_zero());
    }

    /// The light blend only ever brightens a pixel and clamps at 255.
    #[test]
    fn light_alpha_never_darkens_or_overflows() {
        let px = [10, 20, 30, 255];
        let lit = light_pixel(px, 31);
        assert!(lit[0] >= px[0] && lit[1] >= px[1] && lit[2] >= px[2]);
        assert_eq!(light_pixel(px, 0), px);
    }
}
