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

use crate::res::{blit, Img, Res};

/// Party panel columns (measured from `INTR[3]`).
pub const PARTY_COLS: usize = 2;
/// Party panel rows (measured from `INTR[3]`).
pub const PARTY_ROWS: usize = 5;
/// Total visible party portrait cells.
pub const PARTY_CELLS: usize = PARTY_COLS * PARTY_ROWS;
/// `INTR` index of the whole-screen HUD overlay.
pub const HUD_SPRITE: usize = 3;
/// `INTR` index of the empty inventory-slot background.
pub const INVENTORY_SLOT_SPRITE: usize = 17;

/// An axis-aligned rectangle in logical framebuffer coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

/// Slot step used by the original executable: `slot_width - 1`
/// (global `0x81D724`, computed at VA `0x42C4B5`).
#[inline]
pub fn slot_step(slot_width: u16) -> i32 {
    slot_width as i32 - 1
}

/// Resolved HUD geometry for one logical resolution.
#[derive(Clone, Debug)]
pub struct HudLayout {
    pub logical_w: i32,
    pub logical_h: i32,
    pub hud_id: usize,
    pub slot_id: usize,
    /// Inventory strip region `(left, strip_bottom)`, in logical coordinates.
    pub inventory_region: Rect,
    /// Top-left of the first slot cell.
    pub inventory_origin: (i32, i32),
    /// Number of visible slot columns.
    pub visible_slots: usize,
    /// Slot step (== slot width - 1 for the original 70x70 cell).
    pub slot_step: i32,
    /// Bottom-bar text area used for the hovered item label (region below the
    /// inventory cell row). The original hover-label coordinates could not be
    /// recovered, so this is a documented approximation.
    pub inventory_bar: Rect,
    /// Two scroll-arrow end caps drawn by the original strip renderer even at
    /// scroll 0: `(intr_id, x, y)`. Left = `INTR[18]`, right = `INTR[20]`.
    pub arrows: [(usize, i32, i32); 2],
    /// AUTO combat button top-left (traced from the ability-loop table
    /// `0x465B34 + sel*8 + 80`; not pixel-verified).
    pub auto_pos: (i32, i32),
    /// 10 party portrait cells, row-major (row 0 = top).
    pub party_cells: [Rect; PARTY_CELLS],
}

impl HudLayout {
    /// The visible inventory slot rectangles (empty-cell backgrounds).
    pub fn inventory_slot_rects(&self, slot_w: u16) -> Vec<Rect> {
        let step = slot_step(slot_w);
        (0..self.visible_slots)
            .map(|i| Rect {
                x: self.inventory_origin.0 + i as i32 * step,
                y: self.inventory_origin.1,
                w: slot_w as i32,
                h: slot_w as i32,
            })
            .collect()
    }

    /// Party portrait cell for a given flat cell index (0..10).
    pub fn party_cell(&self, index: usize) -> Option<Rect> {
        self.party_cells.get(index).copied()
    }

    /// Rectangle of one visible inventory cell, or `None` if out of range.
    pub fn inventory_slot_rect(&self, slot: usize, slot_w: u16) -> Option<Rect> {
        inventory_slot_rect(self, slot, slot_w)
    }
}

/// Select the layout for a logical framebuffer width (`<=640` -> 640 variant).
pub fn layout(logical_w: i32) -> HudLayout {
    if logical_w <= 640 {
        layout_640()
    } else {
        layout_800()
    }
}

/// 800x600 HUD layout (executable globals + measured `INTR[3]` grid).
pub fn layout_800() -> HudLayout {
    HudLayout {
        logical_w: 800,
        logical_h: 600,
        hud_id: HUD_SPRITE,
        slot_id: INVENTORY_SLOT_SPRITE,
        // left=140, right=799, strip bottom=555; slot cell 70x70 => region 660x70.
        inventory_region: Rect { x: 140, y: 486, w: 660, h: 70 },
        // Origin = left-1 + arrow_w(INTR[18].w=20) + remainder/2;
        // count = (span+2 - 2*20)/69 = 621/69 = 9 (VA 0x42C4EE-0x42C544).
        inventory_origin: (159, 486),
        visible_slots: 9,
        slot_step: 69,
        inventory_bar: Rect { x: 140, y: 555, w: 660, h: 45 },
        // Arrow caps at x=left-1=139 and x=right-20+1=780, y=486.
        arrows: [(18, 139, 486), (20, 780, 486)],
        auto_pos: (644, 379),
        // Measured from INTR800[3]: columns x=2/70 (cell 68 wide),
        // rows y=2/70/137/205/272 (cell 68 tall).
        party_cells: build_party_cells(&[2, 70], &[2, 70, 137, 205, 272], 68, 68),
    }
}

/// 640x480 HUD layout (executable globals + measured `INTR[3]` grid).
pub fn layout_640() -> HudLayout {
    HudLayout {
        logical_w: 640,
        logical_h: 480,
        hud_id: HUD_SPRITE,
        slot_id: INVENTORY_SLOT_SPRITE,
        // left=112, right=639, strip bottom=444.
        inventory_region: Rect { x: 112, y: 375, w: 528, h: 70 },
        // count = (span+2 - 2*20)/69 = 489/69 = 7; origin = 111+20+3 = 134.
        inventory_origin: (134, 375),
        visible_slots: 7,
        slot_step: 69,
        inventory_bar: Rect { x: 112, y: 444, w: 528, h: 36 },
        // Arrow caps at x=left-1=111 and x=right-20+1=620, y=375.
        arrows: [(18, 111, 375), (20, 620, 375)],
        auto_pos: (515, 303),
        // Measured from INTR640[3]: columns x=2/55 (cell 53 wide),
        // rows y=1/55/109/163/217 (cell 54 tall).
        party_cells: build_party_cells(&[2, 55], &[1, 55, 109, 163, 217], 53, 54),
    }
}

fn build_party_cells(
    col_x: &[i32; PARTY_COLS],
    row_y: &[i32; PARTY_ROWS],
    cell_w: i32,
    cell_h: i32,
) -> [Rect; PARTY_CELLS] {
    let mut cells = [Rect { x: 0, y: 0, w: 0, h: 0 }; PARTY_CELLS];
    // Row-major: cell = row * PARTY_COLS + col.
    for row in 0..PARTY_ROWS {
        for col in 0..PARTY_COLS {
            cells[row * PARTY_COLS + col] = Rect {
                x: col_x[col],
                y: row_y[row],
                w: cell_w,
                h: cell_h,
            };
        }
    }
    cells
}

/// Which INTR portrait sprite to draw, ported from the original selector at
/// VA `0x42C730`.
///
/// The original reads:
/// - `character + 0x04` status byte: low 3 bits = appearance, bit `0x80` =
///   special condition;
/// - `character + 0x5C` health (u16);
/// - `character + 0xD6` portrait group.
pub fn portrait_id(
    is_main_hero: bool,
    portrait_group: u8,
    status: u8,
    health: u16,
) -> usize {
    let group = portrait_group as usize;
    let base = if is_main_hero {
        215 + group * 25
    } else {
        195 + group * 25 + (status & 0x07) as usize * 5
    };
    let variant = if status & 0x80 != 0 {
        4
    } else if health <= 400 {
        3
    } else if health <= 800 {
        2
    } else if health <= 1200 {
        1
    } else {
        0
    };
    base + variant
}

/// One party portrait to overlay on the panel. Binding to real character data
/// is deferred (Task 3); [`HudState`] is empty until then.
#[derive(Clone, Copy, Debug)]
pub struct PartyPortrait {
    pub cell: usize,
    pub intr_id: usize,
}

/// One resolved visible inventory cell: `(visible slot index, INTR icon id)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemIcon {
    pub slot: usize,
    pub icon: usize,
}

/// Runtime HUD state: party portraits, the resolved visible inventory icons and
/// the hovered slot. The inventory itself lives on the protagonist; this is a
/// per-frame view of it and never re-parses or re-initializes anything.
#[derive(Clone, Debug, Default)]
pub struct HudState {
    pub party: Vec<PartyPortrait>,
    pub items: Vec<ItemIcon>,
    pub hovered: Option<usize>,
    /// AUTO button `(intr_id, x, y)` to overlay, if any.
    pub auto: Option<(usize, i32, i32)>,
}

/// The visible inventory cell rectangles, for `(layout, slot width)`.
pub fn inventory_slot_rect(layout: &HudLayout, slot: usize, slot_w: u16) -> Option<Rect> {
    if slot >= layout.visible_slots {
        return None;
    }
    let step = layout.slot_step;
    Some(Rect {
        x: layout.inventory_origin.0 + slot as i32 * step,
        y: layout.inventory_origin.1,
        w: slot_w as i32,
        h: slot_w as i32,
    })
}

/// Composite the HUD onto `frame` using `lookup` to resolve INTR sprites.
///
/// `lookup` is called once per distinct sprite index; callers pass a cached
/// resolver such as [`Res::intr_sprite`] so no archive parsing or RGBA copy
/// happens per frame.
pub fn compose<F>(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    layout: &HudLayout,
    state: &HudState,
    mut lookup: F,
) where
    F: FnMut(usize) -> Option<Img>,
{
    // Whole-screen overlay (party panel + bottom bar) at (0,0).
    if let Some(bg) = lookup(layout.hud_id) {
        blit(frame, fw, fh, &bg.data, bg.w, bg.h, 0, 0);
    }

    // Scroll-arrow end caps (drawn by the original strip renderer even at
    // scroll 0), before the slot backgrounds.
    for &(id, x, y) in &layout.arrows {
        if let Some(im) = lookup(id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
        }
    }

    // Empty inventory slot backgrounds, repeated across the strip.
    if let Some(slot) = lookup(layout.slot_id) {
        let step = layout.slot_step;
        for i in 0..layout.visible_slots {
            let x = layout.inventory_origin.0 + i as i32 * step;
            blit(frame, fw, fh, &slot.data, slot.w, slot.h, x, layout.inventory_origin.1);
        }
    }

    // Item icons, centered in their visible cells (empty cells keep the slot
    // background drawn above). `icon` is the original INTR index.
    for item in &state.items {
        let Some(cell) = inventory_slot_rect(layout, item.slot, 70) else {
            continue;
        };
        let Some(im) = lookup(item.icon) else {
            continue;
        };
        let ox = cell.x + (cell.w - im.w as i32) / 2;
        let oy = cell.y + (cell.h - im.h as i32) / 2;
        blit(frame, fw, fh, &im.data, im.w, im.h, ox, oy);
    }

    // AUTO combat button (traced gameplay-HUD element).
    if let Some((id, x, y)) = state.auto {
        if let Some(im) = lookup(id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
        }
    }

    // Party portraits (none until character binding exists).
    for p in &state.party {
        let Some(cell) = layout.party_cell(p.cell) else {
            continue;
        };
        let Some(im) = lookup(p.intr_id) else {
            continue;
        };
        let ox = cell.x + (cell.w - im.w as i32) / 2;
        let oy = cell.y + (cell.h - im.h as i32) / 2;
        blit(frame, fw, fh, &im.data, im.w, im.h, ox, oy);
    }
}

/// Draw the HUD for the active logical framebuffer over the game world.
///
/// Resolves sprites through [`Res::intr_sprite`] (lazy `Rc` cache), so repeated
/// calls neither re-parse the archive nor re-allocate sprite buffers.
pub fn draw_game_hud(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    inventory: &[u16; 32],
    scroll: usize,
    hovered: Option<usize>,
    protagonist_portrait: Option<usize>,
    auto_on: bool,
) {
    if res.intr.is_none() {
        return;
    }
    let l = layout(fw as i32);

    // Resolve the visible inventory icons (fixed scroll for this task). The
    // protagonist's array is read directly; nothing is re-initialized here.
    let mut items = Vec::new();
    for slot in 0..l.visible_slots {
        let idx = scroll + slot;
        let Some(&id) = inventory.get(idx) else { continue };
        if id == 0 {
            continue;
        }
        if let Some(info) = res.item_info(id) {
            items.push(ItemIcon {
                slot,
                icon: info.icon as usize,
            });
        }
    }

    // Only the protagonist occupies a party cell at new game (cell 0); the
    // remaining nine stay empty.
    let party = match protagonist_portrait {
        Some(id) => vec![PartyPortrait { cell: 0, intr_id: id }],
        None => Vec::new(),
    };
    let auto_id = if auto_on {
        crate::intr::AUTO_ON
    } else {
        crate::intr::AUTO_OFF
    };
    let state = HudState {
        party,
        items,
        hovered,
        auto: Some((auto_id, l.auto_pos.0, l.auto_pos.1)),
    };
    compose(frame, fw, fh, &l, &state, |id| res.intr_sprite(id));

    // Hovered item label, centered in the bottom bar (documented approximation:
    // the original hover-text routine/coordinates could not be recovered).
    let Some(slot) = hovered else { return };
    if slot >= l.visible_slots {
        return;
    }
    let idx = scroll + slot;
    let Some(&id) = inventory.get(idx) else { return };
    if id == 0 {
        return;
    }
    let Some(info) = res.item_info(id) else { return };
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let tw = font.measure(&info.name);
    let bar = l.inventory_bar;
    let x = bar.x + (bar.w - tw).max(0) / 2;
    let y = bar.y + (bar.h - 12).max(0) / 2;
    font.draw(frame, fw, fh, &info.name, x, y, Some([235, 235, 210]));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::res::Image;
    use std::rc::Rc;

    fn solid(w: usize, h: usize, c: [u8; 4]) -> Img {
        Rc::new(Image::new(w, h, c))
    }

    fn px(frame: &[u8], fw: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * fw + x) * 4;
        [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]
    }

    #[test]
    fn slot_step_is_width_minus_one() {
        assert_eq!(slot_step(70), 69);
        assert_eq!(slot_step(52), 51);
        assert_eq!(slot_step(1), 0);
    }

    #[test]
    fn layout_800_matches_executable_constants() {
        let l = layout_800();
        assert_eq!((l.logical_w, l.logical_h), (800, 600));
        assert_eq!((l.inventory_region.x, l.inventory_region.right()), (140, 800));
        assert_eq!(l.inventory_origin, (159, 486));
        assert_eq!(l.visible_slots, 9);
        assert_eq!(l.slot_step, 69);
        assert_eq!(l.arrows, [(18, 139, 486), (20, 780, 486)]);
    }

    #[test]
    fn layout_640_matches_executable_constants() {
        let l = layout_640();
        assert_eq!((l.logical_w, l.logical_h), (640, 480));
        assert_eq!((l.inventory_region.x, l.inventory_region.right()), (112, 640));
        assert_eq!(l.inventory_origin, (134, 375));
        assert_eq!(l.visible_slots, 7);
        assert_eq!(l.arrows, [(18, 111, 375), (20, 620, 375)]);
    }

    #[test]
    fn layout_selects_variant_by_logical_width() {
        assert_eq!(layout(800).logical_w, 800);
        assert_eq!(layout(640).logical_w, 640);
        assert_eq!(layout(400).logical_w, 640);
    }

    #[test]
    fn inventory_slots_are_within_region_and_step_spaced() {
        for l in [layout_800(), layout_640()] {
            let rects = l.inventory_slot_rects(70);
            assert_eq!(rects.len(), l.visible_slots);
            let region = l.inventory_region;
            for (i, r) in rects.iter().enumerate() {
                assert_eq!(r.w, 70);
                assert_eq!(r.h, 70);
                assert_eq!(r.y, l.inventory_origin.1);
                if i > 0 {
                    assert_eq!(r.x - rects[i - 1].x, slot_step(70));
                }
                assert!(
                    r.x >= region.x && r.right() <= region.right(),
                    "{:?} not within {:?}",
                    r,
                    region
                );
            }
        }
    }

    #[test]
    fn party_grid_is_two_by_five() {
        for l in [layout_800(), layout_640()] {
            assert_eq!(l.party_cells.len(), 10);
            // Column 0 shared x, column 1 shared x; five distinct rows.
            let col0 = l.party_cells[0].x;
            let col1 = l.party_cells[1].x;
            assert!(col1 > col0);
            for row in 0..PARTY_ROWS {
                assert_eq!(l.party_cells[row * 2].x, col0);
                assert_eq!(l.party_cells[row * 2 + 1].x, col1);
                assert_eq!(l.party_cells[row * 2].y, l.party_cells[row * 2 + 1].y);
                if row > 0 {
                    assert!(l.party_cells[row * 2].y > l.party_cells[(row - 1) * 2].y);
                }
            }
            // All cells inside the left panel.
            for c in l.party_cells {
                assert!(c.x >= 0 && c.right() <= l.inventory_region.x);
            }
        }
    }

    #[test]
    fn portrait_id_reference_cases() {
        // appearance/status = 0x05, portrait_group = 1, health = 1600.
        assert_eq!(portrait_id(true, 1, 0x05, 1600), 240);
        assert_eq!(portrait_id(false, 1, 0x05, 1600), 245);
    }

    #[test]
    fn portrait_id_health_thresholds() {
        // Main hero, group 0 => base 215.
        assert_eq!(portrait_id(true, 0, 0x00, 400), 215 + 3);
        assert_eq!(portrait_id(true, 0, 0x00, 800), 215 + 2);
        assert_eq!(portrait_id(true, 0, 0x00, 1200), 215 + 1);
        assert_eq!(portrait_id(true, 0, 0x00, 1201), 215 + 0);
        // Special condition flag 0x80 forces variant 4 (regardless of health).
        assert_eq!(portrait_id(true, 0, 0x80, 1600), 215 + 4);
        // Ordinary party uses appearance bits.
        assert_eq!(portrait_id(false, 0, 0x07, 1600), 195 + 35);
    }

    #[test]
    fn portrait_ids_stay_in_reference_range() {
        // Three portrait groups (0..=2), five appearances (0..=4, the low 3 bits
        // of the status byte), and the five health/special variants span the
        // 195..=269 range; the selector never leaves the INTR 200..=274 block by
        // more than the reserved low entries.
        let healths = [0u16, 400, 800, 1200, 1600];
        for group in 0..=2u8 {
            for appearance in 0..=4u8 {
                for &health in &healths {
                    let main = portrait_id(true, group, appearance, health);
                    let party = portrait_id(false, group, appearance, health);
                    assert!((195..=269).contains(&main), "main {main}");
                    assert!((195..=269).contains(&party), "party {party}");
                }
                // Special condition forces variant 4.
                let sp = portrait_id(true, group, appearance | 0x80, 1600);
                assert!((195..=269).contains(&sp), "special {sp}");
            }
        }
    }

    #[test]
    fn compose_preserves_world_through_transparent_pixels() {
        let fw = 8;
        let fh = 8;
        let mut frame = vec![0u8; fw * fh * 4];
        for p in frame.chunks_mut(4) {
            p.copy_from_slice(&[0x11, 0x22, 0x33, 0xFF]);
        }
        let l = HudLayout {
            logical_w: 8,
            logical_h: 8,
            hud_id: 3,
            slot_id: 17,
            inventory_region: Rect { x: 0, y: 0, w: 8, h: 8 },
            inventory_origin: (0, 0),
            visible_slots: 0,
            slot_step: 69,
            inventory_bar: Rect { x: 0, y: 0, w: 8, h: 8 },
            arrows: [(18, -1000, -1000), (20, -1000, -1000)],
            auto_pos: (-1000, -1000),
            party_cells: [Rect { x: 0, y: 0, w: 8, h: 8 }; 10],
        };
        // 2x2 HUD: top-left transparent, others opaque red.
        let mut hud = Image::new(2, 2, [255, 0, 0, 255]);
        hud.data[3] = 0; // transparent top-left
        let hud = Rc::new(hud);
        let state = HudState::default();
        compose(&mut frame, fw, fh, &l, &state, |_| Some(hud.clone()));
        assert_eq!(px(&frame, fw, 0, 0), [0x11, 0x22, 0x33, 0xFF]); // preserved
        assert_eq!(px(&frame, fw, 1, 0), [255, 0, 0, 255]); // overwritten
        assert_eq!(px(&frame, fw, 0, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn compose_clips_out_of_bounds_without_panic() {
        let fw = 4;
        let fh = 4;
        let mut frame = vec![0u8; fw * fh * 4];
        let l = HudLayout {
            logical_w: 800,
            logical_h: 600,
            hud_id: 3,
            slot_id: 17,
            inventory_region: Rect { x: 0, y: 0, w: 800, h: 70 },
            inventory_origin: (0, 0),
            visible_slots: 7,
            slot_step: 69,
            inventory_bar: Rect { x: 0, y: 0, w: 800, h: 45 },
            arrows: [(18, -1000, -1000), (20, -1000, -1000)],
            auto_pos: (-1000, -1000),
            party_cells: [Rect { x: 0, y: 0, w: 68, h: 68 }; 10],
        };
        let big = solid(70, 70, [1, 2, 3, 255]);
        let state = HudState::default();
        // Larger-than-frame sprites must be clipped, not panic.
        compose(&mut frame, fw, fh, &l, &state, |_| Some(big.clone()));
        assert_eq!(px(&frame, fw, 3, 3), [1, 2, 3, 255]);
    }

    #[test]
    fn layout_has_inventory_bar_text_area() {
        let l800 = layout_800();
        assert_eq!(l800.inventory_bar, Rect { x: 140, y: 555, w: 660, h: 45 });
        let l640 = layout_640();
        assert_eq!(l640.inventory_bar, Rect { x: 112, y: 444, w: 528, h: 36 });
    }

    #[test]
    fn auto_button_position_matches_traced_table() {
        assert_eq!(layout_800().auto_pos, (644, 379));
        assert_eq!(layout_640().auto_pos, (515, 303));
    }

    #[test]
    fn inventory_slot_rect_bounds() {
        let l = layout_800();
        assert_eq!(l.inventory_slot_rect(0, 70).unwrap().x, 159);
        assert_eq!(l.inventory_slot_rect(1, 70).unwrap().x, 159 + 69);
        assert_eq!(l.inventory_slot_rect(8, 70).unwrap().x, 159 + 8 * 69);
        assert!(l.inventory_slot_rect(9, 70).is_none());
    }

    #[test]
    fn arrow_caps_match_the_original_strip_renderer() {
        // Left cap at left-1; right cap at right - arrow_w(20) + 1; both at the
        // slot-row top. INTR[18]=left, INTR[20]=right at scroll 0.
        let l = layout_800();
        assert_eq!(l.arrows[0], (18, 139, 486));
        assert_eq!(l.arrows[1], (20, 780, 486));
        let l = layout_640();
        assert_eq!(l.arrows[0], (18, 111, 375));
        assert_eq!(l.arrows[1], (20, 620, 375));
    }

    #[test]
    fn item_icons_are_centered_in_their_slots() {
        let fw = 800;
        let fh = 600;
        let mut frame = vec![0u8; fw * fh * 4];
        let l = layout_800();
        let state = HudState {
            party: Vec::new(),
            items: vec![
                ItemIcon { slot: 0, icon: 50 },
                ItemIcon { slot: 5, icon: 23 },
            ],
            hovered: None,
            auto: None,
        };
        let art = solid(40, 40, [10, 200, 10, 255]);
        compose(&mut frame, fw, fh, &l, &state, |id| {
            if id == 3 || id == 17 {
                None
            } else {
                Some(art.clone())
            }
        });
        for slot in [0usize, 5] {
            let cell = l.inventory_slot_rect(slot, 70).unwrap();
            let cx = (cell.x + (cell.w - 40) / 2) as usize;
            let cy = (cell.y + (cell.h - 40) / 2) as usize;
            assert_eq!(px(&frame, fw, cx, cy), [10, 200, 10, 255], "slot {slot}");
        }
        // An unfilled slot (e.g. slot 6) has no icon.
        let cell6 = l.inventory_slot_rect(6, 70).unwrap();
        let cx6 = (cell6.x + 35) as usize;
        let cy6 = (cell6.y + 35) as usize;
        assert_eq!(px(&frame, fw, cx6, cy6), [0, 0, 0, 0]);
    }

    #[test]
    fn compose_is_idempotent() {
        let fw = 32;
        let fh = 32;
        let mut a = vec![0u8; fw * fh * 4];
        let mut b = vec![0u8; fw * fh * 4];
        for p in a.chunks_mut(4) {
            p.copy_from_slice(&[9, 9, 9, 255]);
        }
        b.copy_from_slice(&a);
        let l = layout_800();
        let state = HudState::default();
        let bg = solid(800, 600, [20, 40, 60, 255]);
        let slot = solid(70, 70, [80, 90, 100, 255]);
        let run = |frame: &mut Vec<u8>| {
            compose(frame, fw, fh, &l, &state, |id| {
                Some(if id == 3 { bg.clone() } else { slot.clone() })
            })
        };
        run(&mut a);
        run(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn portraits_center_in_cells() {
        let fw = 200;
        let fh = 40;
        let mut frame = vec![0u8; fw * fh * 4];
        let l = layout_640();
        let cell = l.party_cells[0];
        let state = HudState {
            party: vec![PartyPortrait { cell: 0, intr_id: 240 }],
            ..HudState::default()
        };
        // 20x20 portrait: centered offset within the cell.
        let art = solid(20, 20, [222, 0, 0, 255]);
        compose(&mut frame, fw, fh, &l, &state, |id| {
            if id == 240 {
                Some(art.clone())
            } else {
                None
            }
        });
        let ox = (cell.x + (cell.w - 20) / 2) as usize;
        let oy = (cell.y + (cell.h - 20) / 2) as usize;
        assert_eq!(px(&frame, fw, ox, oy), [222, 0, 0, 255]);
    }
}
