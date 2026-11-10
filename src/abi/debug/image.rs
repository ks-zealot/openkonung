//! Menu/INTR/cursor image diagnostics: decode one original image to RGBA8.
//!
//! Every exporter here writes the shared [`DEBUG_IMAGE`] buffer plus its width
//! and height, which the PNG tools read through [`debug_image_ptr`] /
//! [`debug_image_w`] / [`debug_image_h`]. The decoders are the runtime decoders,
//! so a captured image proves what the game would draw.

use super::super::*;

use crate::constants::debug::*;

// --------------------------------------------------------------------------
// Debug: decoded menu images (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------

/// Retained RGBA8 pixels of the last decoded image.
///
/// `None` before the first successful decode; overwritten by every later
/// `debug_decode_*` call.
pub static mut DEBUG_IMAGE: Option<Vec<u8>> = None;

/// Width in pixels of the image in [`DEBUG_IMAGE`] (`0` when nothing is
/// decoded).
pub static mut DEBUG_IMAGE_W: u32 = 0;

/// Height in pixels of the image in [`DEBUG_IMAGE`] (`0` when nothing is
/// decoded).
pub static mut DEBUG_IMAGE_H: u32 = 0;

/// Decode `kind` (menu RES family) image `index` into a static RGBA buffer and
/// return 1 on success. Used by `tools/menu_png.mjs`; not used at runtime.
///
/// `kind` is one of the `KIND_MENU640`/`MENU800`/`NEWH640`/`NEWH800`/
/// `SAVE640`/`SAVE800` selectors; any other value, an unloaded archive, a parse
/// error or an out-of-range index returns 0 and leaves the buffer untouched.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
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

/// Start of the last decoded image.
///
/// # Memory contract
///
/// Borrowed RGBA8 owned by the debug buffer; never free it. Null before the
/// first successful decode. Valid for `debug_image_w() * debug_image_h() * 4`
/// bytes.
#[no_mangle]
pub extern "C" fn debug_image_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_IMAGE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Width of the last decoded image in pixels, or 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_image_w() -> u32 {
    unsafe { DEBUG_IMAGE_W }
}

/// Height of the last decoded image in pixels, or 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_image_h() -> u32 {
    unsafe { DEBUG_IMAGE_H }
}

/// Decode INTR interface sprite `index` from `variant` (`640` or anything else
/// = 800) into the shared debug RGBA buffer and return 1 on success. Used by
/// `tools/intr_png.mjs`; not used at runtime.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_decode_intr_image(variant: u32, index: u32) -> u32 {
    let (bytes, graph) = {
        let l = loader_mut();
        let intr = if variant == INTR_VARIANT_640 {
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
// Debug: original mouse-cursor frames (diagnostic tooling; same decoder)
// --------------------------------------------------------------------------

/// Number of original cursor frames (11). Diagnostic only.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_cursor_count() -> u32 {
    res::CURSOR_COUNT as u32
}

/// Decode original mouse-cursor frame `i` (0..11) from the loaded `GRAPH.RES`
/// into the shared debug RGBA buffer and return 1 on success. Used by
/// `tools/cursor_png.mjs`; not used at runtime.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_decode_cursor_image(i: u32) -> u32 {
    let graph = loader_mut().graph.clone();
    if graph.is_empty() {
        return 0;
    }
    let Some(img) = res::cursor_image(&graph, i as usize) else {
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
