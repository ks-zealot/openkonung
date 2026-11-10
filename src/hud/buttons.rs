//! Original HUD button strip (`0x81CF68[0..7]`, geometry `0x45AEA0`,
//! hit-test tables consumed by `FUN_00438900`).
//!
//! Seven INTR sprites are composited into the empty lower-left panel by
//! `FUN_0042CA44`: a weapon plaque (slot 0) and six buttons. The tables in
//! [`super::buttons_data`] are generated from `konung.res` and are exact.
//!
//! Slots 0 and 1 are state-dependent (`Entity+0x12` weapon slot and
//! `Entity+0x03 & 4` stance); slots 2..=6 are fixed icons. A click is resolved
//! by [`button_at`] into one of the command codes below, which the app turns
//! back into the original handler call.

use super::buttons_data::{ICON_HITS_640, ICON_HITS_800, ICON_POS};

/// Number of entries in the original `0x81CF68` icon array.
pub const HUD_ICON_COUNT: usize = 7;

/// Command code of the weapon plaque: cycles the active weapon slot
/// `Entity+0x12` (`FUN_004216DC(-1)`).
pub const CODE_WEAPON: u8 = 1;

/// Command code of the stance toggle (`FUN_00421554`, flips `Entity+0x03 & 4`).
pub const CODE_STANCE: u8 = 2;

/// Command code of the world map (`FUN_0042CBD0(4)`, interface mode 5).
pub const CODE_MAP: u8 = 3;

/// Command code of the party halt order (`FUN_0042246C`).
pub const CODE_HALT: u8 = 4;

/// Command code of the party engage order (`FUN_00421458`).
pub const CODE_ENGAGE: u8 = 5;

/// Command code of the quest log (`FUN_004242BC`, interface mode 2).
pub const CODE_QUEST: u8 = 6;

/// Command code of the character screen (`FUN_00424330`, interface mode 3).
pub const CODE_CHARACTER: u8 = 7;

/// `INTR` sprite for the active weapon plaque (`Entity+0x12`: 0 axe, 1 bow,
/// else sword), from the original HUD refresh routine.
pub fn weapon_icon(raw_0x12: u8) -> usize {
    match raw_0x12 {
        0 => 8,
        1 => 9,
        _ => 16,
    }
}

/// `INTR` sprite for the stance button: fist in battle (`Entity+0x03 & 4`),
/// open palm in peace.
pub fn stance_icon(battle: bool) -> usize {
    if battle {
        10
    } else {
        15
    }
}

/// Fixed icon ids for slots 2..=6 (map, halt, engage, quest log, character).
const FIXED_ICONS: [(u8, usize); 5] = [
    (CODE_MAP, 14),
    (CODE_HALT, 11),
    (CODE_ENGAGE, 7),
    (CODE_QUEST, 12),
    (CODE_CHARACTER, 13),
];

/// Resolved `(intr_id, x, y)` for all seven icons at the requested logical
/// width. Slot order matches `0x81CF68` / `FUN_0042CA44` draw order.
pub fn icon_plan(logical_w: i32, raw_0x12: u8, battle: bool) -> [(usize, i32, i32); 7] {
    let mut out = [(0usize, 0i32, 0i32); 7];
    for (slot, plan) in out.iter_mut().enumerate() {
        let (x640, y640, x800, y800) = ICON_POS[slot];
        let (x, y) = if logical_w <= 640 {
            (x640, y640)
        } else {
            (x800, y800)
        };
        let id = match slot {
            0 => weapon_icon(raw_0x12),
            1 => stance_icon(battle),
            n => FIXED_ICONS[n - 2].1,
        };
        *plan = (id, x, y);
    }
    out
}

/// Exact `FUN_00438900` lookup over the original icon hit table.
///
/// Returns the original command code (1..=7) under `(x, y)`, or `None` when
/// the cursor is outside the button strip / on a non-button row.
pub fn button_at(x: i32, y: i32, logical_w: i32) -> Option<u8> {
    if x < 0 || y < 0 {
        return None;
    }
    let rows: &[(i16, &[(u8, u16)])] = if logical_w <= 640 {
        ICON_HITS_640
    } else {
        ICON_HITS_800
    };
    let row = rows.iter().find(|(row_y, _)| *row_y as i32 == y)?;
    let runs = row.1;
    let mut acc = 0i32;
    for &(value, width) in runs {
        acc += width as i32;
        if width == 0 || x < acc {
            return (1..=7).contains(&value).then_some(value);
        }
    }
    None
}

/// Icon slot index (`0..7`) for a command code (`1..=7`).
pub fn slot_for_code(code: u8) -> Option<usize> {
    (1..=7).contains(&code).then_some(code as usize - 1)
}
