//! Original startup / main menu (`MENU{width}R.RES`).
//!
//! The six original buttons are declared in [`super::layout::MAIN_MENU`] with
//! their exact recovered rectangles. On a fresh boot there is no live game, so
//! the original menu loop begins at item index `2` (task §13); the startup menu
//! therefore exposes only `Load / New Game / Settings / Exit`. `Resume` and
//! `Save` are omitted rather than given an invented disabled style.
//!
//! `New Game` is action index `3` and is the only action implemented in this
//! milestone; `Exit` requests a quit. `Load`/`Settings` are explicit no-ops
//! pending their original screens.

use super::layout::{FrontendRes, MainMenuAction, MAIN_MENU};

/// The buttons visible given the live-game flag, in original table order.
/// Kept as indices into [`MAIN_MENU`] so callers can index rects/images.
pub fn visible_indices(game_active: bool) -> Vec<usize> {
    MAIN_MENU
        .iter()
        .enumerate()
        .filter(|(i, _)| game_active || *i >= MainMenuAction::Load.index())
        .map(|(i, _)| i)
        .collect()
}

/// Hit-test only the visible buttons, returning the [`MAIN_MENU`] index of the
/// first match (original table order).
pub fn hit_test(res: FrontendRes, game_active: bool, mx: i32, my: i32) -> Option<usize> {
    visible_indices(game_active)
        .into_iter()
        .find(|&i| res.main_rect(&MAIN_MENU[i]).contains(mx, my))
}
