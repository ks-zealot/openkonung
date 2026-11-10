//! `config` subsystem of [`crate`]: `KONUNG.CFG` browser persistence.
//!
//! `www/app.js` keeps the exact original 28 bytes as base64 in
//! `localStorage["openkonung.cfg"]`; these exports copy them in/out of the
//! wasm-side [`crate::config`] globals without inventing a filesystem.
//!
//! The 28-byte layout is seven little-endian `u32` words: video mode (640/800),
//! transparency, day/night, light effects, music, shadows and sound. Only a
//! full-size payload is accepted; an invalid video-mode word falls back to
//! `0x111` (640x480) exactly like the original loader, and a rejected payload
//! leaves the previous configuration in force.

use super::*;

/// Load one 28-byte `KONUNG.CFG` payload.
///
/// Decoding is all-or-nothing: a wrong length or bad word leaves the live
/// configuration untouched and returns 0.
///
/// # Memory contract
///
/// - `ptr` must be a host allocation from [`alloc`] valid for `len` bytes; the
///   bytes are copied and the buffer is released with `dealloc` before return,
///   so the caller must not use or free it again.
/// - A null `ptr` is not dereferenced, but the call still fails (`0`).
/// - Returns 1 when the payload was applied, 0 when it was rejected.
#[no_mangle]
pub extern "C" fn config_load(ptr: *mut u8, len: u32) -> u32 {
    let data = unsafe { take_bytes(ptr, len) };
    let result = match crate::config::GameConfig::from_bytes(&data) {
        Ok(c) => {
            crate::config::set_config(c);
            1
        }
        Err(e) => {
            eprintln!("{e}");
            0
        }
    };
    dealloc(ptr, len);
    result
}

/// Copy the live configuration into a caller-allocated 28-byte buffer.
///
/// # Memory contract
///
/// - `ptr` is host-owned and must be writable for at least
///   `crate::config::CONFIG_LEN` (28) bytes; this side never frees it.
/// - A null `ptr` or `len < 28` writes nothing and returns 0.
/// - Returns the number of bytes written (always 28 on success), which encodes
///   both "accepted" and "this is the whole payload".
#[no_mangle]
pub extern "C" fn config_write(ptr: *mut u8, len: u32) -> u32 {
    if ptr.is_null() || len < crate::config::CONFIG_LEN as u32 {
        return 0;
    }
    let bytes = crate::config::config().to_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
    crate::config::CONFIG_LEN as u32
}

/// Monotonic revision, bumped whenever the live configuration changes.
///
/// The shell compares it against the revision it last persisted so a settings
/// edit is written to `localStorage` exactly once.
#[no_mangle]
pub extern "C" fn config_revision() -> u32 {
    crate::config::revision()
}

/// Debug/tests: one configuration word by field index 0..7.
///
/// Index order is `video_mode, transparency, day_night, light_effects, music,
/// shadows, sound`; any other index returns 0. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_config_word(index: u32) -> u32 {
    let c = crate::config::config();
    match index {
        0 => c.video_mode,
        1 => c.transparency,
        2 => c.day_night,
        3 => c.light_effects,
        4 => c.music,
        5 => c.shadows,
        6 => c.sound,
        _ => 0,
    }
}
