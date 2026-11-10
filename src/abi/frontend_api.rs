//! `frontend_api` subsystem of [`crate`] (split from `lib.rs`).
//!
//! Read-only view of the original menu shell for the browser smoke tests: which
//! runtime is active, which frontend screen is showing, and the current hero /
//! class / hover selection. Every getter returns a neutral sentinel (`-1` or
//! `255`) instead of panicking when the frontend has not been built.

use super::*;

/// 1 while the original frontend is active, 0 while the game is active.
#[no_mangle]
pub extern "C" fn frontend_is_active() -> u32 {
    (runtime_mut().active == ActiveMode::Frontend) as u32
}

/// Frontend screen code (0 MainMenu, 1 NewHero, 2 Settings, 3 LoadGame,
/// 4 SaveGame), or 255 when no frontend exists.
#[no_mangle]
pub extern "C" fn frontend_state() -> u32 {
    match frontend_mut() {
        Some(f) => f.screen_code(),
        None => 255,
    }
}

/// Selected hero archetype (0/1/2) or -1.
#[no_mangle]
pub extern "C" fn frontend_selected_hero() -> i32 {
    frontend_mut().map(|f| f.selected_hero()).unwrap_or(-1)
}

/// Hovered index for the current frontend screen, or -1.
#[no_mangle]
pub extern "C" fn frontend_hover() -> i32 {
    frontend_mut().map(|f| f.hover_index()).unwrap_or(-1)
}

/// Selected class `Entity+0x04` value (5/3/4/2), or -1.
#[no_mangle]
pub extern "C" fn frontend_selected_class() -> i32 {
    frontend_mut()
        .map(|f| f.selected_class_entity())
        .unwrap_or(-1)
}
