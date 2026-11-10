//! `memory` subsystem of [`crate`] (split from `lib.rs`).
//!
//! The host owns every buffer it hands in: it allocates with [`alloc`], writes
//! the file bytes, passes the same `(ptr, len)` pair to a `load_*` function,
//! and frees with [`dealloc`]. The loaders copy the bytes, so the buffer may be
//! released as soon as the call returns.

use super::*;

/// Reserve `len` zeroed bytes in the wasm linear memory and leak them.
///
/// # Memory contract
///
/// - The returned pointer is valid for exactly `len` bytes until it is passed
///   back to [`dealloc`] with the **same** length.
/// - `len == 0` returns a non-null dangling pointer that must not be
///   dereferenced; freeing it with `len == 0` is a no-op.
/// - The allocation is intentionally leaked so no Rust allocator state is
///   shared with the host; nothing else in the crate can free it.
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let mut b = vec![0u8; len as usize].into_boxed_slice();
    let p = b.as_mut_ptr();
    std::mem::forget(b);
    p
}

/// Release a buffer previously returned by [`alloc`].
///
/// # Memory contract
///
/// - `ptr` must come from [`alloc`] and `len` must be the length it was
///   allocated with; any other pair is undefined behaviour.
/// - A null `ptr` is ignored, so a failed allocation can be freed safely.
/// - The buffer must not be used after this call.
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

/// Copy a host-owned buffer into a `Vec<u8>`.
///
/// # Safety
///
/// `ptr` must be valid for reads of `len` bytes for the duration of the call;
/// the bytes are copied, so the host may free its buffer afterwards.
#[inline]
pub unsafe fn take_bytes(ptr: *mut u8, len: u32) -> Vec<u8> {
    std::slice::from_raw_parts(ptr, len as usize).to_vec()
}

// --------------------------------------------------------------------------
// Memory
// --------------------------------------------------------------------------
