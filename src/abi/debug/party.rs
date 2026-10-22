//! Party diagnostics.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: party (recruited companions) — diagnostic only
// --------------------------------------------------------------------------


/// Number of recruited companions (party slots `1..`), or 0 when no game.
#[no_mangle]
pub extern "C" fn debug_party_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.party.len() as u32)
        .unwrap_or(0)
}

/// Source `Entity` id of recruited companion `i`, or `0xFFFFFFFF`.
#[no_mangle]
pub extern "C" fn debug_party_member_id(i: u32) -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .and_then(|g| g.party.get(i as usize))
        .map(|m| m.entity.index as u32)
        .unwrap_or(u32::MAX)
}
