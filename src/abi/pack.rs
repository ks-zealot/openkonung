//! `pack_api` subsystem of [`crate::lib`] (split from `lib.rs`).
//!
//! Loads the single-file resource archive produced by the native packer
//! (`examples/pack.rs`). The archive is a standard ZIP whose `bundle.bin`
//! entry is a `bincode`-serialized [`crate::resource_pack::ResourcePack`].
//! On success this fills the same [`crate::Loader`] the per-file `load_*`
//! functions use, so every downstream decoder is unchanged.
//!
//! Music bytes are retained for the browser shell (WebAudio); the Rust runtime
//! itself never plays audio.

use super::*;

use crate::resource_pack::{MusicBlob, ResourcePack, PACK_ENTRY};

pub struct PackState {
    pub build: Vec<u8>,
    pub music: Vec<MusicBlob>,
}

pub static mut PACK: Option<PackState> = None;

fn pack_state() -> Option<&'static PackState> {
    unsafe { (&*addr_of!(PACK)).as_ref() }
}

/// Load a packed resource archive. Returns 1 when the loader was populated,
/// 0 when the archive is absent, truncated, corrupt or unsupported. On failure
/// the loader is left untouched, so the shell can fall back to the original
/// per-file resources.
#[no_mangle]
pub extern "C" fn load_pack(ptr: *mut u8, len: u32) -> u32 {
    if ptr.is_null() || len == 0 {
        return 0;
    }

    let result = (|| -> Result<PackState, crate::resource_pack::PackError> {
        let zip = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        let bundle = crate::resource_pack::zip::read_entry(zip, PACK_ENTRY)?;
        let pack = ResourcePack::from_bytes(&bundle)?;
        let applied = pack.apply()?;
        Ok(PackState {
            build: applied.build.into_bytes(),
            music: applied.music,
        })
    })();

    dealloc(ptr, len);

    match result {
        Ok(state) => {
            eprintln!(
                "pack: loaded {} music track(s), build {}",
                state.music.len(),
                String::from_utf8_lossy(&state.build)
            );
            unsafe { *addr_of_mut!(PACK) = Some(state) };
            1
        }
        Err(e) => {
            eprintln!("{e}");
            0
        }
    }
}

/// 1 when a resource pack has populated the loader.
#[no_mangle]
pub extern "C" fn debug_pack_loaded() -> u32 {
    pack_state().is_some() as u32
}

/// Pack content build id bytes (empty when no pack is loaded).
#[no_mangle]
pub extern "C" fn debug_pack_build_ptr() -> *const u8 {
    pack_state()
        .map(|s| s.build.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn debug_pack_build_len() -> u32 {
    pack_state().map(|s| s.build.len() as u32).unwrap_or(0)
}

// --------------------------------------------------------------------------
// Background-music payloads (browser shell -> WebAudio)
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn pack_music_count() -> u32 {
    pack_state().map(|s| s.music.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn pack_music_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.bytes.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn pack_music_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.bytes.len() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn pack_music_name_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.name.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn pack_music_name_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.name.len() as u32)
        .unwrap_or(0)
}
