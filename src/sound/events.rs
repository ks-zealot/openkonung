//! Original sound-event ids and the tables that select them.
//!
//! Every entry here is recovered from a direct `FUN_0042A13C` call site in
//! `konung.res` (see `SOUND_SYSTEM.md`):
//!
//! - UI / menu click `4` (`0x43687e`, `0x435dca`, `0x422914`, `0x4242e6`,
//!   `0x424362`, `0x42146c`, `0x421568`, `0x4216f0`, `0x422480`,
//!   `0x41e8a8`);
//! - item use `9` (`0x41e0e9`) and class-11 map/scroll item `10`
//!   (`0x414de4` cases `0x14`/`0x18` and the common tail);
//! - equipment `0` (additional slot, `0x41e872`) or `11`/`1` (main slot,
//!   `0x41e680`: `1` when the equipped or displaced item class is `2`);
//! - dodge `6` (`0x421e1c`, `FUN_0041c3cc` return `-2`);
//! - human action table `0x4146e8` (death `34`, idle `35`, attack `36`,
//!   walk `7`, ranged `3`, melee `2`/`8`);
//! - monster action table `0x414480` (`arch*8 - 0x1D0`, `+2`, `+3`, `+4`);
//! - ambient bank `0x100 + (location - 1) * 8 + slot` (`0x4365d4`, 2% per
//!   gameplay frame).
//!
//! Some ids have no clip in `SOUNDS.RES` (notably human walk `7` and idle
//! `35`); the original `FUN_0042A13C` checks the clip size and stays silent,
//! and the browser shell does the same.

/// Shared menu / HUD / inventory click (`FUN_0042A13C(4, ...)`).
pub const UI_CLICK: u16 = 4;
/// Successful facing dodge (`FUN_0041c3cc` returns `-2`).
pub const DODGE: u16 = 6;
/// In-place item use (`FUN_0041dab0` / `FUN_0041e0e9`).
pub const ITEM_USE: u16 = 9;
/// Class-11 map/scroll item action (`FUN_00414de4`).
pub const MAP_ITEM: u16 = 10;
/// Equipment placed into an additional/special slot (`FUN_0041e6bc`).
pub const EQUIP_OTHER: u16 = 0;
/// Equipment placed into a main weapon slot (`FUN_0041e450`).
pub const EQUIP_MAIN: u16 = 11;
/// Main-slot weapon class that selects `1` instead of [`EQUIP_MAIN`].
pub const WEAPON_CLASS: u8 = 2;

/// Human action-row ids (`0x4146e8`); `action` is `Entity+0x00`.
///
/// Idle/walk ids are absent from the shipped archive, so they are returned for
/// completeness and filtered by the player. The `roll` argument is `rand()%100`
/// (`0..=99`) and gates only the idle sound (95%).
pub fn human_action(action: u8, weapon_slot: u8, leader: bool, roll: u32) -> Option<u16> {
    match action {
        0 | 6 => (roll < 95).then_some(35),
        1 | 7 => leader.then_some(7),
        2 | 8 => Some(36),
        3 | 9 => Some(34),
        4 => Some(3),
        5 => Some(if weapon_slot == 2 { 8 } else { 2 }),
        _ => None,
    }
}

/// Melee impact id for a human (`FUN_0041435C` action row `5`).
pub fn human_melee_impact(weapon_slot: u8) -> u16 {
    if weapon_slot == 2 {
        8
    } else {
        2
    }
}

/// Monster action-row ids (`0x414480`); `action` is `Entity+0x00`.
///
/// The bank starts at `arch*8 - 0x1D0`:
/// `+0` melee, `+1` ranged, `+2` death, `+3` idle/walk, `+4` attack.
/// `roll` gates idle (95%) and walk (75%) exactly like the original.
pub fn monster_action(archetype: u8, action: u8, roll: u32) -> Option<u16> {
    if archetype & 0x40 == 0 {
        return None;
    }
    let base = (archetype as u16).wrapping_mul(8).wrapping_sub(0x1D0);
    match action {
        0 => (roll < 95).then_some(base + 3),
        1 => (roll < 75).then_some(base + 3),
        2 | 8 => Some(base + 4),
        3 | 9 => Some(base + 2),
        4 => Some(base + 1),
        5 => Some(base),
        _ => None,
    }
}

/// Select the action sound for either graphics family.
pub fn actor_action(
    archetype: u8,
    action: u8,
    weapon_slot: u8,
    leader: bool,
    roll: u32,
) -> Option<u16> {
    if archetype & 0x40 != 0 {
        monster_action(archetype, action, roll)
    } else {
        human_action(action, weapon_slot, leader, roll)
    }
}

/// Equipment sound for the main weapon slot (`FUN_0041e450`).
///
/// The original checks the class of both the incoming and the displaced item:
/// class `2` selects `1`, otherwise `11`.
pub fn equip_main(class: u8, displaced_class: Option<u8>) -> u16 {
    if class == WEAPON_CLASS || displaced_class == Some(WEAPON_CLASS) {
        1
    } else {
        EQUIP_MAIN
    }
}

/// Per-location ambient bank id (`0x4365d4`).
///
/// `location` is the 1-based `Konung.N` id (`_DAT_0081D6C4`) and `slot` is
/// `rand()%8`. Missing slots/locations have no clip and are skipped by the
/// shell.
pub fn ambient(location: u32, slot: u32) -> u16 {
    let base = 0x100u32 + location.saturating_sub(1) * 8;
    (base + slot % 8) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Human action rows map to the documented clip ids, with the 95% idle gate
    /// and the leader-only walk clip.
    #[test]
    fn human_table_matches_original_cases() {
        assert_eq!(human_action(3, 0, true, 0), Some(34)); // death
        assert_eq!(human_action(0, 0, true, 0), Some(35)); // idle
        assert_eq!(human_action(2, 0, true, 0), Some(36)); // attack
        assert_eq!(human_action(1, 0, true, 0), Some(7)); // leader walk
        assert_eq!(human_action(1, 0, false, 0), None); // companion walk
        assert_eq!(human_action(4, 0, true, 0), Some(3)); // ranged
        assert_eq!(human_action(5, 0, true, 0), Some(2));
        assert_eq!(human_action(5, 2, true, 0), Some(8));
        assert_eq!(human_action(0, 0, true, 95), None);
    }

    /// Archetype 65 yields bank base 56 and the `+0..+4` action offsets.
    #[test]
    fn monster_bank_matches_arch65() {
        // 65*8 - 0x1D0 = 56.
        assert_eq!(monster_action(65, 5, 0), Some(56));
        assert_eq!(monster_action(65, 4, 0), Some(57));
        assert_eq!(monster_action(65, 3, 0), Some(58));
        assert_eq!(monster_action(65, 0, 94), Some(59));
        assert_eq!(monster_action(65, 0, 95), None);
        assert_eq!(monster_action(65, 1, 74), Some(59));
        assert_eq!(monster_action(65, 1, 75), None);
        assert_eq!(monster_action(65, 2, 0), Some(60));
    }

    /// Human archetypes (the `0x40` bit clear) have no monster bank at all.
    #[test]
    fn non_object_archetype_has_no_monster_bank() {
        assert_eq!(monster_action(0x04, 2, 0), None);
    }

    /// Item class `2` - incoming or displaced - selects clip `1` instead of `11`.
    #[test]
    fn equipment_class_two_selects_one() {
        assert_eq!(equip_main(1, None), 11);
        assert_eq!(equip_main(2, None), 1);
        assert_eq!(equip_main(1, Some(2)), 1);
    }

    /// Each location owns eight consecutive ambient ids starting at `0x100`.
    #[test]
    fn ambient_banks_are_eight_clips_per_location() {
        assert_eq!(ambient(1, 0), 0x100);
        assert_eq!(ambient(1, 7), 0x107);
        assert_eq!(ambient(4, 3), 0x100 + 3 * 8 + 3);
        assert_eq!(ambient(50, 7), 0x100 + 49 * 8 + 7);
    }
}
