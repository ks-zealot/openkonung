//! Isolated HUD preview and world-map diagnostics.

use super::super::*;

use crate::constants::debug::*;

// --------------------------------------------------------------------------
// Debug: isolated HUD preview (diagnostic tooling; same composer as runtime)
// --------------------------------------------------------------------------

/// Retained `(RGBA8 frame, width, height)` of the last HUD preview.
///
/// Written only by [`debug_render_intr_hud`]; the image stays valid until the
/// next preview and is read through [`debug_hud_ptr`]/[`debug_hud_w`]/
/// [`debug_hud_h`].
pub static mut DEBUG_HUD: Option<(Vec<u8>, u32, u32)> = None;

/// Compose the INTR HUD for `variant` (`640`, else 800) over a checkerboard into
/// a static frame and return 1 on success. Lets `tools/frame_png.mjs` capture the
/// 640x480 HUD even though the live game is fixed at 800x600. Not used at runtime.
///
/// Returns 0 when the INTR archive or `GRAPH.RES` is not loaded, or the archive
/// fails its structural validation (the error is printed to stderr).
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
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
            let c = if ((x / HUD_PREVIEW_CHECKER_CELL) + (y / HUD_PREVIEW_CHECKER_CELL)) & 1 == 0 {
                HUD_PREVIEW_CHECKER_DARK
            } else {
                HUD_PREVIEW_CHECKER_LIGHT
            };
            let i = (y * fw + x) * 4;
            frame[i] = c;
            frame[i + 1] = c;
            frame[i + 2] = c;
            frame[i + 3] = 255;
        }
    }
    let layout = hud::layout(w as i32);
    let mut state = hud::HudState::default();
    // Diagnostic preview uses the sword plaque + peaceful stance (the runtime
    // passes the real `Entity+0x12` and battle flag).
    state.icons = hud::icon_plan(w as i32, 2, false).to_vec();
    hud::compose(&mut frame, fw, fh, &layout, &state, |id| archive.get(id));
    unsafe {
        let slot = addr_of_mut!(DEBUG_HUD);
        *slot = Some((frame, w, h));
    }
    1
}

/// Start of the last [`debug_render_intr_hud`] image.
///
/// # Memory contract
///
/// Borrowed RGBA8 owned by the debug buffer; never free it. Null before the
/// first successful preview. Valid for `debug_hud_w() * debug_hud_h() * 4`
/// bytes.
#[no_mangle]
pub extern "C" fn debug_hud_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((v, _, _)) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Width of the last HUD preview in pixels (640 or 800), or 0 when there is
/// none.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_hud_w() -> u32 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((_, w, _)) => *w,
        None => 0,
    }
}

/// Height of the last HUD preview in pixels (480 or 600), or 0 when there is
/// none.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_hud_h() -> u32 {
    match unsafe { &*addr_of!(DEBUG_HUD) } {
        Some((_, _, h)) => *h,
        None => 0,
    }
}

/// Diagnostic: reveal every world-map location (preview/smoke only).
///
/// Sets all 56 entries of the discovered-location mask and drops the cached map
/// overlay so it is recomposed. Returns 1 when a game exists.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_reveal_all_locations() -> u32 {
    match runtime_mut().game.as_deref_mut() {
        Some(g) => {
            g.discovered_locations = [1; 56];
            g.map_overlay = None;
            1
        }
        None => 0,
    }
}

/// Diagnostic: reveal one location index (0..55).
///
/// Returns 1 on success, 0 when no game is active or the index is out of range.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_reveal_location(index: u32) -> u32 {
    match runtime_mut().game.as_deref_mut() {
        Some(g) if index < 56 => {
            g.discovered_locations[index as usize] = 1;
            g.map_overlay = None;
            1
        }
        _ => 0,
    }
}

/// Diagnostic: force the world map open/closed (mode 5 toggle gate).
///
/// Opening also closes the quest log (the two windows are exclusive) and drops
/// the cached overlay. Returns 1 when a game exists.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_set_map_open(open: u32) -> u32 {
    match runtime_mut().game.as_deref_mut() {
        Some(g) => {
            g.map_open = open != 0;
            g.quest_log_open = false;
            g.map_overlay = None;
            1
        }
        None => 0,
    }
}

/// 1 while the world map (interface mode 5) is open, else 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_map_open() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| u32::from(g.map_open))
        .unwrap_or(0)
}

/// Id of the loaded `Konung.N` location the game is currently on, or 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_current_map() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.current_map)
        .unwrap_or(0)
}

/// Packed `(x << 16) | y` of the first map-local pixel in a location run;
/// `0xFFFFFFFF` when the location has no run.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_map_location_point(index: u32) -> u32 {
    match crate::world_map::point_for_location(index as u8) {
        Some((x, y)) => ((x as u32) << 16) | (y as u32 & 0xFFFF),
        None => 0xFFFF_FFFF,
    }
}
