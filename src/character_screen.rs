//! Original Konung character screen (`INTR[2]`): rendering + read-only info.
//!
//! Layout and coordinates are recovered from the original `konung.res`
//! (**BINARY-LAYOUT VERIFIED**):
//!
//! - The window is `INTR[2]` drawn at `(140, 0)` for 800x600 / `(112, 0)` for
//!   640x480, clipped to the gameplay region `(left, 0, right, strip_bottom)`
//!   (VA `0x42D0D4`, blit args from globals `0x81D738/0x81D754/0x81D744/0x81D760`).
//! - Static labels (Опыт/…/Золото) are baked into `INTR[2]`; only dynamic values
//!   are drawn. The character name and class use the original **large** font
//!   (`0x57B73C`); every stat value uses the original **small** font
//!   (`0x58BC34`) at the **absolute** per-resolution anchors recovered from
//!   `FUN_0042cbd0` case 3 (VA `0x42D3xx`, see [`VALUE_640`]/[`VALUE_800`]).
//! - The character name comes from the CP866 table `0x45E71C` (`+0xD4`); the
//!   class from `0x45ADC0` (`+0x04`).
//! - Equipment icons are drawn at **absolute logical screen coordinates** using
//!   the original per-resolution tables, preserving the storage→visual mapping
//!   `[3,4,3,0,1,2]` and the slot 0/2 overlap branch.
//! - The paper doll is `FUN_0042ea6c`: a 160×160 render of the live body frame
//!   plus every equipped GRAPH overlay, blitted at the original position table
//!   (`0x46035C`/`0x460360`: 640 → (470,127), 800 → (570,160)).
//! - The held item is hidden from the equipment slots and previewed at
//!   `(167,322)` @640 / `(215,393)` @800 (`0x465C44`/`0x465C48`).

use crate::character::{can_raise_any, stat_cost, stat_growth_cap, CharacterClass, PrimaryStat};
use crate::inventory::{item_weight, OwnerSnapshot};
use crate::res::{blit, Res};

/// `INTR` index of the character-window background.
pub const WINDOW_SPRITE: usize = 2;
/// `INTR[171]`: the 10x10 "raise stat / level up" cross drawn in the
/// `0x465B34` column (`FUN_0042cbd0` case 3).
pub const RAISE_ICON: usize = 171;

/// Live animation pose used by the paper doll (`FUN_0042ea6c` draws the
/// currently selected entity's action/direction/frame).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaperDollPose {
    pub action: usize,
    pub direction: u8,
    pub frame: usize,
}

/// Everything the screen needs to render one protagonist or companion.
#[derive(Clone, Debug, Default)]
pub struct CharacterSheet {
    pub name_id: u8,
    pub class: u8,
    pub equipment: [u16; 6],
    pub additional: [u16; 5],
    pub special: u16,
    pub inventory: [u16; 32],
    pub raw_0x12: u8,
    pub flags_b: u8,
    pub visual_index: u8,
    pub palette_id: i32,
    pub snapshot: OwnerSnapshot,
    pub pose: PaperDollPose,
}

/// Paper-doll top-left in logical screen coordinates. The original reads these
/// from `konung.res` DGROUP VA `0x46035C` / `0x460360` with stride `0x10`
/// (selector 0 = 640x480, selector 1 = 800x600).
const PAPER_DOLL_POS_640: (i32, i32) = (470, 127);
const PAPER_DOLL_POS_800: (i32, i32) = (570, 160);

/// Held-item preview centre (`0x465C44` / `0x465C48`).
const HELD_POS_640: (i32, i32) = (167, 322);
const HELD_POS_800: (i32, i32) = (215, 393);

pub fn paper_doll_position(logical_w: i32) -> (i32, i32) {
    if resolution_selector(logical_w) == 0 {
        PAPER_DOLL_POS_640
    } else {
        PAPER_DOLL_POS_800
    }
}

/// Resolution selector `0x81D644`: 0 = 640x480, 1 = 800x600.
#[inline]
pub fn resolution_selector(logical_w: i32) -> usize {
    if logical_w <= 640 {
        0
    } else {
        1
    }
}

/// Window rectangle `(x, y, w, h)` for a logical width.
pub fn window_rect(logical_w: i32) -> (i32, i32, i32, i32) {
    if logical_w <= 640 {
        (112, 0, 528, 376)
    } else {
        (140, 0, 642, 460)
    }
}

/// Main-equipment storage-slot base positions (absolute screen coords), already
/// folding the original storage→visual mapping `[3,4,3,0,1,2]`.
const MAIN_800: [(i32, i32); 6] = [
    (182, 103),
    (221, 233),
    (182, 103),
    (255, 126),
    (255, 54),
    (328, 103),
];
const MAIN_640: [(i32, i32); 6] = [
    (136, 70),
    (172, 199),
    (136, 70),
    (208, 94),
    (208, 22),
    (280, 70),
];

/// Additional-equipment slot centers.
const ADDITIONAL_800: [(i32, i32); 5] = [(232, 74), (214, 190), (367, 190), (248, 215), (334, 215)];
const ADDITIONAL_640: [(i32, i32); 5] = [(181, 44), (166, 157), (317, 157), (199, 182), (284, 182)];

/// Special-equipment top-left.
const SPECIAL_800: (i32, i32) = (295, 232);
const SPECIAL_640: (i32, i32) = (245, 199);

pub fn main_base(sel: usize, slot: usize) -> (i32, i32) {
    let table = if sel == 0 { &MAIN_640 } else { &MAIN_800 };
    table[slot.min(5)]
}
pub fn additional_center(sel: usize, slot: usize) -> (i32, i32) {
    let table = if sel == 0 {
        &ADDITIONAL_640
    } else {
        &ADDITIONAL_800
    };
    table[slot.min(4)]
}
pub fn special_base(sel: usize) -> (i32, i32) {
    if sel == 0 {
        SPECIAL_640
    } else {
        SPECIAL_800
    }
}

/// One clickable region of the character screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenHit {
    Main(usize),
    Additional(usize),
    Special,
    Inventory(usize),
    /// The icon-only "level up" row (`FUN_00410B08(entity, 0)` / command 0).
    RaiseLevel,
    /// One of the four stat rows in the "raise" column (command 3/5/2/6).
    Raise(PrimaryStat),
    None,
}

/// Hit-test the character screen (main/additional/special slots and the bottom
/// inventory strip). The original mode-3 hit tables are not recovered; the cell
/// rectangles are the recovered draw positions (documented approximation).
pub fn hit_test(x: i32, y: i32, logical_w: i32, equipment: &[u16; 6], raw_0x12: u8) -> ScreenHit {
    let sel = resolution_selector(logical_w);
    for slot in 0..6 {
        if !main_slot_drawn(equipment, raw_0x12, slot) {
            continue;
        }
        let (bx, by) = main_base(sel, slot);
        if x >= bx && x < bx + MAIN_CELL && y >= by && y < by + MAIN_CELL {
            return ScreenHit::Main(slot);
        }
    }
    for slot in 0..5 {
        let (cx, cy) = additional_center(sel, slot);
        let half = MAIN_CELL / 2;
        if x >= cx - half && x < cx + half && y >= cy - half && y < cy + half {
            return ScreenHit::Additional(slot);
        }
    }
    let (sx, sy) = special_base(sel);
    if x >= sx && x < sx + MAIN_CELL && y >= sy && y < sy + MAIN_CELL {
        return ScreenHit::Special;
    }
    // `0x465B34` raise column. The original dispatches these rows through its
    // mode-7 widget table; the remake uses the drawn glyph extent (documented
    // approximation, like the equipment cells above).
    for (slot, stat) in [
        (0usize, None),
        (1, Some(PrimaryStat::Charisma)),
        (2, Some(PrimaryStat::Strength)),
        (3, Some(PrimaryStat::Dexterity)),
        (4, Some(PrimaryStat::Endurance)),
    ] {
        let (x0, y0) = if sel == 0 {
            TABLE2_640[slot]
        } else {
            TABLE2_800[slot]
        };
        let (w, h) = if stat.is_none() { (14, 14) } else { (36, 14) };
        if x >= x0 && x < x0 + w && y >= y0 && y < y0 + h {
            return match stat {
                Some(stat) => ScreenHit::Raise(stat),
                None => ScreenHit::RaiseLevel,
            };
        }
    }
    let l = crate::hud::layout(logical_w);
    for slot in 0..l.visible_slots {
        if let Some(r) = l.inventory_slot_rect(slot, 70) {
            if r.contains(x, y) {
                return ScreenHit::Inventory(slot);
            }
        }
    }
    ScreenHit::None
}

/// Width of the inventory slot cell used by the main-equipment centering:
/// `draw_x = base_x + floor((slot_step + 1 - sprite_width) / 2)` where
/// `slot_step + 1 = 70`.
pub const MAIN_CELL: i32 = 70;

/// Whether main-equipment storage `slot` is drawn, preserving the original
/// overlap branch for slots 0 and 2 (VA `0x42D2DE`-`0x42D320`).
///
/// Slot 0 is skipped when `raw_0x12 == 2 && equipment[2] != 0`; slot 2 is then
/// drawn in its place. Otherwise slot 0 wins and slot 2 is not drawn.
pub fn main_slot_drawn(equipment: &[u16; 6], raw_0x12: u8, slot: usize) -> bool {
    if slot >= 6 {
        return false;
    }
    let slot0_drawn = equipment[0] != 0 && !(raw_0x12 == 2 && equipment[2] != 0);
    match slot {
        0 => slot0_drawn,
        2 => equipment[2] != 0 && !slot0_drawn,
        _ => equipment[slot] != 0,
    }
}

/// Draw the character screen over the current frame.
pub fn draw(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    sheet: &CharacterSheet,
    held: Option<u16>,
    _cursor: (i32, i32),
) {
    let logical_w = fw as i32;
    let (wx, wy, _ww, _wh) = window_rect(logical_w);
    let sel = resolution_selector(logical_w);

    // Original window background (its own static labels are baked in).
    if let Some(bg) = res.intr_sprite(WINDOW_SPRITE) {
        blit(frame, fw, fh, &bg.data, bg.w, bg.h, wx, wy);
    }

    // Main equipment (6 storage slots).
    for slot in 0..6 {
        if !main_slot_drawn(&sheet.equipment, sheet.raw_0x12, slot) {
            continue;
        }
        let id = sheet.equipment[slot];
        if Some(id) == held {
            continue;
        }
        let Some(info) = res.item_info(id) else {
            continue;
        };
        let Some(icon) = res.intr_sprite(info.icon as usize) else {
            continue;
        };
        let (bx, by) = main_base(sel, slot);
        let x = bx + (MAIN_CELL - icon.w as i32) / 2;
        blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, by);
    }

    // Additional equipment (5 slots), centered.
    for slot in 0..5 {
        let id = sheet.additional[slot];
        if id == 0 || Some(id) == held {
            continue;
        }
        let Some(info) = res.item_info(id) else {
            continue;
        };
        let Some(icon) = res.intr_sprite(info.icon as usize) else {
            continue;
        };
        let (cx, cy) = additional_center(sel, slot);
        let x = cx - icon.w as i32 / 2;
        let y = cy - icon.h as i32 / 2;
        blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y);
    }

    // Special equipment (top-left direct).
    if sheet.special != 0 && Some(sheet.special) != held {
        if let Some(info) = res.item_info(sheet.special) {
            if let Some(icon) = res.intr_sprite(info.icon as usize) {
                let (x, y) = special_base(sel);
                blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y);
            }
        }
    }

    // Original paper doll (`FUN_0042ea6c`): live body frame + all equipped
    // overlays, including the battle "in hand" weapon variant.
    draw_paper_doll(frame, fw, fh, res, sheet, logical_w);

    // Held item preview (`0x465C44` / `0x465C48`).
    if let Some(id) = held {
        if let Some(info) = res.item_info(id) {
            if let Some(icon) = res.intr_sprite(info.icon as usize) {
                let (cx, cy) = if sel == 0 { HELD_POS_640 } else { HELD_POS_800 };
                blit(
                    frame,
                    fw,
                    fh,
                    &icon.data,
                    icon.w,
                    icon.h,
                    cx - icon.w as i32 / 2,
                    cy - icon.h as i32 / 2,
                );
            }
        }
    }

    draw_text(frame, fw, fh, res, sheet, sel);
}

/// Render the original 160x160 paper doll into the character window.
fn draw_paper_doll(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    sheet: &CharacterSheet,
    logical_w: i32,
) {
    let battle = crate::anim::battle_from_flags(sheet.flags_b);
    let Some(doll) = res.paper_doll_frame(
        &sheet.equipment,
        sheet.raw_0x12,
        battle,
        sheet.visual_index as usize,
        sheet.palette_id,
        sheet.pose.action,
        sheet.pose.direction as usize,
        sheet.pose.frame,
    ) else {
        return;
    };
    let (x, y) = paper_doll_position(logical_w);
    blit(frame, fw, fh, &doll.data, doll.w, doll.h, x, y);
}

/// Horizontal center used by the original title/class text: the midpoint of the
/// gameplay region (`([0x81D738]+[0x81D744])/2`).
fn text_center(logical_w: i32) -> i32 {
    if logical_w <= 640 {
        (112 + 639) / 2
    } else {
        (140 + 799) / 2
    }
}

fn draw_text(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    sheet: &CharacterSheet,
    sel: usize,
) {
    let Some(name) = res.character_name(sheet.name_id) else {
        // Still draw the rest of the screen if the name table is unavailable.
        return draw_stats(frame, fw, fh, res, sheet);
    };
    // The original uses the large font and centers the name/class.
    let center = text_center(fw as i32);
    let height = if fw <= 640 { 480 } else { 600 };
    let name_y = height * 10 / 640;
    let (class_a, class_b) = if sel == 0 { (110, 126) } else { (138, 158) };
    let class_y = ((class_b - class_a) + 14) * height / 640;

    if let Some(font) = res.fonts.large.as_ref() {
        let w = font.measure(&name);
        font.draw(
            frame,
            fw,
            fh,
            &name,
            center - w / 2,
            name_y,
            Some([235, 235, 210]),
        );
        if let Some(class) = res.class_name(sheet.class) {
            let w = font.measure(&class);
            font.draw(
                frame,
                fw,
                fh,
                &class,
                center - w / 2,
                class_y,
                Some([235, 235, 210]),
            );
        }
    }
    draw_stats(frame, fw, fh, res, sheet);
}

/// Per-value `(x, y)` anchor tables, recovered from the original character-screen
/// renderer `FUN_0042cbd0` case 3 (VA `0x42D3xx`). Index order matches the draw
/// order:
///
/// ```text
/// 0 Опыт              6 Здоровье          12 Фехтование       18 Текущий вес
/// 1 Уровень           7 Харизма(c)        13 Идентиф.         19 Максим. вес
/// 2 Харизма           8 Сила(c)           14 Торговля         20 Отравление
/// 3 Сила              9 Ловкость(c)       15 Знахарство       21 Броня
/// 4 Ловкость         10 Выносливость(c)   16 Кузнечное        22 Удар
/// 5 Выносливость     11 Меткость          17 Строительство    23 Золото
///                                                             24 Свободный опыт
/// ```
const VALUE_640: [(i32, i32); 25] = [
    (413, 48),
    (415, 68),
    (532, 110),
    (532, 126),
    (532, 145),
    (532, 162),
    (310, 281),
    (482, 110),
    (482, 126),
    (482, 145),
    (482, 162),
    (482, 179),
    (482, 196),
    (567, 220),
    (567, 237),
    (567, 254),
    (567, 271),
    (567, 288),
    (470, 320),
    (470, 340),
    (310, 298),
    (310, 315),
    (310, 332),
    (310, 349),
    (575, 50),
];
const VALUE_800: [(i32, i32); 25] = [
    (519, 63),
    (519, 86),
    (665, 138),
    (665, 158),
    (665, 181),
    (665, 203),
    (388, 352),
    (602, 138),
    (602, 158),
    (602, 181),
    (602, 203),
    (602, 224),
    (602, 245),
    (715, 275),
    (715, 296),
    (715, 318),
    (715, 339),
    (715, 360),
    (588, 400),
    (588, 425),
    (388, 373),
    (388, 394),
    (388, 415),
    (388, 436),
    (700, 63),
];

/// Secondary `(x, y)` anchors at `0x465B34`: row 0 is the «Свободный опыт»
/// centering anchor; rows 0..=4 are the «Модиф. уровень» requirement slots.
const TABLE2_640: [(i32, i32); 5] = [(578, 70), (573, 110), (573, 126), (573, 145), (573, 162)];
const TABLE2_800: [(i32, i32); 5] = [(722, 89), (716, 138), (716, 158), (716, 181), (716, 203)];

fn class_from_entity(class: u8) -> Option<CharacterClass> {
    match class {
        2 => Some(CharacterClass::Merchant),
        3 => Some(CharacterClass::Hunter),
        4 => Some(CharacterClass::Chief),
        5 => Some(CharacterClass::Warrior),
        _ => None,
    }
}

/// Draw the dynamic values of the character screen. All coordinates are the
/// original **absolute** screen anchors and the original **small** font
/// (`0x58BC34`) is used, matching `FUN_0042cbd0` case 3 exactly.
fn draw_stats(frame: &mut [u8], fw: usize, fh: usize, res: &mut Res, sheet: &CharacterSheet) {
    let raise_icon = res.intr_sprite(RAISE_ICON);
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let sel = resolution_selector(fw as i32);
    let va = |i: usize| if sel == 0 { VALUE_640[i] } else { VALUE_800[i] };
    let t2 = |i: usize| {
        if sel == 0 {
            TABLE2_640[i]
        } else {
            TABLE2_800[i]
        }
    };
    let s = &sheet.snapshot;

    // «Текущий вес» = `FUN_0041b7e4`: sum of every carried item's weight.
    let weight: u32 = sheet
        .equipment
        .iter()
        .chain(sheet.additional.iter())
        .chain(sheet.inventory.iter())
        .chain(std::iter::once(&sheet.special))
        .map(|&id| item_weight(res, id))
        .sum();
    // «Броня» = `FUN_0041ab7c`: base armor (`+0x4E`) plus equipment slots 3..=5.
    let mut armor = s.base_armor as u32;
    for slot in 3..6 {
        if let Some(info) = res.item_info(sheet.equipment[slot]) {
            armor += info.rating as u32;
        }
    }
    // «Удар» = `FUN_0041aea8` normal path: active weapon slot (`+0x12` in {0,2}).
    let attack = if s.raw_0x12 == 0 || s.raw_0x12 == 2 {
        res.item_info(sheet.equipment[s.raw_0x12 as usize])
            .map(|i| i.rating as u32)
            .unwrap_or(0)
    } else {
        0
    };
    let health = (s.hp_internal as u32 + 15) / 16;

    let vals: [String; 25] = [
        format!("{}", s.invested_xp),
        format!("{}", s.level),
        format!("{}", s.charisma_base),
        format!("{}", s.strength_base),
        format!("{}", s.dexterity_base),
        format!("{}", s.endurance_base),
        format!("{}", health),
        format!("{}", s.charisma_current),
        format!("{}", s.strength_current),
        format!("{}", s.dexterity_current),
        format!("{}", s.endurance_current),
        format!("{}", s.archery_mastery / 100),
        format!("{}", s.melee_mastery / 100),
        format!("{}", s.identification),
        format!("{}", s.trade),
        format!("{}", s.healing),
        format!("{}", s.smithing),
        format!("{}", s.building),
        format!("{:.1}", weight as f64 * 0.001),
        format!("{:.1}", crate::character::max_load(s.endurance_base)),
        format!("{}", s.periodic_damage),
        format!("{}", armor),
        format!("{}", attack),
        format!("{}", s.money),
        format!("{}", s.available_xp),
    ];

    for (i, text) in vals.iter().enumerate() {
        let color = Some([235, 235, 210]);
        if i == 24 {
            let (cx, _) = t2(0);
            let (_, y) = va(24);
            let w = font.measure(text);
            let _ = font.draw(frame, fw, fh, text, cx - w / 2, y, color);
        } else {
            let (x, y) = va(i);
            let _ = font.draw(frame, fw, fh, text, x, y, color);
        }
    }

    // «поднять» column (the 5-slot loop): `FUN_00410b08` returns the tier cost to
    // raise the stat (`character::stat_cost`) when the player can afford it.
    if let Some(class) = class_from_entity(sheet.class) {
        // Row 0 is the icon-only "level up" control: `FUN_00410b08(entity, 0)`
        // gates it and the number slot is skipped.
        if can_raise_any(
            class,
            s.level,
            s.dexterity_current,
            s.charisma_current,
            s.strength_current,
            s.endurance_current,
        ) {
            if let Some(icon) = raise_icon.as_ref() {
                let (x, y) = t2(0);
                blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y + 2);
            }
        }
        for (slot, stat, value) in [
            (1usize, PrimaryStat::Charisma, s.charisma_current),
            (2, PrimaryStat::Strength, s.strength_current),
            (3, PrimaryStat::Dexterity, s.dexterity_current),
            (4, PrimaryStat::Endurance, s.endurance_current),
        ] {
            let cost = needed_raise(class, stat, value, s.level, s.available_xp);
            if cost != 0 {
                let (x, y) = t2(slot);
                let text = format!("{}", cost);
                let _ = font.draw(frame, fw, fh, &text, x, y, Some([235, 235, 210]));
                if let Some(icon) = raise_icon.as_ref() {
                    blit(frame, fw, fh, &icon.data, icon.w, icon.h, x + 0x18, y + 2);
                }
            }
        }
    }
}

/// `FUN_00410b08` value for one stat request (commands 2/3/5/6): `0` when the
/// level cap for the stat is already reached, otherwise the tier cost when
/// `available_xp` covers it.
pub fn stat_raise_cost(sheet: &CharacterSheet, stat: PrimaryStat) -> u32 {
    let Some(class) = class_from_entity(sheet.class) else {
        return 0;
    };
    let s = &sheet.snapshot;
    let value = match stat {
        PrimaryStat::Charisma => s.charisma_current,
        PrimaryStat::Strength => s.strength_current,
        PrimaryStat::Dexterity => s.dexterity_current,
        PrimaryStat::Endurance => s.endurance_current,
    };
    needed_raise(class, stat, value, s.level, s.available_xp)
}

/// `FUN_00410b08(entity, 0)`: whether the icon-only level-up row is available.
pub fn level_up_available(sheet: &CharacterSheet) -> bool {
    let Some(class) = class_from_entity(sheet.class) else {
        return false;
    };
    let s = &sheet.snapshot;
    can_raise_any(
        class,
        s.level,
        s.dexterity_current,
        s.charisma_current,
        s.strength_current,
        s.endurance_current,
    )
}

fn needed_raise(
    class: CharacterClass,
    stat: PrimaryStat,
    value: u16,
    level: u8,
    available_xp: u32,
) -> u32 {
    if stat_growth_cap(class, stat, level) as u16 <= value {
        return 0;
    }
    let cost = stat_cost(class, stat, value);
    if available_xp >= cost {
        cost
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
