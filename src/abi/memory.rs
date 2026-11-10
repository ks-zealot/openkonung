//! `memory` subsystem of [`crate`] (split from `lib.rs`).

use super::*;

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let mut b = vec![0u8; len as usize].into_boxed_slice();
    let p = b.as_mut_ptr();
    std::mem::forget(b);
    p
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(slice_from_raw_parts_mut(ptr, len as usize)));
    }
}

// --------------------------------------------------------------------------
// Resource loading
// --------------------------------------------------------------------------

#[inline]
pub unsafe fn take_bytes(ptr: *mut u8, len: u32) -> Vec<u8> {
    std::slice::from_raw_parts(ptr, len as usize).to_vec()
}

// --------------------------------------------------------------------------
// Memory
// --------------------------------------------------------------------------
