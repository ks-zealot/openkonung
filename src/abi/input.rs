//! `input` subsystem of [`crate::lib`] (split from `lib.rs`).

use super::*;

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
/// `_DAT_0081D624` flag read by `FUN_00424D8C`.
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
#[no_mangle]
pub extern "C" fn set_ctrl(down: u32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.set_ctrl(down != 0);
        }
    }
}

#[no_mangle]
pub extern "C" fn wheel(delta: f32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.wheel(delta as f64);
        }
    }
}

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
