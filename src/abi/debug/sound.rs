//! Sound archive and playback-queue diagnostics.
//!
//! [`crate::sound::SoundArchive`] parses `SOUNDS.RES` (8-byte header plus
//! `1000 x {u32 offset, u32 size}` at 8008, raw s16 mono PCM). The
//! `sound_*` exports are the runtime path: `www/app.js` drains the one-frame
//! queue after every [`step`] and plays each id through WebAudio
//! with the pan/volume/rate the original DirectSound buffers used.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: sound archive (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------

/// Parsed `SOUNDS.RES` shared by the debug accessors (parsed lazily once).
pub static mut SOUND_ARCHIVE: Option<crate::sound::SoundArchive> = None;

/// Parse the loaded `SOUNDS.RES` into the shared debug archive once. Returns
/// false when no sound data is loaded or it is malformed.
///
/// Called only from the `debug_sound_*` exports; the runtime keeps the raw bytes
/// in the loader and parses its own copy.
fn ensure_sound_archive() -> bool {
    if unsafe { (*addr_of!(SOUND_ARCHIVE)).is_some() } {
        return true;
    }
    let bytes = loader_mut().sounds.clone();
    if bytes.is_empty() {
        return false;
    }
    match crate::sound::SoundArchive::parse(&bytes) {
        Ok(a) => {
            unsafe { *addr_of_mut!(SOUND_ARCHIVE) = Some(a) };
            true
        }
        Err(e) => {
            eprintln!("{e}");
            false
        }
    }
}

/// Number of occupied clips in the loaded `SOUNDS.RES`, or 0. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_sound_count() -> u32 {
    if ensure_sound_archive() {
        unsafe { (&*addr_of!(SOUND_ARCHIVE)).as_ref().unwrap().len() as u32 }
    } else {
        0
    }
}

/// Pointer to clip `id`'s raw s16 mono PCM, or null.
///
/// # Memory contract
///
/// Borrowed bytes owned by the parsed archive; never free them. Null when the
/// archive is absent, malformed, or `id` is empty/out of range. Pair with
/// [`debug_sound_clip_len`].
#[no_mangle]
pub extern "C" fn debug_sound_clip_ptr(id: u32) -> *const u8 {
    if !ensure_sound_archive() {
        return std::ptr::null();
    }
    unsafe {
        (&*addr_of!(SOUND_ARCHIVE))
            .as_ref()
            .and_then(|a| a.clip_bytes(id as usize))
            .map(|b| b.as_ptr())
            .unwrap_or(std::ptr::null())
    }
}

/// Byte length of clip `id`, or 0.
#[no_mangle]
pub extern "C" fn debug_sound_clip_len(id: u32) -> u32 {
    if !ensure_sound_archive() {
        return 0;
    }
    unsafe {
        (&*addr_of!(SOUND_ARCHIVE))
            .as_ref()
            .and_then(|a| a.clip_bytes(id as usize))
            .map(|b| b.len() as u32)
            .unwrap_or(0)
    }
}

/// Base playback sample rate of the archive (22050 Hz).
///
/// `id` is ignored: the primary DirectSound buffer runs at one fixed rate, and
/// per-clip pitch variation is applied through [`sound_rate`] instead.
#[no_mangle]
pub extern "C" fn debug_sound_rate(_id: u32) -> u32 {
    crate::sound::SAMPLE_RATES[0]
}

// --------------------------------------------------------------------------
// Sound playback queue (requested ids, drained by the browser shell)
// --------------------------------------------------------------------------

/// Number of sound ids requested by the active frontend/game this frame.
///
/// The queue holds exactly one frame: it is cleared at the start of every
/// [`step`] and drained by the shell before the next one.
#[no_mangle]
pub extern "C" fn sound_count() -> u32 {
    let rt = runtime_mut();
    let len = match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| f.sounds.len()).unwrap_or(0),
        ActiveMode::Game => rt.game.as_deref().map(|g| g.sounds.len()).unwrap_or(0),
    };
    len as u32
}

/// Requested sound id `i` from the active frontend/game, or 0 when out of range.
#[no_mangle]
pub extern "C" fn sound_id(i: u32) -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt
            .frontend
            .as_ref()
            .and_then(|f| f.sounds.get(i as usize))
            .map(|id| *id as u32)
            .unwrap_or(0),
        ActiveMode::Game => rt
            .game
            .as_deref()
            .and_then(|g| g.sounds.get(i as usize))
            .map(|e| e.id as u32)
            .unwrap_or(0),
    }
}

/// Positional pan (`-10000..10000`) of sound `i`; `0` for the frontend.
///
/// The value is `round_ties_even((source_row - player_row) * 62.5)` from
/// `src/sound/position.rs` (`FUN_00438DC4`); `www/app.js` maps it onto a
/// `StereoPannerNode`.
#[no_mangle]
pub extern "C" fn sound_pan(i: u32) -> i32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Game => rt
            .game
            .as_deref()
            .and_then(|g| g.sounds.get(i as usize))
            .map(|e| e.pan)
            .unwrap_or(0),
        ActiveMode::Frontend => 0,
    }
}

/// Volume in hundredths of a decibel (`0` full, `-10000` silent) of sound `i`.
#[no_mangle]
pub extern "C" fn sound_volume(i: u32) -> i32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Game => rt
            .game
            .as_deref()
            .and_then(|g| g.sounds.get(i as usize))
            .map(|e| e.volume)
            .unwrap_or(0),
        ActiveMode::Frontend => 0,
    }
}

/// Secondary-buffer sample rate of sound `i` (22050/19050/25050).
#[no_mangle]
pub extern "C" fn sound_rate(i: u32) -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Game => rt
            .game
            .as_deref()
            .and_then(|g| g.sounds.get(i as usize))
            .map(|e| e.rate)
            .unwrap_or(crate::sound::SAMPLE_RATES[0]),
        ActiveMode::Frontend => crate::sound::SAMPLE_RATES[0],
    }
}
