//! Party (recruited companion) diagnostics.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: party (recruited companions) — diagnostic only
// --------------------------------------------------------------------------

/// Number of recruited companions (party slots `1..`), or 0 when no game.
///
/// This excludes the protagonist, who is held separately in
/// `PlayableApp::player`; it counts the original reserved group 0 beyond slot 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_party_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.party.len() as u32)
        .unwrap_or(0)
}

/// Source `Entity` id of recruited companion `i`, or `0xFFFFFFFF`.
///
/// The id indexes the `Game.N` Entity table, which is how the smoke checks that
/// the recruit opcode `0x123` (`FUN_00431820`) appended the right NPC.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_party_member_id(i: u32) -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .and_then(|g| g.party.get(i as usize))
        .map(|m| m.entity.index as u32)
        .unwrap_or(u32::MAX)
}
