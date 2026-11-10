//! Save/load wasm ABI for the filesystem-less browser build.
//!
//! `www/app.js` owns `localStorage`; Rust owns the payload. The shell calls
//! `save_snapshot()`, reads `save_snapshot_ptr/len`, base64-encodes the bytes
//! and stores them under a slot key. Loading reverses the process and calls
//! `load_snapshot(ptr, len)`.
//!
//! Two formats are supported:
//!
//! * The remake's versioned `OPKS` envelope (`bincode` + deflate) - the native
//!   snapshot, carrying the whole runtime state.
//! * The original `KONUNG.SAn` (offset `0xB07A4`) and `KONUNG.SAV` (500-byte
//!   CP866 name table) - imported/exported for interoperability with the 1999
//!   save files; the projectile pool has no recovered field map and exports
//!   empty.
//!
//! All pointers here are host-owned inputs except the retained output buffers,
//! which live in statics and are never freed by the caller.

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
///
/// # Memory contract
///
/// Borrowed from the retained save buffer; never free it. Call
/// [`save_snapshot`] first and check its 1/0 result; the pointer stays valid
/// until the next [`save_snapshot`] call. Pair with [`save_snapshot_len`].
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
///
/// # Memory contract
///
/// Borrowed from the retained name buffer; never free it. Call
/// [`save_default_name`] first and check its 1/0 result. Pair with
/// [`save_default_name_len`].
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
///
/// # Memory contract
///
/// - `ptr` is **borrowed for the duration of the call only** (unlike the
///   `load_*` uploads it is not freed here); the bytes are decoded immediately,
///   so a host `alloc` buffer must be released by the caller afterwards.
/// - A null `ptr` or `len == 0` returns 0 without decoding.
/// - Returns 1 when the payload decoded and `apply_save` succeeded (the runtime
///   is then in game mode); 0 when decoding failed, no base game could be
///   started, or the snapshot was rejected.
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
///
/// # Memory contract
///
/// - `ptr` is read for `len` bytes and not retained; the name is copied into the
///   wasm-side table, so the caller keeps ownership of the buffer.
/// - Out-of-range slots (`>= 10`), null `ptr`, `len == 0` and invalid UTF-8 are
///   silently ignored (no return value).
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
    unsafe {
        (&*std::ptr::addr_of!(SAVE_NAMES))
            .get(slot)
            .is_some_and(|s| !s.is_empty())
    }
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

// --- Legacy `KONUNG.SAV` / `KONUNG.SA0..9` import and export -----------------

static mut LEGACY_FILE: Vec<u8> = Vec::new();
static mut LEGACY_TEXT: Vec<u8> = Vec::new();

/// Build the live game's original `KONUNG.SAn` payload. Returns its length, or
/// 0 when no game is running.
#[no_mangle]
pub extern "C" fn legacy_export_size() -> u32 {
    let Some(game) = super::runtime_mut().game.as_deref() else {
        return 0;
    };
    let bytes = game.export_legacy_snapshot();
    let len = bytes.len() as u32;
    unsafe {
        LEGACY_FILE = bytes;
    }
    len
}

/// Start of the retained legacy `KONUNG.SAn` payload built by
/// [`legacy_export_size`].
///
/// # Memory contract
///
/// Borrowed from the retained export buffer; never free it. Call
/// [`legacy_export_size`] first - before it, or when it returned 0, the buffer
/// may be empty and the pointer must not be read.
#[no_mangle]
pub extern "C" fn legacy_export_ptr() -> *const u8 {
    unsafe { (*std::ptr::addr_of!(LEGACY_FILE)).as_ptr() }
}

/// Import an original snapshot into the live game. Returns 1 on success.
///
/// # Memory contract
///
/// - `ptr` is borrowed for the call only and never freed here; the caller keeps
///   ownership. Null or `len == 0` returns 0.
/// - Returns 1 when the `KONUNG.SAn` payload parsed and was applied (the runtime
///   is then in game mode); 0 otherwise.
#[no_mangle]
pub extern "C" fn legacy_import(ptr: *const u8, len: u32) -> u32 {
    if ptr.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    if super::runtime_mut().game.is_none() {
        // Match `load_snapshot`: start a base game first, then apply the
        // snapshot. The leader's class comes from the snapshot record.
        let class = match crate::legacy_save::LegacySnapshot::parse(bytes)
            .ok()
            .and_then(|s| s.leader())
            .and_then(|e| crate::character::CharacterClass::from_entity_class(e.archetype & 7))
        {
            Some(c) => c,
            None => return 0,
        };
        let build = CharacterBuild::from_class(class);
        if super::lifecycle::start_game(0, build, -1) == 0 {
            return 0;
        }
    }
    let rt = super::runtime_mut();
    let Some(game) = rt.game.as_deref_mut() else {
        return 0;
    };
    match game.import_legacy_snapshot(bytes) {
        Ok(()) => {
            rt.active = super::ActiveMode::Game;
            1
        }
        Err(e) => {
            eprintln!("legacy_import: {e}");
            0
        }
    }
}

/// Parse a 500-byte `KONUNG.SAV` table into NUL-separated UTF-8. Returns 1.
///
/// # Memory contract
///
/// - `ptr` is borrowed for the call only; the decoded text is copied into the
///   retained buffer (read it with [`legacy_names_text_ptr`]/
///   [`legacy_names_text_len`]), so the caller keeps ownership of the input.
/// - Null or `len == 0` returns 0; a malformed table also returns 0 and leaves
///   the previous text in place.
#[no_mangle]
pub extern "C" fn legacy_names_parse(ptr: *const u8, len: u32) -> u32 {
    if ptr.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    match crate::legacy_save::parse_slot_names(bytes) {
        Ok(names) => {
            let text = names.join("\0");
            unsafe {
                LEGACY_TEXT = text.into_bytes();
            }
            1
        }
        Err(e) => {
            eprintln!("legacy_names_parse: {e}");
            0
        }
    }
}

/// Start of the UTF-8, NUL-separated slot-name text produced by the last
/// [`legacy_names_parse`].
///
/// # Memory contract
///
/// Borrowed from the retained text buffer; never free it. Null/empty is not
/// implied - call [`legacy_names_parse`] first and check its return code. Pair
/// with [`legacy_names_text_len`].
#[no_mangle]
pub extern "C" fn legacy_names_text_ptr() -> *const u8 {
    unsafe { (*std::ptr::addr_of!(LEGACY_TEXT)).as_ptr() }
}

/// Byte length of the retained slot-name text, or 0 when none was parsed.
#[no_mangle]
pub extern "C" fn legacy_names_text_len() -> u32 {
    unsafe { (*std::ptr::addr_of!(LEGACY_TEXT)).len() as u32 }
}

/// Encode NUL-separated UTF-8 slot names into a 500-byte `KONUNG.SAV` payload.
/// Returns the byte length, or 0 on a malformed request.
///
/// # Memory contract
///
/// - `ptr` is borrowed for the call and never freed here. Null, `len == 0` or
///   invalid UTF-8 returns 0.
/// - On success the 500 payload bytes live in the retained export buffer and are
///   read with [`legacy_export_ptr`]; the return value is the byte length the
///   caller should read (0 means "nothing was built").
#[no_mangle]
pub extern "C" fn legacy_names_build(ptr: *const u8, len: u32) -> u32 {
    if ptr.is_null() || len == 0 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let mut names: [String; crate::legacy_save::SLOT_COUNT] = Default::default();
    for (i, part) in text
        .split('\0')
        .take(crate::legacy_save::SLOT_COUNT)
        .enumerate()
    {
        names[i] = part.to_string();
    }
    let out = crate::legacy_save::write_slot_names(&names);
    let n = out.len() as u32;
    unsafe {
        LEGACY_FILE = out;
    }
    n
}
