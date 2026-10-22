//! Centralised movement/AI constants (extracted from `actors/motion.rs`).

/// Follow-formation distance threshold (cells): `FUN_0041f3c8` re-targets the
/// leader once `FUN_0043881c(leader, member) >= 10`.
pub const FOLLOW_DISTANCE: i32 = 10;

/// Maximum logic ticks consumed by one population/follower update.
pub const MAX_MOTION_TICKS_PER_FRAME: i64 = 10;

/// Wander idle cooldown base (logic ticks).
pub const WANDER_WAIT_MIN: u16 = 30;
/// Wander idle cooldown random span.
pub const WANDER_WAIT_SPAN: u32 = 60;

/// Citizen-group class bit (`Group+0x1E & 0x80`).
pub const CITIZEN_GROUP_CLASS_BIT: u8 = 0x80;

/// Citizen idle think skip: `rng() % 2 == 0` skips a think.
pub const WANDER_THINK_SKIP_DIVISOR: u32 = 2;
