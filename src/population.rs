//! Typed parser for the `Game.N` save-state Group/Entity tables.
//!
//! Layout (`KONUNG_SOURCE_OF_TRUTH.md` §3–§5), BINARY-LAYOUT VERIFIED:
//!
//! ```text
//! offset    count   stride   table
//! 0x20000   200     0x20     Group
//! 0x2A69A   2000    0x100    Entity
//! ```
//!
//! `Entity` records are contiguous per group: `first_entity .. first_entity +
//! entity_count` (§4.1). This module only parses and enumerates; it performs no
//! coordinate conversion, no collision queries and no rendering (Phase B).
//!
//! Coordinate-domain note (§3): `Entity.x/+0x08` and `Entity.y/+0x0A` **are**
//! the 320x160 runtime/Entity/collision grid (`index = x*160 + y`) — the same
//! domain written by Group spawn placement (`FUN_00411de4`) and marked by
//! Transition trigger rectangles (`FUN_0043adf0`). They are stored here exactly
//! as persisted and never rescaled; the grid→pixel projection is draw-time only.
//!
//! # Population growth (documented, **not implemented**)
//!
//! When the original game creates a **genuinely new** group member beyond the
//! current `entity_count` (constructor VA `0x420A8C`), it averages the existing
//! group members' `dexterity_base`, `charisma_base`, `level`, `money`,
//! `base_armor`, `Entity+0x52`, `strength_base`, `Entity+0x11` and
//! `endurance_base`, assigns those averages to the new Entity, sets
//! `current = averaged base` for the primary stats and `HP = 1600`, copies
//! `visual_index`/`palette_offset` from a randomly-chosen existing member,
//! clears equipment/inventory, sets name/nickname to 0 and dialogue to `0xFF`.
//!
//! Existing entities (including authored `(0,0)` records) are **never** grown or
//! regenerated; their parsed stats are the source of truth.

use std::fmt;
use std::ops::Range;

use crate::res::{i32v, u16, u32};

/// Exact `Game.N` size (matches `edges::GAME_SIZE`).

mod types;
mod parse;

pub use self::types::*;
pub use self::parse::*;

pub use crate::constants::game_layout::GAME_SIZE;

pub use crate::constants::game_layout::GROUP_OFF;
pub use crate::constants::game_layout::GROUP_STRIDE;
pub use crate::constants::game_layout::GROUP_COUNT;

pub use crate::constants::game_layout::ENTITY_OFF;
pub use crate::constants::game_layout::ENTITY_STRIDE;
pub use crate::constants::game_layout::ENTITY_COUNT;

pub use crate::constants::game_layout::ENTITY_END;

// --- Settlement / citizen waypoint table (`KONUNG_SOURCE_OF_TRUTH.md` §14) ---

pub use crate::constants::game_layout::SETTLEMENT_OFF;
pub use crate::constants::game_layout::SETTLEMENT_STRIDE;
pub use crate::constants::game_layout::SETTLEMENT_COUNT;
pub use crate::constants::game_layout::WAYPOINT_OFF;
pub use crate::constants::game_layout::WAYPOINT_STRIDE;
pub use crate::constants::game_layout::WAYPOINT_MAX;

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeError::StartOutOfRange { start } => write!(
                f,
                "group first_entity {start} is outside the 0..{ENTITY_COUNT} Entity table"
            ),
            RangeError::EndOutOfRange { start, count, end } => write!(
                f,
                "group entity range {start}..{end} (first {start} + count {count}) exceeds the {ENTITY_COUNT}-entry Entity table"
            ),
        }
    }
}

impl std::error::Error for RangeError {}

#[cfg(test)]
mod tests;
