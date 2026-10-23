//! Save/load wasm ABI for the filesystem-less browser build.
//!
//! `www/app.js` owns `localStorage`; Rust owns the payload. The shell calls
//! `save_snapshot()`, reads `save_snapshot_ptr/len`, base64-encodes the bytes
//! and stores them under a slot key. Loading reverses the process and calls
//! `load_snapshot(ptr, len)`.

use crate::character::CharacterBuild;
use crate::save::SaveEnvelope;

static mut SAVE_BUF: Vec<u8> = Vec::new();
static mut NAME_BUF: Vec<u8> = Vec::new();
static mut SAVE_NAMES: Vec<String> = Vec::new();
static mut SAVE_REQUEST: i32 = -1;
static mut LOAD_REQUEST: i32 = -1;

/// Build, compress and retain the current game snapshot. Returns 1 on success.
#[no_mangle]
pub extern "C" fn save_snapshot() -> u32 {
    let Some(game) = super::runtime_mut().game.as_deref() else {
        return 0;
    };
    match game.capture_save().encode() {
        Ok(bytes) => {
            unsafe {
                SAVE_BUF = bytes;
            }
            1
        }
        Err(e) => {
            eprintln!("save_snapshot: {e}");
            0
        }
    }
}

/// Pointer to the retained snapshot bytes (valid until the next save).
#[no_mangle]
pub extern "C" fn save_snapshot_ptr() -> *const u8 {
    unsafe { (*std::ptr::addr_of!(SAVE_BUF)).as_ptr() }
}

/// Length of the retained snapshot bytes.
#[no_mangle]
pub extern "C" fn save_snapshot_len() -> u32 {
    unsafe { (*std::ptr::addr_of!(SAVE_BUF)).len() as u32 }
}

/// Fill the retained name buffer with the current location label. Returns 1
/// when a live game exists.
#[no_mangle]
pub extern "C" fn save_default_name() -> u32 {
    let Some(game) = super::runtime_mut().game.as_deref() else {
        return 0;
    };
    unsafe {
        NAME_BUF = game.current_location_name().into_bytes();
    }
    1
}

/// Pointer to the retained default save name (UTF-8).
#[no_mangle]
pub extern "C" fn save_default_name_ptr() -> *const u8 {
    unsafe { (*std::ptr::addr_of!(NAME_BUF)).as_ptr() }
}

/// Length of the retained default save name.
#[no_mangle]
pub extern "C" fn save_default_name_len() -> u32 {
    unsafe { (*std::ptr::addr_of!(NAME_BUF)).len() as u32 }
}

/// Decode and apply a snapshot. Builds a base game first when none is live.
/// Returns 1 on success.
#[no_mangle]
pub extern "C" fn load_snapshot(ptr: *const u8, len: u32) -> u32 {
    if ptr.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let save = match SaveEnvelope::decode(bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("load_snapshot: {e}");
            return 0;
        }
    };
    let has_game = super::runtime_mut().game.is_some();
    if !has_game {
        let hero = save.game.meta.hero_index;
        let build = CharacterBuild::from_class(save.game.player.class);
        if super::lifecycle::start_game(hero, build, -1) == 0 {
            return 0;
        }
    }
    let rt = super::runtime_mut();
    let Some(game) = rt.game.as_deref_mut() else {
        return 0;
    };
    match game.apply_save(save) {
        Ok(()) => {
            rt.active = super::ActiveMode::Game;
            1
        }
        Err(e) => {
            eprintln!("load_snapshot: {e}");
            0
        }
    }
}

/// Submit one UTF-8 slot name from the browser storage layer.
#[no_mangle]
pub extern "C" fn set_save_name(slot: u32, ptr: *const u8, len: u32) {
    let slot = slot as usize;
    if slot >= 10 || ptr.is_null() || len == 0 {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let Ok(name) = std::str::from_utf8(bytes) else {
        return;
    };
    unsafe {
        let names = &mut *std::ptr::addr_of_mut!(SAVE_NAMES);
        if names.len() < 10 {
            names.resize(10, String::new());
        }
        names[slot] = name.to_string();
    }
}

/// Decoded display name for one slot. Empty slots use the original
/// `"Пустая строка"` default.
pub fn save_slot_name(slot: u32) -> String {
    let slot = slot as usize;
    if slot >= 10 {
        return String::new();
    }
    unsafe {
        (&*std::ptr::addr_of!(SAVE_NAMES))
            .get(slot)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| "Пустая строка".to_string())
    }
}

/// True when a slot has a submitted name (i.e. a save may exist).
pub fn save_slot_named(slot: u32) -> bool {
    let slot = slot as usize;
    if slot >= 10 {
        return false;
    }
    unsafe { (&*std::ptr::addr_of!(SAVE_NAMES)).get(slot).is_some_and(|s| !s.is_empty()) }
}

/// Called by the frontend when a save slot is clicked.
pub fn request_save_slot(slot: u32) {
    unsafe {
        SAVE_REQUEST = slot as i32;
    }
}

/// Called by the frontend when a load slot is clicked.
pub fn request_load_slot(slot: u32) {
    unsafe {
        LOAD_REQUEST = slot as i32;
    }
}

/// Browser shell: take the pending save request (`-1` when none).
#[no_mangle]
pub extern "C" fn take_save_request() -> i32 {
    unsafe {
        let v = SAVE_REQUEST;
        SAVE_REQUEST = -1;
        v
    }
}

/// Browser shell: take the pending load request (`-1` when none).
#[no_mangle]
pub extern "C" fn take_load_request() -> i32 {
    unsafe {
        let v = LOAD_REQUEST;
        LOAD_REQUEST = -1;
        v
    }
}

/// Diagnostic: 1 when a live game exists.
#[no_mangle]
pub extern "C" fn debug_save_slot_named(slot: u32) -> u32 {
    save_slot_named(slot) as u32
}
