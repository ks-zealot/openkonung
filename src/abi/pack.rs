//! `pack_api` subsystem of [`crate`] (split from `lib.rs`).
//!
//! Loads the single-file resource archive produced by the native packer
//! (`examples/pack.rs`). The archive is a standard ZIP whose `bundle.bin`
//! entry is a `bincode`-serialized [`crate::resource_pack::ResourcePack`].
//! On success this fills the same [`crate::Loader`] the per-file `load_*`
//! functions use, so every downstream decoder is unchanged.
//!
//! Music bytes are retained for the browser shell (WebAudio); the Rust runtime
//! itself never plays audio. Movie bytes are retained for the same reason: the
//! shell turns them into `Blob` URLs for a `<video>` element.
//!
//! Acceptance for the packed boot: after a successful [`load_pack`] the page
//! needs only `manifest.json`, the wasm, the shell files and the archive - no
//! individual `.res`, `.exe`, `Game.N`, `Konung.N` or `/music/` request.

use super::*;

use crate::resource_pack::{MusicBlob, ResourcePack, VideoBlob, PACK_ENTRY};

/// Retained payloads of a successfully applied resource pack.
pub struct PackState {
    /// Pack content build id (UTF-8 bytes; FNV of resource names + bytes).
    pub build: Vec<u8>,
    /// Original Ogg background-music tracks, in pack order.
    pub music: Vec<MusicBlob>,
    /// Transcoded H.264/AAC MP4 movies, in pack order.
    pub videos: Vec<VideoBlob>,
}

/// Live pack payloads; `None` until a [`load_pack`] succeeds.
pub static mut PACK: Option<PackState> = None;

/// Borrow the live pack payloads, if any.
fn pack_state() -> Option<&'static PackState> {
    unsafe { (&*addr_of!(PACK)).as_ref() }
}

/// Load a packed resource archive. Returns 1 when the loader was populated,
/// 0 when the archive is absent, truncated, corrupt or unsupported. On failure
/// the loader is left untouched, so the shell can fall back to the original
/// per-file resources.
///
/// # Memory contract
///
/// - `ptr` must be a host allocation from [`alloc`] valid for `len` bytes. The
///   archive is read in place and the buffer is released with `dealloc` before
///   return, so the caller must not use or free it again.
/// - A null `ptr` or `len == 0` returns 0 without dereferencing.
/// - Returns 1 only when `bundle.bin` was found, inflated (CRC-checked), decoded
///   and applied; `PACK` is then set and the debug/music/video accessors become
///   valid.
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
            videos: applied.videos,
        })
    })();

    dealloc(ptr, len);

    match result {
        Ok(state) => {
            eprintln!(
                "pack: loaded {} music track(s), {} video(s), build {}",
                state.music.len(),
                state.videos.len(),
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
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by [`PACK`]; never free it. Null when no pack is loaded.
/// Not NUL terminated - pair with [`debug_pack_build_len`].
#[no_mangle]
pub extern "C" fn debug_pack_build_ptr() -> *const u8 {
    pack_state()
        .map(|s| s.build.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of the pack build id, or 0 when no pack is loaded.
#[no_mangle]
pub extern "C" fn debug_pack_build_len() -> u32 {
    pack_state().map(|s| s.build.len() as u32).unwrap_or(0)
}

// --------------------------------------------------------------------------
// Background-music payloads (browser shell -> WebAudio)
// --------------------------------------------------------------------------

/// Number of packed music tracks, or 0 when no pack is loaded.
#[no_mangle]
pub extern "C" fn pack_music_count() -> u32 {
    pack_state().map(|s| s.music.len() as u32).unwrap_or(0)
}

/// Start of packed music track `i`'s Ogg bytes, or null.
///
/// # Memory contract
///
/// Borrowed bytes owned by [`PACK`] for the lifetime of the pack; never free
/// them. The shell copies them into a `Blob` URL. Pair with [`pack_music_len`].
#[no_mangle]
pub extern "C" fn pack_music_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.bytes.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of packed music track `i`, or 0 when out of range.
#[no_mangle]
pub extern "C" fn pack_music_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.bytes.len() as u32)
        .unwrap_or(0)
}

/// Start of packed music track `i`'s display name (UTF-8), or null.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by [`PACK`]; never free it. Pair with
/// [`pack_music_name_len`].
#[no_mangle]
pub extern "C" fn pack_music_name_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.name.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of packed music track `i`'s name, or 0 when out of range.
#[no_mangle]
pub extern "C" fn pack_music_name_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.music.get(i as usize))
        .map(|m| m.name.len() as u32)
        .unwrap_or(0)
}

// --------------------------------------------------------------------------
// Browser-playable movie payloads (packed H.264/AAC MP4)
// --------------------------------------------------------------------------

/// Number of packed movies, or 0 when no pack is loaded.
#[no_mangle]
pub extern "C" fn pack_video_count() -> u32 {
    pack_state().map(|s| s.videos.len() as u32).unwrap_or(0)
}

/// Start of packed movie `i`'s MP4 bytes, or null.
///
/// # Memory contract
///
/// Borrowed bytes owned by [`PACK`]; never free them. The shell copies them into
/// a `Blob` URL. Pair with [`pack_video_len`].
#[no_mangle]
pub extern "C" fn pack_video_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.videos.get(i as usize))
        .map(|v| v.bytes.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of packed movie `i`, or 0 when out of range.
#[no_mangle]
pub extern "C" fn pack_video_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.videos.get(i as usize))
        .map(|v| v.bytes.len() as u32)
        .unwrap_or(0)
}

/// Start of packed movie `i`'s file name (UTF-8, e.g. `poster.mp4`), or null.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by [`PACK`]; never free it. Pair with
/// [`pack_video_name_len`].
#[no_mangle]
pub extern "C" fn pack_video_name_ptr(i: u32) -> *const u8 {
    pack_state()
        .and_then(|s| s.videos.get(i as usize))
        .map(|v| v.name.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of packed movie `i`'s name, or 0 when out of range.
#[no_mangle]
pub extern "C" fn pack_video_name_len(i: u32) -> u32 {
    pack_state()
        .and_then(|s| s.videos.get(i as usize))
        .map(|v| v.name.len() as u32)
        .unwrap_or(0)
}
