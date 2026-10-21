//! Original entity action / stance constants (`konung.res`, PE base `0x400000`).
//!
//! The original stores the current action at `Entity+0x00` and selects the
//! action from `Entity+0x03` bit `0x04` (weapon drawn / battle stance):
//!
//! | logical action | battle (`+3 & 4` set) | peaceful (`+3 & 4` clear) |
//! |---|---|---|
//! | idle/stand | `0` | `6` |
//! | walk       | `1` | `7` |
//! | attack     | `2` | `8` |
//! | death      | `3` | `9` |
//!
//! Evidence:
//! - `FUN_00414A34` (`0x414A34`) idle: `6` when clear, `0` when set.
//! - `FUN_004142D4` (`0x4142D4`) walk: `7` when clear, `1` when set.
//! - attack `2`/`8` (`0x4123BC` / `0x415FC8` case 5).
//! - death `3`/`9` (`FUN_0041D048` `0x41D048`).
//! - special actions `4` (use/melee) and `5` (engage) are not mode-paired
//!   (`FUN_00412E00` `0x412E00`).
//!
//! The human GRAPH block has 10 action rows of `0xD0` bytes (`8 * 0x1A`), so
//! these values index the table directly (no modulo). The object/monster block
//! has 6 rows of `0x130`; actions `6..=11` alias `0..=5`.

/// `Entity+0x03` bit `0x04`: weapon drawn / battle stance.
pub const FLAG_BATTLE: u8 = 0x04;

/// Human/object logical action rows.
pub const ACT_BATTLE_IDLE: usize = 0;
pub const ACT_BATTLE_WALK: usize = 1;
pub const ACT_BATTLE_ATTACK: usize = 2;
pub const ACT_BATTLE_DEATH: usize = 3;
/// Ranged/special attack row used by the archery path (`FUN_004123BC` case 4).
pub const ACT_RANGED: usize = 4;
pub const ACT_SPECIAL_4: usize = 4;
pub const ACT_SPECIAL_5: usize = 5;
pub const ACT_PEACE_IDLE: usize = 6;
pub const ACT_PEACE_WALK: usize = 7;
pub const ACT_PEACE_ATTACK: usize = 8;
pub const ACT_PEACE_DEATH: usize = 9;

/// Idle/stand action for the current stance.
pub fn idle_action(battle: bool) -> usize {
    if battle {
        ACT_BATTLE_IDLE
    } else {
        ACT_PEACE_IDLE
    }
}

/// Walk action for the current stance.
pub fn walk_action(battle: bool) -> usize {
    if battle {
        ACT_BATTLE_WALK
    } else {
        ACT_PEACE_WALK
    }
}

/// Attack action for the current stance.
pub fn attack_action(battle: bool) -> usize {
    if battle {
        ACT_BATTLE_ATTACK
    } else {
        ACT_PEACE_ATTACK
    }
}

/// Death action for the current stance.
pub fn death_action(battle: bool) -> usize {
    if battle {
        ACT_BATTLE_DEATH
    } else {
        ACT_PEACE_DEATH
    }
}

/// Battle flag from a raw `Entity+0x03` (`flags_b`) byte.
pub fn battle_from_flags(flags_b: u8) -> bool {
    flags_b & FLAG_BATTLE != 0
}

#[cfg(test)]
mod tests;
