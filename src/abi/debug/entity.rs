//! Source-entity diagnostics.

use super::super::*;

pub static mut DEBUG_ENTITY_STATE: Option<Vec<u8>> = None;


/// Format the active game's source `Entity` with the given id (task §7) into a
/// static buffer. Returns 1 when a game is active and the id is in range.
#[no_mangle]
pub extern "C" fn debug_entity_state(id: u32) -> u32 {
    let block = match runtime_mut().game.as_deref() {
        Some(g) => g.entity_debug_block(id as usize),
        None => None,
    };
    match block {
        Some(s) => {
            unsafe {
                *addr_of_mut!(DEBUG_ENTITY_STATE) = Some(s.into_bytes());
            }
            1
        }
        None => 0,
    }
}


#[no_mangle]
pub extern "C" fn debug_entity_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}


#[no_mangle]
pub extern "C" fn debug_entity_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}
