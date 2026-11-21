//! Centralised movement/AI constants (extracted from `actors/motion.rs`).

/// Follow-formation distance threshold (cells): `FUN_0041f3c8` re-targets the
/// leader once `FUN_0043881c(leader, member) >= 10`.
pub const FOLLOW_DISTANCE: i32 = 10;

/// `FUN_0041f3c8` formation slots: `[leader.x & 1][direction][slot]` as signed
/// `(dx, dy)` cell offsets from the leader.
///
/// Recovered from `0x45FF1C` (the decompiler reads the words at `0x45FF19 + off`
/// and `0x45FF1a + off` and shifts by 24, i.e. bytes `off+3`/`off+4`; the payload
/// therefore starts at `0x45FF1C`). The block layout is `2 × 8 × 12 × 2` bytes:
/// the x-parity of the leader, then the eight facing codes, then twelve slots.
/// Slots sit 1–7 cells from the leader and the parity blocks mirror the same
/// ring for the odd/even projection column.
pub const FOLLOW_SLOTS: [[[(i8, i8); 12]; 8]; 2] = [
    [
        [(-1, 0), (-1, -1), (0, -2), (-3, -1), (-2, -2), (-1, -2), (-3, -2), (-2, -3), (-5, -2), (-4, -3), (-3, -3), (-1, -3)],
        [(-2, -1), (-3, 0), (-5, 0), (-4, -1), (-6, 0), (-6, -1), (-5, -1), (-4, -2), (-7, 0), (-7, -1), (-6, -2), (-7, -2)],
        [(-2, 1), (-3, 1), (-5, 1), (-4, 1), (-6, 0), (-6, 1), (-5, 2), (-4, 2), (-7, 1), (-7, 2), (-6, 2), (-7, 3)],
        [(-1, 1), (-1, 2), (0, 2), (-3, 2), (-2, 2), (-1, 3), (-3, 3), (-2, 3), (-5, 3), (-4, 3), (-3, 4), (-1, 4)],
        [(1, 1), (1, 2), (0, 2), (3, 2), (2, 2), (1, 3), (3, 3), (2, 3), (5, 3), (4, 3), (3, 4), (1, 4)],
        [(2, 1), (3, 1), (5, 1), (4, 1), (6, 0), (6, 1), (5, 2), (4, 2), (7, 1), (7, 2), (6, 2), (7, 3)],
        [(2, -1), (3, 0), (5, 0), (4, -1), (6, 0), (6, -1), (5, -1), (4, -2), (7, 0), (7, -1), (6, -2), (7, -2)],
        [(1, 0), (1, -1), (0, -2), (3, -1), (2, -2), (1, -2), (3, -2), (2, -3), (5, -2), (4, -3), (3, -3), (1, -3)],
    ],
    [
        [(-1, -1), (-1, -2), (0, -2), (-3, -2), (-2, -2), (-1, -3), (-3, -3), (-2, -3), (-5, -3), (-4, -3), (-3, -4), (-1, -4)],
        [(-2, -1), (-3, -1), (-5, -1), (-4, -1), (-6, 0), (-6, -1), (-5, -2), (-4, -2), (-7, -1), (-7, -2), (-6, -2), (-7, -3)],
        [(-2, 1), (-3, 0), (-5, 0), (-4, 1), (-6, 0), (-6, 1), (-5, 1), (-4, 2), (-7, 0), (-7, 1), (-6, 2), (-7, 2)],
        [(-1, 0), (-1, 1), (0, 2), (-3, 1), (-2, 2), (-1, 2), (-3, 2), (-2, 3), (-5, 2), (-4, 3), (-3, 3), (-1, 3)],
        [(1, 0), (1, 1), (0, 2), (3, 1), (2, 2), (1, 2), (3, 2), (2, 3), (5, 2), (4, 3), (3, 2), (1, 3)],
        [(2, 1), (3, 0), (5, 0), (4, 1), (6, 0), (6, 1), (5, 1), (4, 2), (7, 0), (7, 1), (6, 2), (7, 2)],
        [(2, -1), (3, -1), (5, -1), (4, -1), (6, 0), (6, -1), (5, -2), (4, -2), (7, -1), (7, -2), (6, -2), (7, -3)],
        [(1, -1), (1, -2), (0, -2), (3, -2), (2, -2), (1, -3), (3, -3), (2, -3), (5, -3), (4, -3), (3, -4), (1, -4)],
    ],
];

/// Maximum logic ticks consumed by one population/follower update.
pub const MAX_MOTION_TICKS_PER_FRAME: i64 = 10;

/// Wander idle cooldown base (logic ticks).
pub const WANDER_WAIT_MIN: u16 = 30;
/// Wander idle cooldown random span.
pub const WANDER_WAIT_SPAN: u32 = 60;
/// Citizen idle think skip: `rand()%100 < 50` skips a think for an ordinary
/// wanderer (`FUN_004168A4`).
pub const WANDER_THINK_SKIP_PERCENT: u32 = 50;
/// `FUN_004168A4`: the prop-like archetypes `Q`/`R` (`0x51`/`0x52`) skip
/// `rand()%100 < 75` of their thinks instead.
pub const WANDER_THINK_SKIP_PERCENT_QR: u32 = 75;
/// Archetypes `Q` (`0x51`) and `R` (`0x52`).
pub const WANDER_QR_ARCHETYPES: [u8; 2] = [0x51, 0x52];

/// Citizen-group class bit (`Group+0x1E & 0x80`).
pub const CITIZEN_GROUP_CLASS_BIT: u8 = 0x80;