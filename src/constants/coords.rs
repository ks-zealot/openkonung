//! Centralised constants for `coords` (extracted from the module sources).

/// Collision/runtime-grid width (source §3 / `loc.rs`).
pub const RUNTIME_W: i32 = 320;
/// Collision/runtime-grid height (source §3 / `loc.rs`).
pub const RUNTIME_H: i32 = 160;
/// Maximum runtime-grid `x` (inclusive).
pub const RUNTIME_MAX_X: i32 = RUNTIME_W - 1;
/// Maximum runtime-grid `y` (inclusive).
pub const RUNTIME_MAX_Y: i32 = RUNTIME_H - 1;
/// Entity projection: local-x step per entity `y`.
pub const ENTITY_COL_STEP: i32 = 58;
/// Entity projection: odd/even `x` horizontal stagger half-step.
pub const ENTITY_HALF_STEP: i32 = 29;
/// Entity projection: local-y step per entity `x`.
pub const ENTITY_ROW_STEP: i32 = 16;
/// Entity projection: constant local-y bias.
pub const ENTITY_ROW_BIAS: i32 = 16;
pub const MAP_COUNT: usize = 20;
pub const MAP_SIZE: usize = 0x2F120;
pub const MAP_W: usize = 160;
pub const MAP_H: usize = 80;
pub const GRID_X_STEP: i32 = 58;
pub const GRID_Y_STEP: i32 = 16;
pub const GRID_ODD_SHIFT: i32 = 29;
pub const TERRAIN_TILE_W: i32 = 114;
pub const TERRAIN_TILE_H: i32 = 64;
pub const MAP_ORIGIN_X: i32 = 29;
pub const MAP_PIXEL_W: i32 = MAP_ORIGIN_X + (MAP_W as i32 - 1) * GRID_X_STEP + TERRAIN_TILE_W;
pub const MAP_PIXEL_H: i32 = (MAP_H as i32 - 1) * GRID_Y_STEP + TERRAIN_TILE_H;
/// Base-terrain record lattice: horizontal column step (source §7.2).
pub const TERRAIN_COL_STEP: i32 = 116;
/// Base-terrain record lattice: vertical row step (source §7.2).
pub const TERRAIN_ROW_STEP: i32 = 32;
/// Base-terrain record lattice: odd-row horizontal shift (source §7.2).
pub const TERRAIN_ODD_OFFSET: i32 = 58;
/// Base-terrain arrangement: 80 columns x 160 rows (12800 records).
pub const LOCAL_TERRAIN_W: i32 = 80;
pub const LOCAL_TERRAIN_H: i32 = 160;
/// Legacy terrain origin (`MAP_ORIGIN_X = 29`), retained for reference only.
/// All local-world render layers (terrain, props, GRAPH, entities) use origin
/// `0` per source-of-truth §7/§9/§10 (camera applied uniformly). Currently
/// unreferenced; do not treat it as a live render origin.
pub const LOCAL_ORIGIN_X: i32 = MAP_ORIGIN_X;
pub const LOCAL_PIXEL_H: i32 = (LOCAL_TERRAIN_H - 1) * TERRAIN_ROW_STEP + TERRAIN_TILE_H;
