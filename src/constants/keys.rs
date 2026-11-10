//! Centralised Win32 virtual-key codes used by [`crate::app`] input handling.
//!
//! These are the `VK_*` values the original `WM_KEYDOWN` dispatch compares
//! against; the browser shell maps physical `event.code` to the same numbers
//! (see `www/keymap.js`).

/// `VK_ESCAPE` (27): closes the open window, or opens the menu in gameplay.
pub const KEY_ESCAPE: u32 = 27;
/// `VK_BACK` (8): switches the detail view into the GRAPH/terrain debug mode.
pub const KEY_BACKSPACE: u32 = 8;
/// `VK_TAB` (9): remake alias for the stance toggle, paired with `KEY_SPACE`.
pub const KEY_TAB: u32 = 9;
/// `VK_SPACE` (32): original stance command (`FUN_00421554`) over the selection.
pub const KEY_SPACE: u32 = 32;
/// `VK_HOME` (36): camera fit to the current map (`fit_detail`).
pub const KEY_HOME: u32 = 36;
/// `VK_1` (49): select weapon slot 2 (`FUN_004216DC`).
pub const KEY_1: u32 = 49;
/// `VK_2` (50): select weapon slot 0.
pub const KEY_2: u32 = 50;
/// `VK_3` (51): select weapon slot 1.
pub const KEY_3: u32 = 51;
/// `VK_A` (65): party engage (`FUN_00421458`); Shift+A selects all party units.
pub const KEY_A: u32 = 65;
/// `VK_C` (67): remake alias for the character screen (`FUN_00424330`).
pub const KEY_C: u32 = 67;
/// `VK_D` (68): toggles the debug entity-id overlay.
pub const KEY_D: u32 = 68;
/// `VK_F` (70): party halt (`FUN_0042246C`).
pub const KEY_F: u32 = 70;
/// `VK_H` (72): toggles the GRAPH/terrain debug detail mode.
pub const KEY_H: u32 = 72;
/// `VK_I` (73): character screen (original `FUN_00424330` binding).
pub const KEY_I: u32 = 73;
/// `VK_K` (75): frontend debug hitbox overlay toggle.
pub const KEY_K: u32 = 75;
/// `VK_L` (76): toggles the layer debug overlay.
pub const KEY_L: u32 = 76;
/// `VK_M` (77): world map (`FUN_0042CBD0(4)`).
pub const KEY_M: u32 = 77;
/// `VK_P` (80): toggles the population debug overlay.
pub const KEY_P: u32 = 80;
/// `VK_Q` (81): quest log (`FUN_004242BC`, mode 2).
pub const KEY_Q: u32 = 81;
/// `VK_R` (82): drops the world's cached decoded surfaces.
pub const KEY_R: u32 = 82;
/// `VK_T` (84): toggles the teleport debug overlay.
pub const KEY_T: u32 = 84;
/// `VK_V` (86): toggles camera follow.
pub const KEY_V: u32 = 86;
