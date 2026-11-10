//! `motion` subsystem of [`crate::actors`] (split from `actors.rs`).
//!
//! Movement state ([`ActorMotion`]), the citizen waypoint/wander pass, party
//! follow formation, the projected draw order and the walk-segment commit. The
//! dispatch split follows the original `FUN_00421B50`: citizen groups
//! (`Group+0x1E & 0x80`, no `0x40`) wander, object/enemy groups idle in place.

mod citizen;
mod formation;
mod impls;
mod sort;
mod types;

pub use self::citizen::*;
pub use self::formation::*;
pub use self::sort::*;
pub use self::types::*;

pub use crate::constants::motion::FOLLOW_DISTANCE;
