//! Lighting diagnostics for the headless smoke.

use crate::*;

/// Set the live game's world frame (u64 split into two u32 halves).
#[no_mangle]
pub extern "C" fn debug_set_world_frame(lo: u32, hi: u32) {
    if let Some(g) = runtime_mut().game.as_deref_mut() {
        g.set_world_frame(((hi as u64) << 32) | lo as u64);
    }
}

/// World frame low/high halves (diagnostics).
#[no_mangle]
pub extern "C" fn debug_world_frame_lo() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.world_frame() as u32)
        .unwrap_or(0)
}

/// Number of attached `LIGHTS.N` maps.
#[no_mangle]
pub extern "C" fn debug_light_map_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.world.light_maps.len() as u32)
        .unwrap_or(0)
}
