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

#![allow(clippy::missing_safety_doc)]

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
#[path = "abi/resource.rs"]
mod resource;

pub use self::debug::*;
pub use self::frame::*;
pub use self::frontend_api::*;
pub use self::input::*;
pub use self::lifecycle::*;
pub use self::memory::*;
pub use self::resource::*;

pub mod actors;
pub mod anim;
pub mod app;
pub mod character;
pub mod character_screen;
pub mod combat;
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
pub mod loc;
pub mod menu_res;
pub mod names;
pub mod population;
pub mod quest;
pub mod render;
pub mod res;
pub mod script;
pub mod sound;
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

#[no_mangle]
pub extern "C" fn debug_player_y() -> f32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.player_y as f32)
        .unwrap_or(0.0)
}
