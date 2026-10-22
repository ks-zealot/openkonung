//! Character creation: class presets, the final build, and the persistent
//! runtime player.
//!
//! This module separates **hero identity** ([`crate::hero::HeroTemplate`]:
//! Game.N / start location / name / visual / palette) from **character class**
//! ([`CharacterClass`]: `Entity+0x04` and the stat preset). The four class
//! presets and the initial skill values are the original character-creation
//! values (`KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** for
//! the values; the UI controls that edit them are still unimplemented).
//!
//! These values are **persistent data only** in this milestone: nothing here
//! affects damage, armor, movement, trading, dialogue, healing, crafting, item
//! requirements, XP or level-up.

use crate::hero::HeroTemplate;

#[cfg(test)]
mod tests;

mod build;
mod class;
mod player;

pub use self::build::*;
pub use self::class::*;
pub use self::player::*;
