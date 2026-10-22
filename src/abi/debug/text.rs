//! Text overlay queue.

use super::super::*;

// --------------------------------------------------------------------------
// Text overlay queue (rendered by the browser with real fonts)
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn text_count() -> u32 {
    texts().map(|t| t.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_x(i: u32) -> i32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.x)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_y(i: u32) -> i32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.y)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_rgb(i: u32) -> u32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.rgb)
        .unwrap_or(0xFFFFFF)
}

#[no_mangle]
pub extern "C" fn text_scale(i: u32) -> f32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.scale)
        .unwrap_or(1.0)
}

#[no_mangle]
pub extern "C" fn text_ptr(i: u32) -> *const u8 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn text_len(i: u32) -> u32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.len() as u32)
        .unwrap_or(0)
}
