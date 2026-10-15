//! Local Konung.N map parsing.
//!
//! `KONUNG_SOURCE_OF_TRUTH.md` Revision 3: the base-terrain layer is
//! 12800 `u16` records arranged as **80 columns x 160 rows**, rendered on the
//! original macro lattice:
//!
//! ```text
//! terrain_local_x = col * 116 + (row & 1) * 58
//! terrain_local_y = row * 32
//! ```
//!
//! The 80x160 terrain-record lattice has twice the step of the 320x160
//! runtime/entity grid (§7.3); the two are different resolutions of the same
//! map and must not be forced into one another. The pixel surface below is a
//! renderer surface only — it is **not** a runtime-world bound, so runtime
//! Entity/collision coordinates must never be clamped to it.

use crate::coords::{RUNTIME_H, RUNTIME_W};
use crate::res::{i32v, u16, u32, MAP_ORIGIN_X, GRID_ODD_SHIFT, GRID_X_STEP, GRID_Y_STEP, TERRAIN_TILE_H, TERRAIN_TILE_W};

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
/// Pixel surface implied by the corrected base-terrain lattice (renderer
/// surface only; not a runtime-world bound). Origin is 0.
pub const LOCAL_PIXEL_W: i32 =
    (LOCAL_TERRAIN_W - 1) * TERRAIN_COL_STEP + TERRAIN_ODD_OFFSET + TERRAIN_TILE_W;
pub const LOCAL_PIXEL_H: i32 = (LOCAL_TERRAIN_H - 1) * TERRAIN_ROW_STEP + TERRAIN_TILE_H;

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
// GRAPH.RES placement layer: 1000 records x 16 bytes, sitting between the
// metadata block and the OBJECTS.RES placement table.
pub const GRAPH_PLACEMENTS_OFF: usize = 0x12C00;
pub const GRAPH_PLACEMENT_SIZE: usize = 16;
pub const GRAPH_PLACEMENT_MAX: usize = 1000;
pub const KONUNG_SIZE: usize = 0x1F720;
pub const TELEPORT_IDS: [u16; 2] = [244, 245];

/// Pure base-terrain-record lattice (source §7.2/§23.3). No renderer origin.
pub fn terrain_to_local_px(col: i32, row: i32) -> (i32, i32) {
    (
        col * TERRAIN_COL_STEP + (row & 1) * TERRAIN_ODD_OFFSET,
        row * TERRAIN_ROW_STEP,
    )
}

/// GRAPH placement anchor → local pixel (source §9/§21.2). The `+0C/+0E` fields
/// are already local pixel anchors; this is identity and must never be run
/// through the terrain or Entity grid projections.
pub fn graph_anchor_to_local_px(x: i32, y: i32) -> (i32, i32) {
    (x, y)
}

/// Prop/object runtime-grid → local pixel (source §10.3/§21.3):
/// canonical fields `grid_x = +0x20`, `grid_y = +0x1E`.
pub fn prop_to_local_px(grid_x: i32, grid_y: i32) -> (i32, i32) {
    (
        grid_y * GRID_X_STEP - (grid_x & 1) * GRID_ODD_SHIFT,
        grid_x * GRID_Y_STEP,
    )
}

/// Split a base-terrain `u16` into its two independent drawable layers
/// (source §7.1/§23.5). Each byte is `0 => no tile`, else `resource index =
/// byte - 1`. Returns `(layer0_low, layer1_high)`.
pub fn terrain_layers(record: u16) -> (Option<usize>, Option<usize>) {
    (layer_resource((record & 0x00FF) as u8), layer_resource((record >> 8) as u8))
}

fn layer_resource(b: u8) -> Option<usize> {
    if b == 0 {
        None
    } else {
        Some((b - 1) as usize)
    }
}

#[derive(Clone)]
pub struct Prop {
    pub ri: usize,
    pub object_id: usize,
    /// Runtime-grid `+0x20` (source §10.2).
    pub grid_x: i32,
    /// Runtime-grid `+0x1E` (source §10.2).
    pub grid_y: i32,
    pub palette: u32,
}

#[derive(Clone)]
pub struct Gate {
    pub index: usize,
    pub props: Vec<Prop>,
    pub x: f64,
    pub y: f64,
    pub side: &'static str,
}

/// One record of the Konung.N GRAPH.RES placement layer at 0x12C00.
///
/// `tile` indexes the full 0..511 GRAPH terrain bank. `palette` is the
/// file-side palette offset (runtime-patched by the original, and diverging
/// from the GRAPH table on many records), so rendering uses the GRAPH table
/// palette via `Res::terrain_tile` instead. `x`/`y` are already local pixel
/// anchors (source §9/§21.2) and are used directly, never grid-projected.
#[derive(Clone)]
pub struct GraphPlacement {
    pub tile: u16,
    pub palette: u32,
    pub x: u16,
    pub y: u16,
}

pub struct Location {
    pub loc_id: u32,
    pub data: Vec<u8>,
    pub props: Vec<Prop>,
    pub gates: Vec<Gate>,
    pub graph_placements: Vec<GraphPlacement>,
    /// Expanded collision table: `320 * 160` u16, `index = x * 160 + y`
    /// (§8). Low 12 bits are the occupancy/passability state; bit `0x1000`
    /// marks a Transition trigger cell and `0x2000` a world-object/portal prop.
    ///
    /// It shares the Game.N Entity/transition runtime grid and **is** applied
    /// to player and NPC movement (`app.rs` / `actors.rs`).
    pub collision: Vec<u16>,
    pub max_x: i32,
    pub max_y: i32,
}

impl Location {
    /// True when the low 12 bits of the cell are clear (MD §2.2). The cell
    /// indices are collision-grid coordinates, not transition coordinates.
    pub fn collision_free(&self, x: i32, y: i32) -> bool {
        collision_is_free(&self.collision, x, y)
    }

    pub fn terrain_cells(&self) -> Vec<u16> {
        let mut cells = Vec::with_capacity(LOCAL_TERRAIN_W as usize * LOCAL_TERRAIN_H as usize);
        let end = (TERRAIN_OFF + TERRAIN_SIZE).min(self.data.len());
        let mut o = TERRAIN_OFF;
        while o + 2 <= end {
            cells.push(u16(&self.data, o) as u16);
            o += 2;
        }
        cells
    }
}

/// Expand the Konung.N collision/passability source block at `0x06400`.
///
/// MD §2.2: the block is a `320 x 160` `u8` grid with `index = x * 160 + y`,
/// expanded into the runtime `u16` table as:
///
/// ```text
/// if b == 0xFF:              collision = 0x0FFF
/// else if (b & 0x0F) == 0x0F: collision = (b << 8) | 0x0FFF
/// else:                       collision = b << 8
/// ```
///
/// Returns an error (rather than a silent all-free table) when the file is too
/// short to contain the block, so a truncated Konung.N fails loudly.
pub fn parse_collision(data: &[u8]) -> Result<Vec<u16>, String> {
    let required = COLLISION_OFF + COLLISION_BYTES;
    if data.len() < required {
        return Err(format!(
            "collision block truncated: file is {} bytes, need at least {} (0x{:X})",
            data.len(),
            required,
            required
        ));
    }
    let mut out = vec![0u16; (RUNTIME_W * RUNTIME_H) as usize];
    for x in 0..RUNTIME_W as usize {
        for y in 0..RUNTIME_H as usize {
            let b = data[COLLISION_OFF + x * RUNTIME_H as usize + y];
            let v = if b == 0xFF {
                0x0FFF
            } else if (b & 0x0F) == 0x0F {
                ((b as u16) << 8) | 0x0FFF
            } else {
                (b as u16) << 8
            };
            out[x * RUNTIME_H as usize + y] = v;
        }
    }
    Ok(out)
}

/// Query the runtime collision table with runtime bounds checking.
pub fn collision_is_free(collision: &[u16], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= RUNTIME_W || y >= RUNTIME_H {
        return false;
    }
    let idx = (x * RUNTIME_H + y) as usize;
    idx < collision.len() && (collision[idx] & 0x0FFF) == 0
}

pub fn parse_graph_placements(data: &[u8]) -> Vec<GraphPlacement> {
    let mut out = Vec::new();
    for ri in 0..GRAPH_PLACEMENT_MAX {
        let o = GRAPH_PLACEMENTS_OFF + ri * GRAPH_PLACEMENT_SIZE;
        if o + GRAPH_PLACEMENT_SIZE > data.len() {
            break;
        }
        let rec = &data[o..o + GRAPH_PLACEMENT_SIZE];
        let tile = u32(rec, 0);
        if tile == 0xFFFFFFFF {
            continue;
        }
        out.push(GraphPlacement {
            tile: (tile & 0x1FF) as u16,
            palette: u32(rec, 4),
            x: u16(rec, 12) as u16,
            y: u16(rec, 14) as u16,
        });
    }
    out
}

pub fn parse_props(data: &[u8]) -> Vec<Prop> {
    let mut props = Vec::new();
    if data.len() < KONUNG_SIZE {
        return props;
    }
    for ri in 0..PLACEMENT_MAX {
        let o = PLACEMENTS_OFF + ri * PLACEMENT_SIZE;
        if o + PLACEMENT_SIZE > data.len() {
            break;
        }
        let rec = &data[o..o + PLACEMENT_SIZE];
        let raw = i32v(rec, 0);
        if raw < 0 {
            break;
        }
        let oid = raw + 30;
        if !(30..512).contains(&oid) {
            continue;
        }
        // Canonical runtime-grid fields (source §10.2): +0x20 = grid_x,
        // +0x1E = grid_y.
        let grid_x = u16(rec, 0x20) as i32;
        let grid_y = u16(rec, 0x1E) as i32;
        let pal = u32(rec, 0x04);
        props.push(Prop {
            ri,
            object_id: oid as usize,
            grid_x,
            grid_y,
            palette: pal,
        });
    }
    props
}

pub fn gate_side(x: f64, y: f64, max_x: i32, max_y: i32) -> &'static str {
    let cx = max_x as f64 / 2.0;
    let cy = max_y as f64 / 2.0;
    let dx = (x - cx) / (max_x as f64).max(1.0);
    let dy = (y - cy) / (max_y as f64).max(1.0);
    let a = dy.atan2(dx);
    let dirs = ["E", "SE", "S", "SW", "W", "NW", "N", "NE"];
    let idx = (a / (std::f64::consts::PI / 4.0)).round() as i64;
    dirs[(idx.rem_euclid(8)) as usize]
}

pub fn group_gates(props: &[Prop]) -> Vec<Gate> {
    let tp: Vec<Prop> = props
        .iter()
        .filter(|p| TELEPORT_IDS.contains(&(p.object_id as u16)))
        .cloned()
        .collect();
    if tp.is_empty() {
        return Vec::new();
    }
    let max_x = tp.iter().map(|p| p.grid_x).max().unwrap_or(0).max(LOCAL_TERRAIN_W - 1);
    let max_y = tp.iter().map(|p| p.grid_y).max().unwrap_or(0).max(LOCAL_TERRAIN_H - 1);

    let mut unused: Vec<usize> = (0..tp.len()).collect();
    let mut groups = Vec::new();
    while !unused.is_empty() {
        let i = unused.remove(0);
        let a = &tp[i];
        let mut best: Option<usize> = None;
        let mut bestd = i64::MAX;
        for (k, &j) in unused.iter().enumerate() {
            let b = &tp[j];
            let d = (a.grid_x - b.grid_x) as i64 * (a.grid_x - b.grid_x) as i64
                + (a.grid_y - b.grid_y) as i64 * (a.grid_y - b.grid_y) as i64;
            if d < bestd {
                bestd = d;
                best = Some(k);
            }
        }
        let mut members = vec![a.clone()];
        if let Some(k) = best {
            if bestd <= 12 * 12 {
                let j = unused.remove(k);
                members.push(tp[j].clone());
            }
        }
        let gx = members.iter().map(|p| p.grid_x as f64).sum::<f64>() / members.len() as f64;
        let gy = members.iter().map(|p| p.grid_y as f64).sum::<f64>() / members.len() as f64;
        let index = groups.len();
        groups.push(Gate {
            index,
            props: members,
            x: gx,
            y: gy,
            side: gate_side(gx, gy, max_x, max_y),
        });
    }
    groups
}

pub fn make_location(loc_id: u32, data: Vec<u8>) -> Result<Location, String> {
    let props = parse_props(&data);
    let max_x = props.iter().map(|p| p.grid_x).max().unwrap_or(0).max(LOCAL_TERRAIN_W - 1);
    let max_y = props.iter().map(|p| p.grid_y).max().unwrap_or(0).max(LOCAL_TERRAIN_H - 1);
    let gates = group_gates(&props);
    let graph_placements = parse_graph_placements(&data);
    let collision = parse_collision(&data).map_err(|e| format!("Konung.{}: {}", loc_id, e))?;
    Ok(Location {
        loc_id,
        data,
        props,
        gates,
        graph_placements,
        collision,
        max_x,
        max_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collision_buffer() -> Vec<u8> {
        vec![0u8; COLLISION_OFF + COLLISION_BYTES]
    }

    fn cell_idx(x: usize, y: usize) -> usize {
        COLLISION_OFF + x * RUNTIME_H as usize + y
    }

    #[test]
    fn collision_expansion_rules() {
        let mut data = collision_buffer();
        data[cell_idx(10, 20)] = 0xFF;
        data[cell_idx(5, 7)] = 0x12;
        data[cell_idx(300, 150)] = 0x1F;

        let c = parse_collision(&data).unwrap();
        assert_eq!(c[10 * RUNTIME_H as usize + 20], 0x0FFF);
        assert_eq!(c[5 * RUNTIME_H as usize + 7], 0x1200);
        assert_eq!(c[300 * RUNTIME_H as usize + 150], 0x1FFF);
    }

    #[test]
    fn collision_decode_canonical_bytes() {
        // (input byte, expanded u16, passable).
        let cases: [(u8, u16, bool); 5] = [
            (0x00, 0x0000, true),
            // Low nibble 0x0F => (b << 8) | 0x0FFF, whose low 12 bits are 0xFFF.
            (0x0F, 0x0FFF, false),
            (0x10, 0x1000, true),
            (0x1F, 0x1FFF, false),
            (0xFF, 0x0FFF, false),
        ];

        let mut data = collision_buffer();
        for (i, &(b, _, _)) in cases.iter().enumerate() {
            data[cell_idx(i, i)] = b;
        }

        let c = parse_collision(&data).unwrap();
        for (i, &(b, expanded, passable)) in cases.iter().enumerate() {
            let got = c[i * RUNTIME_H as usize + i];
            assert_eq!(got, expanded, "byte 0x{b:02X} expanded to 0x{got:04X}");
            assert_eq!(
                collision_is_free(&c, i as i32, i as i32),
                passable,
                "byte 0x{b:02X} passability"
            );
        }
    }

    #[test]
    fn collision_index_is_x_major_not_transposed() {
        let mut data = collision_buffer();
        data[cell_idx(10, 20)] = 0x11;
        data[cell_idx(20, 10)] = 0x22;

        let c = parse_collision(&data).unwrap();
        assert_eq!(c[10 * RUNTIME_H as usize + 20], 0x1100);
        assert_eq!(c[20 * RUNTIME_H as usize + 10], 0x2200);
    }

    #[test]
    fn collision_truncated_file_errors() {
        let data = vec![0xFFu8; 16];
        assert!(parse_collision(&data).is_err());
    }

    #[test]
    fn collision_free_checks_low_12_bits_and_bounds() {
        let mut data = collision_buffer();
        // 0x10 -> 0x1000: low nibble 0 => low 12 bits clear => free.
        data[cell_idx(1, 1)] = 0x10;
        // 0x20 -> 0x2000: free.
        data[cell_idx(2, 2)] = 0x20;
        // 0x00 -> 0: free.
        data[cell_idx(3, 3)] = 0x00;
        // 0x01 -> 0x0100: low 12 bits non-zero => blocked.
        data[cell_idx(4, 4)] = 0x01;
        // 0x12 -> 0x1200: blocked.
        data[cell_idx(5, 5)] = 0x12;
        // 0xFF -> 0x0FFF: blocked.
        data[cell_idx(6, 6)] = 0xFF;

        let c = parse_collision(&data).unwrap();
        assert!(collision_is_free(&c, 1, 1));
        assert!(collision_is_free(&c, 2, 2));
        assert!(collision_is_free(&c, 3, 3));
        assert!(!collision_is_free(&c, 4, 4));
        assert!(!collision_is_free(&c, 5, 5));
        assert!(!collision_is_free(&c, 6, 6));
        assert!(!collision_is_free(&c, -1, 0));
        assert!(!collision_is_free(&c, 0, -1));
        assert!(!collision_is_free(&c, RUNTIME_W, 0));
        assert!(!collision_is_free(&c, 0, RUNTIME_H));
    }

    #[test]
    fn prop_projection_exact_cases() {
        // Source-of-truth §23.2 (grid_x = +0x20, grid_y = +0x1E).
        assert_eq!(prop_to_local_px(0, 0), (0, 0));
        assert_eq!(prop_to_local_px(1, 0), (-29, 16));
        assert_eq!(prop_to_local_px(2, 0), (0, 32));
        assert_eq!(prop_to_local_px(5, 7), (377, 80));
    }

    #[test]
    fn terrain_layers_decode_both_bytes() {
        assert_eq!(terrain_layers(0x0201), (Some(0), Some(1)));
        assert_eq!(terrain_layers(0x0000), (None, None));
        assert_eq!(terrain_layers(0x0100), (None, Some(0)));
        assert_eq!(terrain_layers(0x0001), (Some(0), None));
        assert_eq!(terrain_layers(0xFFFF), (Some(254), Some(254)));
    }

    #[test]
    fn graph_anchor_is_identity() {
        assert_eq!(graph_anchor_to_local_px(0, 0), (0, 0));
        assert_eq!(graph_anchor_to_local_px(1624, 320), (1624, 320));
        assert_eq!(graph_anchor_to_local_px(3654, 3646), (3654, 3646));
    }

    #[test]
    fn terrain_lattice_exact_cases() {
        assert_eq!(terrain_to_local_px(0, 0), (0, 0));
        assert_eq!(terrain_to_local_px(1, 0), (116, 0));
        assert_eq!(terrain_to_local_px(0, 1), (58, 32));
        assert_eq!(terrain_to_local_px(1, 1), (174, 32));
        assert_eq!(terrain_to_local_px(2, 1), (2 * 116 + 58, 32));
    }

    #[test]
    fn surface_dimensions_double_vs_half_scale() {
        // Old half-scale surface was 4725 x 2608; corrected lattice roughly
        // doubles both dimensions.
        assert!(LOCAL_PIXEL_W > 9000 && LOCAL_PIXEL_W < 9600, "W={LOCAL_PIXEL_W}");
        assert!(LOCAL_PIXEL_H > 5000 && LOCAL_PIXEL_H < 5300, "H={LOCAL_PIXEL_H}");
        // Entity maxima still fit inside the surface (source §3 / CHECK F).
        let (mx, _) = crate::coords::entity_to_local_px(0, 159);
        let (_, my) = crate::coords::entity_to_local_px(319, 0);
        assert!(mx < LOCAL_PIXEL_W, "entity max x {mx} >= {LOCAL_PIXEL_W}");
        assert!(my < LOCAL_PIXEL_H, "entity max y {my} >= {LOCAL_PIXEL_H}");
    }
}
