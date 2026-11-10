//! Draw ordering and walk-segment commit.
//!
//! The original draws actors back to front by projected vertical position, then
//! commits each walk step at the START of its animation segment
//! (`FUN_0041435C`), which is why the stored cell leads the drawn sprite.

use super::super::*;

/// Projected draw key: local vertical position, then local horizontal position
/// as a stable tie-break. Uses the protagonist's persisted-coordinate
/// projection, not the collision grid.
pub fn projected_sort_key(a: &RuntimeActor) -> (i64, i64) {
    let (x, y) = a.display_raw();
    let (lx, ly) = entity_to_local_px(x.round() as i32, y.round() as i32);
    (ly as i64, lx as i64)
}

/// Back-to-front sort for painter's-algorithm drawing.
pub fn sort_actors(actors: &mut [RuntimeActor]) {
    actors.sort_by_key(projected_sort_key);
}

/// Begin the next walk segment exactly like the original `FUN_0041435C`: take
/// the draw cell (`render_cell`) from the CURRENT cell, face and step one
/// collision cell, then reset the walk frame. The stored `cell` therefore leads
/// the drawn sprite by one step.
pub(crate) fn begin_actor_segment(
    m: &mut ActorMotion,
    occupied: &mut HashSet<(i32, i32)>,
    collision: &[u16],
) {
    let Some(&dir) = m.path.first() else {
        return;
    };
    m.render_cell = m.cell;
    m.facing = dir;
    m.frame = 0;
    m.walking = true;
    m.path.remove(0);
    let (nx, ny) = step_by_direction(m.cell.0, m.cell.1, dir);
    if is_free(collision, nx, ny) && !occupied.contains(&(nx, ny)) {
        occupied.remove(&m.cell);
        m.cell = (nx, ny);
        occupied.insert((nx, ny));
    }
}
