//! Text overlay queue.
//!
//! Some frontend labels and in-game messages are not rasterised in wasm; the
//! game/frontend pushes positioned `TextItem` entries and the shell renders them
//! with a real font (`14 * scale` px). [`text_count`] reports the queue length;
//! the remaining accessors read item `i`. All of them return neutral defaults for
//! an out-of-range index, so the shell can iterate without a bounds check.

use super::super::*;

// --------------------------------------------------------------------------
// Text overlay queue (rendered by the browser with real fonts)
// --------------------------------------------------------------------------

/// Number of queued overlay strings for the active frontend/game.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_count() -> u32 {
    texts().map(|t| t.len() as u32).unwrap_or(0)
}

/// Logical screen x of string `i`, or 0 when out of range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_x(i: u32) -> i32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.x)
        .unwrap_or(0)
}

/// Logical screen y of string `i`, or 0 when out of range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_y(i: u32) -> i32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.y)
        .unwrap_or(0)
}

/// Packed `0xRRGGBB` colour of string `i`, or white (`0xFFFFFF`) when out of
/// range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_rgb(i: u32) -> u32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.rgb)
        .unwrap_or(0xFFFFFF)
}

/// Font scale factor of string `i` (1.0 = original size), or 1.0 when out of
/// range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_scale(i: u32) -> f32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.scale)
        .unwrap_or(1.0)
}

/// Start of string `i`'s UTF-8 bytes, or null when out of range.
///
/// # Memory contract
///
/// Borrowed from the active runtime's text queue; never free it. The queue is
/// cleared and rebuilt on every draw, so the pointer is only valid for the
/// current frame. Not NUL terminated - always use [`text_len`].
#[no_mangle]
pub extern "C" fn text_ptr(i: u32) -> *const u8 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of string `i`, or 0 when out of range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn text_len(i: u32) -> u32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.len() as u32)
        .unwrap_or(0)
}
