//! `layout` subsystem of [`crate::hud`] (split from `hud.rs`).

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


pub fn build_party_cells(
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
