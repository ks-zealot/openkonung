//! Mouse and keyboard ABI: host events in, original command codes out.
//!
//! `www/app.js` translates DOM events into these calls. Coordinates are in the
//! active runtime's logical pixels (the `frame_w`/`frame_h` space, not the
//! canvas backing store), so the shell scales before calling. Button numbers are
//! remapped by the shell to the original's 1-based ids: `1` left, `2` middle
//! (drag/camera), `3` right - the original's `_DAT_0081D5B0 & 1` / `& 2` split
//! (`FUN_0042260C`), where left moves/issues commands and right uses an item or
//! opens dialogue/attacks. Key codes are the original Win32 virtual-key values
//! mapped from `event.code` by `www/keymap.js`, so Cyrillic layouts keep the
//! same bindings.
//!
//! Events are ignored while a movie is pending; the shell owns input then.

use super::*;

/// Press a mouse button at a logical screen point.
///
/// The frontend may return a [`FrontendCommand`] (New Game, Resume, Quit), which
/// is resolved after the runtime borrow ends so the active mode can switch. In
/// game mode the event is dispatched to the world/HUD hit tests and may queue a
/// frontend open (Escape) or a sound, but never returns a command.
#[no_mangle]
pub extern "C" fn pointer_down(button: u32, x: f32, y: f32) {
    if crate::video::is_pending() {
        return;
    }
    let action = {
        let rt = runtime_mut();
        match rt.active {
            ActiveMode::Frontend => match rt.frontend.as_mut() {
                Some(f) => f
                    .pointer_down(button, x as f64, y as f64)
                    .map(InputAction::Command)
                    .unwrap_or(InputAction::None),
                None => InputAction::None,
            },
            ActiveMode::Game => {
                if let Some(g) = rt.game.as_deref_mut() {
                    g.pointer_down(button, x as f64, y as f64);
                }
                InputAction::None
            }
        }
    };
    resolve_input(action);
    apply_pending_frontend_resolution();
}

/// Release a mouse button; ends a drag in game mode and clears frontend
/// press state. `(x, y)` are accepted for ABI symmetry and unused.
#[no_mangle]
pub extern "C" fn pointer_up(button: u32, x: f32, y: f32) {
    let _ = (x, y);
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.pointer_up(button);
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.pointer_up(button);
            }
        }
    }
}

/// Move the pointer to a logical screen point (hover highlighting, cursor
/// frame selection, held-item drag preview).
#[no_mangle]
pub extern "C" fn pointer_move(x: f32, y: f32) {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.pointer_move(x as f64, y as f64);
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.pointer_move(x as f64, y as f64);
            }
        }
    }
}

/// Additive party-selection modifier (`Event.shiftKey`); mirrors the original
/// `_DAT_0081D624` flag read by `FUN_00424D8C`. Ignored in the frontend.
#[no_mangle]
pub extern "C" fn set_shift(down: u32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.set_shift(down != 0);
        }
    }
}

/// Force-talk modifier (`Event.ctrlKey`); mirrors the original `_DAT_0081D5E4`
/// flag read by the cursor selector and the world-click dialogue branch.
/// Ignored in the frontend.
#[no_mangle]
pub extern "C" fn set_ctrl(down: u32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.set_ctrl(down != 0);
        }
    }
}

/// Mouse-wheel delta in notches (positive = away from the user); forwarded to
/// the game's inventory/trade row paging. Ignored in the frontend.
#[no_mangle]
pub extern "C" fn wheel(delta: f32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.wheel(delta as f64);
        }
    }
}

/// Press an original Win32 virtual-key code.
///
/// The game's Escape handler does not switch mode directly: it sets
/// `request_menu`, which is turned into [`InputAction::OpenMenu`] here so the
/// frontend can be built after the runtime borrow ends. The frontend may return
/// a [`FrontendCommand`]; both outcomes are resolved below.
#[no_mangle]
pub extern "C" fn key_down(code: u32) {
    if crate::video::is_pending() {
        return;
    }
    let action = {
        let rt = runtime_mut();
        match rt.active {
            ActiveMode::Frontend => match rt.frontend.as_mut() {
                Some(f) => f
                    .key_down(code)
                    .map(InputAction::Command)
                    .unwrap_or(InputAction::None),
                None => InputAction::None,
            },
            ActiveMode::Game => {
                let mut open = false;
                if let Some(g) = rt.game.as_deref_mut() {
                    g.key_down(code);
                    if g.request_menu {
                        g.request_menu = false;
                        open = true;
                    }
                }
                if open {
                    InputAction::OpenMenu
                } else {
                    InputAction::None
                }
            }
        }
    };
    resolve_input(action);
    apply_pending_frontend_resolution();
}
