//! Positional sound helpers (`FUN_00438DC4` pan, `FUN_00438E24` volume) and the
//! three original sample-rate banks selected by `FUN_0042A13C`.
//!
//! The original plays every clip through one of three `DirectSound` secondary
//! buffers created by `FUN_00429C0C` at 22050 / 19050 / 25050 Hz. `FUN_0042A13C`
//! keeps buffer 0 for short ids (`id < 0x20`) except `2,3,6,7,8`, and otherwise
//! picks `rand() % 3`, so most world clips are pitched randomly. A request is
//! dropped before that choice when the volume is below `-4000`.

/// Pan scale: `0x450660 = 10000.0`, `0x450668 = 0.00625` -> `delta * 62.5`.
pub const PAN_SCALE: f64 = 62.5;
/// `FUN_00438E24`: full attenuation at 1600 projected pixels.
pub const MAX_DISTANCE: i32 = 1600;
/// `FUN_0042A13C`: requests with a volume below this are never played.
pub const MIN_VOLUME: i32 = -4000;

/// `FUN_00438DC4`: horizontal pan from the **runtime cell** y difference.
///
/// The original returns `0` when the source is on the player's row, otherwise
/// `round((source_y - player_y) * 10000 * 0.00625)` using the FPU's
/// round-to-nearest-even mode (`FISTP`), which differs from Rust's default
/// half-away-from-zero `round` on odd deltas.
pub fn source_pan(source_y: u16, player_y: u16) -> i32 {
    if source_y == player_y {
        return 0;
    }
    let delta = source_y as i32 - player_y as i32;
    (delta as f64 * PAN_SCALE).round_ties_even() as i32
}

/// `FUN_00438E24`: volume in hundredths of a decibel (`0` = full, `-10000` =
/// silent) from the projected sprite distance.
///
/// `dx/dy` are the `FUN_00438A78` projections of both cells; the distance is
/// rounded, then `dist * -10000 / 1600` (truncating integer division) for
/// `dist <= 1600`, else `-10000`.
pub fn source_volume(source: (i32, i32), player: (i32, i32)) -> i32 {
    let (sx, sy) = crate::coords::entity_to_local_px(source.0, source.1);
    let (px, py) = crate::coords::entity_to_local_px(player.0, player.1);
    let dx = (px - sx) as i64;
    let dy = (py - sy) as i64;
    let dist = ((dx * dx + dy * dy) as f64).sqrt().round_ties_even() as i32;
    if dist > MAX_DISTANCE {
        -10000
    } else {
        dist * -10000 / MAX_DISTANCE
    }
}

/// Whether `FUN_0042A13C` would play a request at this volume.
pub fn audible(volume: i32) -> bool {
    volume >= MIN_VOLUME
}

/// Buffer rate chosen by `FUN_0042A13C` for clip `id`; `roll` is `rand() % 3`.
pub fn buffer_rate(id: u16, roll: u32) -> u32 {
    const FIXED_BUFFER_EXCLUDED: [u16; 5] = [2, 3, 6, 7, 8];
    if id < 0x20 && !FIXED_BUFFER_EXCLUDED.contains(&id) {
        super::SAMPLE_RATES[0]
    } else {
        super::SAMPLE_RATES[(roll % 3) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_uses_sixty_two_and_a_half_per_cell() {
        assert_eq!(source_pan(10, 10), 0);
        assert_eq!(source_pan(11, 10), 62); // 62.5 -> nearest even
        assert_eq!(source_pan(12, 10), 125);
        assert_eq!(source_pan(9, 10), -62);
        assert_eq!(source_pan(10, 20), -625);
    }

    #[test]
    fn volume_falls_off_to_minus_ten_thousand() {
        // Same cell -> distance 0 -> full volume.
        assert_eq!(source_volume((5, 5), (5, 5)), 0);
        // One row tile away: the projection moves 16 px vertically.
        let v = source_volume((5, 5), (6, 5));
        assert!(v < 0 && v > -10000, "v = {v}");
        // Far away clamps to -10000.
        assert_eq!(source_volume((0, 0), (300, 150)), -10000);
    }

    #[test]
    fn volume_matches_the_integer_division_exactly() {
        // Two rows apart: the vertical projection step is 16 px, so dist 32 ->
        // 32 * -10000 / 1600 = -200 (truncating division).
        assert_eq!(source_volume((5, 5), (7, 5)), -200);
        // One column + one row: dx 29, dy 16 -> dist 33 -> -206.
        assert_eq!(source_volume((0, 0), (1, 0)), -206);
    }

    #[test]
    fn audible_threshold_is_minus_four_thousand() {
        assert!(audible(-4000));
        assert!(!audible(-4001));
        assert!(audible(0));
    }

    #[test]
    fn buffer_selection_matches_the_original_id_rules() {
        // Small ids keep buffer 0 regardless of the roll.
        for roll in 0..3 {
            assert_eq!(buffer_rate(4, roll), 22050);
            assert_eq!(buffer_rate(9, roll), 22050);
            assert_eq!(buffer_rate(11, roll), 22050);
            assert_eq!(buffer_rate(0x0C, roll), 22050);
        }
        // Excluded small ids and every large id use the random bank.
        assert_eq!(buffer_rate(2, 0), 22050);
        assert_eq!(buffer_rate(2, 1), 19050);
        assert_eq!(buffer_rate(2, 2), 25050);
        assert_eq!(buffer_rate(34, 1), 19050);
        assert_eq!(buffer_rate(0x100, 2), 25050);
    }
}
