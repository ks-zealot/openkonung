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
/// The code reads each entry as a dword and right-shifts it by 16, so the
/// comparison at `base + 2k` uses entry `k + 1`; the leading `0` is never tested.
/// [`ambient`] uses the resulting boundaries literally; the array is kept as the
/// provenance record and checked by the tests.
pub const AMBIENT_TABLE: [u16; 13] = [
    0, 1800, 1830, 3570, 3600, 5400, 8100, 11700, 11880, 11910, 16155, 16200, 18900,
];

/// The phase window `FUN_00428FE0` marks as night (`_DAT_0082D168`), in 60 ms
/// frames: `8100 <= phase < 18900`.
///
/// The flag is set when the phase reaches the night branch (the comparison
/// against the shifted `0x45AE18` entry, i.e. 8100) and cleared at the sunrise
/// fade (18900), so the `LIGHTS.N` re-light masks run through the deep night and
/// the sunrise plateau. Verified live against `0x82D168` and the masks' effect
/// on the frame.
pub fn is_night_phase(world_frame: u64) -> bool {
    (8100..18900).contains(&((world_frame % AMBIENT_PERIOD) as i32))
}

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
///
/// Exact port of `FUN_00428FE0` (see `KONUNG_SOURCE_OF_TRUTH.md` §35.5). The
/// function reads each threshold as a dword at `0x45AE0E + 2k` and shifts it
/// right by 16, so the comparison uses table entry `k+1` (the leading `0` entry
/// is never tested). The effective boundaries are `1800, 1830, 3570, 3600,
/// 5400, 8100, 11700, 11880, 11910, 16155, 16200, 18900`; every branch body was
/// re-derived and verified live against `0x5076BC`. The returned word packs
/// signed blue/green/red deltas; the phases where the function sets
/// `_DAT_0082D168` are [`is_night_phase`].
///
/// `_DAT_0081D608` selects the animated sunrise green in `[11880, 16200)`; the
/// original randomises it once per cycle (90% set) and does not save it, so the
/// remake keeps the common "set" case.
pub fn ambient(world_frame: u64, day_night: u32) -> Ambient {
    if day_night == 0 {
        return Ambient::default();
    }
    let p = (world_frame % AMBIENT_PERIOD) as i32;
    let rgb = |r: i32, g: i32, b: i32| -> Ambient { Ambient::from_packed(pack(r, g, b)) };
    let round = |v: f64| -> i32 { v.round() as i32 };

    // [0, 1800): neutral.
    if p < 1800 {
        return Ambient::default();
    }
    // [1800, 1830): dawn warm-up to (12, 6, 0).
    if p < 1830 {
        let q = p - 1800;
        if q > 24 {
            return rgb(0x0C, 0x06, 0);
        }
        let q2 = q / 2;
        return rgb(q2, q2 / 2, 0);
    }
    // [1830, 3570): full dawn tint.
    if p < 3570 {
        return rgb(0x0C, 0x06, 0);
    }
    // [3570, 3600): fade out.
    if p < 3600 {
        let q = p - 3570;
        if q > 24 {
            return Ambient::default();
        }
        let q2 = (0x18 - q) / 2;
        return rgb(q2, q2 / 2, 0);
    }
    // [3600, 5400): neutral.
    if p < 5400 {
        return Ambient::default();
    }
    // [5400, 8100): first dusk ramp.
    if p < 8100 {
        let q = p - 5400;
        if q > 100 {
            return rgb(0x0A, -0x14, -0x32);
        }
        let q2 = q / 2;
        return rgb(round(0.2 * q2 as f64), -round(0.4 * q2 as f64), -q2);
    }
    // [8100, 11700): night ramp into the (0, -50, -70) plateau.
    if p < 11700 {
        let q = p - 8100;
        if q > 60 {
            return rgb(0, -0x32, -0x46);
        }
        let q2 = q / 2;
        return rgb(
            round(10.0 - q2 as f64 / 3.0),
            -20 - q2,
            round(-50.0 - q2 as f64 * 2.0 / 3.0),
        );
    }
    // [11700, 11880): deep-night ramp into the sunrise plateau.
    if p < 11880 {
        let q = p - 11700;
        if q > 120 {
            return rgb(-0x3C, -0x46, -0x46);
        }
        let q2 = q / 2;
        return rgb(-q2, -50 - q2 / 3, -0x46);
    }
    // [11880, 16200): sunrise plateau (`_DAT_0081D608` set).
    if p < 16200 {
        return rgb(-0x3C, -0x32, -0x46);
    }
    // [16200, 18900): dawn ramp into the (-30, -60, -20) plateau.
    if p < 18900 {
        let q = p - 16200;
        if q > 100 {
            return rgb(-0x1E, -0x3C, -0x14);
        }
        let q2 = q / 2;
        return rgb(
            round(-60.0 + 0.5988023952095808 * q2 as f64),
            q2 / 5 - 0x46,
            q2 - 0x46,
        );
    }
    // [18900, 19020): fade to neutral; afterwards neutral until the wrap.
    let q = p - 18900;
    if q > 120 {
        return Ambient::default();
    }
    let q2 = q / 2;
    rgb(q2 / 2 - 0x1E, q2 - 0x3C, q2 / 3 - 0x14)
}

/// Pack a channel triple into the original word order (byte 0 blue, byte 1
/// green, byte 2 red), masking each channel to 8 bits so the round trip through
/// [`Ambient::from_packed`] sign-extends the same value.
fn pack(r: i32, g: i32, b: i32) -> u32 {
    (((r & 0xff) << 16) | ((g & 0xff) << 8) | (b & 0xff)) as u32
}

/// The original's ramp factor for |delta| = `index`.
///
/// `FUN_004392B4` builds the ramps with `f += 0.01f` in **single precision**
/// (100 rows), then the x87 palette builder computes `v * (1 - f)` in extended
/// precision and truncates. The accumulated f32 factor matters: e.g. at
/// `index = 20` it is slightly above `0.2`, so `10 * (1 - f)` truncates to 7,
/// not 8.
fn fade_factor(index: usize) -> f64 {
    static FACTORS: std::sync::OnceLock<[f32; 100]> = std::sync::OnceLock::new();
    let table = FACTORS.get_or_init(|| {
        let mut out = [0.0f32; 100];
        let mut f = 0.0f32;
        for slot in &mut out {
            *slot = f;
            f += 0.01f32;
        }
        out
    });
    table[index.min(99)] as f64
}

/// Apply the `FUN_00428FE0` ambient delta the way the original does: through
/// the prebuilt **palette ramps** (`FUN_004392B4`), not by adding to the 8-bit
/// RGB value.
///
/// For a negative delta `d` the original computes
/// `trunc(v * (1 - f))` (a fade toward 0, `f = |d| * 0.01` accumulated in f32);
/// for a positive delta it computes `v + trunc((max - v) * f)` (a fade toward
/// the maximum). `v`/`max` are the 5-bit red/blue and the **6-bit** green
/// fields of the 565 palette (the game runs 16bpp), and `FUN_0043EAAE`
/// truncates toward zero. The ramp tables cover `|d| <= 99`.
///
/// The RGBA pixels the remake shades are palette expansions, so the source
/// field is recovered exactly (`>> 3` for 5-bit, `>> 2` for 6-bit) and the
/// result is expanded back with the original bit replication.
#[inline]
pub fn shade_rgb(rgb: [u8; 3], ambient: Ambient) -> [u8; 3] {
    if ambient.is_zero() {
        return rgb;
    }
    let fade = |v: i32, delta: i16, max: i32| -> i32 {
        let d = delta as i32;
        let f = fade_factor(d.unsigned_abs() as usize);
        let out = if d < 0 {
            (v as f64 * (1.0 - f)).trunc() as i32
        } else if d > 0 {
            v + (((max - v) as f64) * f).trunc() as i32
        } else {
            v
        };
        out.clamp(0, max)
    };
    let r5 = fade((rgb[0] >> 3) as i32, ambient.r, 31) as u8;
    let g6 = fade((rgb[1] >> 2) as i32, ambient.g, 63) as u8;
    let b5 = fade((rgb[2] >> 3) as i32, ambient.b, 31) as u8;
    [
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
    ]
}

/// Torch highlight: blend one pixel toward the warm mask colour by `alpha`
/// (`0..=31`), used by the 64x43 `LIGHTS.RES` torch mask (`_DAT_0081D628`).
///
/// The original draws the masked party sprites through the **base** palette
/// (`0x5076F4` unshaded) instead of the ambient one and lays the mask over
/// them; the remake has RGBA pixels, so this warms the finished pixel. It is
/// only reached with the torch out, and is a visual approximation of that
/// palette swap (no recovered per-pixel ramp exists for it).
#[inline]
pub fn torch_pixel(dst: [u8; 4], alpha: u8) -> [u8; 4] {
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

/// The per-frame fire flicker words `0x82D16C` (red) / `0x82D170` (green).
///
/// `FUN_0042863C` draws them from `rand()` every frame:
/// `0x82D16C = rand()%4 + 8` (8..=11) and `0x82D170 = rand()%8 + 0x10`
/// (16..=23). The mask pass halves both (`idiv` by 2, truncating toward zero)
/// unless the `0x81D580` selector takes the `== 1` table branch, which uses the
/// full values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flicker {
    /// `0x82D16C`: red ramp delta (`rand()%4 + 8`, 8..=11).
    pub r: u8,
    /// `0x82D170`: green ramp delta (`rand()%8 + 0x10`, 16..=23).
    pub g: u8,
}

impl Flicker {
    /// No lift (used when the mask pass is skipped entirely).
    pub const NONE: Flicker = Flicker { r: 0, g: 0 };

    /// `0x82D16C` / `0x82D170` from two `rand()` draws.
    pub fn roll(rng: &mut crate::combat::OriginalRng) -> Flicker {
        let r = (rng.roll() % 4) as u8 + 8;
        let g = (rng.roll() % 8) as u8 + 0x10;
        Flicker { r, g }
    }

    /// The `idiv 2` halving the mask pass applies on the default path.
    pub fn halved(self) -> Flicker {
        Flicker {
            r: self.r / 2,
            g: self.g / 2,
        }
    }
}

/// One `LIGHTS.N` re-lit terrain pixel (`FUN_0043CC49`).
///
/// The original does **not** blend toward a warm colour; it rebuilds the
/// channel through two prebuilt palettes:
///
/// 1. `FUN_0043E7FA(0x82D16C, 0x82D170)` fills `0x479C0C`/`0x4801D0` with
///    `level + trunc((max - level) * f)` for the flicker delta (a brightening),
///    where `max` is 63 for red and 31 for green (`0x439C6C`/`0x439C1B`);
/// 2. `0x479C8C[L]` / `0x480250[L]` are `0x479D0C + |delta + L| * 0x80` /
///    `0x4A02D0 + |delta + L| * 0x40`, i.e. the ambient ramp whose index is
///    `|delta| - L` (the light tile byte `L` reduces the darkness; `L` past
///    `|delta|` clamps to the identity table). Blue keeps the plain ambient
///    ramp `0x5076B8` with no lift and no `L`.
///
/// So a lit pixel is *less* dark than its surroundings by `L` ramp steps, minus
/// the flicker brightening applied first.
#[inline]
pub fn shade_lit(rgb: [u8; 3], ambient: Ambient, light: u8, flicker: Flicker) -> [u8; 3] {
    if ambient.is_zero() {
        return rgb;
    }
    let light = light.min(31) as i32;
    let compose = |v: i32, delta: i16, max: i32, lift: u8, dim: i32| -> i32 {
        let d = delta as i32;
        if d >= 0 {
            return v.clamp(0, max);
        }
        let mut x = v;
        if lift != 0 {
            let f = fade_factor(lift as usize);
            x += (((max - x) as f64) * f).trunc() as i32;
        }
        let index = (-d - dim).max(0) as usize;
        let out = (x as f64 * (1.0 - fade_factor(index))).trunc() as i32;
        out.clamp(0, max)
    };
    let r5 = compose((rgb[0] >> 3) as i32, ambient.r, 31, flicker.r, light) as u8;
    let g6 = compose((rgb[1] >> 2) as i32, ambient.g, 63, flicker.g, light) as u8;
    let b5 = compose((rgb[2] >> 3) as i32, ambient.b, 31, 0, 0) as u8;
    [
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
    ]
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

    /// The ambient is a **palette fade**, not an additive delta: negative deltas
    /// scale the 5-bit red/blue and 6-bit green fields toward 0 (truncating),
    /// positive deltas fade toward the field maximum (`FUN_004392B4` ramps +
    /// `0x43E3E4` palette builder). Cases measured from the live original.
    #[test]
    fn shade_rgb_matches_the_original_palette_fade() {
        let night = Ambient {
            r: -60,
            g: -50,
            b: -70,
        };
        assert_eq!(shade_rgb([49, 40, 16], night), [16, 20, 0]);
        assert_eq!(shade_rgb([41, 52, 8], night), [16, 24, 0]);
        assert_eq!(shade_rgb([148, 130, 82], night), [57, 65, 24]);
        let dusk = Ambient {
            r: 10,
            g: -20,
            b: -50,
        };
        assert_eq!(shade_rgb([222, 186, 123], dusk), [222, 146, 57]);
        assert_eq!(shade_rgb([20, 32, 8], Ambient::default()), [20, 32, 8]);
    }

    /// The frame counter is taken modulo the period, so frame `0x5460` shades
    /// exactly like frame 0.
    #[test]
    fn phase_wraps_and_stays_shaded_at_night() {
        assert_eq!(ambient(AMBIENT_PERIOD, 1), ambient(0, 1));
        assert!(!ambient(12000, 1).is_zero());
    }

    /// `_DAT_0082D168` covers `[8100, 18900)`, verified live: the masks are on
    /// at 8100 and off again at 18900.
    #[test]
    fn night_window_matches_the_original_flag() {
        assert!(!is_night_phase(0));
        assert!(!is_night_phase(6000));
        assert!(!is_night_phase(8099));
        assert!(is_night_phase(8100));
        assert!(is_night_phase(12000));
        assert!(is_night_phase(18899));
        assert!(!is_night_phase(18900));
        assert!(is_night_phase(18000));
        assert!(is_night_phase(AMBIENT_PERIOD + 12000));
    }

    /// Spot values measured from the live `0x5076BC` word at held phases.
    #[test]
    fn ambient_branches_match_the_original_words() {
        assert!(ambient(100, 1).is_zero());
        // [1800,1830): r = g = (p-1800)/2 style warm-up.
        assert_eq!(ambient(1810, 1), Ambient { r: 5, g: 2, b: 0 });
        // [1830,3570): full dawn tint.
        assert_eq!(ambient(3000, 1), Ambient { r: 0x0C, g: 0x06, b: 0 });
        // [3570,3600): fade out.
        assert_eq!(ambient(3580, 1), Ambient { r: 7, g: 3, b: 0 });
        // Neutral windows.
        assert!(ambient(4000, 1).is_zero());
        assert!(ambient(5000, 1).is_zero());
        // Dusk constant and deep-night plateau (live).
        assert_eq!(ambient(6000, 1), Ambient { r: 0x0A, g: -0x14, b: -0x32 });
        assert_eq!(ambient(9000, 1), Ambient { r: 0, g: -0x32, b: -0x46 });
        // Sunrise plateau (live) and the dawn plateau before the fade.
        assert_eq!(ambient(12000, 1), Ambient { r: -0x3C, g: -0x32, b: -0x46 });
        assert_eq!(ambient(17000, 1), Ambient { r: -0x1E, g: -0x3C, b: -0x14 });
        assert_eq!(ambient(18000, 1), Ambient { r: -0x1E, g: -0x3C, b: -0x14 });
        // Post-sunrise fade and neutral.
        assert_eq!(ambient(19000, 1), Ambient { r: -5, g: -0x0A, b: -4 });
        assert!(ambient(20000, 1).is_zero());
    }

    /// A lit pixel is the ambient fade with the ramp index reduced by the light
    /// tile byte, so it is never darker than the same pixel unshaded; `L` past
    /// `|delta|` clamps to the identity table (never a brightening).
    #[test]
    fn shade_lit_reduces_the_darkness_by_the_light_byte() {
        let night = Ambient {
            r: -60,
            g: -50,
            b: -70,
        };
        let px = [48, 48, 16];
        let unlit = shade_rgb(px, night);
        let lit = shade_lit(px, night, 31, Flicker::NONE);
        assert!(lit[0] >= unlit[0] && lit[1] >= unlit[1]);
        // Blue has no light term: it keeps the plain ambient ramp.
        assert_eq!(lit[2], unlit[2]);
        // `L = 0` is exactly the plain ambient pixel.
        assert_eq!(shade_lit(px, night, 0, Flicker::NONE), unlit);
        // Measured at phase 12000 (`Ambient { -0x3C, -0x32, -0x46 }`): the same
        // terrain pixel shows `(16,24,0)` with light effects off and `(33,36,0)`
        // with them on, because `L = 31` drops the ramp index from 60/50 to
        // 29/19.
        assert_eq!(unlit, [16, 24, 0]);
        assert_eq!(lit, [33, 36, 0]);
    }

    /// The flicker only lifts a lit pixel, never darkens it.
    #[test]
    fn flicker_adds_a_brightening_step() {
        let night = Ambient {
            r: -60,
            g: -50,
            b: -70,
        };
        let base = shade_lit([16, 24, 0], night, 31, Flicker::NONE);
        let lifted = shade_lit([16, 24, 0], night, 31, Flicker { r: 11, g: 23 });
        assert!(lifted[0] >= base[0] && lifted[1] >= base[1]);
        assert_eq!(lifted[2], base[2]);
    }
}
