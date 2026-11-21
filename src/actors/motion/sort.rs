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
///
/// The destination is re-checked against the **live** collision table:
///
/// * free -> clear the old marker, move and mark the new cell;
/// * blocked by the ordered target (`motion.target_marker`) -> stay put and let
///   the combat tick resolve the contact attack;
/// * blocked by anything else -> stop, clear the direction queue and let the AI
///   re-plan.
///
/// # Returns
///
/// `true` when the actor actually entered the next cell.
pub(crate) fn begin_actor_segment(
    m: &mut ActorMotion,
    entity_index: usize,
    occupied: &mut HashSet<(i32, i32)>,
    collision: &mut [u16],
) -> bool {
    let Some(&dir) = m.path.first() else {
        return false;
    };
    // `FUN_004123BC` requires `facing == queue head`; `FUN_00438090` turns one
    // step per frame and consumes no cell until the unit is aligned. `walking`
    // stays set so the movement tick keeps servicing the actor.
    if m.facing != dir {
        crate::coords::turn_toward(&mut m.facing, dir);
        m.turning = true;
        m.walking = true;
        m.frame = 0;
        m.render_cell = m.cell;
        return false;
    }
    m.turning = false;
    let old = m.cell;
    m.render_cell = old;
    m.facing = dir;
    m.frame = 0;
    m.path.remove(0);
    let (nx, ny) = step_by_direction(old.0, old.1, dir);
    let next = (nx, ny);
    let occupant = occupancy::occupant(collision, next);
    if is_free(collision, nx, ny) && !occupied.contains(&next) {
        occupancy::vacate(collision, old);
        occupancy::occupy(collision, next, entity_index);
        occupied.remove(&old);
        occupied.insert(next);
        m.cell = next;
        m.walking = true;
        return true;
    }
    // Blocked: `+0x13 = 0xFF` (the whole direction queue is cleared).
    m.path.clear();
    m.walking = false;
    m.render_cell = m.cell;
    if m.target_marker != 0
        && occupant == m.target_marker
        && occupant != occupancy::RESERVED_OCCUPANT
    {
        // Contact attack: the target is adjacent; keep `combat_target` and let
        // the unit pass resolve the hit on its attack row.
        m.target = Some(next);
    } else {
        m.target = None;
    }
    false
}
