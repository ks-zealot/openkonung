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
//! runtime/entity grid (section 7.3); the two are different resolutions of the same
//! map and must not be forced into one another. The pixel surface below is a
//! renderer surface only - it is **not** a runtime-world bound, so runtime
//! Entity/collision coordinates must never be clamped to it.
//!
//! Konung.N block map as parsed here:
//!
//! ```text
//! 0x00000  0x06400  12800 x u16 base-terrain records (80 cols x 160 rows)
//! 0x06400  0x0C800  320 x 160 u8 collision/passability grid (index x*160 + y)
//! 0x12C00           1000 x 16-byte GRAPH terrain placements
//! 0x16A80           1000 x 36-byte object/prop placements
//! 0x1F720           full file size (KONUNG_SIZE) required for prop parsing
//! ```

use crate::coords::{RUNTIME_H, RUNTIME_W};
use crate::res::{i32v, u16, u32, GRID_ODD_SHIFT, GRID_X_STEP, GRID_Y_STEP, TERRAIN_TILE_W};

pub use crate::constants::coords::TERRAIN_COL_STEP;
pub use crate::constants::coords::TERRAIN_ODD_OFFSET;
pub use crate::constants::coords::TERRAIN_ROW_STEP;

pub use crate::constants::coords::LOCAL_ORIGIN_X;
pub use crate::constants::coords::LOCAL_TERRAIN_H;
pub use crate::constants::coords::LOCAL_TERRAIN_W;
/// Pixel surface implied by the corrected base-terrain lattice (renderer
/// surface only; not a runtime-world bound). Origin is 0.
pub const LOCAL_PIXEL_W: i32 =
    (LOCAL_TERRAIN_W - 1) * TERRAIN_COL_STEP + TERRAIN_ODD_OFFSET + TERRAIN_TILE_W;
pub use crate::constants::coords::LOCAL_PIXEL_H;

pub use crate::constants::konung_layout::COLLISION_BYTES;
pub use crate::constants::konung_layout::COLLISION_OFF;
pub use crate::constants::konung_layout::PLACEMENTS_OFF;
pub use crate::constants::konung_layout::PLACEMENT_MAX;
pub use crate::constants::konung_layout::PLACEMENT_SIZE;
pub use crate::constants::konung_layout::TERRAIN_OFF;
pub use crate::constants::konung_layout::TERRAIN_SIZE;

/// Fully blocked runtime cell (`FUN_0043ADF0`: `b == 0xFF` expands to `0x0FFF`).
pub const COLLISION_BLOCKED: u16 = 0x0FFF;
// GRAPH.RES placement layer: 1000 records x 16 bytes, sitting between the
// metadata block and the OBJECTS.RES placement table.
pub use crate::constants::konung_layout::GRAPH_PLACEMENTS_OFF;
pub use crate::constants::konung_layout::GRAPH_PLACEMENT_MAX;
pub use crate::constants::konung_layout::GRAPH_PLACEMENT_SIZE;
pub use crate::constants::konung_layout::KONUNG_SIZE;
pub use crate::constants::konung_layout::TELEPORT_IDS;

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
    (
        layer_resource((record & 0x00FF) as u8),
        layer_resource((record >> 8) as u8),
    )
}

/// Convert one terrain layer byte to a tile index: `0` means "no tile", else
/// `byte - 1`.
fn layer_resource(b: u8) -> Option<usize> {
    if b == 0 {
        None
    } else {
        Some((b - 1) as usize)
    }
}

/// One object/prop placement record from the Konung.N placement block.
///
/// The record's first word is stored as an object index biased by `30`; the
/// runtime grid position is read from `+0x20`/`+0x1E`, the palette offset from
/// `+0x04` and the interactive flag from bit 0 of `+0x22`.
#[derive(Clone)]
pub struct Prop {
    /// Placement-record index inside the 1000-record block.
    pub ri: usize,
    /// Resolved `OBJECTS.RES` block id (`record[0] + 30`).
    pub object_id: usize,
    /// Runtime-grid `+0x20` (source section 10.2).
    pub grid_x: i32,
    /// Runtime-grid `+0x1E` (source section 10.2).
    pub grid_y: i32,
    /// Record `+0x22 & 1` — interactive prop. The original renderer only sets
    /// the hovered-prop global `_DAT_0081D57C` for these, and the cursor/prop
    /// order paths (`FUN_00428740` `0x4287f7`, `FUN_00420F30`) test the same
    /// bit.
    /// Hovered-prop bit test source (`+0x22 & 1`), see the field above.
    pub interactive: bool,
    /// Palette offset from record `+0x04` (overrides the GRAPH tile palette for
    /// object sprites).
    pub palette: u32,
    /// Current object animation frame (`FUN_0043ADF0` prop `+0x23`).
    pub frame: u8,
    /// Flame emitters parsed from the object record (`+0x1C + i*5`).
    pub flame_emitters: Vec<crate::res::FlameEmitter>,
    /// Current FLAMES frame per emitter (prop record `+0x08 + i`).
    pub flame_frames: Vec<u8>,
}

/// A paired teleport-gate marker (two nearby `TELEPORT_IDS` props).
#[derive(Clone)]
pub struct Gate {
    /// Gate index in creation order (the order `group_gates` emits pairs).
    pub index: usize,
    /// The one or two props that form the gate.
    pub props: Vec<Prop>,
    /// Gate centroid x in runtime grid cells.
    pub x: f64,
    /// Gate centroid y in runtime grid cells.
    pub y: f64,
    /// Compass side (`"E"`, `"SE"`, ...) from the map centre, see [`gate_side`].
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
    /// Terrain tile index, `0..=511` (word at `+0x00`, masked to 9 bits).
    pub tile: u16,
    /// File-side palette offset (`+0x04`).
    pub palette: u32,
    /// Local pixel anchor x (`u16` at `+0x0C`).
    pub x: u16,
    /// Local pixel anchor y (`u16` at `+0x0E`).
    pub y: u16,
}

/// One fully parsed Konung.N map: placements, gates and the runtime collision
/// table.
pub struct Location {
    /// Map number (the `N` of the Konung.N file).
    pub loc_id: u32,
    /// The raw file bytes (terrain records are read on demand).
    pub data: Vec<u8>,
    /// Object/prop placements from `PLACEMENTS_OFF`.
    pub props: Vec<Prop>,
    /// Teleport-gate pairs derived from the props.
    pub gates: Vec<Gate>,
    /// GRAPH terrain placements from `GRAPH_PLACEMENTS_OFF`.
    pub graph_placements: Vec<GraphPlacement>,
    /// Expanded collision table: `320 * 160` u16, `index = x * 160 + y`
    /// (§8). Low 12 bits are the occupancy/passability state; bit `0x1000`
    /// marks a Transition trigger cell and `0x2000` a world-object/portal prop.
    ///
    /// It shares the Game.N Entity/transition runtime grid and **is** applied
    /// to player and NPC movement (`app.rs` / `actors.rs`).
    pub collision: Vec<u16>,
    /// Largest prop grid x or `LOCAL_TERRAIN_W - 1`, used as the gate-side
    /// reference width.
    pub max_x: i32,
    /// Largest prop grid y or `LOCAL_TERRAIN_H - 1`.
    pub max_y: i32,
}

impl Location {
    /// True when the low 12 bits of the cell are clear (MD §2.2). The cell
    /// indices are collision-grid coordinates, not transition coordinates.
    pub fn collision_free(&self, x: i32, y: i32) -> bool {
        collision_is_free(&self.collision, x, y)
    }

    /// Raw runtime collision word at `(x, y)`, or [`COLLISION_BLOCKED`] out of
    /// bounds. The high bits carry the original runtime flags (`0x1000`
    /// transition, `0x2000` ground WorldObject); the low 12 bits are
    /// occupancy/passability.
    pub fn collision_cell(&self, x: i32, y: i32) -> u16 {
        if x < 0 || y < 0 || x >= RUNTIME_W || y >= RUNTIME_H {
            return COLLISION_BLOCKED;
        }
        let idx = (x * RUNTIME_H + y) as usize;
        self.collision
            .get(idx)
            .copied()
            .unwrap_or(COLLISION_BLOCKED)
    }

    /// The 12800 `u16` base-terrain records of the file, read from
    /// `TERRAIN_OFF` up to `TERRAIN_OFF + TERRAIN_SIZE`.
    ///
    /// This is the authored terrain arrangement, not runtime occupancy; a
    /// truncated file simply yields fewer records.
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

/// Parse the 1000 GRAPH terrain-placement records at `0x12C00`.
///
/// Each 16-byte record is `{u32 tile, u32 palette, .., u16 x @+0x0C, u16 y
/// @+0x0E}`; `0xFFFFFFFF` marks an unused record. `x`/`y` are already local
/// pixel anchors and are used as-is.
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

/// Parse the object/prop placements at `PLACEMENTS_OFF`.
///
/// Parsing stops at the first record whose leading `i32` is negative (the
/// original terminator), skips ids outside `30..512`, and yields an empty list
/// when the file is shorter than `KONUNG_SIZE` (the block is then incomplete).
/// Flame emitters/frames start empty and are filled by the runtime layer.
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
            interactive: rec[0x22] & 1 != 0,
            palette: pal,
            frame: 0,
            flame_emitters: Vec::new(),
            flame_frames: Vec::new(),
        });
    }
    props
}

/// Compass side of a point relative to the map centre, quantized to eight
/// directions starting at east (`E`, `SE`, `S`, `SW`, `W`, `NW`, `N`, `NE`).
///
/// `max_x`/`max_y` define the half-extent the offset is normalised by; the
/// index is `round(angle / (PI/4))` wrapped into `0..8`.
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

/// Pair the teleport props (`TELEPORT_IDS`, `244`/`245`) into gates.
///
/// Each unused prop starts a gate and consumes its nearest remaining teleport
/// within 12 grid cells (squared distance `<= 144`); a lone prop forms a
/// single-member gate. Gates are numbered in creation order and their centroid
/// is classified with [`gate_side`] against the largest prop coordinate.
pub fn group_gates(props: &[Prop]) -> Vec<Gate> {
    let tp: Vec<Prop> = props
        .iter()
        .filter(|p| TELEPORT_IDS.contains(&(p.object_id as u16)))
        .cloned()
        .collect();
    if tp.is_empty() {
        return Vec::new();
    }
    let max_x = tp
        .iter()
        .map(|p| p.grid_x)
        .max()
        .unwrap_or(0)
        .max(LOCAL_TERRAIN_W - 1);
    let max_y = tp
        .iter()
        .map(|p| p.grid_y)
        .max()
        .unwrap_or(0)
        .max(LOCAL_TERRAIN_H - 1);

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

/// Parse one Konung.N map into a [`Location`].
///
/// Rejects a file too short for the collision block (with the map number in the
/// message) instead of producing an all-free map; props, gates and GRAPH
/// placements are parsed leniently as described on their parsers.
pub fn make_location(loc_id: u32, data: Vec<u8>) -> Result<Location, String> {
    let props = parse_props(&data);
    let max_x = props
        .iter()
        .map(|p| p.grid_x)
        .max()
        .unwrap_or(0)
        .max(LOCAL_TERRAIN_W - 1);
    let max_y = props
        .iter()
        .map(|p| p.grid_y)
        .max()
        .unwrap_or(0)
        .max(LOCAL_TERRAIN_H - 1);
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
mod tests;
