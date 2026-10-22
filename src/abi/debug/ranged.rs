//! Ranged/prop combat diagnostics.

use super::super::*;

pub static mut DEBUG_RANGED_STATE: Option<Vec<u8>> = None;

#[no_mangle]
pub extern "C" fn debug_ranged_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_test_setup(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_make_dodge() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_make_dodge(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_target_arrows() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_target_arrows(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_prop_setup(prop_index: u32) -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.prop_test_setup(prop_index as usize),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            unsafe {
                *addr_of_mut!(DEBUG_RANGED_STATE) = Some(g.ranged_debug_report().into_bytes());
            }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

