//! Frame pump, framebuffer access and host-size exports.
//!
//! [`step`] advances whichever runtime is active (frontend or game) by one host
//! frame; [`frame_ptr`]/[`frame_w`]/[`frame_h`] expose the resulting RGBA8
//! framebuffer that `www/app.js` blits into a canvas. Sizes are dynamic: the
//! frontend follows the configured 640/800 menu layout, while the live game is
//! fixed at 800x600. [`on_resize`] records the real canvas backing-store size so
//! input mapping survives mode switches, and [`should_quit`] forwards the
//! frontend's Quit command to the shell.

use super::*;

/// Advance the active runtime by one host frame.
///
/// `dt` is the elapsed host time in seconds; it is what the game feeds into the
/// 60 ms logic tick (`ORIGINAL_FRAME_MS`, ~16.67 Hz), so the original animation
/// cadence is preserved on any refresh rate. The call is a no-op while a movie
/// is pending ([`crate::video::is_pending`]): the shell owns the clock then.
///
/// # Memory contract
///
/// No pointers cross this boundary and there is no return value.
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

/// Start of the active runtime's RGBA8 framebuffer.
///
/// # Memory contract
///
/// - Borrowed, read-only; the runtime owns the storage. Do not free it.
/// - Valid for `frame_w() * frame_h() * 4` bytes and only until the next
///   [`step`] (or a mode switch), which may reallocate.
/// - Null when the active mode has no runtime yet (no `boot`/`frontend_boot`
///   success); `frame_w`/`frame_h` still report the default size in that case.
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

/// Framebuffer width in pixels: the frontend's logical width (640 or 800) or
/// the fixed 800 of the live game.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
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

/// Framebuffer height in pixels: the frontend's logical height (480 or 600) or
/// the fixed 600 of the live game.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
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

/// 1 when the frontend requested a quit (`FrontendCommand::Quit`), else 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn should_quit() -> u32 {
    runtime_mut().quit as u32
}

// --------------------------------------------------------------------------
// Input
// --------------------------------------------------------------------------

/// Record the canvas backing-store size and forward it to both runtimes.
///
/// The original game scales its fixed 640x480/800x600 layout to the window; the
/// remake keeps the logical frame and only needs the host size for pointer
/// mapping, so a resize never changes the framebuffer dimensions.
///
/// # Memory contract
///
/// No pointers cross this boundary and there is no return value.
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
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn host_w() -> u32 {
    runtime_mut().host_w.max(0) as u32
}

/// Host (canvas backing-store) height as recorded by the last `on_resize`;
/// negative values are clamped to 0 (`0` = the host has not reported one yet).
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn host_h() -> u32 {
    runtime_mut().host_h.max(0) as u32
}
