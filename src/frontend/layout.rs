//! Recovered original frontend layout tables.
//!
//! `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **BINARY-LAYOUT VERIFIED**.
//!
//! All values below were read directly from `konung.res` (the PE32 frontend,
//! ImageBase `0x400000`) and cross-checked against the decoded
//! `MENU{width}R.RES` / `NEWH{width}R.RES` image dimensions:
//!
//! ```text
//! main-menu rects   file 0x414B4   12 records, 640 (even) / 800 (odd) interleaved, -1 terminated
//! newh draw rects   file 0x4629C   44 records, 640 (even) / 800 (odd) interleaved, -1 terminated
//! newh hit  rects   file 0x466DC   44 records, 640 (even) / 800 (odd) interleaved, -1 terminated
//! ```
//!
//! The file offsets are reverse-engineering landmarks only; they are **not**
//! read at runtime. Coordinates are hard-coded constants per the declarative
//! layout contract (task §26/§39).
//!
//! `NEWH_DRAW_*` are the image placement rectangles (portrait/label blit
//! position); `NEWH_HIT_*` are the interactive rectangles. Both tables split
//! into `NEWH_REGION_COUNT` logical regions in the same order.

/// An original screen rectangle. Coordinates are top-left inclusive with an
/// explicit width/height, in the logical frontend resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
}

impl Rect {
    pub const fn new(x: i16, y: i16, w: i16, h: i16) -> Rect {
        Rect { x, y, w, h }
    }

    /// Original hit-test semantics: left/top/right/bottom inclusive, where
    /// `right = x + w - 1`, `bottom = y + h - 1`.
    pub fn contains(&self, mx: i32, my: i32) -> bool {
        mx >= self.x as i32
            && mx <= (self.x + self.w) as i32 - 1
            && my >= self.y as i32
            && my <= (self.y + self.h) as i32 - 1
    }
}

/// Original main-menu action ordering. The numeric values are the original
/// dispatcher indices (task §10); the `New Game` index is `3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuAction {
    Resume = 0,
    Save = 1,
    Load = 2,
    NewGame = 3,
    Settings = 4,
    Exit = 5,
}

impl MainMenuAction {
    pub const ALL: [MainMenuAction; 6] = [
        MainMenuAction::Resume,
        MainMenuAction::Save,
        MainMenuAction::Load,
        MainMenuAction::NewGame,
        MainMenuAction::Settings,
        MainMenuAction::Exit,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<MainMenuAction> {
        MainMenuAction::ALL.get(i).copied()
    }
}

/// Declarative main-menu button (task §26/§39). `normal_image`/`hover_image`
/// index into the decoded `MENU{width}R.RES` image list.
#[derive(Clone, Copy, Debug)]
pub struct MenuButtonDef {
    pub action: MainMenuAction,
    pub normal_image: usize,
    pub hover_image: usize,
    pub rect_640: Rect,
    pub rect_800: Rect,
}

/// The six original menu buttons, preserving the confirmed pairing/order.
/// Rectangles are the exact original records from `konung.res` 0x414B4.
pub const MAIN_MENU: [MenuButtonDef; 6] = [
    MenuButtonDef {
        action: MainMenuAction::Resume,
        normal_image: 3,
        hover_image: 4,
        rect_640: Rect::new(233, 130, 192, 31),
        rect_800: Rect::new(290, 161, 300, 49),
    },
    MenuButtonDef {
        action: MainMenuAction::Save,
        normal_image: 5,
        hover_image: 6,
        rect_640: Rect::new(231, 164, 173, 35),
        rect_800: Rect::new(288, 205, 216, 45),
    },
    MenuButtonDef {
        action: MainMenuAction::Load,
        normal_image: 7,
        hover_image: 8,
        rect_640: Rect::new(212, 200, 178, 40),
        rect_800: Rect::new(265, 250, 223, 53),
    },
    MenuButtonDef {
        action: MainMenuAction::NewGame,
        normal_image: 9,
        hover_image: 10,
        rect_640: Rect::new(224, 241, 128, 40),
        rect_800: Rect::new(280, 303, 160, 51),
    },
    MenuButtonDef {
        action: MainMenuAction::Settings,
        normal_image: 11,
        hover_image: 12,
        rect_640: Rect::new(215, 282, 118, 39),
        rect_800: Rect::new(268, 353, 148, 49),
    },
    MenuButtonDef {
        action: MainMenuAction::Exit,
        normal_image: 13,
        hover_image: 14,
        rect_640: Rect::new(222, 327, 78, 44),
        rect_800: Rect::new(276, 409, 90, 55),
    },
];

/// Known `MENU{width}R.RES` image indices.
///
/// `#0` is the startup/in-game main-menu background (stone). `#1` is the
/// **confirmed Settings screen** ("Настройки" — 640x480/800x600 toggles plus
/// the option rows and Ok/Отмена). `#2` is a 13x13 image of unknown purpose.
pub const MENU_IMAGE_A: usize = 0;
pub const MENU_IMAGE_SETTINGS: usize = 1;
pub const MENU_IMAGE_SMALL: usize = 2;

/// `SAVE{width}R.RES` images. Confirmed from the decoded artwork: `#0` is the
/// **Load** screen ("Загрузка игры") and `#1` is the **Save** screen
/// ("Сохранение игры").
pub const SAVE_IMAGE_LOAD: usize = 0;
pub const SAVE_IMAGE_SAVE: usize = 1;

/// New-Hero background / auxiliary image indices (`NEWH{width}R.RES`).
pub const NEWH_BACKGROUND: usize = 0;
pub const NEWH_AUX_100: usize = 1;
/// The original round "selected" blob/radio marker (image #2), centered inside
/// the selected class draw rectangle by the original renderer (VA 0x42EA6C).
pub const NEWH_CLASS_MARKER: usize = 2;

/// Hero portrait image pairs: (normal, selected/hover) for hero 0/1/2.
/// Pairing is a **WORKING INTERPRETATION** (task §15); both images are kept.
pub const HERO_IMAGE_PAIRS: [(usize, usize); 3] = [(3, 4), (5, 6), (7, 8)];

/// Bottom New-Hero button image pairs: (normal, alternate) for the two large
/// controls. Pair 0 is "Отмена" (Cancel) and pair 1 is "Ok", confirmed from the
/// decoded artwork (`NEWH{width}R.RES` #9/#10 and #11/#12).
pub const BOTTOM_IMAGE_PAIRS: [(usize, usize); 2] = [(9, 10), (11, 12)];

/// Number of logical New-Hero regions (each has one draw rect and one hit rect).
///
/// The original tables (`NEWH_DRAW` at VA 0x46009C, `NEWH_HIT` at VA 0x4604DC)
/// hold 66 records (even = 640, odd = 800), `-1`-terminated, i.e. **33** logical
/// regions per resolution — not 22. The extra regions are #22 point, #23
/// character-preview panel, #24 top ПАРАМЕТРЫ points box, and #25-32 the eight
/// stat steppers.
pub const NEWH_REGION_COUNT: usize = 33;

/// Top "ПАРАМЕТРЫ" free-parameter-points box (`Entity+0x38`).
pub const NEWH_POINTS_REGION: usize = 24;
/// First stat-stepper region; pairs `(minus, plus)` for stats 0..3 run
/// `25..32` (CHA, STR, DEX, END).
pub const NEWH_STEPPER_FIRST_REGION: usize = 25;

/// Hero-selection regions (task §16 hitboxes #0/#1/#2).
pub const HERO_REGIONS: [usize; 3] = [0, 1, 2];

/// The two large bottom controls (task §20 #20/#21), region indices.
///
/// The decoded artwork labels region 20 "Отмена" (Cancel) and region 21 "Ok"
/// (confirm), so these roles are **DATA-CORRELATED** rather than guessed.
pub const BOTTOM_CANCEL_REGION: usize = 20;
pub const BOTTOM_OK_REGION: usize = 21;
pub const BOTTOM_REGIONS: [usize; 2] = [BOTTOM_CANCEL_REGION, BOTTOM_OK_REGION];

/// `NEWH_DRAW_640`: original image placement rectangles (even records of the
/// 0x4629C table). Hero draw rects equal the `NEWH640R.RES` portrait sizes.
pub const NEWH_DRAW_640: [Rect; NEWH_REGION_COUNT] = [
    Rect::new(44, 94, 57, 66),   // 0 hero 0 portrait
    Rect::new(137, 94, 58, 66),  // 1 hero 1 portrait
    Rect::new(229, 94, 58, 66),  // 2 hero 2 portrait
    Rect::new(481, 68, 20, 20),  // 3 stat control
    Rect::new(448, 68, 20, 20),  // 4 stat control
    Rect::new(448, 36, 20, 20),  // 5 stat control
    Rect::new(481, 36, 20, 20),  // 6 stat control
    Rect::new(465, 160, 20, 15), // 7 stat control
    Rect::new(465, 178, 20, 15), // 8
    Rect::new(465, 196, 20, 15), // 9
    Rect::new(465, 214, 20, 15), // 10
    Rect::new(513, 254, 30, 15), // 11
    Rect::new(513, 272, 30, 15), // 12
    Rect::new(575, 334, 30, 15), // 13
    Rect::new(575, 352, 30, 15), // 14
    Rect::new(434, 333, 20, 20), // 15
    Rect::new(434, 350, 20, 20), // 16
    Rect::new(461, 387, 20, 20), // 17
    Rect::new(461, 404, 20, 20), // 18
    Rect::new(461, 421, 20, 20), // 19
    Rect::new(65, 427, 100, 33), // 20 bottom button 0
    Rect::new(175, 427, 100, 33),// 21 bottom button 1
    Rect::new(470, 127, 0, 0),   // 22 zero-size point (no-op)
    Rect::new(40, 206, 262, 200),// 23 character preview panel
    Rect::new(463, 134, 30, 15), // 24 points box
    Rect::new(435, 160, 30, 15), // 25 CHA -
    Rect::new(485, 160, 40, 15), // 26 CHA +
    Rect::new(435, 178, 30, 15), // 27 STR -
    Rect::new(485, 178, 40, 15), // 28 STR +
    Rect::new(435, 196, 30, 15), // 29 DEX -
    Rect::new(485, 196, 40, 15), // 30 DEX +
    Rect::new(435, 214, 30, 15), // 31 END -
    Rect::new(485, 214, 40, 15), // 32 END +
];

/// `NEWH_DRAW_800`: odd records of the 0x4629C table.
pub const NEWH_DRAW_800: [Rect; NEWH_REGION_COUNT] = [
    Rect::new(44, 108, 76, 82),
    Rect::new(155, 108, 81, 82),
    Rect::new(265, 108, 81, 82),
    Rect::new(582, 81, 20, 20),
    Rect::new(549, 81, 20, 20),
    Rect::new(549, 49, 20, 20),
    Rect::new(582, 49, 20, 20),
    Rect::new(545, 195, 30, 15),
    Rect::new(545, 218, 30, 15),
    Rect::new(545, 239, 30, 15),
    Rect::new(545, 259, 30, 15),
    Rect::new(607, 311, 36, 15),
    Rect::new(607, 333, 36, 15),
    Rect::new(695, 414, 30, 15),
    Rect::new(695, 436, 30, 15),
    Rect::new(542, 413, 20, 20),
    Rect::new(542, 434, 20, 20),
    Rect::new(542, 466, 20, 20),
    Rect::new(542, 487, 20, 20),
    Rect::new(542, 509, 20, 20),
    Rect::new(85, 527, 100, 33),
    Rect::new(195, 527, 100, 33),
    Rect::new(570, 160, 0, 0),   // 22 zero-size point (no-op)
    Rect::new(40, 266, 317, 300),// 23 character preview panel
    Rect::new(543, 164, 34, 15), // 24 points box
    Rect::new(515, 195, 30, 15), // 25 CHA -
    Rect::new(575, 195, 30, 15), // 26 CHA +
    Rect::new(515, 218, 30, 15), // 27 STR -
    Rect::new(575, 218, 30, 15), // 28 STR +
    Rect::new(515, 239, 30, 15), // 29 DEX -
    Rect::new(575, 239, 30, 15), // 30 DEX +
    Rect::new(515, 259, 30, 15), // 31 END -
    Rect::new(575, 259, 30, 15), // 32 END +
];

/// `NEWH_HIT_640`: interactive rectangles (even records of the 0x466DC table).
pub const NEWH_HIT_640: [Rect; NEWH_REGION_COUNT] = [
    Rect::new(44, 94, 57, 96),   // 0 hero 0
    Rect::new(137, 94, 58, 96),  // 1 hero 1
    Rect::new(229, 94, 58, 96),  // 2 hero 2
    Rect::new(481, 68, 99, 20),  // 3
    Rect::new(362, 68, 106, 20), // 4
    Rect::new(362, 36, 106, 20), // 5
    Rect::new(481, 36, 99, 20),  // 6
    Rect::new(360, 160, 70, 15), // 7
    Rect::new(360, 178, 70, 15), // 8
    Rect::new(360, 196, 70, 15), // 9
    Rect::new(360, 214, 70, 15), // 10
    Rect::new(420, 254, 123, 15),// 11
    Rect::new(420, 272, 123, 15),// 12
    Rect::new(464, 334, 141, 15),// 13
    Rect::new(464, 352, 141, 15),// 14
    Rect::new(356, 333, 98, 20), // 15
    Rect::new(356, 350, 98, 20), // 16
    Rect::new(356, 387, 125, 20),// 17
    Rect::new(356, 404, 125, 20),// 18
    Rect::new(361, 421, 120, 20),// 19
    Rect::new(65, 427, 100, 33), // 20 bottom 0
    Rect::new(175, 427, 100, 33),// 21 bottom 1
    Rect::new(470, 127, 0, 0),   // 22
    Rect::new(40, 206, 262, 200),// 23 preview panel
    Rect::new(463, 134, 30, 15), // 24 points
    Rect::new(435, 160, 30, 15), // 25 CHA -
    Rect::new(485, 160, 40, 15), // 26 CHA +
    Rect::new(435, 178, 30, 15), // 27 STR -
    Rect::new(485, 178, 40, 15), // 28 STR +
    Rect::new(435, 196, 30, 15), // 29 DEX -
    Rect::new(485, 196, 40, 15), // 30 DEX +
    Rect::new(435, 214, 30, 15), // 31 END -
    Rect::new(485, 214, 40, 15), // 32 END +
];

/// `NEWH_HIT_800`: odd records of the 0x466DC table.
pub const NEWH_HIT_800: [Rect; NEWH_REGION_COUNT] = [
    Rect::new(44, 108, 76, 124),
    Rect::new(155, 108, 81, 124),
    Rect::new(265, 108, 81, 124),
    Rect::new(582, 81, 96, 20),
    Rect::new(464, 81, 105, 20),
    Rect::new(464, 49, 105, 20),
    Rect::new(582, 49, 96, 20),
    Rect::new(425, 195, 85, 15),
    Rect::new(425, 218, 85, 15),
    Rect::new(425, 239, 85, 15),
    Rect::new(425, 259, 85, 15),
    Rect::new(490, 311, 153, 15),
    Rect::new(490, 333, 153, 15),
    Rect::new(578, 414, 147, 15),
    Rect::new(578, 436, 147, 15),
    Rect::new(428, 413, 134, 20),
    Rect::new(428, 434, 134, 20),
    Rect::new(428, 466, 134, 20),
    Rect::new(428, 487, 134, 20),
    Rect::new(428, 509, 134, 20),
    Rect::new(85, 527, 100, 33),
    Rect::new(195, 527, 100, 33),
    Rect::new(570, 160, 0, 0),   // 22
    Rect::new(40, 266, 317, 300),// 23 preview panel
    Rect::new(543, 164, 34, 15), // 24 points
    Rect::new(515, 195, 30, 15), // 25 CHA -
    Rect::new(575, 195, 30, 15), // 26 CHA +
    Rect::new(515, 218, 30, 15), // 27 STR -
    Rect::new(575, 218, 30, 15), // 28 STR +
    Rect::new(515, 239, 30, 15), // 29 DEX -
    Rect::new(575, 239, 30, 15), // 30 DEX +
    Rect::new(515, 259, 30, 15), // 31 END -
    Rect::new(575, 259, 30, 15), // 32 END +
];

/// Logical frontend resolution. The original selects `MENU{width}R.RES` /
/// `NEWH{width}R.RES` from the current horizontal resolution (task §23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendRes {
    R640,
    R800,
}

impl FrontendRes {
    pub fn width(self) -> i32 {
        match self {
            FrontendRes::R640 => 640,
            FrontendRes::R800 => 800,
        }
    }

    pub fn height(self) -> i32 {
        match self {
            FrontendRes::R640 => 480,
            FrontendRes::R800 => 600,
        }
    }

    pub fn main_rect(self, b: &MenuButtonDef) -> Rect {
        match self {
            FrontendRes::R640 => b.rect_640,
            FrontendRes::R800 => b.rect_800,
        }
    }

    pub fn newh_draw(self) -> &'static [Rect] {
        match self {
            FrontendRes::R640 => &NEWH_DRAW_640,
            FrontendRes::R800 => &NEWH_DRAW_800,
        }
    }

    pub fn newh_hit(self) -> &'static [Rect] {
        match self {
            FrontendRes::R640 => &NEWH_HIT_640,
            FrontendRes::R800 => &NEWH_HIT_800,
        }
    }

    /// Draw rect of the top free-parameter-points box (`NEWH` region 24).
    pub fn newh_points_box(self) -> Rect {
        self.newh_draw()[NEWH_POINTS_REGION]
    }

    /// `(minus, plus)` hit rects for primary stat `i` (0 = CHA, 1 = STR,
    /// 2 = DEX, 3 = END) — `NEWH` regions 25..32.
    pub fn newh_stepper(self, i: usize) -> (Rect, Rect) {
        let hit = self.newh_hit();
        let minus = hit[NEWH_STEPPER_FIRST_REGION + i * 2];
        let plus = hit[NEWH_STEPPER_FIRST_REGION + i * 2 + 1];
        (minus, plus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_menu_ordering_and_new_game_index() {
        assert_eq!(MainMenuAction::NewGame.index(), 3);
        assert_eq!(MainMenuAction::from_index(3), Some(MainMenuAction::NewGame));
        for (i, a) in MainMenuAction::ALL.iter().enumerate() {
            assert_eq!(a.index(), i);
        }
    }

    #[test]
    fn main_menu_pairs_match_task_table() {
        let expect = [
            (3, 4, MainMenuAction::Resume),
            (5, 6, MainMenuAction::Save),
            (7, 8, MainMenuAction::Load),
            (9, 10, MainMenuAction::NewGame),
            (11, 12, MainMenuAction::Settings),
            (13, 14, MainMenuAction::Exit),
        ];
        for (i, (n, h, act)) in expect.iter().enumerate() {
            let b = &MAIN_MENU[i];
            assert_eq!((b.normal_image, b.hover_image, b.action), (*n, *h, *act));
        }
    }

    #[test]
    fn main_menu_640_rects_match_recovered_table() {
        let expect = [
            (233, 130, 192, 31),
            (231, 164, 173, 35),
            (212, 200, 178, 40),
            (224, 241, 128, 40),
            (215, 282, 118, 39),
            (222, 327, 78, 44),
        ];
        for (i, (x, y, w, h)) in expect.iter().enumerate() {
            assert_eq!(
                MAIN_MENU[i].rect_640,
                Rect::new(*x, *y, *w, *h),
                "button {i}"
            );
        }
    }

    #[test]
    fn hero_hit_rects_match_task_section_16() {
        // task §16 #0/#1/#2 (640).
        assert_eq!(NEWH_HIT_640[0], Rect::new(44, 94, 57, 96));
        assert_eq!(NEWH_HIT_640[1], Rect::new(137, 94, 58, 96));
        assert_eq!(NEWH_HIT_640[2], Rect::new(229, 94, 58, 96));
    }

    #[test]
    fn bottom_regions_match_task_section_20() {
        assert_eq!(NEWH_HIT_640[BOTTOM_REGIONS[0]], Rect::new(65, 427, 100, 33));
        assert_eq!(NEWH_HIT_640[BOTTOM_REGIONS[1]], Rect::new(175, 427, 100, 33));
    }

    #[test]
    fn newh_tables_have_33_regions() {
        assert_eq!(NEWH_HIT_640.len(), 33);
        assert_eq!(NEWH_HIT_800.len(), 33);
        assert_eq!(NEWH_DRAW_640.len(), 33);
        assert_eq!(NEWH_DRAW_800.len(), 33);
    }

    #[test]
    fn points_box_and_steppers_match_recovered_tables() {
        // Region 24 (top ПАРАМЕТРЫ points box).
        assert_eq!(NEWH_DRAW_640[NEWH_POINTS_REGION], Rect::new(463, 134, 30, 15));
        assert_eq!(NEWH_DRAW_800[NEWH_POINTS_REGION], Rect::new(543, 164, 34, 15));
        // Steppers: #25/26 CHA, #27/28 STR, #29/30 DEX, #31/32 END.
        assert_eq!(FrontendRes::R800.newh_stepper(0), (Rect::new(515, 195, 30, 15), Rect::new(575, 195, 30, 15)));
        assert_eq!(FrontendRes::R800.newh_stepper(1), (Rect::new(515, 218, 30, 15), Rect::new(575, 218, 30, 15)));
        assert_eq!(FrontendRes::R800.newh_stepper(2), (Rect::new(515, 239, 30, 15), Rect::new(575, 239, 30, 15)));
        assert_eq!(FrontendRes::R800.newh_stepper(3), (Rect::new(515, 259, 30, 15), Rect::new(575, 259, 30, 15)));
        assert_eq!(FrontendRes::R640.newh_stepper(0), (Rect::new(435, 160, 30, 15), Rect::new(485, 160, 40, 15)));
        assert_eq!(FrontendRes::R640.newh_stepper(3), (Rect::new(435, 214, 30, 15), Rect::new(485, 214, 40, 15)));
    }

    #[test]
    fn skill_selector_hitboxes_match_verified_rects() {
        let r640 = [
            (356, 333, 98, 20),
            (356, 350, 98, 20),
            (356, 387, 125, 20),
            (356, 404, 125, 20),
            (361, 421, 120, 20),
        ];
        for (i, (x, y, w, h)) in r640.iter().enumerate() {
            assert_eq!(NEWH_HIT_640[15 + i], Rect::new(*x, *y, *w, *h), "640 #{i}");
        }
        let r800 = [
            (428, 413, 134, 20),
            (428, 434, 134, 20),
            (428, 466, 134, 20),
            (428, 487, 134, 20),
            (428, 509, 134, 20),
        ];
        for (i, (x, y, w, h)) in r800.iter().enumerate() {
            assert_eq!(NEWH_HIT_800[15 + i], Rect::new(*x, *y, *w, *h), "800 #{i}");
        }
    }

    #[test]
    fn rect_inclusive_contains() {
        let r = Rect::new(10, 20, 5, 4);
        assert!(r.contains(10, 20));
        assert!(r.contains(14, 23));
        assert!(!r.contains(15, 23));
        assert!(!r.contains(14, 24));
        assert!(!r.contains(9, 20));
    }
}
