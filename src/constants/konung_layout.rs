//! Centralised constants for `konung_layout` (extracted from the module sources).

/// Base-terrain layer offset in `Konung.N` (`0x00000`).
pub const TERRAIN_OFF: usize = 0x00000;
/// Base-terrain layer size (`0x06400` bytes, 80x160 `u16` records).
pub const TERRAIN_SIZE: usize = 0x06400;
/// Collision/passability source block (§8). Sits immediately after the base
/// terrain layer and is a 320x160 `u8` grid, `index = x * 160 + y`.
///
/// ORIGINAL-CODE VERIFIED (`FUN_0043adf0`): this **is** the Game.N
/// Entity/Group-spawn/Transition runtime grid. The loader expands it, marks
/// transition triggers (`|= 0x1000`) and world-object portals (`|= 0x2000`),
/// and movement indexes it directly with Entity `x/y`.
pub const COLLISION_OFF: usize = 0x06400;
/// Collision block size in bytes (`0x0C800` = 320x160 cells).
pub const COLLISION_BYTES: usize = 0x0C800;
/// Prop placement table offset in `Konung.N` (`0x16A80`).
pub const PLACEMENTS_OFF: usize = 0x16A80;
/// One prop placement record size (`36` bytes).
pub const PLACEMENT_SIZE: usize = 36;
/// Prop placement record count (`1000`).
pub const PLACEMENT_MAX: usize = 1000;
/// GRAPH placement table offset in `Konung.N` (`0x12C00`).
pub const GRAPH_PLACEMENTS_OFF: usize = 0x12C00;
/// One GRAPH placement record size (`16` bytes).
pub const GRAPH_PLACEMENT_SIZE: usize = 16;
/// GRAPH placement record count (`1000`).
pub const GRAPH_PLACEMENT_MAX: usize = 1000;
/// Full `Konung.N` size (`0x1F720`); shorter files fail boot.
pub const KONUNG_SIZE: usize = 0x1F720;
/// Object ids of the paired teleport-gate props (`244`/`245`).
pub const TELEPORT_IDS: [u16; 2] = [244, 245];
