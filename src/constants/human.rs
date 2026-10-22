//! Centralised human/GRAPH sprite constants and original draw-order tables
//! (extracted from `res/human.rs`).

/// Peaceful-stance overlay draw order, indexed by facing.
///
/// `konung.res` DGROUP VA `0x4631FC` (`FUN_00427e9c` peace branch). The battle
/// stance uses separate tables below because the active weapon is drawn in the
/// hand rather than sheathed.
pub const HUMAN_DRAW_ORDER_PEACE: [[usize; 6]; 8] = [
    [3, 2, 4, 1, 0, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 1, 5],
    [3, 2, 4, 0, 5, 1],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 5, 4, 1, 0],
    [3, 2, 4, 5, 1, 0],
];

/// Battle-stance overlay order A (`konung.res` DGROUP VA `0x462BFC`):
/// `[action_bucket][facing][draw_index] = equipment_slot`.
///
/// Used when `Entity+0x12 != 1` or the action bucket is `2` (action `5`).
pub const HUMAN_DRAW_ORDER_BATTLE_A: [[[usize; 6]; 8]; 4] = [
    [
        [3, 2, 0, 4, 5, 1],
        [3, 5, 2, 4, 0, 1],
        [3, 5, 4, 0, 2, 1],
        [3, 5, 4, 1, 0, 2],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 1, 4, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 0, 1, 2, 5],
        [3, 2, 5, 0, 4, 1],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
    [
        [3, 2, 0, 4, 1, 5],
        [3, 2, 0, 5, 4, 1],
        [3, 5, 4, 2, 0, 1],
        [3, 5, 4, 0, 1, 2],
        [3, 2, 5, 4, 0, 1],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 0, 4, 5, 1],
        [3, 5, 4, 2, 1, 0],
        [3, 5, 4, 2, 1, 0],
        [3, 5, 4, 1, 0, 2],
        [3, 4, 2, 1, 0, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 1, 0, 2, 4, 5],
        [3, 0, 2, 1, 4, 5],
    ],
];

/// Battle-stance overlay order B (`konung.res` DGROUP VA `0x462EFC`), used when
/// `Entity+0x12 == 1` and the action bucket is not `2`.
pub const HUMAN_DRAW_ORDER_BATTLE_B: [[[usize; 6]; 8]; 4] = [
    [
        [3, 2, 0, 1, 4, 5],
        [3, 2, 0, 1, 4, 5],
        [3, 4, 0, 2, 1, 5],
        [3, 4, 5, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 0, 1, 2, 5],
        [3, 2, 5, 0, 4, 1],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
    [
        [3, 2, 0, 4, 1, 5],
        [3, 2, 0, 5, 4, 1],
        [3, 5, 4, 2, 0, 1],
        [3, 5, 4, 0, 1, 2],
        [3, 2, 5, 4, 0, 1],
        [3, 4, 1, 0, 2, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 0, 2, 1, 4, 5],
    ],
    [
        [3, 2, 1, 4, 0, 5],
        [3, 2, 1, 4, 0, 5],
        [3, 4, 2, 1, 0, 5],
        [3, 4, 1, 0, 2, 5],
        [3, 2, 5, 4, 1, 0],
        [3, 5, 0, 4, 1, 2],
        [3, 5, 4, 1, 0, 2],
        [3, 2, 1, 4, 0, 5],
    ],
];

/// GRAPH human animation row stride (`block + action*0xD0`).
pub const HUMAN_ACTION_STRIDE: usize = 0xD0;
/// GRAPH human direction stride (`block + direction*0x1A`).
pub const HUMAN_DIRECTION_STRIDE: usize = 0x1A;
/// Frame-count field offset inside a human animation row.
pub const HUMAN_FRAME_COUNT_OFFSET: usize = 0x08;
/// Frame-id array offset inside a human animation row.
pub const HUMAN_FRAME_ID_OFFSET: usize = 0x0A;
/// One frame-id entry width.
pub const HUMAN_FRAME_ID_SIZE: usize = 2;

/// Per-frame screen anchor table (`block + 0x1728 + fi*8`).
pub const HUMAN_ANCHOR_TABLE_OFFSET: usize = 0x1728;
/// One anchor record width.
pub const HUMAN_ANCHOR_RECORD_SIZE: usize = 8;
/// Indexed sprite payload base (`block + 0x4D28`).
pub const HUMAN_BLOCK_DATA_OFFSET: usize = 0x4D28;

/// Body anchor table (`block + 0x2F28 + action*0x300 + direction*0x60 + fi*8`).
pub const HUMAN_BODY_ANCHOR_TABLE_OFFSET: usize = 0x2F28;
pub const HUMAN_BODY_ACTION_STRIDE: usize = 0x300;
pub const HUMAN_BODY_DIRECTION_STRIDE: usize = 0x60;

/// Overlay anchor table (`block + 0x828 + action*0x180 + direction*0x30 + fi*4`).
pub const HUMAN_OVERLAY_ANCHOR_TABLE_OFFSET: usize = 0x828;
pub const HUMAN_OVERLAY_ACTION_STRIDE: usize = 0x180;
pub const HUMAN_OVERLAY_DIRECTION_STRIDE: usize = 0x30;
pub const HUMAN_OVERLAY_ANCHOR_RECORD_SIZE: usize = 4;

/// Game.res variant field offsets.
pub const HUMAN_VARIANT_DIRECTION_OFFSET: usize = 0x01;
pub const HUMAN_VARIANT_FLAGS_OFFSET: usize = 0x04;
pub const HUMAN_VARIANT_BASE_OFFSET: usize = 0x3F;
pub const HUMAN_VARIANT_PALETTE_OFFSET: usize = 0x41;
pub const HUMAN_VARIANT_EQUIPMENT_OFFSET: usize = 0x45;
/// Minimum variant record length accepted by `find_fallback_human`.
pub const HUMAN_MIN_VARIANT_LEN: usize = 0x91;
/// Equipment slots on a human variant/entity.
pub const HUMAN_EQUIPMENT_SLOTS: usize = 6;

/// Palette offsets are stored in 512-byte units.
pub const PALETTE_OFFSET_SCALE: u32 = 512;
/// Number of palettes / maximum valid palette id.
pub const PALETTE_COUNT: i32 = 256;
/// Highest valid palette index.
pub const PALETTE_MAX_INDEX: i32 = 255;
/// Number of `equipped_visual` overlay selectors (`0..=23`).
pub const HUMAN_OVERLAY_SELECTOR_COUNT: i16 = 24;
/// Minimum object-family block length (`0x3228` table + 8-byte header).
pub const OBJECT_FAMILY_MIN_BLOCK_LEN: usize = 0x3228 + 8;

/// Offscreen paper-doll surface size (`FUN_0042ea6c`: clip `0x9f x 0x13f`,
/// final visible blit `0xA0 x 0xA0`). The remake only needs the visible square.
pub const PAPER_DOLL_W: usize = 160;
pub const PAPER_DOLL_H: usize = 160;
/// Original paper-doll body origin inside the offscreen surface.
pub const PAPER_DOLL_BODY_ORIGIN: (i32, i32) = (0x3C, 0x14);
