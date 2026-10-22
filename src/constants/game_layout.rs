//! Centralised constants for `game_layout` (extracted from the module sources).

pub const GAME_SIZE: usize = 0xAD4BE;
/// Group table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3).
pub const GROUP_OFF: usize = 0x20000;
pub const GROUP_STRIDE: usize = 0x20;
pub const GROUP_COUNT: usize = 200;
/// Entity table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3/§5).
///
/// `ENTITY_OFF` is intentionally unaligned (`0x2A69A`); reads go through the
/// byte-wise `res::{u16,u32}` helpers.
pub const ENTITY_OFF: usize = 0x2A69A;
pub const ENTITY_STRIDE: usize = 0x100;
pub const ENTITY_COUNT: usize = 2000;
/// One past the last byte of the Entity table.
pub const ENTITY_END: usize = ENTITY_OFF + ENTITY_COUNT * ENTITY_STRIDE;
/// Settlement table offset in `Game.N` (`Settlement[20]`, stride `0x4B5`).
pub const SETTLEMENT_OFF: usize = 0xA769A;
pub const SETTLEMENT_STRIDE: usize = 0x4B5;
pub const SETTLEMENT_COUNT: usize = 20;
/// Per-settlement waypoint list, at runtime `_DAT_0081d580 + 0x3D0`
/// (`FUN_004168a4` / `FUN_0041182c`).
pub const WAYPOINT_OFF: usize = 0x3D0;
pub const WAYPOINT_STRIDE: usize = 8;
pub const WAYPOINT_MAX: usize = 8;
/// Settlement merchant stock lists (`FUN_00431E2C`): role 2 = 24 entries at
/// `+0x412`, role 3 = 32 at `+0x444`, role 4 = 23 at `+0x484`.
pub const TRADE_STOCK2_OFF: usize = 0x412;
pub const TRADE_STOCK2_COUNT: usize = 24;
pub const TRADE_STOCK3_OFF: usize = 0x444;
pub const TRADE_STOCK3_COUNT: usize = 32;
pub const TRADE_STOCK4_OFF: usize = 0x484;
pub const TRADE_STOCK4_COUNT: usize = 23;
/// Role-2 herb/potion producer entity (`FUN_004175FC`), role 3 (`+0x3E2`),
/// role 4 equipment producer (`FUN_004179C8`), role 5 (`+0x3F2`).
pub const TRADE_PROD2_OFF: usize = 0x3DA;
pub const TRADE_PROD3_OFF: usize = 0x3E2;
pub const TRADE_PROD4_OFF: usize = 0x3EA;
pub const TRADE_PROD5_OFF: usize = 0x3F2;
/// Settlement production timers (`FUN_004175FC` +0x4, `FUN_004179C8` +0x8,
/// role-3 producer +0xC) and role-4 pending stock slot `+0x4B4`.
pub const TRADE_TIMER2_OFF: usize = 0x4;
pub const TRADE_TIMER4_OFF: usize = 0x8;
pub const TRADE_TIMER3_OFF: usize = 0xC;
pub const TRADE_LAST_SLOT_OFF: usize = 0x4B4;
/// Role-2 production quota counters at `+0x40C/+0x40E/+0x410`.
pub const TRADE_QUOTA0_OFF: usize = 0x40C;
pub const TRADE_QUOTA1_OFF: usize = 0x40E;
pub const TRADE_QUOTA2_OFF: usize = 0x410;
/// Game.N WorldObject table (`KONUNG_SOURCE_OF_TRUTH.md` §14/§28.7).
/// The original loader (`FUN_0043A870`) reads 500 × 0x40 bytes into
/// `0x77DEB4`; `FUN_0043ADF0` filters active records by `+0x12` location.
pub const WORLD_OBJECT_OFF: usize = 0x21900;
pub const WORLD_OBJECT_STRIDE: usize = 0x40;
pub const WORLD_OBJECT_COUNT: usize = 500;
pub const TRANS_OFF: usize = 0x29600;
pub const TRANS_REC: usize = 17;
pub const TRANS_COUNT: usize = 250;
pub const GAME_ITEMS_OFF: usize = 0x00000;
pub const GAME_VARIANTS_OFF: usize = 0x4279A;
pub const GAME_VARIANT_SIZE: usize = 256;
pub const GAME_VARIANT_COUNT: usize = 320;
