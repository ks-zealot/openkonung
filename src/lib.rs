//! OpenKonung browser port — raw wasm ABI.
//!
//! No `wasm-bindgen`: JS copies resource bytes into linear memory via `alloc`,
//! hands them over with `load_*`, then drives the app with `step` and reads the
//! RGBA framebuffer from `frame_ptr`.
//!
//! The runtime owns **both** the original frontend and the live game at once
//! ([`Runtime`]), so opening the menu from gameplay (Escape) never recreates or
//! discards [`PlayableApp`]. `active` selects which one is stepped/rendered and
//! receives input. `frontend_boot` starts at the original startup menu; `boot`
//! is retained as a direct game start.
//!
//! # ABI lifecycle
//!
//! The crate exports plain `extern "C"` functions; there is no generated glue.
//! A browser host (or a headless Node harness) drives one [`Runtime`] through
//! this sequence:
//!
//! 1. [`alloc`] reserves `len` bytes in linear memory so the host can copy a
//!    file into it.
//! 2. [`load_resource`] takes that `(kind, ptr, len)` triple, [`load_profile`]
//!    takes a `Game.N` save, [`load_konung`] takes a `Konung.N` map and
//!    [`load_pack`] takes the zipped resource bundle. Each call copies the
//!    bytes into the loader, so the host buffer is free immediately after.
//! 3. [`frontend_boot`] starts at the original startup menu; [`boot`] starts the
//!    game directly. Both validate that every required resource is present.
//! 4. [`step`] advances one animation/logic frame (the original budget is
//!    60 ms), then [`frame_ptr`] plus [`frame_w`]/[`frame_h`] expose the RGBA
//!    framebuffer, and `sound_count`/`sound_id` expose the clips queued by that
//!    frame.
//! 5. Progress is persisted by [`save_snapshot`]/[`load_snapshot`] and
//!    [`config_write`]; the host owns the storage, the crate never touches a
//!    filesystem.
//!
//! Pointer-taking functions document their memory contract individually: who
//! owns the buffer, what a null pointer means, and what the return value
//! encodes.
//!
//! # Module map
//!
//! | layer | modules | responsibility |
//! |---|---|---|
//! | ABI | `abi/*` modules, re-exported at the crate root | memory, lifecycle, input, frame, save, config, pack, video |
//! | data | [`res`], [`loc`], [`item`], [`intr`], [`quest`], [`font`], [`menu_res`], [`names`], [`edges`], [`world_objects`], [`resource_pack`] | decode the shipped `*.res` archives and save maps |
//! | rules | [`combat`], [`inventory`], [`trade`], [`save`], [`legacy_save`], [`script`], [`dialog`], [`character`], [`hero`], [`anim`], [`coords`], [`constants`] | pure gameplay rules and recovered constants |
//! | runtime | [`app`], [`actors`], [`population`], [`lighting`], [`video`] | the live game: population, motion, combat, UI, save/load |
//! | presentation | [`render`], [`hud`], [`character_screen`], [`trade_screen`], [`world_map`], [`frontend`], [`sound`], [`config`] | framebuffer composition, screens, audio |
//!
//! # Documentation conventions
//!
//! - `FUN_0040ABCD` names a function in the original executable by its virtual
//!   address (`0x40ABCD`); `Entity+0x08` names a field of the original entity
//!   record. Both are evidence anchors, not remake identifiers.
//! - `KONUNG_SOURCE_OF_TRUTH.md` section numbers are cited where a rule was
//!   recovered from the binaries; the document is the binding contract.
//! - Comments describe what the code actually does. Behaviour that could not be
//!   traced back to the original carries a `TODO(docs)` note instead of a
//!   guessed explanation.
//!
//! Build the documentation with:
//!
//! ```text
//! cargo doc --no-deps --document-private-items
//! ```
//!
//! `--document-private-items` matters: most of the ported logic lives in
//! `pub(super)` and private helpers that are part of the explanation but not of
//! the exported API.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
#![allow(clippy::missing_safety_doc)]

#[path = "abi/config.rs"]
mod config_api;
#[path = "abi/debug.rs"]
mod debug;
#[path = "abi/frame.rs"]
mod frame;
#[path = "abi/frontend_api.rs"]
mod frontend_api;
#[path = "abi/input.rs"]
mod input;
#[path = "abi/lifecycle.rs"]
mod lifecycle;
#[path = "abi/memory.rs"]
mod memory;
#[path = "abi/pack.rs"]
mod pack_api;
#[path = "abi/resource.rs"]
mod resource;
#[path = "abi/save_api.rs"]
mod save_api;
#[path = "abi/video.rs"]
mod video_api;

pub use self::config_api::*;
pub use self::debug::*;
pub use self::frame::*;
pub use self::frontend_api::*;
pub use self::input::*;
pub use self::lifecycle::*;
pub use self::memory::*;
pub use self::pack_api::*;
pub use self::resource::*;
pub use self::save_api::*;
pub use self::video_api::*;

pub mod actors;
pub mod anim;
pub mod app;
pub mod character;
pub mod character_screen;
pub mod combat;
pub mod config;
pub mod constants;
pub mod coords;
pub mod dialog;
pub mod edges;
pub mod font;
pub mod frontend;
pub mod hero;
pub mod hud;
pub mod intr;
pub mod inventory;
pub mod item;
pub mod legacy_save;
pub mod lighting;
pub mod loc;
pub mod menu_res;
pub mod names;
pub mod population;
pub mod quest;
pub mod render;
pub mod res;
pub mod resource_pack;
pub mod save;
pub mod script;
pub mod sound;
pub mod trade;
pub mod trade_screen;
pub mod video;
pub mod world_map;
pub mod world_objects;

use std::collections::HashMap;
use std::ptr::{addr_of, addr_of_mut, slice_from_raw_parts_mut};

use app::{PlayableApp, TextItem};
use character::{CharacterBuild, CharacterClass, PlayerCharacter};
use frontend::layout::FrontendRes;
use frontend::{Frontend, FrontendCommand, FrontendScreen};
use loc::Location;
use render::World;

/// 1 while a live game is retained (active or suspended).
#[no_mangle]
pub extern "C" fn debug_game_exists() -> u32 {
    runtime_mut().game.is_some() as u32
}

/// Number of occupied INTR interface sprites attached to the live game's
/// resources, or 0 when there is no game / no archive. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_intr_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.world.res.intr.as_ref().map(|a| a.len()).unwrap_or(0) as u32)
        .unwrap_or(0)
}

/// Semantic role code of an INTR index (see `intr::IntrRole`); tooling only.
#[no_mangle]
pub extern "C" fn debug_intr_role(id: u32) -> u32 {
    use intr::IntrRole::*;
    match intr::role_of(id as usize) {
        HudOverlay => 0,
        CharacterWindow => 1,
        InventorySlot => 2,
        InventoryArrow => 3,
        AutoButton => 4,
        Portrait => 5,
        ItemIcon => 6,
        WindowFrame => 7,
        Panel => 8,
        Button => 9,
        InterfaceControl => 10,
        SmallUi => 11,
        Unclassified => 12,
        // Appended after the original codes so existing tooling indices stay put.
        SelectionMarker => 13,
        HealthBar => 14,
        HudIcon => 15,
        MapParchment => 16,
        MapOverlay => 17,
    }
}

/// 1 while the character screen (`INTR[2]`) is open in the live game, else 0.
#[no_mangle]
pub extern "C" fn debug_character_screen() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.character_screen as u32)
        .unwrap_or(0)
}

/// Live player runtime x/y (0 when no game). Used to prove state preservation.
#[no_mangle]
pub extern "C" fn debug_player_x() -> f32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.player_x as f32)
        .unwrap_or(0.0)
}

/// Companion to [`debug_player_x`]: live player runtime y, `0.0` when no game.
#[no_mangle]
pub extern "C" fn debug_player_y() -> f32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.player_y as f32)
        .unwrap_or(0.0)
}
