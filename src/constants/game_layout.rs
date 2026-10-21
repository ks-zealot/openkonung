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
