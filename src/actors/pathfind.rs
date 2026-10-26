//! `pathfind` subsystem of [`crate::actors`] (split from `actors.rs`).

use super::*;

/// Runtime collision-grid passability: in bounds and low-12 collision bits clear.
pub fn is_free(collision: &[u16], x: i32, y: i32) -> bool {
    if x < 0 || x >= RUNTIME_W || y < 0 || y >= RUNTIME_H {
        return false;
    }
    let i = x as usize * 160 + y as usize;
    i < collision.len() && (collision[i] & 0x0FFF) == 0
}

/// True when this entity is a wandering civilian, mirroring the original
/// dispatch: `flags_a(+0x02) & 0x40` set, `archetype & 0x80 == 0`, kind not
/// `3`/`9` (`FUN_00415fc8` -> `FUN_004168a4`).
///
/// This is the per-entity flag gate only. Movement additionally requires the
/// actor to belong to a **citizen** group (`Group+0x1E & 0x80 != 0` and
/// `& 0x40 == 0`, `FUN_00421b50`); object/enemy groups (`& 0x40`) idle in place
/// (`FUN_00412e00`) and are excluded in [`update_population`].
///
/// Movement also honors settlement ownership: an entity that owns a
/// home waypoint (`FUN_0041182C`, `waypoint.owner == Entity index`) walks to
/// that waypoint and then stays put, so it is wander-eligible by flag but
/// anchored in practice ([`update_population`]).
pub fn wanders(e: &Entity) -> bool {
    e.flags_a & FLAG_WANDER != 0 && e.archetype & 0x80 == 0 && !matches!(e.kind, 3 | 9)
}

/// Port of `FUN_00413a84`: pick a random collision-free point strictly inside the
/// group's spawn rectangle A, near the current position (or the rectangle centre
/// when the actor is outside it). Up to [`WANDER_MAX_ATTEMPTS`] tries; `None` on
/// failure (the original never falls back to a fixed point).
pub fn wander_target(
    rect: &Rect,
    from: (i32, i32),
    rng: &mut DeterministicRng,
    collision: &[u16],
) -> Option<(i32, i32)> {
    let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
    let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
    // Base = current cell when inside rect A, else rect A centre (`FUN_00413a84`).
    let outside = from.0 < x1 || x2 < from.0 || from.1 < y1 || y2 < from.1;
    let (bx, by) = if outside {
        ((x1 + x2) / 2, (y1 + y2) / 2)
    } else {
        (from.0, from.1)
    };
    for _ in 0..WANDER_MAX_ATTEMPTS {
        let dir = rng.next_u32() % 4;
        let ox = if WANDER_RADIUS_X < 2 {
            0
        } else {
            (rng.next_u32() % (WANDER_RADIUS_X as u32 * 2)) as i32
        };
        let oy = if WANDER_RADIUS_Y < 2 {
            0
        } else {
            (rng.next_u32() % (WANDER_RADIUS_Y as u32 * 2)) as i32
        };
        let (tx, ty) = match dir {
            0 => (bx - ox, by - oy),
            1 => (bx - ox, by + oy),
            2 => (bx + ox, by - oy),
            _ => (bx + ox, by + oy),
        };
        if x1 < tx && tx < x2 && y1 < ty && ty < y2 && is_free(collision, tx, ty) {
            return Some((tx, ty));
        }
    }
    None
}

/// Original route builder (`FUN_0043DA1F`): a weighted search over the staggered
/// movement grid that returns the list of direction codes (0..=7) to walk from
/// `start` to `goal`, or `None` when the goal is blocked ("not an acceptable
/// point") or unreachable.
///
/// Neighbours are the parity-selected [`crate::coords::neighbors`] of each cell
/// (`FUN_0043D922`). `occupied` blocks actor cells; the goal itself is allowed
/// even when occupied. Edge weights match the original: dirs 1/3/5/7 cost 10,
/// dirs 0/2/4/6 cost 14. The result is capped at [`MAX_PATH_STEPS`].
pub fn find_path(
    collision: &[u16],
    occupied: &HashSet<(i32, i32)>,
    start: (i32, i32),
    goal: (i32, i32),
    bounds: (i32, i32, i32, i32),
) -> Option<Vec<u8>> {
    find_path_with_exception(collision, occupied, start, goal, bounds, None)
}

/// As [`find_path`], with the original `FUN_0043DA1F` blocked-goal exception:
/// an occupied destination is accepted when its low 12 bits equal Entity+0x06
/// (the ordered target's entity index) and the order byte's low three bits are
/// non-zero. The all-ones collision marker (`0xFFF`) is never a target.
pub fn find_path_with_exception(
    collision: &[u16],
    occupied: &HashSet<(i32, i32)>,
    start: (i32, i32),
    goal: (i32, i32),
    bounds: (i32, i32, i32, i32),
    exception_id: Option<u16>,
) -> Option<Vec<u8>> {
    let (x0, y0, x1, y1) = bounds;
    let w = (x1 - x0 + 1).max(1) as usize;
    let h = (y1 - y0 + 1).max(1) as usize;
    if w * h > 320 * 160 {
        return None;
    }
    let in_bounds = |x: i32, y: i32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    if !in_bounds(start.0, start.1) || !in_bounds(goal.0, goal.1) {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }
    // The destination must be an "acceptable" (passable) point; the original
    // aborts the command otherwise. `Entity+0x06` is the ordered target id and
    // is the sole exception.
    let goal_blocked = !is_free(collision, goal.0, goal.1);
    let allowed = exception_id.is_some_and(|id| {
        let marker = collision[goal.0 as usize * 160 + goal.1 as usize] & 0x0FFF;
        marker != 0 && marker != 0x0FFF && marker == id
    });
    if goal_blocked && !allowed {
        return None;
    }
    let idx = |x: i32, y: i32| ((y - y0) as usize) * w + ((x - x0) as usize);
    let si = idx(start.0, start.1);
    let gi = idx(goal.0, goal.1);
    let mut dist = vec![u32::MAX; w * h];
    let mut prev = vec![usize::MAX; w * h];
    let mut pdir = vec![0u8; w * h];
    dist[si] = 0;
    let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
    heap.push(Reverse((0, si)));
    while let Some(Reverse((d, u))) = heap.pop() {
        if d > dist[u] {
            continue;
        }
        if u == gi {
            break;
        }
        let cx = x0 + (u % w) as i32;
        let cy = y0 + (u / w) as i32;
        for (dir, nx, ny) in neighbors(cx, cy) {
            if !in_bounds(nx, ny) {
                continue;
            }
            let ni = idx(nx, ny);
            if !is_free(collision, nx, ny) && !(ni == gi && allowed) {
                continue;
            }
            if ni != gi && occupied.contains(&(nx, ny)) {
                continue;
            }
            let nd = d + crate::coords::direction_cost(dir);
            if nd < dist[ni] {
                dist[ni] = nd;
                prev[ni] = u;
                pdir[ni] = dir;
                heap.push(Reverse((nd, ni)));
            }
        }
    }
    if dist[gi] == u32::MAX {
        return None;
    }
    let mut dirs = Vec::new();
    let mut c = gi;
    while c != si {
        dirs.push(pdir[c]);
        c = prev[c];
        if c == usize::MAX {
            return None;
        }
    }
    dirs.reverse();
    dirs.truncate(MAX_PATH_STEPS);
    Some(dirs)
}

/// Bounds covering a rectangle plus start/goal, clamped to the runtime grid.
pub fn path_bounds(rect: &Rect, a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    let xs = [rect.x1 as i32, rect.x2 as i32, a.0, b.0];
    let ys = [rect.y1 as i32, rect.y2 as i32, a.1, b.1];
    let x0 = xs.iter().copied().min().unwrap_or(0) - 2;
    let x1 = xs.iter().copied().max().unwrap_or(0) + 2;
    let y0 = ys.iter().copied().min().unwrap_or(0) - 2;
    let y1 = ys.iter().copied().max().unwrap_or(0) + 2;
    (
        x0.max(0),
        y0.max(0),
        x1.min(RUNTIME_W - 1),
        y1.min(RUNTIME_H - 1),
    )
}
