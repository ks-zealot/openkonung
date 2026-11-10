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
/// Number of map locations (`20`); legacy count re-exported by `res`.
pub const MAP_COUNT: usize = 20;
/// Legacy per-map buffer size (`0x2F120` bytes).
///
/// TODO(docs): provenance not recovered.
pub const MAP_SIZE: usize = 0x2F120;
/// Legacy projected-grid column count (`160`) fed into [`MAP_PIXEL_W`].
pub const MAP_W: usize = 160;
/// Legacy projected-grid row count (`80`) fed into [`MAP_PIXEL_H`].
pub const MAP_H: usize = 80;
/// Projected isometric column step per grid `y` (`local_x = y*58`).
pub const GRID_X_STEP: i32 = 58;
/// Projected isometric row step per grid `x` (`local_y = x*16`).
pub const GRID_Y_STEP: i32 = 16;
/// Odd-row horizontal half-step of the projected grid (`29`).
pub const GRID_ODD_SHIFT: i32 = 29;
/// Base-terrain tile width in pixels (`114`).
pub const TERRAIN_TILE_W: i32 = 114;
/// Base-terrain tile height in pixels (`64`).
pub const TERRAIN_TILE_H: i32 = 64;
/// Projected-surface left origin in pixels (`29`, one half column).
pub const MAP_ORIGIN_X: i32 = 29;
/// Total projected-surface pixel width for the legacy grid.
pub const MAP_PIXEL_W: i32 = MAP_ORIGIN_X + (MAP_W as i32 - 1) * GRID_X_STEP + TERRAIN_TILE_W;
/// Total projected-surface pixel height for the legacy grid.
pub const MAP_PIXEL_H: i32 = (MAP_H as i32 - 1) * GRID_Y_STEP + TERRAIN_TILE_H;
/// Base-terrain record lattice: horizontal column step (source §7.2).
pub const TERRAIN_COL_STEP: i32 = 116;
/// Base-terrain record lattice: vertical row step (source §7.2).
pub const TERRAIN_ROW_STEP: i32 = 32;
/// Base-terrain record lattice: odd-row horizontal shift (source §7.2).
pub const TERRAIN_ODD_OFFSET: i32 = 58;
/// Base-terrain arrangement: 80 columns x 160 rows (12800 records).
pub const LOCAL_TERRAIN_W: i32 = 80;
/// Base-terrain arrangement: row count (`160`), the second axis of the record
/// lattice (`local_y = row * TERRAIN_ROW_STEP`).
pub const LOCAL_TERRAIN_H: i32 = 160;
/// Legacy terrain origin (`MAP_ORIGIN_X = 29`), retained for reference only.
/// All local-world render layers (terrain, props, GRAPH, entities) use origin
/// `0` per source-of-truth §7/§9/§10 (camera applied uniformly). Currently
/// unreferenced; do not treat it as a live render origin.
pub const LOCAL_ORIGIN_X: i32 = MAP_ORIGIN_X;
/// Total local-world pixel height of the base-terrain lattice.
pub const LOCAL_PIXEL_H: i32 = (LOCAL_TERRAIN_H - 1) * TERRAIN_ROW_STEP + TERRAIN_TILE_H;
