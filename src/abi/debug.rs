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


pub static mut DEBUG_COMBAT_STATE: Option<Vec<u8>> = None;

/// Diagnostic: move a live object-family actor beside the protagonist, make it
/// a weak poison carrier, put the leader in battle stance and issue the attack
/// order. Returns `entity_id + 1` or 0.
#[no_mangle]
pub extern "C" fn debug_combat_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_test_setup(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_set_seed(seed: u32) {
    if let Some(g) = runtime_mut().game.as_mut() {
        g.combat_set_seed(seed);
    }
}

/// Diagnostic: format the current combat state into the shared debug buffer.
#[no_mangle]
pub extern "C" fn debug_combat_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            unsafe {
                *addr_of_mut!(DEBUG_COMBAT_STATE) = Some(g.combat_debug_report().into_bytes());
            }
            1
        }
        None => 0,
    }
}

/// Diagnostic: kill the protagonist and return 1.
#[no_mangle]
pub extern "C" fn debug_combat_kill_player() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_kill_player(),
        None => 0,
    }
}

/// Diagnostic: force one player -> staged-target melee resolution; returns the
/// final damage (0 for miss/no target).
#[no_mangle]
pub extern "C" fn debug_combat_force() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_force(),
        None => 0,
    }
}

pub static mut DEBUG_RANGED_STATE: Option<Vec<u8>> = None;

#[no_mangle]
pub extern "C" fn debug_ranged_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_test_setup(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_make_dodge() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_make_dodge(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_target_arrows() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_target_arrows(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_prop_setup(prop_index: u32) -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.prop_test_setup(prop_index as usize),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            unsafe {
                *addr_of_mut!(DEBUG_RANGED_STATE) = Some(g.ranged_debug_report().into_bytes());
            }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_ranged_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

/// Diagnostic: teleport onto the newest corpse marker and run the original
/// pickup. Returns 1 when a marker was present.
#[no_mangle]
pub extern "C" fn debug_combat_collect() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.combat_debug_collect(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_combat_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_COMBAT_STATE) } {
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
// Debug: original mouse-cursor frames (diagnostic tooling; same decoder)
// --------------------------------------------------------------------------


/// Number of original cursor frames (11). Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_cursor_count() -> u32 {
    res::CURSOR_COUNT as u32
}


/// Decode original mouse-cursor frame `i` (0..11) from the loaded `GRAPH.RES`
/// into the shared debug RGBA buffer and return 1 on success. Used by
/// `tools/cursor_png.mjs`; not used at runtime.
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
// Debug: dialogue/quest database (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------


/// Number of dialogue ids in the loaded `QUEST_R.RES`, or 0 when absent or the
/// structural validation fails. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_quest_dialogue_count() -> u32 {
    let bytes = loader_mut().quest.clone();
    if bytes.is_empty() {
        return 0;
    }
    match quest::QuestData::parse(&bytes) {
        Ok(_) => quest::DIALOGUE_ENTRIES as u32,
        Err(e) => {
            eprintln!("{e}");
            0
        }
    }
}


// --------------------------------------------------------------------------
// Debug: sound archive (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------


pub static mut SOUND_ARCHIVE: Option<crate::sound::SoundArchive> = None;


/// Parse the loaded `SOUNDS.RES` into the shared debug archive once. Returns
/// false when no sound data is loaded or it is malformed.
fn ensure_sound_archive() -> bool {
    if unsafe { (*addr_of!(SOUND_ARCHIVE)).is_some() } {
        return true;
    }
    let bytes = loader_mut().sounds.clone();
    if bytes.is_empty() {
        return false;
    }
    match crate::sound::SoundArchive::parse(&bytes) {
        Ok(a) => {
            unsafe { *addr_of_mut!(SOUND_ARCHIVE) = Some(a) };
            true
        }
        Err(e) => {
            eprintln!("{e}");
            false
        }
    }
}


/// Number of occupied clips in the loaded `SOUNDS.RES`, or 0. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_sound_count() -> u32 {
    if ensure_sound_archive() {
        unsafe { (&*addr_of!(SOUND_ARCHIVE)).as_ref().unwrap().len() as u32 }
    } else {
        0
    }
}


/// Pointer to clip `id`'s raw s16 mono PCM, or null.
#[no_mangle]
pub extern "C" fn debug_sound_clip_ptr(id: u32) -> *const u8 {
    if !ensure_sound_archive() {
        return std::ptr::null();
    }
    unsafe {
        (&*addr_of!(SOUND_ARCHIVE))
            .as_ref()
            .and_then(|a| a.clip_bytes(id as usize))
            .map(|b| b.as_ptr())
            .unwrap_or(std::ptr::null())
    }
}


/// Byte length of clip `id`, or 0.
#[no_mangle]
pub extern "C" fn debug_sound_clip_len(id: u32) -> u32 {
    if !ensure_sound_archive() {
        return 0;
    }
    unsafe {
        (&*addr_of!(SOUND_ARCHIVE))
            .as_ref()
            .and_then(|a| a.clip_bytes(id as usize))
            .map(|b| b.len() as u32)
            .unwrap_or(0)
    }
}


/// Base playback sample rate of the archive (22050 Hz).
#[no_mangle]
pub extern "C" fn debug_sound_rate(_id: u32) -> u32 {
    crate::sound::SAMPLE_RATES[0]
}


// --------------------------------------------------------------------------
// Sound playback queue (requested ids, drained by the browser shell)
// --------------------------------------------------------------------------


/// Number of sound ids requested by the active game this frame.
#[no_mangle]
pub extern "C" fn sound_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.sounds.len() as u32)
        .unwrap_or(0)
}


/// Requested sound id `i`.
#[no_mangle]
pub extern "C" fn sound_id(i: u32) -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .and_then(|g| g.sounds.get(i as usize))
        .map(|&id| id as u32)
        .unwrap_or(0)
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

// --------------------------------------------------------------------------
// Debug: party (recruited companions) — diagnostic only
// --------------------------------------------------------------------------


/// Number of recruited companions (party slots `1..`), or 0 when no game.
#[no_mangle]
pub extern "C" fn debug_party_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.party.len() as u32)
        .unwrap_or(0)
}

/// Source `Entity` id of recruited companion `i`, or `0xFFFFFFFF`.
#[no_mangle]
pub extern "C" fn debug_party_member_id(i: u32) -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .and_then(|g| g.party.get(i as usize))
        .map(|m| m.entity.index as u32)
        .unwrap_or(u32::MAX)
}


