//! Centralised constants for `game_layout` (extracted from the module sources).

/// Total `Game.N` save-file size in bytes (`0xAD4BE`).
pub const GAME_SIZE: usize = 0xAD4BE;
/// Group table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3).
pub const GROUP_OFF: usize = 0x20000;
/// Group record stride (`0x20` bytes).
pub const GROUP_STRIDE: usize = 0x20;
/// Group table count (`200`, group ids `0..=199`).
pub const GROUP_COUNT: usize = 200;
/// Entity table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3/§5).
///
/// `ENTITY_OFF` is intentionally unaligned (`0x2A69A`); reads go through the
/// byte-wise `res::{u16,u32}` helpers.
pub const ENTITY_OFF: usize = 0x2A69A;
/// Entity record stride (`0x100` bytes).
pub const ENTITY_STRIDE: usize = 0x100;
/// Entity table count (`2000` records).
pub const ENTITY_COUNT: usize = 2000;
/// One past the last byte of the Entity table.
pub const ENTITY_END: usize = ENTITY_OFF + ENTITY_COUNT * ENTITY_STRIDE;
/// Settlement table offset in `Game.N` (`Settlement[20]`, stride `0x4B5`).
pub const SETTLEMENT_OFF: usize = 0xA769A;
/// Settlement record stride (`0x4B5` bytes).
pub const SETTLEMENT_STRIDE: usize = 0x4B5;
/// Settlement table count (`20`).
pub const SETTLEMENT_COUNT: usize = 20;
/// Per-settlement waypoint list, at runtime `_DAT_0081d580 + 0x3D0`
/// (`FUN_004168a4` / `FUN_0041182c`).
pub const WAYPOINT_OFF: usize = 0x3D0;
/// Waypoint entry stride (`8` bytes: cell x/y + facing/role byte + owner).
pub const WAYPOINT_STRIDE: usize = 8;
/// Number of waypoint slots per settlement (`8`).
pub const WAYPOINT_MAX: usize = 8;
/// Settlement merchant stock lists (`FUN_00431E2C`): role 2 = 24 entries at
/// `+0x412`, role 3 = 32 at `+0x444`, role 4 = 23 at `+0x484`.
pub const TRADE_STOCK2_OFF: usize = 0x412;
/// Role-2 stock list length (`24` entries).
pub const TRADE_STOCK2_COUNT: usize = 24;
/// Role-3 stock list offset (`+0x444`, 32 entries).
pub const TRADE_STOCK3_OFF: usize = 0x444;
/// Role-3 stock list length (`32` entries).
pub const TRADE_STOCK3_COUNT: usize = 32;
/// Role-4 stock list offset (`+0x484`, 23 entries).
pub const TRADE_STOCK4_OFF: usize = 0x484;
/// Role-4 stock list length (`23` entries).
pub const TRADE_STOCK4_COUNT: usize = 23;
/// Role-2 herb/potion producer entity (`FUN_004175FC`), role 3 (`+0x3E2`),
/// role 4 equipment producer (`FUN_004179C8`), role 5 (`+0x3F2`).
pub const TRADE_PROD2_OFF: usize = 0x3DA;
/// Role-3 producer entity id (`+0x3E2`); no per-frame producer runs for role 3.
pub const TRADE_PROD3_OFF: usize = 0x3E2;
/// Role-4 equipment producer entity id (`+0x3EA`, `FUN_004179C8`).
pub const TRADE_PROD4_OFF: usize = 0x3EA;
/// Role-5 (voivode/barracks) entity id (`+0x3F2`, `FUN_00417F84`).
pub const TRADE_PROD5_OFF: usize = 0x3F2;
/// Settlement production timers (`FUN_004175FC` +0x4, `FUN_004179C8` +0x8,
/// role-3 producer +0xC) and role-4 pending stock slot `+0x4B4`.
pub const TRADE_TIMER2_OFF: usize = 0x4;
/// Role-4 production timer at settlement `+0x8` (`FUN_004179C8`).
pub const TRADE_TIMER4_OFF: usize = 0x8;
/// Role-3 producer timer at settlement `+0xC`.
pub const TRADE_TIMER3_OFF: usize = 0xC;
/// Role-4 pending-stock slot byte at `+0x4B4`.
pub const TRADE_LAST_SLOT_OFF: usize = 0x4B4;
/// Role-2 production quota counters at `+0x40C/+0x40E/+0x410`.
pub const TRADE_QUOTA0_OFF: usize = 0x40C;
/// Role-2 production quota counter at `+0x40E`.
pub const TRADE_QUOTA1_OFF: usize = 0x40E;
/// Role-2 production quota counter at `+0x410`.
pub const TRADE_QUOTA2_OFF: usize = 0x410;
/// Game.N WorldObject table (`KONUNG_SOURCE_OF_TRUTH.md` §14/§28.7).
/// The original loader (`FUN_0043A870`) reads 500 × 0x40 bytes into
/// `0x77DEB4`; `FUN_0043ADF0` filters active records by `+0x12` location.
pub const WORLD_OBJECT_OFF: usize = 0x21900;
/// WorldObject record stride (`0x40` bytes, 22 item ids at `+0x14`).
pub const WORLD_OBJECT_STRIDE: usize = 0x40;
/// WorldObject table count (`500` records).
pub const WORLD_OBJECT_COUNT: usize = 500;
/// Transition table offset (`250` records of [`TRANS_REC`] bytes).
pub const TRANS_OFF: usize = 0x29600;
/// Transition record size (`17` bytes; cell, facing, destination and arrival).
pub const TRANS_REC: usize = 17;
/// Transition table count (`250`).
pub const TRANS_COUNT: usize = 250;
/// Item-instance table offset (the loaded save file's instances start at 0).
pub const GAME_ITEMS_OFF: usize = 0x00000;
/// `Game.res` variant table offset (`320` records of [`GAME_VARIANT_SIZE`]).
pub const GAME_VARIANTS_OFF: usize = 0x4279A;
/// One `Game.res` variant record size (`256` bytes).
pub const GAME_VARIANT_SIZE: usize = 256;
/// Number of `Game.res` variant records (`320`).
pub const GAME_VARIANT_COUNT: usize = 320;
