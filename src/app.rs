//! OpenKonung live-game state machine: [`PlayableApp`] and its subsystem modules.

use std::collections::{HashMap, HashSet};

use crate::actors::{build_population, find_path, sort_actors, update_population, PopulationBuild, LOGIC_TICK_HZ};
use crate::coords::{entity_to_local_px_f, local_px_to_entity, step_by_direction};
use crate::edges::{Edge, Pair};
use crate::character::PlayerCharacter;
use crate::loc::prop_to_local_px;
use crate::population::{GameTables, Settlement};
use crate::render::{scale_image, LayerCounts, World, WorldSprite};
use crate::res::{blit, fill_ellipse, fill_rect, line, Img};


mod camera;
mod debug_overlay;
mod graph;
mod input;
mod player;
mod population;
mod portals;
mod text;
mod world_draw;

use self::portals::*;

pub use crate::constants::render::VIRTUAL_W;
pub use crate::constants::render::VIRTUAL_H;

pub use crate::constants::keys::KEY_ESCAPE;
pub use crate::constants::keys::KEY_BACKSPACE;
pub use crate::constants::keys::KEY_TAB;
pub use crate::constants::keys::KEY_C;
pub use crate::constants::keys::KEY_D;
pub use crate::constants::keys::KEY_F;
pub use crate::constants::keys::KEY_H;
pub use crate::constants::keys::KEY_L;
pub use crate::constants::keys::KEY_P;
pub use crate::constants::keys::KEY_R;
pub use crate::constants::keys::KEY_T;
pub use crate::constants::keys::KEY_V;

pub struct TextItem {
    pub x: i32,
    pub y: i32,
    pub rgb: u32,
    pub scale: f32,
    pub s: String,
}

/// Approximate rendered width of a monospace text run in virtual pixels. The
/// browser shell renders `14 * scale` px Consolas (~0.6em advance); 0.62 is a
/// slight over-estimate so wrapping errs on the safe side.
fn text_px(s: &str, scale: f32) -> i32 {
    (s.chars().count() as f32 * 14.0 * scale * 0.62).ceil() as i32
}

/// True while a marker is close enough to the 800x600 play view to be labelled.
fn on_screen(x: i32, y: i32) -> bool {
    (-8..=VIRTUAL_W + 8).contains(&x) && (-8..=VIRTUAL_H + 8).contains(&y)
}

/// Greedy word wrap to at most `max_px` virtual pixels per line, hard-splitting
/// any single word that cannot fit on its own line.
fn wrap_to_px(s: &str, scale: f32, max_px: i32) -> Vec<String> {
    let max_px = max_px.max(48);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        if cur.is_empty() {
            if text_px(word, scale) > max_px {
                let mut chunk = String::new();
                for ch in word.chars() {
                    if text_px(&format!("{}{}", chunk, ch), scale) > max_px && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                cur = chunk;
            } else {
                cur.push_str(word);
            }
            continue;
        }
        let trial = format!("{} {}", cur, word);
        if text_px(&trial, scale) <= max_px {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    lines.push(cur);
    lines
}

pub struct PlayableApp {
    pub world: World,
    pub edges: Vec<Edge>,
    pub pairs: Vec<Pair>,
    pub one_way: Vec<usize>,
    pub self_edges: Vec<usize>,
    pub layout: HashMap<u32, (f64, f64)>,
    pub profile: u32,
    location_names: Vec<String>,
    debug_ids: bool,

    pub current_map: u32,
    pub selected: Option<u32>,
    pub mode_graph: bool,

    /// One authoritative runtime player: hero identity + class + the
    /// character-creation values copied from the final [`crate::character::CharacterBuild`].
    pub player: PlayerCharacter,
    /// Player position in runtime/Entity coordinates (continuous during motion).
    pub player_x: f64,
    pub player_y: f64,
    pub player_dir: i32,
    target: Option<(f64, f64)>,
    /// Remaining movement-direction codes (0..=7) to `target`. Each completed
    /// walk-animation cycle consumes one code and steps one cell
    /// (`FUN_0043DA1F` queue + `FUN_0041435C`), like the original.
    player_path: Vec<u8>,
    /// Seconds accumulated toward the next player cell step.
    player_step_t: f64,
    anim_time: f64,
    teleport_cooldown: f64,
    last_teleport: Option<(u32, u32, usize)>,
    /// Last teleport failure (destination not loaded), surfaced on the HUD.
    teleport_error: Option<String>,
    /// Toggle the canonical transition debug overlay (trigger rects, arrival
    /// points, rec / from->to, visual 244/245 gate props). Cosmetic only.
    teleport_debug: bool,

    // Phase C1 population (Game.N Group/Entity tables). Pending `(0,0)`
    // entities are deferred to C2; no runtime placement/AI is implemented.
    tables: Option<GameTables>,
    /// Parsed `Game.N` settlements (citizen home/role waypoints, `+0x3D0`).
    settlements: Vec<Settlement>,
    population_cache: HashMap<u32, PopulationBuild>,
    population_debug: bool,
    population_rejected: Vec<String>,

    // R2 render-layer diagnostics.
    layer_debug: bool,
    layer_counts: LayerCounts,
    props_drawn: usize,
    actors_drawn: usize,

    detail_pan: [f64; 2],
    camera_follow: bool,
    camera_follow_strength: f64,

    portal_bindings: HashMap<u32, Vec<PortalBinding>>,

    // graph view
    graph_zoom: f64,
    graph_pan: [f64; 2],
    node_rects: HashMap<u32, (i32, i32, i32, i32)>,

    // presentation
    pub frame: Vec<u8>,
    pub texts: Vec<TextItem>,
    /// Visible inventory slot under the mouse (HUD hover; read-only).
    hovered_slot: Option<usize>,
    /// Character screen (`INTR[2]`) open. Escape closes it.
    pub character_screen: bool,
    pub host_w: i32,
    pub host_h: i32,
    drag: bool,
    last_mouse: (f64, f64),
    pub quit: bool,
    /// Set by Escape: the host runtime should open the frontend/menu while
    /// preserving this live game state (do **not** quit or rebuild).
    pub request_menu: bool,
}

impl PlayableApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: World,
        edges: Vec<Edge>,
        pairs: Vec<Pair>,
        one_way: Vec<usize>,
        self_edges: Vec<usize>,
        layout: HashMap<u32, (f64, f64)>,
        profile: u32,
        location_names: Vec<String>,
        player: PlayerCharacter,
        start_map: u32,
        start_x: f64,
        start_y: f64,
        start_dir: i32,
        tables: Option<GameTables>,
        settlements: Vec<Settlement>,
    ) -> PlayableApp {
        let mut app = PlayableApp {
            world,
            edges,
            pairs,
            one_way,
            self_edges,
            layout,
            profile,
            location_names,
            debug_ids: false,
            current_map: start_map,
            selected: Some(start_map),
            mode_graph: false,
            player,
            player_x: start_x,
            player_y: start_y,
            player_dir: start_dir & 7,
            target: None,
            player_path: Vec::new(),
            player_step_t: 0.0,
            anim_time: 0.0,
            teleport_cooldown: 0.0,
            last_teleport: None,
            teleport_error: None,
            teleport_debug: false,
            tables,
            settlements,
            population_cache: HashMap::new(),
            population_debug: false,
            population_rejected: Vec::new(),
            layer_debug: false,
            layer_counts: LayerCounts::default(),
            props_drawn: 0,
            actors_drawn: 0,
            detail_pan: [0.0, 0.0],
            camera_follow: true,
            camera_follow_strength: 11.0,
            portal_bindings: HashMap::new(),
            graph_zoom: 1.0,
            graph_pan: [0.0, 0.0],
            node_rects: HashMap::new(),
            frame: vec![0u8; (VIRTUAL_W * VIRTUAL_H * 4) as usize],
            texts: Vec::new(),
            hovered_slot: None,
            character_screen: false,
            host_w: VIRTUAL_W,
            host_h: VIRTUAL_H,
            drag: false,
            last_mouse: (0.0, 0.0),
            quit: false,
            request_menu: false,
        };
        app.rebuild_portal_bindings();
        app.center_camera_on_player(true, 0.0);
        app.fit_graph();
        app
    }

    // --- camera ---


    pub fn step(&mut self, dt: f64) {
        if !self.mode_graph {
            self.ensure_population(self.current_map);
            self.update_actors(dt);
            self.update_player(dt);
            if self.camera_follow {
                self.center_camera_on_player(false, dt);
            }
            self.draw_detail();
        } else {
            self.draw_graph();
        }
    }


}
fn ellipse_outline(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    thickness: i32,
    c: [u8; 4],
) {
    let steps = ((rx.max(ry) * 4.0) as i32).clamp(16, 512);
    let mut prev: Option<(i32, i32)> = None;
    for i in 0..=steps {
        let a = 2.0 * std::f32::consts::PI * i as f32 / steps as f32;
        let x = (cx + rx * a.cos()).round() as i32;
        let y = (cy + ry * a.sin()).round() as i32;
        if let Some((px, py)) = prev {
            line(dst, dw, dh, px, py, x, y, c);
        }
        if thickness > 1 {
            fill_rect(dst, dw, dh, x - thickness / 2, y - thickness / 2, thickness, thickness, c);
        }
        prev = Some((x, y));
    }
}

fn rect_outline(dst: &mut [u8], dw: usize, dh: usize, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
    fill_rect(dst, dw, dh, x, y, w, 1, c);
    fill_rect(dst, dw, dh, x, y + h - 1, w, 1, c);
    fill_rect(dst, dw, dh, x, y, 1, h, c);
    fill_rect(dst, dw, dh, x + w - 1, y, 1, h, c);
}

/// Helper because `i32::from_f64` is not stable in all contexts here.
trait FromF64 {
    fn from_f64(v: f64) -> i32;
}
impl FromF64 for i32 {
    fn from_f64(v: f64) -> i32 {
        v.round() as i32
    }
}

#[cfg(test)]
mod tests;
