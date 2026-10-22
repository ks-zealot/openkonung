//! Centralised combat runtime constants (extracted from `app/combat.rs`).

/// `Entity+0x04` bit selecting the object/monster graphics family.
pub const OBJECT_ARCHETYPE_BIT: u8 = 0x40;

/// Periodic combat damage is applied every 16 logic frames
/// (`FUN_0041D048`: `combat_frame & 0xF == 0`).
pub const DOT_INTERVAL_MASK: u64 = 0xF;

/// `FUN_00418480` prop settlement roll (`rng % 100 < 21`).
pub const SETTLEMENT_PROP_HIT_CHANCE: u32 = 21;
/// Settlement target state written when a blocked prop is hit.
pub const SETTLEMENT_PROP_HIT_STATE: u8 = 3;
/// Settlement target timer written when a blocked prop is hit.
pub const SETTLEMENT_PROP_HIT_TIMER: u16 = 1;
/// Settlement alert flags set by the prop-hit branch (`+0x4B2 |= 0x15`).
pub const SETTLEMENT_ALERT_FLAGS: u8 = 0x15;

/// Ranged kill money reward: `power / 4`.
pub const RANGED_KILL_MONEY_DIVISOR: i32 = 4;

/// Collision grid row stride (`index = x * 160 + y`).
pub const COLLISION_ROW_STRIDE: i32 = 160;
/// Collision flags marking a blocked cell (bits `8..=9`).
pub const COLLISION_SOLID_MASK: u16 = 0xC0;

/// Maximum logic ticks consumed by one combat update.
pub const MAX_COMBAT_TICKS_PER_FRAME: i64 = 10;

/// Diagnostic combat staging values (used by the headless smoke tests).
pub const COMBAT_TEST_TARGET_HP: u16 = 120;
pub const COMBAT_TEST_TARGET_MONEY: i32 = 7;
pub const COMBAT_TEST_VENOM: u16 = 2;
pub const RANGED_TEST_BOW_ITEM: u16 = 23;
pub const RANGED_TEST_AMMO_ITEM: u16 = 27;
pub const RANGED_TEST_AMMO_QUANTITY: u32 = 30;
pub const RANGED_TEST_BOW_DURABILITY: f32 = 56.0;
pub const RANGED_TEST_TARGET_HP: u16 = 3000;
pub const RANGED_TEST_TARGET_STRENGTH: u16 = 100;
pub const RANGED_TEST_TARGET_ENDURANCE: u16 = 10;
pub const RANGED_TEST_MIN_DISTANCE: i32 = 4;
pub const RANGED_TEST_MAX_DISTANCE: i32 = 10;
pub const RANGED_TEST_TARGET_ARROW_ITEM: u16 = 102;
pub const RANGED_TEST_DODGE_DEXTERITY: u16 = 2000;
pub const PROP_TEST_SEARCH_RADIUS: i32 = 10;
