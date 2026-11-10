//! `config` subsystem of [`crate`]: `KONUNG.CFG` browser persistence.
//!
//! `www/app.js` keeps the exact original 28 bytes as base64 in
//! `localStorage["openkonung.cfg"]`; these exports copy them in/out of the
//! wasm-side [`crate::config`] globals without inventing a filesystem.

use super::*;

/// Load one 28-byte `KONUNG.CFG` payload. Returns 1 on success, 0 on error
/// (the previous configuration is kept).
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
/// Returns the byte count written (0 when `len` is too small).
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
#[no_mangle]
pub extern "C" fn config_revision() -> u32 {
    crate::config::revision()
}

/// Debug/tests: one configuration word by field index 0..7.
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
