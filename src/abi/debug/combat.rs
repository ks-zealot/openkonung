//! Melee combat and mob-AI diagnostics.
//!
//! Every export here stages state or formats a report for the headless smokes
//! (`tools/combat_smoke.mjs`, `tools/mob_ai_smoke.mjs`). Gameplay never calls
//! them; they are the only way to force a deterministic hit, move an actor onto
//! a chosen cell or read the autonomous target of a world actor.

use super::super::*;

/// Retained UTF-8 report of the last [`debug_combat_state`] call.
///
/// `None` until the first report; the bytes stay valid until the next
/// `debug_combat_state` call replaces them.
pub static mut DEBUG_COMBAT_STATE: Option<Vec<u8>> = None;

/// Diagnostic: move a live object-family actor beside the protagonist, make it
/// a weak poison carrier, put the leader in battle stance and issue the attack
/// order. Returns `entity_id + 1` or 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_combat_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_test_setup(),
        None => 0,
    }
}

/// Diagnostic: reseed the combat RNG so hit/dodge rolls repeat.
///
/// # Memory contract
///
/// Pure value argument; no pointers. A no-op when no game is running.
#[no_mangle]
pub extern "C" fn debug_combat_set_seed(seed: u32) {
    if let Some(g) = runtime_mut().game.as_mut() {
        g.combat_set_seed(seed);
    }
}

/// Diagnostic: format the current combat state into the shared debug buffer.
///
/// # Memory contract
///
/// Returns 1 when a game exists and the UTF-8 report was stored (read it with
/// [`debug_combat_state_ptr`]/[`debug_combat_state_len`]), 0 when no game is
/// running. The report owns its storage; the caller never frees it.
#[no_mangle]
pub extern "C" fn debug_combat_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            unsafe {
                *addr_of_mut!(DEBUG_COMBAT_STATE) = Some(g.combat_debug_report().into_bytes());
            }
            1
        }
        None => 0,
    }
}

/// Diagnostic: kill the protagonist and return 1.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_combat_kill_player() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_kill_player(),
        None => 0,
    }
}

/// Diagnostic: force one player -> staged-target melee resolution; returns the
/// final damage (0 for miss/no target).
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_combat_force() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_force(),
        None => 0,
    }
}

/// Diagnostic: teleport onto the newest corpse marker and run the original
/// pickup. Returns 1 when a marker was present.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_combat_collect() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_collect(),
        None => 0,
    }
}

/// Start of the last [`debug_combat_state`] report.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by the debug buffer; never free it. Null until a report
/// has been produced (and stays valid only until the next report). Read the
/// exact length from [`debug_combat_state_len`] - the buffer is not NUL
/// terminated.
#[no_mangle]
pub extern "C" fn debug_combat_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Byte length of the last [`debug_combat_state`] report, or 0 when there is
/// none.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_combat_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

/// Diagnostic: stage the autonomous mob-AI scenario for the smoke.
///
/// Puts the party on a free cell inside the first current-location object
/// group's spawn rectangle A, places one live member of that group on a
/// neighbouring free cell with weak stats, sets `Group+0x1D = 1` (activity:
/// combat) and records the actor as the staged target. Returns `entity_id + 1`
/// (0 on failure).
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_ai_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ai_test_setup(),
        None => 0,
    }
}

/// Diagnostic: encoded [`crate::combat::CombatTarget`] of world actor
/// `entity_id`.
///
/// Encoding: `0` leader, `1000 + slot` party member, `2000 + id` world actor,
/// `3000 + id` world prop, `-1` for "no target" or an actor that is not in the
/// current location's population.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_world_target(entity_id: u32) -> i32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.world_target_code(entity_id as usize),
        None => -1,
    }
}

/// `Group+0x1D` activity byte of group `group_id`, or 0 when out of range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_group_activity(group_id: u32) -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.group_activity(group_id as usize),
        None => 0,
    }
}

/// Diagnostic: overwrite `Group+0x1D` of group `group_id`. Returns 1 when the
/// group exists, 0 otherwise.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_set_group_activity(group_id: u32, activity: u32) -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.set_group_activity(group_id as usize, activity as u8),
        None => 0,
    }
}

/// Enumerating `Group` index of runtime actor `entity_id` on the current map, or
/// -1 when the actor is not populated.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_world_group(entity_id: u32) -> i32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.world_group_id(entity_id as usize),
        None => -1,
    }
}
