//! Trade exchange window (`FUN_00430D6C`, interface mode 7).
//!
//! Geometry is taken from the original tables:
//! * viewport `0x81D738/0x81D754/0x81D744/0x81D760` (640: 112,0..639,444;
//!   800: 140,0..799,555);
//! * four row rectangles `DAT_00465E94`;
//! * buttons `DAT_00465E34` (right = cancel, left = confirm);
//! * money labels `DAT_00465F34/0x465F54`.
//!
//! Draw order: the `INTR[5]` stone tile across the viewport, the `INTR[6]`
//! window frame, then per row the left arrow, five `INTR[17]` cells with their
//! item icons and the right arrow, then the cancel/confirm buttons, the six
//! money values, the partner name, an optional message or the stock-empty
//! notice, and finally the hovered item tooltip. Everything is clipped to the
//! viewport rectangle.

use crate::res::{blit, Res};
use crate::trade::{TradeState, SLOTS, VISIBLE};

/// `INTR[6]`: the trade window frame.
pub const WINDOW_SPRITE: usize = 6;
/// `INTR[5]`: the 100x100 stone tile that fills the viewport behind the window.
pub const TILE_SPRITE: usize = 5;
/// `INTR[17]`: empty slot-cell background for the 32-slot rows.
pub const SLOT_SPRITE: usize = 17;
/// `INTR[18]`: left page arrow while the row is on page 0.
pub const ARROW_LEFT: usize = 18;
/// `INTR[19]`: left page arrow when the row is paged forward.
pub const ARROW_LEFT_ACTIVE: usize = 19;
/// `INTR[20]`: right page arrow when no further cell follows.
pub const ARROW_RIGHT: usize = 20;
/// `INTR[21]`: right page arrow when more cells follow.
pub const ARROW_RIGHT_ACTIVE: usize = 21;
/// `INTR[180]`: cancel button.
pub const CANCEL_SPRITE: usize = 180;
/// `INTR[182]`: confirm (complete the deal) button.
pub const CONFIRM_SPRITE: usize = 182;
/// Stone tile edge in px (`INTR[5]` is 100x100).
const TILE: i32 = 100;
/// Cell step, the same `slot_width - 1` rule as the inventory strip.
const CELL_STEP: i32 = 69;
/// Item-cell edge in px.
const CELL: i32 = 70;
/// Page-arrow width in px (`INTR[18]`/`INTR[20]` are 20 wide).
const ARROW_W: i32 = 20;

/// One hit-test result of the trade window.
///
/// The four rows are the four 32-slot arrays in `0x81CB24` order: player bag,
/// player offer, partner stock, partner offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeHit {
    /// One visible item cell of a row.
    Cell {
        /// Row index `0..=3` (player bag, player offer, partner stock, partner
        /// offer).
        row: usize,
        /// Visible column `0..=4` inside the row's current page.
        col: usize,
    },
    /// The row's left paging arrow.
    PageLeft(usize),
    /// The row's right paging arrow.
    PageRight(usize),
    /// Cancel button `INTR[180]`.
    Cancel,
    /// Confirm button `INTR[182]`.
    Confirm,
    /// The point is not on any trade control.
    None,
}

/// Resolved trade-window geometry for one logical resolution.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Interface viewport `(x, y, w, h)` (`0x81D738/0x81D754/0x81D744/0x81D760`).
    pub viewport: (i32, i32, i32, i32),
    /// `INTR[6]` window top-left, i.e. the viewport origin.
    pub window_pos: (i32, i32),
    /// The four row rectangles `(x, y, w, h)` from `DAT_00465E94`, top to bottom.
    pub rows: [(i32, i32, i32, i32); 4],
    /// Cancel button rectangle (right record of `DAT_00465E34`).
    pub cancel: (i32, i32, i32, i32),
    /// Confirm button rectangle (left record of `DAT_00465E34`).
    pub confirm: (i32, i32, i32, i32),
    /// Six money-value origins (`DAT_00465F34`/`0x465F54`), in the order the
    /// draw pass fills them: partner cash, partner money on the table, partner
    /// side total, player money on the table, player cash, player side total.
    pub labels: [(i32, i32); 6],
    /// Baseline y of the partner name. Both layouts put the viewport top at
    /// y 0, so this is also an absolute screen row.
    ///
    /// TODO(docs): provenance of the 18 / 14 values not recovered.
    pub name_y: i32,
}

/// Build the trade-window layout for a logical width (`>= 800` picks the 800
/// variant, anything narrower the 640 one).
pub fn layout(logical_w: i32) -> Layout {
    if logical_w >= 800 {
        Layout {
            viewport: (140, 0, 660, 555),
            window_pos: (140, 0),
            rows: [
                (288, 50, 506, 60),
                (288, 150, 506, 60),
                (288, 358, 506, 60),
                (288, 438, 506, 60),
            ],
            cancel: (537, 260, 93, 40),
            confirm: (427, 260, 93, 40),
            labels: [
                (205, 94),
                (216, 202),
                (216, 261),
                (214, 344),
                (206, 448),
                (236, 468),
            ],
            name_y: 18,
        }
    } else {
        Layout {
            viewport: (112, 0, 528, 444),
            window_pos: (112, 0),
            rows: [
                (230, 40, 405, 60),
                (230, 120, 405, 60),
                (230, 275, 405, 60),
                (230, 350, 405, 60),
            ],
            cancel: (457, 215, 93, 40),
            confirm: (347, 215, 93, 40),
            labels: [
                (162, 76),
                (172, 156),
                (174, 214),
                (170, 297),
                (162, 382),
                (192, 395),
            ],
            name_y: 14,
        }
    }
}

/// Half-open point-in-rectangle test for a `(x, y, w, h)` tuple.
fn contains(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

/// Map a screen point to a trade-window control.
///
/// The two buttons are tested first (they sit between rows 1 and 2), then each
/// row splits its width into left arrow, five 69 px cells and right arrow, so a
/// click is always attributed to exactly one row.
pub fn hit_test(x: i32, y: i32, logical_w: i32) -> TradeHit {
    let l = layout(logical_w);
    if contains(l.cancel, x, y) {
        return TradeHit::Cancel;
    }
    if contains(l.confirm, x, y) {
        return TradeHit::Confirm;
    }
    for (row, r) in l.rows.iter().enumerate() {
        if y < r.1 || y >= r.1 + r.3 {
            continue;
        }
        let left = r.0;
        if x >= left && x < left + ARROW_W {
            return TradeHit::PageLeft(row);
        }
        let cells = left + ARROW_W;
        for col in 0..VISIBLE {
            let cx = cells + col as i32 * CELL_STEP;
            if x >= cx && x < cx + CELL_STEP {
                return TradeHit::Cell { row, col };
            }
        }
        let right = cells + VISIBLE as i32 * CELL_STEP;
        if x >= right && x < r.0 + r.2 {
            return TradeHit::PageRight(row);
        }
    }
    TradeHit::None
}

/// Blit one sprite clipped to `clip` and to the framebuffer, leaving
/// transparent source pixels untouched.
fn blit_clip(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    img: &crate::res::Img,
    x: i32,
    y: i32,
    clip: (i32, i32, i32, i32),
) {
    let (cx, cy, cw, ch) = clip;
    let x0 = x.max(cx).max(0);
    let y0 = y.max(cy).max(0);
    let x1 = (x + img.w as i32).min(cx + cw).min(dw as i32);
    let y1 = (y + img.h as i32).min(cy + ch).min(dh as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let iw = img.w as usize;
    for yy in y0..y1 {
        for xx in x0..x1 {
            let si = ((yy - y) as usize * iw + (xx - x) as usize) * 4;
            if img.data[si + 3] == 0 {
                continue;
            }
            let di = (yy as usize * dw + xx as usize) * 4;
            dst[di..di + 4].copy_from_slice(&img.data[si..si + 4]);
        }
    }
}

/// Draw the INTR icon of item instance `id` centered in the 70x70 cell whose
/// top-left is `(cx, cy)`; an unresolved item or icon draws nothing.
fn draw_icon(res: &mut Res, frame: &mut [u8], fw: usize, fh: usize, id: u16, cx: i32, cy: i32) {
    let Some(info) = res.item_info(id) else {
        return;
    };
    let Some(icon) = res.intr_sprite(info.icon as usize) else {
        return;
    };
    let x = cx + (CELL - icon.w as i32) / 2;
    let y = cy + (CELL - icon.h as i32) / 2;
    blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y);
}

/// Whether a row can page right: the first off-page cell exists and is not
/// empty (the same condition `TradeState::page_row` uses).
fn can_right(state: &TradeState, row: usize) -> bool {
    state.page[row] + VISIBLE < SLOTS && state.rows[row][state.page[row] + VISIBLE] != 0
}

/// Draw the trade window over the frame.
pub fn draw(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    state: &TradeState,
    npc_name: &str,
    message: Option<&str>,
    cursor: (i32, i32),
) {
    let l = layout(fw as i32);
    let (vx, vy, vw, vh) = l.viewport;
    if let Some(tile) = res.intr_sprite(TILE_SPRITE) {
        let cols = (vw + TILE - 1) / TILE;
        let rows = (vh + TILE - 1) / TILE;
        for ry in 0..rows {
            for cx in 0..cols {
                blit_clip(
                    frame,
                    fw,
                    fh,
                    &tile,
                    vx + cx * TILE,
                    vy + ry * TILE,
                    (vx, vy, vw, vh),
                );
            }
        }
    }
    if let Some(w) = res.intr_sprite(WINDOW_SPRITE) {
        blit_clip(
            frame,
            fw,
            fh,
            &w,
            l.window_pos.0,
            l.window_pos.1,
            (vx, vy, vw, vh),
        );
    }

    for (row, rect) in l.rows.iter().enumerate() {
        let (rx, ry, _rw, rh) = *rect;
        let left_sprite = if state.page[row] > 0 {
            ARROW_LEFT_ACTIVE
        } else {
            ARROW_LEFT
        };
        let right_sprite = if can_right(state, row) {
            ARROW_RIGHT_ACTIVE
        } else {
            ARROW_RIGHT
        };
        if let Some(a) = res.intr_sprite(left_sprite) {
            blit_clip(frame, fw, fh, &a, rx, ry, (vx, vy, vw, vh));
        }
        let cells_x = rx + ARROW_W;
        for col in 0..VISIBLE {
            let cx = cells_x + col as i32 * CELL_STEP;
            if let Some(slot) = res.intr_sprite(SLOT_SPRITE) {
                blit_clip(frame, fw, fh, &slot, cx, ry, (vx, vy, vw, vh));
            }
            let id = state.visible_cell(row, col);
            if id != 0 {
                draw_icon(res, frame, fw, fh, id, cx, ry);
            }
        }
        if let Some(a) = res.intr_sprite(right_sprite) {
            let x = cells_x + VISIBLE as i32 * CELL_STEP;
            blit_clip(frame, fw, fh, &a, x, ry, (vx, vy, vw, vh));
        }
        let _ = rh;
    }

    if let Some(b) = res.intr_sprite(CANCEL_SPRITE) {
        blit_clip(frame, fw, fh, &b, l.cancel.0, l.cancel.1, (vx, vy, vw, vh));
    }
    if let Some(b) = res.intr_sprite(CONFIRM_SPRITE) {
        blit_clip(
            frame,
            fw,
            fh,
            &b,
            l.confirm.0,
            l.confirm.1,
            (vx, vy, vw, vh),
        );
    }

    let small = res.fonts.small.as_ref();
    let name_font = res.fonts.large.as_ref().or(small);
    let partner_offer = state.partner_offer_value(|id| res.item_trade_value(id).unwrap_or(0));
    let player_offer = state.player_offer_value(|id| res.item_trade_value(id).unwrap_or(0));
    let values = [
        state.partner_money,
        state.table_money[1],
        partner_offer + state.table_money[1],
        state.table_money[0],
        state.player_money,
        player_offer + state.table_money[0],
    ];
    if let Some(font) = small {
        for (i, &(x, y)) in l.labels.iter().enumerate() {
            font.draw(frame, fw, fh, &values[i].to_string(), x, y, None);
        }
        if let Some(name_font) = name_font {
            let name_w = name_font.measure(npc_name);
            name_font.draw(
                frame,
                fw,
                fh,
                npc_name,
                vx + (vw - name_w) / 2,
                l.name_y,
                None,
            );
        }
        if let Some(msg) = message {
            let w = font.measure(msg);
            font.draw(
                frame,
                fw,
                fh,
                msg,
                vx + (vw - w) / 2,
                vy + vh / 2 - 8,
                Some([255, 210, 90]),
            );
        }
        let stock_empty = (0..VISIBLE).all(|c| state.visible_cell(2, c) == 0);
        if stock_empty {
            let msg = if state.role == 3 {
                "Купец покупает товары, но не продаёт"
            } else {
                "Нет товаров на продажу"
            };
            let w = font.measure(msg);
            font.draw(
                frame,
                fw,
                fh,
                msg,
                vx + (vw - w) / 2,
                l.name_y + 24,
                Some([210, 200, 160]),
            );
        }
    }

    if cursor.0 >= vx && cursor.0 < vx + vw && cursor.1 >= 0 && cursor.1 < vh {
        if let TradeHit::Cell { row, col } = hit_test(cursor.0, cursor.1, fw as i32) {
            let id = state.visible_cell(row, col);
            if id != 0 {
                crate::app::tooltip::draw_item_tooltip(
                    frame,
                    fw,
                    fh,
                    res,
                    id,
                    cursor,
                    (vx, vy, vw, vh),
                );
            }
        }
    }
}
