//! Original Konung gameplay HUD composition.
//!
//! Draws the original interface over the already-rendered world using the INTR
//! sprites decoded by [`crate::intr`]. This module is **layout + compositing
//! only**: it does not implement the character screen, inventory binding,
//! equipment, portrait clicks, or any input handling.
//!
//! Recovered layout (**BINARY-LAYOUT VERIFIED** against the original game
//! executable `konung.res`, VA references are into its `BEGTEXT` section):
//!
//! - The whole-screen overlay is `INTR[3]` (800x600 / 640x480) drawn at `(0,0)`
//!   (broken portrait frames, party panel and bottom bar are baked into it).
//! - Inventory strip region and grid (`0x81D738`/`0x81D744`/`0x81D760` set at
//!   file offsets `0x17C80`/`0x17D46`; layout at VA `0x42C4EE`-`0x42C544`):
//!   `left=140/112`, `right=799/639`, strip bottom `555/444`, slot cell `70x70`,
//!   `step = slot.width - 1 = 69`, columns centered with side margins:
//!   800 -> 7 slots at x origin 228, top 486; 640 -> 5 slots at x origin 203,
//!   top 375.
//! - The party panel is a **2 columns x 5 rows = 10** cell grid measured from
//!   the decoded `INTR[3]` (each visible "wide frame" is two cells sharing a
//!   single 1px middle border).
//!
//! Per-frame order: `compose` draws the `INTR[3]` overlay, the seven-icon strip,
//! the arrow caps, the empty slot backgrounds, the item icons, the AUTO button
//! and finally each portrait with its selection marker and HP bar;
//! `draw_game_hud` resolves the inventory icons and class-12 stack badges around
//! that pass, and `draw_hover_text` paints the lower-panel description over the
//! bottom bar.

use crate::res::{blit, Img, Res};

#[cfg(test)]
mod tests;

mod buttons;
mod buttons_data;
mod compose;
mod layout;

pub use self::buttons::*;
pub use self::compose::*;
pub use self::layout::*;
