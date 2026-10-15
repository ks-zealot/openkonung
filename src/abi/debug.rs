//! `debug` subsystem of [`crate::lib`] (split from `lib.rs`).

use super::*;

pub static mut DEBUG_PLAYER_STATE: Option<Vec<u8>> = None;


/// Format the active game's [`PlayerCharacter::debug_block`] into a static
/// buffer. Returns 1 when a game exists, 0 otherwise. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_player_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            let bytes = g.player.debug_block().into_bytes();
            unsafe {
                *addr_of_mut!(DEBUG_PLAYER_STATE) = Some(bytes);
            }
            1
        }
        None => 0,
    }
}


#[no_mangle]
pub extern "C" fn debug_player_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}


#[no_mangle]
pub extern "C" fn debug_player_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}


pub static mut DEBUG_ENTITY_STATE: Option<Vec<u8>> = None;


/// Format the active game's source `Entity` with the given id (task §7) into a
/// static buffer. Returns 1 when a game is active and the id is in range.
#[no_mangle]
pub extern "C" fn debug_entity_state(id: u32) -> u32 {
    let block = match runtime_mut().game.as_deref() {
        Some(g) => g.entity_debug_block(id as usize),
        None => None,
    };
    match block {
        Some(s) => {
            unsafe {
                *addr_of_mut!(DEBUG_ENTITY_STATE) = Some(s.into_bytes());
            }
            1
        }
        None => 0,
    }
}


#[no_mangle]
pub extern "C" fn debug_entity_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}


#[no_mangle]
pub extern "C" fn debug_entity_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

// --------------------------------------------------------------------------
// Debug: decoded menu images (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------


pub static mut DEBUG_IMAGE: Option<Vec<u8>> = None;

pub static mut DEBUG_IMAGE_W: u32 = 0;

pub static mut DEBUG_IMAGE_H: u32 = 0;


/// Decode `kind` (menu RES family) image `index` into a static RGBA buffer and
/// return 1 on success. Used by `tools/menu_png.mjs`; not used at runtime.
#[no_mangle]
pub extern "C" fn debug_decode_menu_image(kind: u32, index: u32) -> u32 {
    let bytes: Vec<u8> = {
        let l = loader_mut();
        match kind {
            KIND_MENU640 => l.menu640.clone(),
            KIND_MENU800 => l.menu800.clone(),
            KIND_NEWH640 => l.newh640.clone(),
            KIND_NEWH800 => l.newh800.clone(),
            KIND_SAVE640 => l.save640.clone(),
            KIND_SAVE800 => l.save800.clone(),
            _ => return 0,
        }
    };
    let name = format!("kind{kind}");
    let res = match menu_res::MenuRes::parse_named(&name, &bytes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 0;
        }
    };
    let Some(img) = res.images.get(index as usize) else {
        return 0;
    };
    unsafe {
        let slot = addr_of_mut!(DEBUG_IMAGE);
        *slot = Some(img.rgba.clone());
        DEBUG_IMAGE_W = img.width as u32;
        DEBUG_IMAGE_H = img.height as u32;
    }
    1
}


#[no_mangle]
pub extern "C" fn debug_image_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_IMAGE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}


#[no_mangle]
pub extern "C" fn debug_image_w() -> u32 {
    unsafe { DEBUG_IMAGE_W }
}


#[no_mangle]
pub extern "C" fn debug_image_h() -> u32 {
    unsafe { DEBUG_IMAGE_H }
}


/// Decode INTR interface sprite `index` from `variant` (`640` or anything else
/// = 800) into the shared debug RGBA buffer and return 1 on success. Used by
/// `tools/intr_png.mjs`; not used at runtime.
#[no_mangle]
pub extern "C" fn debug_decode_intr_image(variant: u32, index: u32) -> u32 {
    let (bytes, graph) = {
        let l = loader_mut();
        let intr = if variant == 640 {
            l.intr640.clone()
        } else {
            l.intr800.clone()
        };
        (intr, l.graph.clone())
    };
    if bytes.is_empty() || graph.is_empty() {
        return 0;
    }
    let mut archive = match intr::IntrArchive::load(&bytes, &graph) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 0;
        }
    };
    let Some(img) = archive.get(index as usize) else {
        return 0;
    };
    unsafe {
        let slot = addr_of_mut!(DEBUG_IMAGE);
        *slot = Some(img.data.clone());
        DEBUG_IMAGE_W = img.w as u32;
        DEBUG_IMAGE_H = img.h as u32;
    }
    1
}

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
        let intr = if variant == 640 {
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
    let (w, h) = if variant == 640 {
        (640u32, 480u32)
    } else {
        (800u32, 600u32)
    };
    let fw = w as usize;
    let fh = h as usize;
    let mut frame = vec![0u8; fw * fh * 4];
    // Checkerboard so transparent HUD regions are visible in the preview.
    for y in 0..fh {
        for x in 0..fw {
            let c = if ((x / 16) + (y / 16)) & 1 == 0 { 0x30u8 } else { 0x50u8 };
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

// --------------------------------------------------------------------------
// Text overlay queue (rendered by the browser with real fonts)
// --------------------------------------------------------------------------


#[no_mangle]
pub extern "C" fn text_count() -> u32 {
    texts().map(|t| t.len() as u32).unwrap_or(0)
}


#[no_mangle]
pub extern "C" fn text_x(i: u32) -> i32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.x).unwrap_or(0)
}


#[no_mangle]
pub extern "C" fn text_y(i: u32) -> i32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.y).unwrap_or(0)
}


#[no_mangle]
pub extern "C" fn text_rgb(i: u32) -> u32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.rgb).unwrap_or(0xFFFFFF)
}


#[no_mangle]
pub extern "C" fn text_scale(i: u32) -> f32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.scale).unwrap_or(1.0)
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


