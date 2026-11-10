//! `FUN_0041130C`: the armed line-of-fire probe used by actor AI.
//!
//! Recovered from `konung.res` (PE32/i386, image base `0x400000`):
//!
//! ```text
//! if max_metric(src, target) < 3            -> return -1
//! range   = u16[0x459B84 + subtype(equipment[0])*0x40]   // 8 samples per unit
//! dir     = FUN_0043897C(target, src)                     // facing 0..7
//! start   = FUN_00438A78(src) + 0x45ACD0[dir]             // facing nudge
//! end     = FUN_00438A78(target); end.y -= 0x1E           // aim at chest height
//! step    = delta / (max(|dx|,|dy|) + 0.001)              // unit DDA
//! repeat 8*range {
//!     start += step
//!     (cx,cy) = FUN_00438AB4(round(start.x), round(start.y) + 0x1E)
//!     if (cx,cy) == target                          -> return 1
//!     cell = collision[cx*160 + cy]
//!     if cell & 0xC000                              -> return 0   // wall
//!     if (cell & 0xFFF) != 0 && != 0xFFF            -> return 0   // occupied
//! }
//! return 0
//! ```
//!
//! The probe is a deterministic 1-pixel-step DDA, **not** a random scatter: the
//! `FUN_0043EAAE` call in the decompiler output is the x87 `FISTP` round helper,
//! and the two `8`/`range` loops are one continuous walk.
//!
//! Callers in the original: `FUN_00410E84` (armed target acquisition, gated on
//! `Entity+0x12 == 1`, `equipment[0] != 0`, `ammo(+0xCC) != 0`) and
//! `FUN_0041ECC8` (actor logic, deciding whether a ranged order stays alive).
//!
//! Not ported: the `mode < 0` projectile branch (`FUN_0043E4EA` height test for
//! a record in the `0x80BD50` pool). The remake resolves arrows through the real
//! projectile flight (`combat::projectile`), so only the actor-aim mode is used.

use crate::coords::{direction_from_cells, entity_to_local_px, local_px_to_entity};

/// `0x45ACD0`: per-facing screen-space start nudge, `(dx, dy)` as `i8`.
pub const DIR_START_OFFSET: [(i8, i8); 8] = [
    (-70, -65),
    (-48, -99),
    (12, -85),
    (68, -90),
    (80, -58),
    (52, -26),
    (-8, -10),
    (-60, -31),
];

/// `FUN_0041130C` returns `-1` when the target is closer than this max-metric
/// cell distance.
pub const MIN_DISTANCE: i32 = 3;
/// Aim point raised above the target projection (`+0x1E`).
pub const AIM_RAISE: i32 = 0x1E;
/// DDA samples per unit of [`crate::item::ItemDb::probe_range`].
pub const SAMPLES_PER_UNIT: i32 = 8;
/// `0x450014`: divisor epsilon added to the dominant axis (`x + 0.001`).
pub const AXIS_EPSILON: f64 = 0.001;
/// Defensive cap on `8 * range` so a malformed table cannot spin forever.
pub const MAX_SAMPLES: i32 = 4096;

/// Result of the probe. `as_code()` reproduces the original `-1 / 0 / 1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOfFire {
    /// Target below [`MIN_DISTANCE`] cells (`-1`).
    TooClose,
    /// The walk reached the target cell (`1`).
    Clear,
    /// A wall/occupant blocked the walk, or the sample budget ran out (`0`).
    Blocked,
}

impl LineOfFire {
    /// The original return value.
    pub fn as_code(self) -> i32 {
        match self {
            LineOfFire::TooClose => -1,
            LineOfFire::Blocked => 0,
            LineOfFire::Clear => 1,
        }
    }

    /// Whether the probe reached the target cell.
    pub fn is_clear(self) -> bool {
        self == LineOfFire::Clear
    }
}

/// `FUN_0041130C` actor-aim mode.
///
/// `collision` is the `Konung.N` 320×160 `u16` grid (`index = x*160 + y`);
/// `range` is `u16[0x459B84 + subtype*0x40]` from the active melee weapon's
/// subtype ([`crate::item::ItemDb::probe_range`]).
pub fn probe(collision: &[u16], src: (i32, i32), target: (i32, i32), range: u16) -> LineOfFire {
    // `FUN_0043881C` (the game's cell distance), not a plain max metric.
    if crate::actors::follow_distance(src, target) < MIN_DISTANCE {
        return LineOfFire::TooClose;
    }

    let dir = direction_from_cells(target.0, target.1, src.0, src.1);
    let (sx, sy) = entity_to_local_px(src.0, src.1);
    let (ox, oy) = DIR_START_OFFSET[(dir & 7) as usize];
    let mut fx = (sx + ox as i32) as f64;
    let mut fy = (sy + oy as i32) as f64;

    let (ex, ey) = entity_to_local_px(target.0, target.1);
    let ex = ex as f64;
    let ey = (ey - AIM_RAISE) as f64;
    let dx = ex - fx;
    let dy = ey - fy;
    let adx = dx.abs() + AXIS_EPSILON;
    let ady = dy.abs() + AXIS_EPSILON;
    // Dominant axis advances by one pixel per sample.
    let (step_x, step_y) = if adx < ady {
        (dx / ady, dy / ady)
    } else {
        (dx / adx, dy / adx)
    };

    let steps = (SAMPLES_PER_UNIT * range as i32).clamp(0, MAX_SAMPLES);
    for _ in 0..steps {
        fx += step_x;
        fy += step_y;
        let px = fx.round_ties_even() as i32;
        let py = fy.round_ties_even() as i32 + AIM_RAISE;
        let (cx, cy) = local_px_to_entity(px, py);
        if (cx, cy) == target {
            return LineOfFire::Clear;
        }
        let Some(&cell) = cell_at(collision, cx, cy) else {
            return LineOfFire::Blocked;
        };
        if cell & 0xC000 != 0 {
            return LineOfFire::Blocked;
        }
        let occupant = cell & 0x0FFF;
        if occupant != 0 && occupant != 0xFFF {
            return LineOfFire::Blocked;
        }
    }
    LineOfFire::Blocked
}

/// Read one collision cell by runtime coordinates, or `None` when the point
/// falls outside the 320x160 grid.
fn cell_at(collision: &[u16], x: i32, y: i32) -> Option<&u16> {
    if x < 0 || y < 0 || x > crate::coords::RUNTIME_MAX_X || y > crate::coords::RUNTIME_MAX_Y {
        return None;
    }
    collision.get(x as usize * 160 + y as usize)
}

#[cfg(test)]
mod tests;
