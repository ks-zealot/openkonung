//! Isolated HUD preview diagnostics.

use super::super::*;

use crate::constants::debug::*;

// --------------------------------------------------------------------------
// Debug: isolated HUD preview (diagnostic tooling; same composer as runtime)
// --------------------------------------------------------------------------


pub static mut DEBUG_HUD: Option<(Vec<u8>, u32, u32)> = None;


/// Compose the INTR HUD for `variant` (`640`, else 800) over a checkerboard into
/// a static frame and return 1 on success. Lets `tools/frame_png.mjs` capture the
/// 640x480 HUD even though the live game is fixed at 800x600. Not used at runtime.
#[no_mangle]
pub extern "C" fn debug_render_intr_hud(variant: u32) -> u32 {
    let (intr, graph) = {
        let l = loader_mut();
        let intr = if variant == INTR_VARIANT_640 {
            l.intr640.clone()
        } else {
            l.intr800.clone()
        };
        (intr, l.graph.clone())
    };
    if intr.is_empty() || graph.is_empty() {
        return 0;
    }
    let mut archive = match intr::IntrArchive::load(&intr, &graph) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 0;
        }
    };
    let (w, h) = if variant == INTR_VARIANT_640 {
        HUD_PREVIEW_640
    } else {
        HUD_PREVIEW_800
    };
    let fw = w as usize;
    let fh = h as usize;
    let mut frame = vec![0u8; fw * fh * 4];
    // Checkerboard so transparent HUD regions are visible in the preview.
    for y in 0..fh {
        for x in 0..fw {
            let c = if ((x / HUD_PREVIEW_CHECKER_CELL) + (y / HUD_PREVIEW_CHECKER_CELL)) & 1 == 0 { HUD_PREVIEW_CHECKER_DARK } else { HUD_PREVIEW_CHECKER_LIGHT };
            let i = (y * fw + x) * 4;
            frame[i] = c;
            frame[i + 1] = c;
            frame[i + 2] = c;
            frame[i + 3] = 255;
        }
    }
    let layout = hud::layout(w as i32);
    let state = hud::HudState::default();
    hud::compose(&mut frame, fw, fh, &layout, &state, |id| archive.get(id));
    unsafe {
        let slot = addr_of_mut!(DEBUG_HUD);
        *slot = Some((frame, w, h));
    }
    1
}


#[no_mangle]
pub extern "C" fn debug_hud_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((v, _, _)) => v.as_ptr(),
        None => std::ptr::null(),
    }
}


#[no_mangle]
pub extern "C" fn debug_hud_w() -> u32 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((_, w, _)) => *w,
        None => 0,
    }
}


#[no_mangle]
pub extern "C" fn debug_hud_h() -> u32 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((_, _, h)) => *h,
        None => 0,
    }
}
