//! `motion` subsystem of [`crate::actors`] (split from `actors.rs`).

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