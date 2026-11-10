//! `layout` subsystem of [`crate::hud`] (split from `hud.rs`).
//!
//! Every rectangle the HUD compositor needs for one logical resolution: the
//! inventory strip, its slot grid and end caps, the 2x5 party panel, and the
//! interface viewport that the window screens (map, quest log, dialogue, trade)
//! tile and clip inside. All values are pixels in logical framebuffer space,
//! i.e. already specific to the 640x480 / 800x600 variant - there is no runtime
//! scaling pass.

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
///
/// `x`/`y` are inclusive, `w`/`h` are extents, so [`Rect::right`] and
/// [`Rect::bottom`] are exclusive - the same convention as the original blit
/// clipper and the hit tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge, inclusive.
    pub x: i32,
    /// Top edge, inclusive.
    pub y: i32,
    /// Width in px.
    pub w: i32,
    /// Height in px.
    pub h: i32,
}

/// HP-indicator bar geometry (`FUN_0042C730` bar branch).
///
/// `height` is the full bar height (`konung.res` table `0x463314`: 32 @640 /
/// 40 @800); `dx`/`dy` are the bar top-left relative to the portrait origin
/// (`0x466012`/`0x466014` high words: 4/8 @640, 6/10 @800).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HpBar {
    /// Full bar height in px, i.e. the height at health 1600.
    pub height: i32,
    /// Bar left offset from the portrait origin.
    pub dx: i32,
    /// Bar top offset from the portrait origin.
    pub dy: i32,
}

/// Recovered party-panel portrait origins (`konung.res` table `0x45AF10`),
/// row-major slots 0..10. The table stores `{x640,y640,x800,y800}` per slot;
/// only the resolution-appropriate pair is kept here. These are the exact
/// portrait / selection-marker / HP-bar anchor positions.
const PARTY_ORIGINS_640: [(i32, i32); PARTY_CELLS] = [
    (3, 1),
    (58, 1),
    (3, 55),
    (58, 55),
    (3, 109),
    (58, 109),
    (3, 163),
    (58, 163),
    (3, 217),
    (58, 217),
];

const PARTY_ORIGINS_800: [(i32, i32); PARTY_CELLS] = [
    (6, 4),
    (74, 4),
    (6, 71),
    (74, 71),
    (6, 139),
    (74, 139),
    (6, 206),
    (74, 206),
    (6, 273),
    (74, 273),
];

/// Slot step used by the original executable: `slot_width - 1`
/// (global `0x81D724`, computed at VA `0x42C4B5`).
#[inline]
pub fn slot_step(slot_width: u16) -> i32 {
    slot_width as i32 - 1
}

/// Resolved HUD geometry for one logical resolution.
#[derive(Clone, Debug)]
pub struct HudLayout {
    /// Framebuffer width this layout was built for (`640` or `800`).
    pub logical_w: i32,
    /// Matching framebuffer height (`480` or `600`).
    pub logical_h: i32,
    /// `INTR` index of the whole-screen overlay (`INTR[3]`).
    pub hud_id: usize,
    /// `INTR` index of the empty inventory-slot background (`INTR[17]`).
    pub slot_id: usize,
    /// Inventory strip region `(left, strip_bottom)`, in logical coordinates.
    pub inventory_region: Rect,
    /// Top-left of the first slot cell.
    pub inventory_origin: (i32, i32),
    /// Number of visible slot columns.
    pub visible_slots: usize,
    /// Slot step (== slot width - 1 for the original 70x70 cell).
    pub slot_step: i32,
    /// Bottom-bar text area used for the hover text. `FUN_0042BD8C` draws at
    /// `x + 10`, `y + 5` and wraps to `w - 16` (the original uses
    /// `_81D738+10`, `_81D760+5`, `(_81D744-_81D738)-0xf`).
    pub inventory_bar: Rect,
    /// Two scroll-arrow end caps drawn by the original strip renderer even at
    /// scroll 0: `(intr_id, x, y)`. Left = `INTR[18]`, right = `INTR[20]`.
    pub arrows: [(usize, i32, i32); 2],
    /// AUTO combat button top-left (traced from the ability-loop table
    /// `0x465B34 + sel*8 + 80`; not pixel-verified).
    pub auto_pos: (i32, i32),
    /// 10 party portrait cells, row-major (row 0 = top).
    pub party_cells: [Rect; PARTY_CELLS],
    /// HP-indicator bar geometry for this resolution.
    pub hp_bar: HpBar,
    /// Per-slot portrait/marker/HP-bar draw origin, from `0x45AF10`.
    pub party_origins: [(i32, i32); PARTY_CELLS],
    /// Interface viewport (`_81D738/_81D754/_81D744/_81D760`): x, y, w, h.
    /// Window screens (map, quest log, dialogue) tile/clip inside it.
    pub viewport: Rect,
    /// World-map overlay top-left (absolute logical coordinates).
    pub map_origin: (i32, i32),
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
        inventory_region: Rect {
            x: 140,
            y: 486,
            w: 660,
            h: 70,
        },
        // Origin = left-1 + arrow_w(INTR[18].w=20) + remainder/2;
        // count = (span+2 - 2*20)/69 = 621/69 = 9 (VA 0x42C4EE-0x42C544).
        inventory_origin: (159, 486),
        visible_slots: 9,
        slot_step: 69,
        inventory_bar: Rect {
            x: 140,
            y: 555,
            w: 660,
            h: 45,
        },
        // Arrow caps at x=left-1=139 and x=right-20+1=780, y=486.
        arrows: [(18, 139, 486), (20, 780, 486)],
        auto_pos: (644, 379),
        // Measured from INTR800[3]: columns x=2/70 (cell 68 wide),
        // rows y=2/70/137/205/272 (cell 68 tall).
        party_cells: build_party_cells(&[2, 70], &[2, 70, 137, 205, 272], 68, 68),
        hp_bar: HpBar {
            height: 40,
            dx: 6,
            dy: 10,
        },
        party_origins: PARTY_ORIGINS_800,
        viewport: Rect {
            x: 140,
            y: 0,
            w: 660,
            h: 556,
        },
        map_origin: (220, 60),
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
        inventory_region: Rect {
            x: 112,
            y: 375,
            w: 528,
            h: 70,
        },
        // count = (span+2 - 2*20)/69 = 489/69 = 7; origin = 111+20+3 = 134.
        inventory_origin: (134, 375),
        visible_slots: 7,
        slot_step: 69,
        inventory_bar: Rect {
            x: 112,
            y: 444,
            w: 528,
            h: 36,
        },
        // Arrow caps at x=left-1=111 and x=right-20+1=620, y=375.
        arrows: [(18, 111, 375), (20, 620, 375)],
        auto_pos: (515, 303),
        // Measured from INTR640[3]: columns x=2/55 (cell 53 wide),
        // rows y=1/55/109/163/217 (cell 54 tall).
        party_cells: build_party_cells(&[2, 55], &[1, 55, 109, 163, 217], 53, 54),
        hp_bar: HpBar {
            height: 32,
            dx: 4,
            dy: 8,
        },
        party_origins: PARTY_ORIGINS_640,
        viewport: Rect {
            x: 112,
            y: 0,
            w: 528,
            h: 445,
        },
        map_origin: (125, 13),
    }
}

/// Build the row-major 2x5 party cell grid from measured column x positions,
/// row y positions and the shared cell size (from `INTR[3]`).
///
/// `cell = row * PARTY_COLS + col`, so index 0 is the top-left portrait - the
/// order the party group uses and the order clicks are mapped back to.
pub fn build_party_cells(
    col_x: &[i32; PARTY_COLS],
    row_y: &[i32; PARTY_ROWS],
    cell_w: i32,
    cell_h: i32,
) -> [Rect; PARTY_CELLS] {
    let mut cells = [Rect {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    }; PARTY_CELLS];
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
    /// Exclusive right edge (`x + w`); a pixel at this column is outside.
    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    /// Exclusive bottom edge (`y + h`).
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// Point-in-rectangle test with the same half-open convention the original
    /// hit tests use (`x1`/`y1` exclusive).
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

    /// Portrait / selection-marker / HP-bar draw origin for a flat party slot
    /// (0..10), from the recovered table `0x45AF10`.
    pub fn party_origin(&self, index: usize) -> Option<(i32, i32)> {
        self.party_origins.get(index).copied()
    }

    /// Rectangle of one visible inventory cell, or `None` if out of range.
    pub fn inventory_slot_rect(&self, slot: usize, slot_w: u16) -> Option<Rect> {
        inventory_slot_rect(self, slot, slot_w)
    }
}
