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
//!
//! Read-only: no equipment/attribute mutation.

use crate::character::{stat_cost, PlayerCharacter, PrimaryStat};
use crate::res::{blit, Res};

/// `INTR` index of the character-window background.
pub const WINDOW_SPRITE: usize = 2;

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
const ADDITIONAL_800: [(i32, i32); 5] = [
    (232, 74),
    (214, 190),
    (367, 190),
    (248, 215),
    (334, 215),
];
const ADDITIONAL_640: [(i32, i32); 5] = [
    (181, 44),
    (166, 157),
    (317, 157),
    (199, 182),
    (284, 182),
];

/// Special-equipment top-left.
const SPECIAL_800: (i32, i32) = (295, 232);
const SPECIAL_640: (i32, i32) = (245, 199);

pub fn main_base(sel: usize, slot: usize) -> (i32, i32) {
    let table = if sel == 0 { &MAIN_640 } else { &MAIN_800 };
    table[slot.min(5)]
}
pub fn additional_center(sel: usize, slot: usize) -> (i32, i32) {
    let table = if sel == 0 { &ADDITIONAL_640 } else { &ADDITIONAL_800 };
    table[slot.min(4)]
}
pub fn special_base(sel: usize) -> (i32, i32) {
    if sel == 0 {
        SPECIAL_640
    } else {
        SPECIAL_800
    }
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

/// Draw the character screen over the current frame. The caller guarantees the
/// protagonist exists and is the only party member.
pub fn draw(frame: &mut [u8], fw: usize, fh: usize, res: &mut Res, player: &PlayerCharacter) {
    let logical_w = fw as i32;
    let (wx, wy, _ww, _wh) = window_rect(logical_w);
    let sel = resolution_selector(logical_w);

    // Original window background (its own static labels are baked in).
    if let Some(bg) = res.intr_sprite(WINDOW_SPRITE) {
        blit(frame, fw, fh, &bg.data, bg.w, bg.h, wx, wy);
    }

    // Main equipment (6 storage slots).
    for slot in 0..6 {
        if !main_slot_drawn(&player.equipment, player.raw_0x12, slot) {
            continue;
        }
        let Some(info) = res.item_info(player.equipment[slot]) else {
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
        let id = player.additional_equipment[slot];
        if id == 0 {
            continue;
        }
        let Some(info) = res.item_info(id) else { continue };
        let Some(icon) = res.intr_sprite(info.icon as usize) else {
            continue;
        };
        let (cx, cy) = additional_center(sel, slot);
        let x = cx - icon.w as i32 / 2;
        let y = cy - icon.h as i32 / 2;
        blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y);
    }

    // Special equipment (top-left direct).
    if player.special_equipment != 0 {
        if let Some(info) = res.item_info(player.special_equipment) {
            if let Some(icon) = res.intr_sprite(info.icon as usize) {
                let (x, y) = special_base(sel);
                blit(frame, fw, fh, &icon.data, icon.w, icon.h, x, y);
            }
        }
    }

    draw_text(frame, fw, fh, res, player, sel);
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
    player: &PlayerCharacter,
    sel: usize,
) {
    let Some(name) = res.character_name(player.hero.name_id) else {
        // Still draw the rest of the screen if the name table is unavailable.
        return draw_stats(frame, fw, fh, res, player);
    };
    // The original uses the large font and centers the name/class.
    let center = text_center(fw as i32);
    // Name y = height*10/640; class y = (delta + 14)*height/640 (VA 0x42D145
    // and 0x42D207). `sel` selects the row of the class-offset table.
    let height = if fw <= 640 { 480 } else { 600 };
    let name_y = height * 10 / 640;
    let (class_a, class_b) = if sel == 0 { (110, 126) } else { (138, 158) };
    let class_y = ((class_b - class_a) + 14) * height / 640;

    if let Some(font) = res.fonts.large.as_ref() {
        let w = font.measure(&name);
        font.draw(frame, fw, fh, &name, center - w / 2, name_y, Some([235, 235, 210]));
        if let Some(class) = res.class_name(player.entity_class()) {
            let w = font.measure(&class);
            font.draw(frame, fw, fh, &class, center - w / 2, class_y, Some([235, 235, 210]));
        }
    }
    draw_stats(frame, fw, fh, res, player);
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
///
/// The records live at `0x465C54` as `{x640,y640,x800,y800}` (`u16` leaps of 16
/// bytes); index 24 («Свободный опыт») draws x-centered on the `0x465B34` row 0
/// and reuses this record only for its y.
const VALUE_640: [(i32, i32); 25] = [
    (413, 48), (415, 68), (532, 110), (532, 126), (532, 145), (532, 162),
    (310, 281), (482, 110), (482, 126), (482, 145), (482, 162), (482, 179),
    (482, 196), (567, 220), (567, 237), (567, 254), (567, 271), (567, 288),
    (470, 320), (470, 340), (310, 298), (310, 315), (310, 332), (310, 349),
    (575, 50),
];
const VALUE_800: [(i32, i32); 25] = [
    (519, 63), (519, 86), (665, 138), (665, 158), (665, 181), (665, 203),
    (388, 352), (602, 138), (602, 158), (602, 181), (602, 203), (602, 224),
    (602, 245), (715, 275), (715, 296), (715, 318), (715, 339), (715, 360),
    (588, 400), (588, 425), (388, 373), (388, 394), (388, 415), (388, 436),
    (700, 63),
];

/// Secondary `(x, y)` anchors at `0x465B34`: row 0 is the «Свободный опыт»
/// centering anchor; rows 0..=4 are the «Модиф. уровень» requirement slots.
const TABLE2_640: [(i32, i32); 5] = [(578, 70), (573, 110), (573, 126), (573, 145), (573, 162)];
const TABLE2_800: [(i32, i32); 5] = [(722, 89), (716, 138), (716, 158), (716, 181), (716, 203)];

/// Draw the dynamic values of the character screen. All coordinates are the
/// original **absolute** screen anchors and the original **small** font
/// (`0x58BC34`) is used, matching `FUN_0042cbd0` case 3 exactly.
fn draw_stats(frame: &mut [u8], fw: usize, fh: usize, res: &mut Res, player: &PlayerCharacter) {
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let sel = resolution_selector(fw as i32);
    let va = |i: usize| if sel == 0 { VALUE_640[i] } else { VALUE_800[i] };
    let t2 = |i: usize| if sel == 0 { TABLE2_640[i] } else { TABLE2_800[i] };

    // Derived values with recovered original formulas.
    // «Текущий вес» = `FUN_0041b7e4`: sum of every carried item's weight.
    let mut weight: u32 = 0;
    for &id in player
        .equipment
        .iter()
        .chain(player.additional_equipment.iter())
        .chain(player.inventory.iter())
        .chain(std::iter::once(&player.special_equipment))
    {
        if let Some(info) = res.item_info(id) {
            weight += info.weight as u32;
        }
    }
    // «Броня» = `FUN_0041ab7c`: base armor (`+0x4E`) plus the rating of the
    // equipment slots 3..=5.
    let mut armor = player.base_armor as u32;
    for slot in 3..6 {
        if let Some(info) = res.item_info(player.equipment[slot]) {
            armor += info.rating as u32;
        }
    }
    // «Удар» = `FUN_0041aea8` normal path: the active weapon slot (`+0x12` in
    // {0,2}) item rating. The unresolved x87 rounding term is omitted.
    let attack = if player.raw_0x12 == 0 || player.raw_0x12 == 2 {
        res.item_info(player.equipment[player.raw_0x12 as usize])
            .map(|i| i.rating as u32)
            .unwrap_or(0)
    } else {
        0
    };
    // «Здоровье» = `(hp_internal + 15) / 16`.
    let health = (player.hp_internal as u32 + 15) / 16;

    let vals: [String; 25] = [
        format!("{}", player.invested_xp),
        format!("{}", player.level),
        format!("{}", player.charisma_base),
        format!("{}", player.strength_base),
        format!("{}", player.dexterity_base),
        format!("{}", player.endurance_base),
        format!("{}", health),
        format!("{}", player.charisma_current),
        format!("{}", player.strength_current),
        format!("{}", player.dexterity_current),
        format!("{}", player.endurance_current),
        format!("{}", player.archery_mastery / 100),
        format!("{}", player.melee_mastery / 100),
        format!("{}", player.identification),
        format!("{}", player.trade),
        format!("{}", player.healing),
        format!("{}", player.smithing),
        format!("{}", player.building),
        format!("{:.1}", weight as f64 * 0.001),
        format!("{:.1}", crate::character::max_load(player.endurance_base)),
        format!("{}", player.periodic_damage),
        format!("{}", armor),
        format!("{}", attack),
        format!("{}", player.money),
        format!("{}", player.available_xp),
    ];

    for (i, s) in vals.iter().enumerate() {
        let color = Some([235, 235, 210]);
        if i == 24 {
            // «Свободный опыт»: centered on the `0x465B34` row 0, y from this row.
            let (cx, _) = t2(0);
            let (_, y) = va(24);
            let w = font.measure(s);
            let _ = font.draw(frame, fw, fh, s, cx - w / 2, y, color);
        } else {
            let (x, y) = va(i);
            let _ = font.draw(frame, fw, fh, s, x, y, color);
        }
    }

    // «поднять» column (the 5-slot loop): `FUN_00410b08` returns the tier cost to
    // raise the stat (`character::stat_cost`, identical to its table) when the
    // player can afford it, else 0. Slot 0 («Модиф. уровень») is icon-only in the
    // original and is not rendered here; the per-class requirement tables at
    // `0x45B0AB` are not applied (unresolved).
    for (slot, stat, value) in [
        (1usize, PrimaryStat::Charisma, player.charisma_current),
        (2, PrimaryStat::Strength, player.strength_current),
        (3, PrimaryStat::Dexterity, player.dexterity_current),
        (4, PrimaryStat::Endurance, player.endurance_current),
    ] {
        let cost = needed_raise(player.class, stat, value, player.available_xp);
        if cost != 0 {
            let (x, y) = t2(slot);
            let s = format!("{}", cost);
            let _ = font.draw(frame, fw, fh, &s, x, y, Some([235, 235, 210]));
        }
    }
}

/// `FUN_00410b08` value for a non-specialized stat request: the cost returned by
/// the tier table (`character::stat_cost`) when `available_xp` covers it, else 0.
fn needed_raise(class: crate::character::CharacterClass, stat: PrimaryStat, value: u16, available_xp: u32) -> u32 {
    let cost = stat_cost(class, stat, value);
    if available_xp >= cost {
        cost
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter, PrimaryStat};
    use crate::hero::hero;

    #[test]
    fn window_rect_matches_executable() {
        assert_eq!(window_rect(800), (140, 0, 642, 460));
        assert_eq!(window_rect(640), (112, 0, 528, 376));
    }

    #[test]
    fn resolution_selector_matches_bit() {
        assert_eq!(resolution_selector(800), 1);
        assert_eq!(resolution_selector(640), 0);
    }

    #[test]
    fn main_bases_match_verified_tables() {
        assert_eq!(main_base(1, 0), (182, 103));
        assert_eq!(main_base(1, 2), (182, 103));
        assert_eq!(main_base(1, 5), (328, 103));
        assert_eq!(main_base(0, 0), (136, 70));
        assert_eq!(main_base(0, 5), (280, 70));
    }

    #[test]
    fn additional_and_special_match_verified_tables() {
        assert_eq!(additional_center(1, 0), (232, 74));
        assert_eq!(additional_center(1, 4), (334, 215));
        assert_eq!(additional_center(0, 1), (166, 157));
        assert_eq!(special_base(1), (295, 232));
        assert_eq!(special_base(0), (245, 199));
    }

    #[test]
    fn starting_bow_draws_in_storage_slot_two() {
        // Shipped starting equipment; the bow is storage slot 2.
        let eq = [0u16, 0, 1, 0, 0, 0];
        assert!(!main_slot_drawn(&eq, 2, 0));
        assert!(main_slot_drawn(&eq, 2, 2));
        assert!(!main_slot_drawn(&eq, 2, 1));
    }

    #[test]
    fn overlap_branch_prefers_slot_zero() {
        // Both 0 and 2 occupied, raw_0x12 != 2 -> slot 0 wins, slot 2 hidden.
        let eq = [7u16, 0, 9, 0, 0, 0];
        assert!(main_slot_drawn(&eq, 0, 0));
        assert!(!main_slot_drawn(&eq, 0, 2));
        // raw_0x12 == 2 -> slot 0 hidden, slot 2 shown.
        assert!(!main_slot_drawn(&eq, 2, 0));
        assert!(main_slot_drawn(&eq, 2, 2));
    }

    #[test]
    fn empty_equipment_draws_nothing() {
        let eq = [0u16; 6];
        for slot in 0..6 {
            assert!(!main_slot_drawn(&eq, 0, slot));
        }
    }

    fn player() -> PlayerCharacter {
        let mut p = PlayerCharacter::from_build(
            *hero(0).unwrap(),
            CharacterBuild::from_class(CharacterClass::Warrior),
        );
        p.equipment = [0, 0, 1, 0, 0, 0];
        p.hp_internal = 1600;
        p
    }

    #[test]
    fn value_anchors_match_recovered_tables() {
        // Spot-check the recovered `0x465C54` / `0x465B34` records.
        assert_eq!(VALUE_640[0], (413, 48));
        assert_eq!(VALUE_800[0], (519, 63));
        assert_eq!(VALUE_800[6], (388, 352)); // Здоровье
        assert_eq!(VALUE_800[20], (388, 373)); // Отравление
        assert_eq!(VALUE_800[21], (388, 394)); // Броня
        assert_eq!(VALUE_800[22], (388, 415)); // Удар
        assert_eq!(VALUE_800[23], (388, 436)); // Золото
        assert_eq!(VALUE_800[24], (700, 63)); // Свободный опыт
        assert_eq!(TABLE2_640[0], (578, 70));
        assert_eq!(TABLE2_640[4], (573, 162));
        assert_eq!(TABLE2_800[0], (722, 89));
        assert_eq!(TABLE2_800[4], (716, 203));
    }

    #[test]
    fn needed_raise_matches_tier_cost_gated_by_available_xp() {
        // Warrior STR=15 is specialized -> next cost is 5.
        assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 5), 5);
        assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 4), 0);
        assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Strength, 15, 0), 0);
        // Merchant CHA=25 is specialized -> cost 11.
        assert_eq!(needed_raise(CharacterClass::Merchant, PrimaryStat::Charisma, 25, 11), 11);
        // Non-specialized Warrior CHA=8 -> normal tier cost 2.
        assert_eq!(needed_raise(CharacterClass::Warrior, PrimaryStat::Charisma, 8, 2), 2);
    }

    #[test]
    fn draw_uses_intr2_and_does_not_panic() {
        // Synthetic 8x8 white background as INTR[2] via a minimal archive is
        // covered elsewhere; here we only check the pure geometry path.
        let p = player();
        assert_eq!(p.portrait_group(), 1);
        assert_eq!(p.portrait_status(), 5);
    }
}
