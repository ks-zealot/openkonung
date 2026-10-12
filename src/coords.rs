//! Canonical coordinate transforms for the original render geometry.
//!
//! `KONUNG_SOURCE_OF_TRUTH.md` Revision 3:
//!
//! - Runtime/Entity/Group-spawn/collision grid (§3): `x = 0..319`,
//!   `y = 0..159`.
//! - Entity → local-world-pixel projection (§4, original `konung.res:0x438A78`):
//!
//! ```text
//! local_x = y * 58 + (2 - (x & 1)) * 29;
//! local_y = x * 16 + 16;
//! ```
//!
//! This is the projection for the protagonist, persisted NPCs/mobs and
//! runtime-positioned actors. Terrain-record and prop projections are different
//! lattices and live in `loc.rs` (§7, §10); they are **not** implemented here.
//!
//! Note: the runtime grid is not the rendered terrain surface. Do not clamp
//! Entity coordinates to terrain dimensions; runtime bounds are 320×160 (§3).

/// Collision/runtime-grid width (source §3 / `loc.rs`).
pub const RUNTIME_W: i32 = 320;
/// Collision/runtime-grid height (source §3 / `loc.rs`).
pub const RUNTIME_H: i32 = 160;
/// Maximum runtime-grid `x` (inclusive).
pub const RUNTIME_MAX_X: i32 = RUNTIME_W - 1;
/// Maximum runtime-grid `y` (inclusive).
pub const RUNTIME_MAX_Y: i32 = RUNTIME_H - 1;

/// Entity projection: local-x step per entity `y`.
pub const ENTITY_COL_STEP: i32 = 58;
/// Entity projection: odd/even `x` horizontal stagger half-step.
pub const ENTITY_HALF_STEP: i32 = 29;
/// Entity projection: local-y step per entity `x`.
pub const ENTITY_ROW_STEP: i32 = 16;
/// Entity projection: constant local-y bias.
pub const ENTITY_ROW_BIAS: i32 = 16;

/// Exact Entity → local-world-pixel projection (source §4/§23.1). This is the
/// canonical actor/protagonist projection; do not replace it with
/// `29 + x*29, y*16`.
pub fn entity_to_local_px(x: i32, y: i32) -> (i32, i32) {
    (
        y * ENTITY_COL_STEP + (2 - (x & 1)) * ENTITY_HALF_STEP,
        x * ENTITY_ROW_STEP + ENTITY_ROW_BIAS,
    )
}

/// Continuous (f64) Entity projection for smooth actor motion. The parity of
/// `x` (used for the horizontal stagger) is taken from `floor(x)`; the vertical
/// and `y` terms stay continuous. Integer inputs equal [`entity_to_local_px`].
pub fn entity_to_local_px_f(x: f64, y: f64) -> (f64, f64) {
    let stagger = (2 - (x.floor() as i32 & 1)) as f64 * ENTITY_HALF_STEP as f64;
    (
        y * ENTITY_COL_STEP as f64 + stagger,
        x * ENTITY_ROW_STEP as f64 + ENTITY_ROW_BIAS as f64,
    )
}

/// Continuous inverse of [`entity_to_local_px_f`] (no clamping).
pub fn local_px_to_entity_f(px: f64, py: f64) -> (f64, f64) {
    let x = (py - ENTITY_ROW_BIAS as f64) / ENTITY_ROW_STEP as f64;
    let stagger = (2 - (x.floor() as i32 & 1)) as f64 * ENTITY_HALF_STEP as f64;
    let y = (px - stagger) / ENTITY_COL_STEP as f64;
    (x, y)
}

/// Nearest runtime/entity cell for a local pixel, clamped to the proven runtime
/// bounds (`0..319`, `0..159`). Used for mouse → entity targeting.
pub fn local_px_to_entity(px: i32, py: i32) -> (i32, i32) {
    let base_x = (py - ENTITY_ROW_BIAS).div_euclid(ENTITY_ROW_STEP);
    let mut best = (0i32, 0i32);
    let mut best_d = i64::MAX;

    for x in (base_x - 2)..=(base_x + 2) {
        let stagger = (2 - (x & 1)) * ENTITY_HALF_STEP;
        let base_y = (px - stagger).div_euclid(ENTITY_COL_STEP);
        for y in (base_y - 1)..=(base_y + 1) {
            let (fx, fy) = entity_to_local_px(x, y);
            let dx = (fx - px) as i64;
            let dy = (fy - py) as i64;
            let d = dx * dx + dy * dy;
            if d < best_d {
                best_d = d;
                best = (x, y);
            }
        }
    }

    (
        best.0.clamp(0, RUNTIME_MAX_X),
        best.1.clamp(0, RUNTIME_MAX_Y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_projection_exact_cases() {
        assert_eq!(entity_to_local_px(0, 0), (58, 16));
        assert_eq!(entity_to_local_px(1, 0), (29, 32));
        assert_eq!(entity_to_local_px(2, 0), (58, 48));
        assert_eq!(entity_to_local_px(5, 7), (435, 96));
        // Even x uses the full 58px offset, odd x uses the 29px half offset.
        assert_eq!(entity_to_local_px(6, 9), (9 * 58 + 58, 6 * 16 + 16));
    }

    #[test]
    fn continuous_matches_integer_projection() {
        for x in -3..330 {
            for y in -3..165 {
                let (ix, iy) = entity_to_local_px(x, y);
                let (fx, fy) = entity_to_local_px_f(x as f64, y as f64);
                assert_eq!((fx, fy), (ix as f64, iy as f64), "mismatch at ({x},{y})");
            }
        }
    }

    #[test]
    fn exhaustive_round_trip_all_cells() {
        for x in 0..RUNTIME_W {
            for y in 0..RUNTIME_H {
                let (px, py) = entity_to_local_px(x, y);
                let cell = local_px_to_entity(px, py);
                assert_eq!(
                    cell,
                    (x, y),
                    "round-trip failed for cell ({x},{y}) -> pixel ({px},{py})"
                );
            }
        }
    }

    #[test]
    fn continuous_round_trip() {
        for &x in &[0.0f64, 0.5, 1.0, 1.25, 12.75, 100.0, 268.0] {
            for &y in &[0.0f64, 3.5, 60.0, 159.0] {
                let (px, py) = entity_to_local_px_f(x, y);
                let (rx, ry) = local_px_to_entity_f(px, py);
                assert!((rx - x).abs() < 1e-6, "x {x} -> {rx}");
                assert!((ry - y).abs() < 1e-6, "y {y} -> {ry}");
            }
        }
    }

    #[test]
    fn inverse_clamps_to_runtime_bounds() {
        assert_eq!(local_px_to_entity(-10_000, -10_000), (0, 0));
        assert_eq!(
            local_px_to_entity(i32::MAX / 2, i32::MAX / 2),
            (RUNTIME_MAX_X, RUNTIME_MAX_Y)
        );
    }
}
