//! Lighting diagnostics for the headless smoke.

use crate::*;

/// One span rectangle of the last shadow pass: `field` 0 = row, 1 = x0,
/// 2 = x1. Returns `-1` when the index is out of range.
#[no_mangle]
pub extern "C" fn debug_shadow_span(index: u32, field: u32) -> i32 {
    runtime_mut()
        .game
        .as_deref()
        .and_then(|g| g.shadow_span(index as usize, field))
        .unwrap_or(-1)
}

/// Number of spans emitted by the last shadow pass (`FUN_0043C040`). Zero when
/// the `shadows` config word is off or nothing on screen casts a shadow.
#[no_mangle]
pub extern "C" fn debug_shadow_spans() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.shadow_span_count())
        .unwrap_or(0)
}

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
