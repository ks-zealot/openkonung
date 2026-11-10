//! Player, cursor, ground-marker and hover-text diagnostics.
//!
//! These exports expose the live [`PlayerCharacter`] and the derived runtime
//! state the smokes assert on: the drawn cursor frame (`FUN_00428740`
//! precedence), the active ground WorldObject markers and the lower-panel hover
//! string (`FUN_0042BD8C`).

use super::super::*;

/// Retained UTF-8 dump of the last [`debug_player_state`] report.
///
/// `None` until the first successful call; replaced on every later one.
pub static mut DEBUG_PLAYER_STATE: Option<Vec<u8>> = None;

/// Format the active game's [`PlayerCharacter::debug_block`] into a static
/// buffer. Returns 1 when a game exists, 0 otherwise. Diagnostic only.
///
/// # Memory contract
///
/// No pointer arguments. On success the report is owned by the debug buffer and
/// read with [`debug_player_state_ptr`]/[`debug_player_state_len`]; on failure
/// the previous contents stay.
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

/// Start of the last [`debug_player_state`] report.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by the debug buffer; never free it. Null before the
/// first successful report and not NUL terminated, so always read the length
/// from [`debug_player_state_len`].
#[no_mangle]
pub extern "C" fn debug_player_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Byte length of the last [`debug_player_state`] report, or 0.
#[no_mangle]
pub extern "C" fn debug_player_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

/// `Entity+0x03` status flags (battle stance `0x04`, AUTO/auto-XP `0x02`).
#[no_mangle]
pub extern "C" fn debug_player_flags_b() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.player.flags_b as u32)
        .unwrap_or(0)
}

/// Active weapon slot (`Entity+0x12`: 0 axe, 1 bow, 2 sword).
#[no_mangle]
pub extern "C" fn debug_player_weapon_slot() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.player.raw_0x12 as u32)
        .unwrap_or(0)
}

/// 1 while the quest log (`INTR` mode 2) is open.
#[no_mangle]
pub extern "C" fn debug_quest_open() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.quest_log_open as u32)
        .unwrap_or(0)
}

/// 1 while the protagonist has an active walk segment/route.
#[no_mangle]
pub extern "C" fn debug_player_active() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.debug_player_active() as u32)
        .unwrap_or(0)
}

/// Current game-drawn cursor frame (`0..11`); diagnostic for the HUD/ground
/// cursor split.
#[no_mangle]
pub extern "C" fn debug_cursor_index() -> u32 {
    runtime_mut()
        .game
        .as_deref_mut()
        .map(|g| g.debug_cursor_frame() as u32)
        .unwrap_or(0)
}

/// Number of active ground WorldObject markers on the current map.
#[no_mangle]
pub extern "C" fn debug_world_object_count() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.debug_active_world_object_count() as u32)
        .unwrap_or(0)
}

/// Runtime cell of active ground marker `index` (`x | y << 16`), or
/// `0xFFFF_FFFF` when out of range.
#[no_mangle]
pub extern "C" fn debug_world_object_cell(index: u32) -> u32 {
    let Some(g) = runtime_mut().game.as_deref() else {
        return 0xFFFF_FFFF;
    };
    match g.debug_active_world_object_cell(index as usize) {
        Some((x, y)) => ((x as u32) & 0xFFFF) | (((y as u32) & 0xFFFF) << 16),
        None => 0xFFFF_FFFF,
    }
}

/// Virtual screen point of runtime cell `(x, y)` under the current pan
/// (`x << 16 | y`). Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_cell_screen(x: i32, y: i32) -> u32 {
    let Some(g) = runtime_mut().game.as_deref() else {
        return 0;
    };
    let (sx, sy) = g.debug_cell_screen_point(x, y);
    (((sx as u32) & 0xFFFF) << 16) | ((sy as u32) & 0xFFFF)
}

/// Item instance under the character-screen cursor, or 0.
#[no_mangle]
pub extern "C" fn debug_hover_item() -> u32 {
    runtime_mut()
        .game
        .as_deref()
        .map(|g| g.debug_hover_item() as u32)
        .unwrap_or(0)
}

/// Retained UTF-8 of the current lower-panel hover string.
///
/// Cleared by [`debug_hover_text`] when nothing is hovered. Read through
/// [`debug_hover_text_ptr`]/[`debug_hover_text_len`].
pub static mut DEBUG_HOVER_TEXT: Option<Vec<u8>> = None;

/// Current lower-panel hover text (`FUN_00430958` / `FUN_0042F5F8` /
/// `FUN_0042E9CC`). Returns 1 and stores the UTF-8 bytes, or 0 when nothing is
/// hovered.
///
/// # Memory contract
///
/// No pointer arguments. On 1 the text is owned by the debug buffer and read
/// with [`debug_hover_text_ptr`]/[`debug_hover_text_len`]; on 0 the buffer is
/// cleared, so a stale string can never be read.
#[no_mangle]
pub extern "C" fn debug_hover_text() -> u32 {
    let text = runtime_mut()
        .game
        .as_deref_mut()
        .and_then(|g| g.debug_hover_text());
    match text {
        Some(s) => {
            unsafe {
                *addr_of_mut!(DEBUG_HOVER_TEXT) = Some(s.into_bytes());
            }
            1
        }
        None => {
            unsafe {
                *addr_of_mut!(DEBUG_HOVER_TEXT) = None;
            }
            0
        }
    }
}

/// Start of the last [`debug_hover_text`] string.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by the debug buffer; never free it. Null when nothing is
/// hovered; not NUL terminated, so always pair it with
/// [`debug_hover_text_len`].
#[no_mangle]
pub extern "C" fn debug_hover_text_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_HOVER_TEXT) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Byte length of the last [`debug_hover_text`] string, or 0.
#[no_mangle]
pub extern "C" fn debug_hover_text_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_HOVER_TEXT) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

/// Test-only: place the protagonist on a runtime cell (path/camera reset).
///
/// Returns 1 when a game exists, 0 otherwise.
#[no_mangle]
pub extern "C" fn debug_set_player_cell(x: i32, y: i32) -> u32 {
    let Some(g) = runtime_mut().game.as_deref_mut() else {
        return 0;
    };
    g.debug_set_player_cell(x, y);
    1
}

/// Protagonist attack target: world entity id, -2 for a non-world target, -1
/// for none.
#[no_mangle]
pub extern "C" fn debug_player_target() -> i32 {
    match runtime_mut().game.as_deref() {
        Some(g) => match g.player_attack_target {
            Some(crate::combat::CombatTarget::World(id)) => id as i32,
            Some(_) => -2,
            None => -1,
        },
        None => -1,
    }
}

/// Test-only: restore the protagonist to `hp` and clear the dead flag.
///
/// Values above `u16::MAX` are clamped (the internal HP field is 16-bit).
/// Returns 1 when a game exists.
#[no_mangle]
pub extern "C" fn debug_set_player_hp(hp: u32) -> u32 {
    let Some(g) = runtime_mut().game.as_deref_mut() else {
        return 0;
    };
    g.player.hp_internal = hp.min(u16::MAX as u32) as u16;
    g.player.dead = false;
    1
}

/// Test-only: clear every actor's combat target and active attack cycle.
/// Returns 1 when a game exists.
#[no_mangle]
pub extern "C" fn debug_clear_combat_targets() -> u32 {
    let Some(g) = runtime_mut().game.as_deref_mut() else {
        return 0;
    };
    g.debug_clear_combat_targets();
    1
}
