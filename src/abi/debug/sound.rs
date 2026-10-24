//! Sound archive and playback-queue diagnostics.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: sound archive (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------

pub static mut SOUND_ARCHIVE: Option<crate::sound::SoundArchive> = None;

/// Parse the loaded `SOUNDS.RES` into the shared debug archive once. Returns
/// false when no sound data is loaded or it is malformed.
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
#[no_mangle]
pub extern "C" fn debug_sound_rate(_id: u32) -> u32 {
    crate::sound::SAMPLE_RATES[0]
}

// --------------------------------------------------------------------------
// Sound playback queue (requested ids, drained by the browser shell)
// --------------------------------------------------------------------------

/// Number of sound ids requested by the active frontend/game this frame.
#[no_mangle]
pub extern "C" fn sound_count() -> u32 {
    let rt = runtime_mut();
    let len = match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| f.sounds.len()).unwrap_or(0),
        ActiveMode::Game => rt.game.as_deref().map(|g| g.sounds.len()).unwrap_or(0),
    };
    len as u32
}

/// Requested sound id `i` from the active frontend/game.
#[no_mangle]
pub extern "C" fn sound_id(i: u32) -> u32 {
    let rt = runtime_mut();
    let id = match rt.active {
        ActiveMode::Frontend => rt
            .frontend
            .as_ref()
            .and_then(|f| f.sounds.get(i as usize))
            .copied(),
        ActiveMode::Game => rt
            .game
            .as_deref()
            .and_then(|g| g.sounds.get(i as usize))
            .copied(),
    };
    id.map(|id| id as u32).unwrap_or(0)
}
