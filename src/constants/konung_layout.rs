//! Centralised constants for `konung_layout` (extracted from the module sources).

pub const TERRAIN_OFF: usize = 0x00000;
pub const TERRAIN_SIZE: usize = 0x06400;
/// Collision/passability source block (§8). Sits immediately after the base
/// terrain layer and is a 320x160 `u8` grid, `index = x * 160 + y`.
///
/// ORIGINAL-CODE VERIFIED (`FUN_0043adf0`): this **is** the Game.N
/// Entity/Group-spawn/Transition runtime grid. The loader expands it, marks
/// transition triggers (`|= 0x1000`) and world-object portals (`|= 0x2000`),
/// and movement indexes it directly with Entity `x/y`.
pub const COLLISION_OFF: usize = 0x06400;
pub const COLLISION_BYTES: usize = 0x0C800;
pub const PLACEMENTS_OFF: usize = 0x16A80;
pub const PLACEMENT_SIZE: usize = 36;
pub const PLACEMENT_MAX: usize = 1000;
pub const GRAPH_PLACEMENTS_OFF: usize = 0x12C00;
pub const GRAPH_PLACEMENT_SIZE: usize = 16;
pub const GRAPH_PLACEMENT_MAX: usize = 1000;
pub const KONUNG_SIZE: usize = 0x1F720;
pub const TELEPORT_IDS: [u16; 2] = [244, 245];
