//! Melee combat diagnostics.

use super::super::*;

pub static mut DEBUG_COMBAT_STATE: Option<Vec<u8>> = None;

/// Diagnostic: move a live object-family actor beside the protagonist, make it
/// a weak poison carrier, put the leader in battle stance and issue the attack
/// order. Returns `entity_id + 1` or 0.
#[no_mangle]
pub extern "C" fn debug_combat_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_test_setup(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_set_seed(seed: u32) {
    if let Some(g) = runtime_mut().game.as_mut() {
        g.combat_set_seed(seed);
    }
}

/// Diagnostic: format the current combat state into the shared debug buffer.
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
#[no_mangle]
pub extern "C" fn debug_combat_kill_player() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_kill_player(),
        None => 0,
    }
}

/// Diagnostic: force one player -> staged-target melee resolution; returns the
/// final damage (0 for miss/no target).
#[no_mangle]
pub extern "C" fn debug_combat_force() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_force(),
        None => 0,
    }
}

/// Diagnostic: teleport onto the newest corpse marker and run the original
/// pickup. Returns 1 when a marker was present.
#[no_mangle]
pub extern "C" fn debug_combat_collect() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_collect(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ai_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ai_test_setup(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_world_target(entity_id: u32) -> i32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.world_target_code(entity_id as usize),
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn debug_group_activity(group_id: u32) -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.group_activity(group_id as usize),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_set_group_activity(group_id: u32, activity: u32) -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.set_group_activity(group_id as usize, activity as u8),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_world_group(entity_id: u32) -> i32 {
    match runtime_mut().game.as_deref() {
        Some(g) => g.world_group_id(entity_id as usize),
        None => -1,
    }
}
