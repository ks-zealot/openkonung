//! Runtime actor occupancy in the Konung.N collision grid.
//!
//! The original merges actor occupancy into the same live `u16` collision word
//! as terrain (`KONUNG_SOURCE_OF_TRUTH.md` §8.1/§28.2):
//!
//! ```c
//! collision[x*160 + y] |= entity_index + 1;   // on entering a cell
//! collision[x*160 + y] &= 0xF000;             // on leaving / death frame - 2
//! ```
//!
//! The marker is **one-based**, so the leader (entity index 0) occupies its cell
//! with marker `1`; `0` therefore means "free" and `0xFFF` stays the reserved
//! blocked marker. The route builder and step commit both test
//! `(cell & 0x0FFF) == 0`.
//!
//! This module is the pure marker layer. [`crate::render::World::collision_mut`]
//! owns the per-location copy and the app rebuilds it on map entry / save load.

use crate::population::Entity;

use super::ActorMotion;

/// Low 12 bits of a collision word: actor occupancy / passability state.
pub const OCCUPANT_MASK: u16 = 0x0FFF;
/// High 4 bits preserved when an actor leaves a cell (`& 0xF000`).
pub const OCCUPANT_KEEP: u16 = 0xF000;
/// Reserved all-ones marker; never a real entity index.
pub const RESERVED_OCCUPANT: u16 = 0x0FFF;

/// One-based collision marker for entity table index `entity_index`.
pub fn marker(entity_index: usize) -> u16 {
    ((entity_index + 1) & OCCUPANT_MASK as usize) as u16
}

/// Flat collision index for runtime cell `(x, y)`, or `None` out of bounds.
pub fn cell_index(x: i32, y: i32) -> Option<usize> {
    if x < 0 || x >= super::RUNTIME_W || y < 0 || y >= super::RUNTIME_H {
        return None;
    }
    Some(x as usize * 160 + y as usize)
}

/// Occupancy marker of a cell (`0` = free / out of bounds).
pub fn occupant(collision: &[u16], cell: (i32, i32)) -> u16 {
    cell_index(cell.0, cell.1)
        .and_then(|i| collision.get(i))
        .map(|&v| v & OCCUPANT_MASK)
        .unwrap_or(0)
}

/// Mark `cell` as occupied by `entity_index` (original `|= index + 1`).
pub fn occupy(collision: &mut [u16], cell: (i32, i32), entity_index: usize) {
    let Some(i) = cell_index(cell.0, cell.1) else {
        return;
    };
    if let Some(word) = collision.get_mut(i) {
        *word |= marker(entity_index);
    }
}

/// Clear the low 12 bits of `cell` (original `&= 0xF000`).
pub fn vacate(collision: &mut [u16], cell: (i32, i32)) {
    let Some(i) = cell_index(cell.0, cell.1) else {
        return;
    };
    if let Some(word) = collision.get_mut(i) {
        *word &= OCCUPANT_KEEP;
    }
}

/// Original placement/AI gate: visible (`archetype & 0x80 == 0`) and not in a
/// death row. `motion` supplies the live action row when available; otherwise
/// the authored `Entity+0x00` row is used.
pub fn should_mark(entity: &Entity, motion: Option<&ActorMotion>) -> bool {
    if entity.archetype & 0x80 != 0 {
        return false;
    }
    match motion {
        Some(m) => !m.dead && !matches!(m.action, 3 | 9) && !matches!(entity.kind, 3 | 9),
        None => !matches!(entity.kind, 3 | 9),
    }
}
