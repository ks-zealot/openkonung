//! `frame` subsystem of [`crate`] (split from `lib.rs`).

use super::*;

#[no_mangle]
pub extern "C" fn step(dt: f32) {
    if crate::video::is_pending() {
        return;
    }
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.step();
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.step(dt as f64);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn frame_ptr() -> *const u8 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt
            .frontend
            .as_ref()
            .map(|f| f.frame_ptr())
            .unwrap_or(std::ptr::null()),
        ActiveMode::Game => rt
            .game
            .as_deref()
            .map(|g| g.frame.as_ptr())
            .unwrap_or(std::ptr::null()),
    }
}

#[no_mangle]
pub extern "C" fn frame_w() -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt
            .frontend
            .as_ref()
            .map(|f| f.logical_w() as u32)
            .unwrap_or(app::VIRTUAL_W as u32),
        ActiveMode::Game => app::VIRTUAL_W as u32,
    }
}

#[no_mangle]
pub extern "C" fn frame_h() -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt
            .frontend
            .as_ref()
            .map(|f| f.logical_h() as u32)
            .unwrap_or(app::VIRTUAL_H as u32),
        ActiveMode::Game => app::VIRTUAL_H as u32,
    }
}

#[no_mangle]
pub extern "C" fn should_quit() -> u32 {
    runtime_mut().quit as u32
}

// --------------------------------------------------------------------------
// Input
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn on_resize(w: u32, h: u32) {
    let rt = runtime_mut();
    rt.host_w = w as i32;
    rt.host_h = h as i32;
    if let Some(f) = rt.frontend.as_mut() {
        f.on_resize(w as i32, h as i32);
    }
    if let Some(g) = rt.game.as_deref_mut() {
        g.on_resize(w as i32, h as i32);
    }
}

/// Host (canvas backing-store) size as recorded by the last `on_resize`,
/// independent of the active mode.
#[no_mangle]
pub extern "C" fn host_w() -> u32 {
    runtime_mut().host_w.max(0) as u32
}

#[no_mangle]
pub extern "C" fn host_h() -> u32 {
    runtime_mut().host_h.max(0) as u32
}
