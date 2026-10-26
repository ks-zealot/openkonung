//! `debug` subsystem of [`crate::lib`] (split from `lib.rs`).
//!
//! Diagnostic-only wasm exports shared by the tools under `tools/`. The runtime
//! never calls these; they expose the same decoders/state that gameplay uses.

#[path = "debug/combat.rs"]
mod combat;
#[path = "debug/entity.rs"]
mod entity;
#[path = "debug/hud.rs"]
mod hud;
#[path = "debug/image.rs"]
mod image;
#[path = "debug/lighting.rs"]
mod lighting;
#[path = "debug/party.rs"]
mod party;
#[path = "debug/player.rs"]
mod player;
#[path = "debug/quest.rs"]
mod quest;
#[path = "debug/ranged.rs"]
mod ranged;
#[path = "debug/sound.rs"]
mod sound;
#[path = "debug/text.rs"]
mod text;
#[path = "debug/trade.rs"]
mod trade;

pub use self::combat::*;
pub use self::entity::*;
pub use self::hud::*;
pub use self::image::*;
pub use self::lighting::*;
pub use self::party::*;
pub use self::player::*;
pub use self::quest::*;
pub use self::ranged::*;
pub use self::sound::*;
pub use self::text::*;
pub use self::trade::*;
