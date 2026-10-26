//! Phase C1/C2 runtime population layer.
//!
//! Builds renderable actors from the Phase B [`crate::population`] Group/Entity
//! tables. It performs **no** population growth, combat or dialogue. Movement
//! advances for **citizen-group** wander-eligible actors via
//! [`update_population`]; object/enemy-group actors idle in place; sprite
//! decoding is done by the render layer.
//!
//! Source selection (`KONUNG_SOURCE_OF_TRUTH.md` §4.1/§4.2):
//! - group `location_id (+0x1B) == current_location_id`;
//! - reserved group 0 is skipped (never an ordinary runtime actor);
//! - `Group +0x1E != 0` (active class) is required;
//! - exactly `first_entity .. first_entity + entity_count (+0x1C)` is
//!   enumerated — never `capacity (+0x19)`, never one actor per group.
//!
//! C1 (persisted): entities with `x != 0 || y != 0` are rendered at their saved
//! coordinates. C2 (runtime): entities with `x == 0 && y == 0` are assigned a
//! position from the group's first spawn rectangle (A). The parsed `Entity`
//! records are never mutated.
//!
//! Coordinate domain (`KONUNG_SOURCE_OF_TRUTH.md` §1.1/§1.2/§15): persisted and
//! runtime positions are the 320×160 runtime/Entity grid (the same grid the
//! collision table indexes as `x*160 + y`) and are projected at draw time by
//! `coords::entity_to_local_px` — the same helper the protagonist uses. They are
//! never clamped to the rendered terrain surface.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;

use crate::coords::{entity_to_local_px, neighbors, step_by_direction};
use crate::population::{Entity, GameTables, Group, Rect, Settlement, Waypoint};

mod growth;
mod motion;
mod pathfind;
/// Maximum path length in cells. The original stores the direction queue in the
/// 32-byte `Entity+0x13` field (terminator at `+0x32`), so at most 31 steps are
/// kept (`FUN_0043DA1F`); longer routes are re-issued from the new position.
mod placement;

pub use self::growth::*;
pub use self::motion::*;
pub use self::pathfind::*;
pub use self::placement::*;

pub use crate::constants::timing::MAX_PATH_STEPS;

pub use crate::constants::timing::MAX_ATTEMPTS;

pub use crate::constants::timing::LOGIC_TICK_HZ;
pub use crate::constants::timing::ORIGINAL_FRAME_MS;

pub use crate::constants::timing::FALLBACK_WALK_FRAMES;

pub use crate::constants::timing::WANDER_RADIUS_X;
pub use crate::constants::timing::WANDER_RADIUS_Y;

pub use crate::constants::timing::WANDER_MAX_ATTEMPTS;

/// Runtime collision-grid bounds (`KONUNG_SOURCE_OF_TRUTH.md` §3).
pub const RUNTIME_W: i32 = 320;
pub const RUNTIME_H: i32 = 160;

pub use crate::constants::timing::FLAG_WANDER;

/// Graphics family selected by `archetype & 0x40` (`KONUNG_SOURCE_OF_TRUTH.md`
/// §5.1) — never by `kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualFamily {
    Human,
    Object,
}

/// Graphics family for an entity archetype.
pub fn visual_family(archetype: u8) -> VisualFamily {
    if archetype & 0x40 != 0 {
        VisualFamily::Object
    } else {
        VisualFamily::Human
    }
}

/// One recruited party member (original reserved **group 0** member, appended by
/// the recruit opcode `0x123` = `FUN_00431820`).
///
/// Holds the canonical parsed [`Entity`] (authored stats/graphics/ids, never
/// mutated) plus its live [`ActorMotion`]. Party slot 0 is the protagonist (held
/// separately as [`crate::character::PlayerCharacter`]); companions occupy
/// slots `1..=N`, and each companion's HUD panel cell is its slot index
/// (`FUN_0042C730`: cell = `entity_index - group.first_entity`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PartyMember {
    pub entity: Entity,
    pub motion: ActorMotion,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementError::NoCandidateAfterAttempts { attempts } => {
                write!(f, "placement_failed_after_{attempts}_attempts")
            }
        }
    }
}

impl std::error::Error for PlacementError {}

#[cfg(test)]
mod tests;
