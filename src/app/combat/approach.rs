//! `FUN_004101E4`: the post-attack step-aside offset.
//!
//! `FUN_004123BC` case 5 (the melee resolution row) ends with
//!
//! ```text
//! if (self.archetype != 0x49 || FUN_004101E4(self, &dx, &dy) == 0 ||
//!     FUN_004142D4(self, cell.x + dx, cell.y + dy, 8) == 0)
//! { FUN_0041435C(self, *self); }              // advance the current action
//! ```
//!
//! so the probe is **exclusive to archetype `0x49`** (`'I'`): every other
//! archetype short-circuits on the first operand and just advances its action.
//! An `0x49` attacker that did **not** kill its target draws one `rand()%4` and
//! tries to walk to `cell + offset`; mode 8 writes the whole order byte, so it
//! drops its target and engage bit and re-acquires on a later AI turn.
//!
//! The radii are the shared `_DAT_0081D6E0` / `_DAT_0081D6A4` globals
//! (`FUN_0042781C`: `0x14`/`9` at 800×600, `0x11`/`7` at 640×480), i.e. the
//! same values as the citizen wander radius.

use crate::combat::OriginalRng;
use crate::constants::timing::{WANDER_RADIUS_X, WANDER_RADIUS_Y};
use crate::coords::{RUNTIME_H, RUNTIME_W};

/// One collision cell: `cell & 0xFFF != 0` means "occupied" (walls alone, the
/// `0xC000` bits, count as free here, exactly like `FUN_004101E4`). Out of
/// bounds is treated as occupied so the slide keeps moving toward its bound.
fn occupied(collision: &[u16], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= RUNTIME_W || y >= RUNTIME_H {
        return true;
    }
    collision
        .get((x * 160 + y) as usize)
        .map_or(true, |&c| c & 0x0FFF != 0)
}

/// `FUN_004101E4(entity, &dx, &dy)`: pick the post-attack step-aside offset.
///
/// `rand() % 4` selects a quadrant; the free-axis coordinate is then slid while
/// the target cell is occupied. Returns `None` when the slide reaches the
/// positive bound (`dx == radius_x` or `dy == radius_y`), which is the
/// original's `return 0`.
pub(crate) fn approach_offset(
    cell: (i32, i32),
    collision: &[u16],
    rng: &mut OriginalRng,
) -> Option<(i32, i32)> {
    let rx = WANDER_RADIUS_X;
    let ry = WANDER_RADIUS_Y;
    let (mut dx, mut dy, slide_y) = match rng.roll() % 4 {
        // case 0/1: fixed ±2·rx, slide dy from -ry up to +ry.
        0 => (-2 * rx, -ry, true),
        1 => (2 * rx, -ry, true),
        // case 2/3: fixed ±2·ry, slide dx from -rx up to +rx.
        2 => (-rx, -2 * ry, false),
        _ => (-rx, 2 * ry, false),
    };
    if slide_y {
        while dy < ry && occupied(collision, cell.0 + dx, cell.1 + dy) {
            dy += 1;
        }
    } else {
        while dx < rx && occupied(collision, cell.0 + dx, cell.1 + dy) {
            dx += 1;
        }
    }
    if dx == rx || dy == ry {
        None
    } else {
        Some((dx, dy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Vec<u16> {
        vec![0u16; RUNTIME_W as usize * RUNTIME_H as usize]
    }

    /// A seed whose first draw selects `quadrant`.
    fn seed_for(quadrant: u32) -> u32 {
        (1..100_000)
            .find(|&s| OriginalRng::new(s).roll() % 4 == quadrant)
            .expect("a seed exists for every quadrant")
    }

    /// Each quadrant returns the authored offset when the cell is free, and
    /// consumes exactly one draw.
    #[test]
    fn quadrant_table_matches_the_original() {
        let cases = [
            (0u32, (-40, -9)),
            (1, (40, -9)),
            (2, (-20, -18)),
            (3, (-20, 18)),
        ];
        let collision = grid();
        for (quadrant, expected) in cases {
            let seed = seed_for(quadrant);
            let mut rng = OriginalRng::new(seed);
            let mut reference = OriginalRng::new(seed);
            let _ = reference.roll();
            let got = approach_offset((100, 80), &collision, &mut rng).expect("free cell");
            assert_eq!(got, expected, "quadrant {quadrant}");
            assert_eq!(rng.roll(), reference.roll(), "one draw only");
        }
    }

    /// The free axis slides while the target cell is occupied.
    #[test]
    fn the_free_axis_slides_past_occupied_cells() {
        let seed = seed_for(0); // dx = -40, dy slides -9 .. +9
        let mut collision = grid();
        let cell = (100, 80);
        for dy in -9..5 {
            let x = cell.0 - 40;
            let y = cell.1 + dy;
            collision[(x * 160 + y) as usize] = 1;
        }
        let mut rng = OriginalRng::new(seed);
        assert_eq!(approach_offset(cell, &collision, &mut rng), Some((-40, 5)));
    }

    /// A fully occupied grid slides both cases to the bound and fails.
    #[test]
    fn a_blocked_slide_returns_none() {
        let collision = vec![1u16; RUNTIME_W as usize * RUNTIME_H as usize];
        for quadrant in 0..4 {
            let mut rng = OriginalRng::new(seed_for(quadrant));
            assert_eq!(
                approach_offset((100, 80), &collision, &mut rng),
                None,
                "quadrant {quadrant}"
            );
        }
    }
}
