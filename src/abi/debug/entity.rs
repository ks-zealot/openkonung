//! Source-entity diagnostics: dump one authored `Game.N` `Entity` record.

use super::super::*;

/// Retained UTF-8 dump of the last formatted source entity.
///
/// `None` before the first successful [`debug_entity_state`]; the bytes stay
/// valid until the next call replaces them.
pub static mut DEBUG_ENTITY_STATE: Option<Vec<u8>> = None;

/// Format the active game's source `Entity` with the given id (task §7) into a
/// static buffer. Returns 1 when a game is active and the id is in range.
///
/// # Memory contract
///
/// No pointer arguments. On success (1) the report is owned by the debug buffer
/// and must be read with [`debug_entity_state_ptr`]/[`debug_entity_state_len`];
/// on failure (0) the previous contents are left in place.
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

/// Start of the last [`debug_entity_state`] dump.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by the debug buffer; never free it. Null before the
/// first successful dump; not NUL terminated, so always pair it with
/// [`debug_entity_state_len`].
#[no_mangle]
pub extern "C" fn debug_entity_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Byte length of the last [`debug_entity_state`] dump, or 0 when there is none.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_entity_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}
