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

pub use crate::constants::coords::RUNTIME_H;
pub use crate::constants::coords::RUNTIME_MAX_X;
pub use crate::constants::coords::RUNTIME_MAX_Y;
pub use crate::constants::coords::RUNTIME_W;

pub use crate::constants::coords::ENTITY_COL_STEP;
pub use crate::constants::coords::ENTITY_HALF_STEP;
pub use crate::constants::coords::ENTITY_ROW_BIAS;
pub use crate::constants::coords::ENTITY_ROW_STEP;

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

// --- Original movement direction model ---------------------------------------
//
// Entity movement is **not** the plain 8-neighbour grid. Each of the 8 movement
// direction codes (0..=7) maps to a cell delta that depends on the parity of the
// current cell's `x` (the staggered isometric lattice). The two 8-entry
// `{i16 dx, i16 dy}` tables were read verbatim from `konung.res`
// (`FUN_0043D922`, bases `0x48010C` odd x / `0x480110` even x):
//
//   x even (0x480110): 0:(0,-1) 1:(-1,0)  2:(-2,0) 3:(-1,1) 4:(0,1) 5:(1,1) 6:(2,0) 7:(1,0)
//   x odd  (0x48010C): 0:(0,-1) 1:(-1,-1) 2:(-2,0) 3:(-1,0) 4:(0,1) 5:(1,0) 6:(2,0) 7:(1,-1)
//
// The pathfinder `FUN_0043DA1F` enumerates exactly these 8 neighbours and stores
// the direction code used to *enter* each visited cell; `FUN_0041435C` then steps
// the cell with `FUN_0043D922`. Facing/`Entity+0x01` uses the same code space
// (`FUN_0043897C`), so a straight walk keeps a constant facing.

/// Movement direction → cell delta for an **even** `x` cell (`0x480110`).
pub const DIR_EVEN: [(i32, i32); 8] = [
    (0, -1),
    (-1, 0),
    (-2, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (2, 0),
    (1, 0),
];

/// Movement direction → cell delta for an **odd** `x` cell (`0x48010C`).
pub const DIR_ODD: [(i32, i32); 8] = [
    (0, -1),
    (-1, -1),
    (-2, 0),
    (-1, 0),
    (0, 1),
    (1, 0),
    (2, 0),
    (1, -1),
];

/// Step one runtime/Entity cell in direction `dir` (0..=7) from `(x, y)` using
/// the parity-selected original direction table (`FUN_0043D922`). Bounds are the
/// caller's concern (the original validates `0..320 × 0..160`).
pub fn step_by_direction(x: i32, y: i32, dir: u8) -> (i32, i32) {
    let table = if x & 1 == 0 { DIR_EVEN } else { DIR_ODD };
    let (dx, dy) = table[(dir & 7) as usize];
    (x + dx, y + dy)
}

/// The 8 movement neighbours of `(x, y)` as `(dir, nx, ny)`, indexed by direction
/// code, using the parity-selected original table. Out-of-bounds candidates are
/// returned too; callers filter them against their own bounds/collision.
pub fn neighbors(x: i32, y: i32) -> [(u8, i32, i32); 8] {
    let table = if x & 1 == 0 { DIR_EVEN } else { DIR_ODD };
    let mut out = [(0u8, 0i32, 0i32); 8];
    for (d, &(dx, dy)) in table.iter().enumerate() {
        out[d] = (d as u8, x + dx, y + dy);
    }
    out
}

/// Per-step cost of a movement direction in `FUN_0043DA1F`: the `±1 x` family
/// (dirs 1/3/5/7) costs 10; the `±y` / `±2 x` family (dirs 0/2/4/6) costs 14.
pub fn direction_cost(dir: u8) -> u32 {
    match dir & 7 {
        1 | 3 | 5 | 7 => 10,
        _ => 14,
    }
}

/// Facing/direction code from current `(cx, cy)` toward `(tx, ty)`
/// (`FUN_0043897C`): 0 = −y, 4 = +y, 2 = −x, 6 = +x, and 1/3/5/7 the staggered
/// diagonals.
pub fn direction_from_cells(tx: i32, ty: i32, cx: i32, cy: i32) -> u8 {
    let adx = (cx - tx).abs();
    let ady = (cy - ty).abs();
    let q = adx / 4;
    if ady / 2 < adx {
        if cx < tx {
            if q < ady {
                if cy < ty {
                    5
                } else {
                    7
                }
            } else {
                6
            }
        } else if q < ady {
            if cy < ty {
                3
            } else {
                1
            }
        } else {
            2
        }
    } else if ty < cy {
        0
    } else {
        4
    }
}

#[cfg(test)]
mod tests;
