//! OpenKonung playable application state and logic.
//!
//! Faithful port of `konung_playable_prototype.py`.

use std::collections::{HashMap, HashSet};

use crate::actors::{build_population, find_path, sort_actors, update_population, PopulationBuild, LOGIC_TICK_HZ};
use crate::coords::{entity_to_local_px_f, local_px_to_entity, step_by_direction};
use crate::edges::{Edge, Pair};
use crate::character::PlayerCharacter;
use crate::loc::prop_to_local_px;
use crate::population::{GameTables, Settlement};
use crate::render::{scale_image, LayerCounts, World, WorldSprite};
use crate::res::{blit, fill_ellipse, fill_rect, line, Img};

pub const VIRTUAL_W: i32 = 800;
pub const VIRTUAL_H: i32 = 600;

pub const KEY_ESCAPE: u32 = 27;
pub const KEY_BACKSPACE: u32 = 8;
pub const KEY_TAB: u32 = 9;
pub const KEY_C: u32 = 67;
pub const KEY_D: u32 = 68;
pub const KEY_F: u32 = 70;
pub const KEY_H: u32 = 72;
pub const KEY_L: u32 = 76;
pub const KEY_P: u32 = 80;
pub const KEY_R: u32 = 82;
pub const KEY_T: u32 = 84;
pub const KEY_V: u32 = 86;

/// Cosmetic binding between one outgoing transition edge and its nearest visual
/// 244/245 gate. The canonical gameplay trigger is the source of truth: the
/// normal portal ring/label is drawn at `trigger` (raw runtime/Entity), while the
/// associated visual gate center (`gate_grid`, raw runtime-grid) is retained
/// only for the `T` debug overlay. `dist` is the gate-center → trigger-rectangle
/// distance in grid units.
#[derive(Clone)]
struct PortalBinding {
    edge: usize,
    gate: usize,
    /// Canonical trigger rectangle center in raw runtime/Entity coordinates.
    trigger: [f64; 2],
    /// Associated visual gate center in raw runtime-grid coordinates (grid_x, grid_y).
    gate_grid: [f64; 2],
    /// Gate-center → trigger-rectangle distance (grid units).
    dist: f64,
}

/// Maximum gate-center → trigger-rectangle distance (grid units) for a cosmetic
/// binding. Empirical Game.1 distribution: 100/108 outgoing transitions have a
/// nearest gate within 8.0, then a clear gap to 18.34; 13.0 sits in that gap, so
/// unrelated 244/245 objects stay unbound rather than drawing a misleading ring.
const MAX_GATE_TRIGGER_DIST: f64 = 13.0;

pub struct TextItem {
    pub x: i32,
    pub y: i32,
    pub rgb: u32,
    pub scale: f32,
    pub s: String,
}

/// Center of a trigger rectangle given its raw (possibly unnormalized) corners.
/// A point trigger yields its exact coordinate.
fn trigger_center(x1: i32, y1: i32, x2: i32, y2: i32) -> [f64; 2] {
    [(x1 + x2) as f64 * 0.5, (y1 + y2) as f64 * 0.5]
}

/// Distance from a point to the rectangle with the given raw corners (0 if the
/// point lies inside the normalized rectangle).
fn dist_point_to_rect(px: f64, py: f64, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
    let mnx = x1.min(x2) as f64;
    let mxx = x1.max(x2) as f64;
    let mny = y1.min(y2) as f64;
    let mxy = y1.max(y2) as f64;
    let dx = if px < mnx {
        mnx - px
    } else if px > mxx {
        px - mxx
    } else {
        0.0
    };
    let dy = if py < mny {
        mny - py
    } else if py > mxy {
        py - mxy
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

/// Best partial matching found by [`match_triggers_to_gates`].
struct GateMatchBest {
    count: usize,
    cost: f64,
    /// Sorted `(rec, gate)` pairs: the deterministic tie-break key.
    key: Vec<(usize, usize)>,
    assign: Vec<Option<usize>>,
}

fn gate_match_better(a: &GateMatchBest, b: &GateMatchBest) -> bool {
    if a.count != b.count {
        return a.count > b.count;
    }
    if (a.cost - b.cost).abs() > 1e-9 {
        return a.cost < b.cost;
    }
    a.key < b.key
}

fn gate_match_key(assign: &[Option<usize>], recs: &[usize]) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = assign
        .iter()
        .enumerate()
        .filter_map(|(e, g)| g.map(|gi| (recs[e], gi)))
        .collect();
    v.sort_unstable();
    v
}

#[allow(clippy::too_many_arguments)]
fn gate_match_dfs(
    k: usize,
    used: u64,
    cur: &mut Vec<Option<usize>>,
    count: usize,
    cost: f64,
    best: &mut GateMatchBest,
    candidates: &[Vec<(usize, f64)>],
    order: &[usize],
    recs: &[usize],
) {
    if k == order.len() {
        let cand = GateMatchBest {
            count,
            cost,
            key: gate_match_key(cur, recs),
            assign: cur.clone(),
        };
        if gate_match_better(&cand, best) {
            *best = cand;
        }
        return;
    }
    // Even binding every remaining gate cannot beat the incumbent.
    if count + (order.len() - k) < best.count {
        return;
    }
    let gi = order[k];
    for &(e, d) in &candidates[gi] {
        if used & (1u64 << e) != 0 {
            continue;
        }
        cur[e] = Some(gi);
        gate_match_dfs(k + 1, used | (1u64 << e), cur, count + 1, cost + d, best, candidates, order, recs);
        cur[e] = None;
    }
    // Leave this gate unbound.
    gate_match_dfs(k + 1, used, cur, count, cost, best, candidates, order, recs);
}

/// Deterministic one-to-one assignment of transitions to gates.
///
/// Lexicographic objective:
///   1. maximize the number of bound transitions within `max_dist`,
///   2. minimize the total gate-center → trigger-rectangle distance,
///   3. tie-break deterministically by `(transition rec, gate index)`.
///
/// `triggers[i] = (x1, y1, x2, y2)` raw corners, `recs[i]` the transition rec,
/// `gates[j] = (grid_x, grid_y)` the visual gate center. Returns `assign` indexed
/// like `triggers`; `Some(j)` binds transition `i` to gate `j`. Exhaustive over
/// the (tiny) candidate sets, which is easier to audit than a weighed matching
/// library; candidates are processed fewest-first and branch-and-bound pruned.
fn match_triggers_to_gates(
    triggers: &[(i32, i32, i32, i32)],
    recs: &[usize],
    gates: &[(f64, f64)],
    max_dist: f64,
) -> Vec<Option<usize>> {
    let n = triggers.len();
    let g = gates.len();
    let mut assign: Vec<Option<usize>> = vec![None; n];
    if n == 0 || g == 0 {
        return assign;
    }

    let candidates: Vec<Vec<(usize, f64)>> = gates
        .iter()
        .map(|&(gx, gy)| {
            let mut v: Vec<(usize, f64)> = (0..n)
                .filter_map(|e| {
                    let (x1, y1, x2, y2) = triggers[e];
                    let d = dist_point_to_rect(gx, gy, x1, y1, x2, y2);
                    if d <= max_dist {
                        Some((e, d))
                    } else {
                        None
                    }
                })
                .collect();
            v.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.0.cmp(&b.0))
            });
            v
        })
        .collect();

    let mut order: Vec<usize> = (0..g).collect();
    order.sort_by_key(|&gi| (candidates[gi].len(), gi));

    let mut best = GateMatchBest {
        count: 0,
        cost: 0.0,
        key: Vec::new(),
        assign: vec![None; n],
    };
    let mut cur = vec![None; n];
    gate_match_dfs(0, 0, &mut cur, 0, 0.0, &mut best, &candidates, &order, recs);
    assign.clone_from_slice(&best.assign);
    assign
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

    fn desired_player_pan(&self) -> [f64; 2] {
        let (px, py) = entity_to_local_px_f(self.player_x, self.player_y);
        [VIRTUAL_W as f64 * 0.5 - px, VIRTUAL_H as f64 * 0.5 - py]
    }

    pub fn center_camera_on_player(&mut self, immediate: bool, dt: f64) {
        let desired = self.desired_player_pan();
        if immediate {
            self.detail_pan = desired;
            return;
        }
        let a = 1.0 - (-self.camera_follow_strength * dt.max(0.0)).exp();
        self.detail_pan[0] += (desired[0] - self.detail_pan[0]) * a;
        self.detail_pan[1] += (desired[1] - self.detail_pan[1]) * a;
    }

    fn fit_detail(&mut self) {
        self.center_camera_on_player(true, 0.0);
    }

    // --- portals ---

    /// Bind each outgoing transition edge to its nearest visual 244/245 gate.
    ///
    /// The association is driven **only** by the canonical source trigger
    /// rectangle (`+09/+0B`..`+0D/+0F`) matched against the gate's raw
    /// runtime-grid center (`loc::Gate.x/.y`, averaged from the 244/245 props'
    /// `grid_x/grid_y`). No arrival coordinate, no `screen_to_transition_xy`, no
    /// side heuristic. Matching is a deterministic one-to-one minimum-cost
    /// assignment (see [`match_triggers_to_gates`]); edges beyond
    /// [`MAX_GATE_TRIGGER_DIST`] stay unbound rather than drawing a misleading
    /// ring. This binding is cosmetic only — gameplay detection lives entirely
    /// in `update_player`.
    fn rebuild_portal_bindings(&mut self) {
        let mut bindings: HashMap<u32, Vec<PortalBinding>> = HashMap::new();
        let loc_ids: Vec<u32> = self.world.locs.iter().map(|l| l.loc_id).collect();
        for loc_id in loc_ids {
            let loc = match self.world.loc(loc_id) {
                Some(l) => l,
                None => continue,
            };
            if loc.gates.is_empty() {
                continue;
            }
            let gates: Vec<(f64, f64)> = loc.gates.iter().map(|g| (g.x, g.y)).collect();

            // Outgoing edges, deterministically ordered by (rec, to).
            let mut edge_idx: Vec<usize> = self
                .edges
                .iter()
                .enumerate()
                .filter(|(_, e)| e.from == loc_id)
                .map(|(i, _)| i)
                .collect();
            if edge_idx.is_empty() {
                continue;
            }
            edge_idx.sort_by_key(|&i| (self.edges[i].rec, self.edges[i].to));

            let triggers: Vec<(i32, i32, i32, i32)> = edge_idx
                .iter()
                .map(|&ei| {
                    let e = &self.edges[ei];
                    (e.dst[0], e.dst[1], e.dst2[0], e.dst2[1])
                })
                .collect();
            let recs: Vec<usize> = edge_idx.iter().map(|&ei| self.edges[ei].rec).collect();

            let assign = match_triggers_to_gates(&triggers, &recs, &gates, MAX_GATE_TRIGGER_DIST);

            let mut bind: Vec<PortalBinding> = Vec::new();
            for (local, maybe_gate) in assign.iter().enumerate() {
                let gi = match maybe_gate {
                    Some(g) => *g,
                    None => continue,
                };
                let ei = edge_idx[local];
                let (x1, y1, x2, y2) = triggers[local];
                let (gx, gy) = gates[gi];
                bind.push(PortalBinding {
                    edge: ei,
                    gate: gi,
                    trigger: trigger_center(x1, y1, x2, y2),
                    gate_grid: [gx, gy],
                    dist: dist_point_to_rect(gx, gy, x1, y1, x2, y2),
                });
            }
            bindings.insert(loc_id, bind);
        }
        self.portal_bindings = bindings;
    }

    fn outgoing(&self, lid: u32) -> Vec<PortalBinding> {
        self.portal_bindings.get(&lid).cloned().unwrap_or_default()
    }

    // --- Phase C1 population ---

    /// Build (once) the runtime actor list for a location from the parsed Game.N
    /// tables. Actors are pre-sorted by projected draw position at build time.
    fn ensure_population(&mut self, loc_id: u32) {
        if self.population_cache.contains_key(&loc_id) {
            return;
        }
        let mut build = {
            let collision = self.world.loc(loc_id).map(|l| l.collision.as_slice());
            match (&self.tables, collision) {
                (Some(t), Some(c)) => build_population(t, loc_id as u8, c),
                _ => PopulationBuild::default(),
            }
        };
        // Per-actor cycle lengths (original `Entity+0xD7`): one collision cell
        // is stepped per this many walk frames. Resolved once, from the spawn
        // facing and the actor's authored stance (`Entity+0x03 & 0x04`).
        for a in &build.actors {
            let facing = (a.entity.facing & 7) as usize;
            let battle = crate::anim::battle_from_flags(a.entity.flags_b);
            let wf = self
                .world
                .res
                .walk_cycle_frames(a.entity.archetype, a.entity.visual_index, facing, battle)
                .unwrap_or(0)
                .min(u8::MAX as usize) as u8;
            let idle = self
                .world
                .res
                .action_cycle_frames(
                    a.entity.archetype,
                    a.entity.visual_index,
                    facing,
                    crate::anim::idle_action(battle),
                )
                .unwrap_or(0)
                .min(u8::MAX as usize) as u8;
            if let Some(m) = build.motion.get_mut(&a.source_entity_id()) {
                m.walk_frames = wf;
                m.idle_frames = idle;
                m.action = crate::anim::idle_action(battle) as u8;
            }
        }
        sort_actors(&mut build.actors);
        self.population_cache.insert(loc_id, build);
    }

    /// Resolve every already-positioned non-player actor (frame 0, same Entity
    /// projection as the protagonist) into `out` for the dynamic depth pass.
    /// Visual-resolution failures are returned with an explicit reason; no actor
    /// is silently skipped.
    fn collect_actor_sprites(&mut self, out: &mut Vec<WorldSprite>) -> Vec<String> {
        let mut rejected: Vec<String> = Vec::new();
        let loc_id = self.current_map;
        if let Some(pop) = self.population_cache.get(&loc_id) {
            for a in &pop.actors {
                // Live motion (cell + facing + action) when present, else the
                // placement with the authored stance's idle action at frame 0.
                let (facing, cell, action, frame) = pop
                    .motion
                    .get(&a.source_entity_id())
                    .map(|m| (m.facing, m.cell, m.action as usize, m.frame as usize))
                    .unwrap_or_else(|| {
                        let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                        (
                            a.entity.facing,
                            (a.entity.x as i32, a.entity.y as i32),
                            crate::anim::idle_action(battle),
                            0,
                        )
                    });
                match self.world.res.actor_frame_state(
                    &a.entity.equipment,
                    a.entity.archetype,
                    a.entity.visual_index,
                    a.entity.palette_offset,
                    facing,
                    action,
                    frame,
                ) {
                    Ok((im, gx, gy)) => {
                        let (lx, ly) = entity_to_local_px_f(cell.0 as f64, cell.1 as f64);
                        out.push(WorldSprite { img: im, gx, gy, lx, ly });
                    }
                    Err(reason) => rejected.push(format!(
                        "entity={} group={} raw_pos=({},{}) archetype=0x{:02X} facing={} visual_index={} palette_offset=0x{:X} reason={}",
                        a.source_entity_id(),
                        a.source_group_id,
                        a.entity.x,
                        a.entity.y,
                        a.entity.archetype,
                        a.entity.facing,
                        a.entity.visual_index,
                        a.entity.palette_offset,
                        reason
                    )),
                }
            }
        }
        rejected
    }

    /// Diagnostic: full authored state of a source `Entity` by id (task §7).
    pub fn entity_debug_block(&self, id: usize) -> Option<String> {
        self.tables
            .as_ref()?
            .entities
            .get(id)
            .map(|e| e.debug_block())
    }

    // --- window mapping ---

    pub fn window_game_rect(&self) -> (i32, i32, i32, i32, f64) {
        let sw = self.host_w.max(1);
        let sh = self.host_h.max(1);
        let scale = (sw as f64 / VIRTUAL_W as f64).min(sh as f64 / VIRTUAL_H as f64);
        let w = (VIRTUAL_W as f64 * scale).round().max(1.0) as i32;
        let h = (VIRTUAL_H as f64 * scale).round().max(1.0) as i32;
        ((sw - w) / 2, (sh - h) / 2, w, h, scale)
    }

    fn window_to_virtual(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (rx, ry, w, h, scale) = self.window_game_rect();
        if x < rx as f64 || x >= (rx + w) as f64 || y < ry as f64 || y >= (ry + h) as f64 {
            return None;
        }
        Some(((x - rx as f64) / scale, (y - ry as f64) / scale))
    }

    /// Route to an acceptable (passable) point using the original pathfinder
    /// (`FUN_0043DA1F`). Returns `None` when the goal is blocked, out of bounds
    /// or unreachable — the original issues no movement command then.
    fn compute_player_path(&self, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<u8>> {
        let l = self.world.loc(self.current_map)?;
        find_path(
            &l.collision,
            &HashSet::new(),
            start,
            goal,
            (0, 0, crate::coords::RUNTIME_W - 1, crate::coords::RUNTIME_H - 1),
        )
    }

    fn set_target_from_mouse(&mut self, x: f64, y: f64) {
        let (mx, my) = match self.window_to_virtual(x, y) {
            Some(v) => v,
            None => return,
        };
        let (ox, oy) = (self.detail_pan[0], self.detail_pan[1]);
        let (lx, ly) = (mx - ox, my - oy);
        // Nearest runtime/entity cell, clamped only to the proven runtime
        // bounds (x 0..319, y 0..159) — not to the terrain surface.
        let (tx, ty) = local_px_to_entity(lx.round() as i32, ly.round() as i32);
        let start = (self.player_x.round() as i32, self.player_y.round() as i32);
        let path = self.compute_player_path(start, (tx, ty));
        self.player_step_t = 0.0;
        // The original `FUN_004142D4` stores the target fields *before* running
        // the pathfinder, so the destination is recorded even when the route
        // fails; only a successful route starts the walk. A blocked/unreachable
        // point therefore never moves the hero.
        self.target = Some((tx as f64, ty as f64));
        match path {
            Some(p) if !p.is_empty() => self.player_path = p,
            _ => self.player_path.clear(),
        }
    }

    // --- update ---

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

    /// Advance the current location's non-player actors (citizen wander) at the
    /// fixed logic rate. Uses split borrows so the world collision table and the
    /// mutable population cache are accessed simultaneously.
    fn update_actors(&mut self, dt: f64) {
        let loc = self.current_map;
        let PlayableApp {
            world,
            settlements,
            population_cache,
            ..
        } = self;
        let Some(pop) = population_cache.get_mut(&loc) else {
            return;
        };
        let collision = world.loc(loc).map(|l| l.collision.as_slice()).unwrap_or(&[]);
        let settlement = settlements.iter().find(|s| s.loc_id as u32 == loc);
        update_population(pop, collision, settlement, dt);
    }

    /// Seconds per player cell step: one collision cell per completed
    /// walk-animation cycle (`FUN_0041435c` state 1/7 + `FUN_00413de4`). Cycle
    /// length is the hero's walk-animation frame count from the GRAPH tables.
    fn player_cell_period(&self) -> f64 {
        (self.player_walk_frames() / LOGIC_TICK_HZ).max(1.0 / LOGIC_TICK_HZ)
    }

    fn player_walk_frames(&self) -> f64 {
        let battle = crate::anim::battle_from_flags(self.player.flags_b);
        let vi = self.player.hero.visual_index as usize;
        match self.world.res.human_base_positions.get(vi).copied() {
            Some(bp) => {
                let n = crate::res::human_frame_count(
                    &self.world.res.graph,
                    bp,
                    crate::anim::walk_action(battle),
                    (self.player_dir.max(0) as usize) & 7,
                );
                if n == 0 {
                    8.0
                } else {
                    n as f64
                }
            }
            None => 8.0,
        }
    }

    fn update_player(&mut self, dt: f64) {
        self.anim_time += dt;
        self.teleport_cooldown = (self.teleport_cooldown - dt).max(0.0);

        // Movement is driven by the direction queue (`FUN_0043DA1F` →
        // `Entity+0x13`), not by the raw target. Each completed walk-animation
        // cycle consumes one direction code and steps exactly one cell
        // (`FUN_0041435C`); facing is the pending direction (`FUN_004123BC` gate
        // `facing == path[0]`), so a straight walk never zigzags.
        if !self.player_path.is_empty() {
            let dir = self.player_path[0];
            self.player_dir = dir as i32;
            self.player_step_t += dt;
            let period = self.player_cell_period();
            if self.player_step_t >= period {
                // Reset (never carry a backlog) so a frame-rate hitch can never
                // consume several cells at once (no teleport).
                self.player_step_t = 0.0;
                let cur = (self.player_x.round() as i32, self.player_y.round() as i32);
                let (nx, ny) = step_by_direction(cur.0, cur.1, dir);
                self.player_x = nx as f64;
                self.player_y = ny as f64;
                self.player_path.remove(0);
                if self.player_path.is_empty() {
                    // Arrived, or a >MAX_PATH_STEPS route prefix was consumed:
                    // re-route while the stored goal is still ahead.
                    if let Some((tx, ty)) = self.target {
                        let goal = (tx.round() as i32, ty.round() as i32);
                        if (nx, ny) == goal {
                            self.target = None;
                        } else {
                            match self.compute_player_path((nx, ny), goal) {
                                Some(p) if !p.is_empty() => self.player_path = p,
                                _ => self.target = None,
                            }
                        }
                    }
                }
            }
        }

        if self.teleport_cooldown <= 0.0 {
            // Canonical gameplay transition detection
            // (`KONUNG_SOURCE_OF_TRUTH.md` §13.1/§21.6): test the player's raw
            // runtime/Entity cell against each outgoing edge's source trigger
            // rectangle (`+09/+0B`..`+0D/+0F`). No screen projection, no visual
            // 244/245 gate-prop binding, no pixel-radius heuristic. Overlapping
            // rectangles resolve deterministically by lowest `rec`.
            let cx = self.player_x.round() as i32;
            let cy = self.player_y.round() as i32;
            let mut best: Option<(usize, usize)> = None;
            for (ei, e) in self.edges.iter().enumerate() {
                if e.from != self.current_map {
                    continue;
                }
                if e.trigger_contains(cx, cy) {
                    match best {
                        Some((rec, _)) if rec <= e.rec => {}
                        _ => best = Some((e.rec, ei)),
                    }
                }
            }
            if let Some((_, ei)) = best {
                let e = self.edges[ei].clone();
                let dst = e.to;
                if self.world.loc_index.contains_key(&dst) {
                    let [ax, ay] = e.arrival();
                    self.current_map = dst;
                    self.selected = Some(dst);
                    self.player_x = ax as f64;
                    self.player_y = ay as f64;
                    self.target = None;
                    self.player_path.clear();
                    self.teleport_cooldown = 0.9;
                    self.last_teleport = Some((e.from, dst, e.rec));
                    self.teleport_error = None;
                    self.fit_detail();
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                } else {
                    // Loud diagnostic instead of a silent skip: the transition
                    // fired but its destination Konung.N is not loaded.
                    let msg = format!(
                        "TELEPORT BLOCKED rec={} {} -> {} : destination Konung.{} not loaded",
                        e.rec, e.from, dst, dst
                    );
                    eprintln!("{}", msg);
                    self.teleport_error = Some(msg);
                    self.teleport_cooldown = 0.9;
                }
            }
        }
    }

    /// Resolve the protagonist through the canonical Entity visual path with
    /// movement animation. The active action is the original stance pair
    /// (`FUN_004142D4`/`FUN_00414A34`): walk `1`/`7` while moving, idle `0`/`6`
    /// otherwise, chosen by `Entity+0x03 & 0x04` (see [`crate::anim`]). The
    /// renderer family/class comes from `PlayerCharacter.class` (`Entity+0x04`);
    /// the visual index and palette come from the hero identity. No Game.res
    /// variant fallback and no equipment overlays. One walk cycle == one cell
    /// step (`FUN_00413de4`).
    fn player_frame(&mut self, moving: bool) -> Result<(Img, i32, i32), String> {
        let battle = crate::anim::battle_from_flags(self.player.flags_b);
        let action = if moving {
            crate::anim::walk_action(battle)
        } else {
            crate::anim::idle_action(battle)
        };
        let fi = self.player_frame_index(moving, action);
        self.world.res.actor_frame_state(
            &self.player.equipment,
            self.player.entity_class(),
            self.player.hero.visual_index,
            self.player.hero.palette_offset,
            self.player_dir as u8,
            action,
            fi,
        )
    }

    /// Protagonist sprite frame for `action`, driven by the original 60 ms
    /// fixed step (`actors::LOGIC_TICK_HZ`): the walk cycle is the cell-step
    /// progress (one full cycle per cell); idle free-runs one frame per logic
    /// tick. Mirrors `FUN_00413de4` advancing `Entity+0x10` once per frame.
    fn player_frame_index(&self, moving: bool, action: usize) -> usize {
        let dir = (self.player_dir.max(0) as usize) & 7;
        let vi = self.player.hero.visual_index as usize;
        let count = match self.world.res.human_base_positions.get(vi).copied() {
            Some(bp) => {
                let n = crate::res::human_frame_count(&self.world.res.graph, bp, action, dir);
                if n == 0 {
                    1
                } else {
                    n
                }
            }
            None => 1,
        };
        if moving {
            let period = self.player_cell_period().max(1e-9);
            let progress = (self.player_step_t / period).clamp(0.0, 1.0);
            ((progress * count as f64) as usize).min(count - 1)
        } else {
            ((self.anim_time * LOGIC_TICK_HZ) as usize) % count
        }
    }

    // --- labels ---

    /// User-facing location label: the original name for Konung.N, falling back
    /// to `Konung.N` when the table entry is missing/empty. In debug mode the
    /// numeric id is appended: `Родники [Konung.1]`.
    fn label(&self, id: u32) -> String {
        let name = if id >= 1 {
            self.location_names.get((id - 1) as usize)
        } else {
            None
        };
        match name {
            Some(n) if !n.is_empty() => {
                if self.debug_ids {
                    format!("{} [Konung.{}]", n, id)
                } else {
                    n.clone()
                }
            }
            _ => format!("Konung.{}", id),
        }
    }

    // --- drawing ---

    fn text(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) {
        self.texts.push(TextItem { x, y, rgb, scale, s });
    }

    /// Emit `s` word-wrapped so no line exceeds the right screen edge. Returns
    /// the y coordinate just below the last emitted line.
    fn text_wrapped(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) -> i32 {
        let max_px = (VIRTUAL_W - x - 4).max(48);
        let line_h = (14.0 * scale).ceil() as i32 + 2;
        let mut cy = y;
        for line in wrap_to_px(&s, scale, max_px) {
            self.text(x, cy, rgb, scale, line);
            cy += line_h;
        }
        cy
    }

    /// Emit `s` shifted left/shrunk so its right edge stays on screen (keeps
    /// marker labels attached to their marker when they sit near the right edge).
    fn text_clamped(&mut self, x: i32, y: i32, rgb: u32, scale: f32, s: String) {
        let w = text_px(&s, scale);
        let x = x.min((VIRTUAL_W - 4 - w).max(2)).max(2);
        let y = y.min(VIRTUAL_H - 16).max(0);
        self.text(x, y, rgb, scale, s);
    }

    fn draw_detail(&mut self) {
        self.texts.clear();
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        for px in self.frame.iter_mut() {
            *px = 0;
        }
        fill_rect(&mut self.frame, fw, fh, 0, 0, VIRTUAL_W, VIRTUAL_H, [7, 10, 9, 255]);

        let ox = self.detail_pan[0];
        let oy = self.detail_pan[1];
        let iox = ox.round() as i32;
        let ioy = oy.round() as i32;

        self.ensure_population(self.current_map);

        // Static background: base terrain (both byte layers) + GRAPH placements.
        self.layer_counts =
            self.world
                .draw_background(&mut self.frame, fw, fh, self.current_map, iox, ioy);

        // Dynamic depth pass: props + NPC/mob actors + protagonist, ordered by
        // projected anchor. The exact original depth key is unresolved (§25);
        // this is a documented provisional anchor sort (never raw Entity x/y).
        let mut sprites: Vec<WorldSprite> = self.world.collect_props(self.current_map);
        self.props_drawn = sprites.len();
        let mut rejected = self.collect_actor_sprites(&mut sprites);

        // Protagonist through the canonical Entity visual path (animated).
        let moving = !self.player_path.is_empty();
        let (plx, ply) = entity_to_local_px_f(self.player_x, self.player_y);
        match self.player_frame(moving) {
            Ok((im, gx, gy)) => sprites.push(WorldSprite { img: im, gx, gy, lx: plx, ly: ply }),
            Err(reason) => rejected.push(format!(
                "player hero={} visual_index={} palette_offset=0x{:X} facing={} reason={}",
                self.player.hero.name,
                self.player.hero.visual_index,
                self.player.hero.palette_offset,
                self.player_dir,
                reason
            )),
        }
        self.population_rejected = rejected;
        self.actors_drawn = sprites.len().saturating_sub(self.props_drawn);

        sprites.sort_by(|a, b| {
            a.ly.partial_cmp(&b.ly)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.lx.partial_cmp(&b.lx).unwrap_or(std::cmp::Ordering::Equal))
        });
        for s in &sprites {
            let ax = iox + s.lx.round() as i32 + s.gx;
            let ay = ioy + s.ly.round() as i32 + s.gy;
            blit(&mut self.frame, fw, fh, &s.img.data, s.img.w, s.img.h, ax, ay);
        }

        // Live portals: drawn at the CANONICAL source trigger (center of the
        // transition trigger rectangle) through the normal Entity/runtime-grid
        // render transform, so the visible ring is exactly the cell gameplay
        // tests. The associated 244/245 gate center is debug-only (T overlay).
        for b in self.outgoing(self.current_map) {
            let (lx, ly) = entity_to_local_px_f(b.trigger[0], b.trigger[1]);
            let sx = iox + lx.round() as i32;
            let sy = ioy + ly.round() as i32;
            if -30 <= sx && sx <= VIRTUAL_W + 30 && -30 <= sy && sy <= VIRTUAL_H + 30 {
                fill_ellipse(&mut self.frame, fw, fh, sx as f32, sy as f32, 18.0, 7.0, [58, 61, 58, 255]);
                ellipse_outline(&mut self.frame, fw, fh, sx as f32, sy as f32, 18.0, 7.0, 2, [150, 155, 145, 255]);
                ellipse_outline(&mut self.frame, fw, fh, sx as f32, sy as f32, 12.0, 4.0, 1, [94, 99, 92, 255]);
                let to = self.edges[b.edge].to;
                let dest = self.label(to);
                self.text_clamped(sx + 20, sy - 8, 0xAF_F0C3, 1.0, format!("->{}", dest));
            }
        }

        if let Some((tx, ty)) = self.target {
            let (x, y) = entity_to_local_px_f(tx, ty);
            let sx = iox + x.round() as i32;
            let sy = ioy + y.round() as i32;
            ellipse_outline(&mut self.frame, fw, fh, sx as f32, sy as f32, 6.0, 6.0, 1, [255, 220, 80, 255]);
        }

        // protagonist position marker (overlay; sprite already drawn in depth pass)
        let pax = iox + plx.round() as i32;
        let pay = ioy + ply.round() as i32;
        ellipse_outline(&mut self.frame, fw, fh, pax as f32, pay as f32, 3.0, 3.0, 1, [255, 245, 120, 255]);

        // Original INTR HUD: whole-screen overlay, empty inventory slot strip and
        // (once bound) party portraits. Composited after the world and its
        // overlays; transparent regions keep the world visible. Debug overlays
        // below draw on top. Split borrows keep `frame` and `world.res` disjoint.
        {
            let PlayableApp {
                frame,
                world,
                player,
                hovered_slot,
                ..
            } = self;
            // Protagonist portrait uses the original selector with the actual
            // character fields (never a fixed INTR id).
            let portrait = crate::hud::portrait_id(
                true,
                player.portrait_group(),
                player.portrait_status(),
                player.hp_internal,
            );
            // Fixed scroll offset for this task; the protagonist's own
            // inventory is read directly (never re-initialized per frame).
            crate::hud::draw_game_hud(
                frame,
                fw,
                fh,
                &mut world.res,
                &player.inventory,
                0,
                *hovered_slot,
                Some(portrait),
                player.auto_enabled(),
            );
        }

        // Original character screen (INTR[2]) over the world area; its window
        // geometry keeps the party panel and bottom inventory strip visible.
        if self.character_screen {
            let PlayableApp {
                frame,
                world,
                player,
                ..
            } = self;
            crate::character_screen::draw(frame, fw, fh, &mut world.res, player);
        }

        let portals = self.outgoing(self.current_map).len();
        let cur = self.label(self.current_map);
        let hdr = format!(
            "OpenKonung v15 | {} | hero:{} Game.{} | 800x600 native camera | pal:{} | portals:{} | V follow | C char | H graph | D ids | P pop | L layers | T portals",
            cur, self.player.hero.name, self.profile, self.player.palette_id(), portals
        );
        let hdr = if self.teleport_debug {
            format!(
                "{} | cell=({},{})",
                hdr,
                self.player_x.round() as i32,
                self.player_y.round() as i32
            )
        } else {
            hdr
        };
        let mut hy = self.text_wrapped(12, 10, 0xEB_F5EB, 1.0, hdr);
        if let Some((a, b, r)) = self.last_teleport {
            let an = self.label(a);
            let bn = self.label(b);
            hy = self.text_wrapped(12, hy, 0xBE_DCC3, 1.0, format!("last teleport {}->{} r{}", an, bn, r));
        }
        if let Some(err) = self.teleport_error.clone() {
            hy = self.text_wrapped(12, hy, 0xFF_6B6B, 1.0, err);
        }

        if self.teleport_debug {
            self.draw_portal_debug(hy + 4);
        }
        if self.population_debug {
            self.draw_population_debug();
        }
        if self.layer_debug {
            self.draw_layer_debug();
        }
    }

    /// Canonical transition debug overlay (`T`):
    ///   - outgoing edges (`from == current_map`): trigger rectangle, rec/from->to, arrival
    ///   - the cosmetic binding: portal ring position (trigger center), the
    ///     associated visual 244/245 gate center, a line gate-center →
    ///     trigger-center, and the binding distance
    ///   - incoming edges (`to == current_map`): arrival point
    ///   - visual 244/245 gate props and their center
    ///   - the player's raw runtime/Entity cell, continuously
    /// Purely cosmetic; gameplay detection does not use any of this.
    /// `top` is the y below the header, so the coordinate panel never overlaps it.
    fn draw_portal_debug(&mut self, top: i32) {
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        let iox = self.detail_pan[0].round() as i32;
        let ioy = self.detail_pan[1].round() as i32;

        let edges = self.edges.clone();
        let bindings = self.outgoing(self.current_map);

        // Outgoing edges: canonical trigger rectangles.
        for e in edges.iter() {
            if e.from != self.current_map {
                continue;
            }
            let (mn, mx) = e.trigger_rect();
            let mut xs: Vec<f64> = Vec::new();
            let mut ys: Vec<f64> = Vec::new();
            for (ex, ey) in [
                (mn[0], mn[1]),
                (mn[0], mx[1]),
                (mx[0], mn[1]),
                (mx[0], mx[1]),
            ] {
                let (lx, ly) = entity_to_local_px_f(ex as f64, ey as f64);
                xs.push(lx);
                ys.push(ly);
            }
            let x1 = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let y1 = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let x2 = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let y2 = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let rx = iox + x1.round() as i32;
            let ry = ioy + y1.round() as i32;
            let rw = ((x2 - x1).round() as i32).max(14) + 6;
            let rh = ((y2 - y1).round() as i32).max(8) + 4;
            rect_outline(&mut self.frame, fw, fh, rx - 3, ry - 2, rw, rh, [80, 200, 255, 255]);
            if on_screen(rx, ry) {
                self.text_clamped(
                    rx + 4,
                    ry - 16,
                    0x80_C8FF,
                    0.8,
                    format!(
                        "T rec{} {}->{} trig({},{})-({},{}) arr({},{})",
                        e.rec, e.from, e.to, e.dst[0], e.dst[1], e.dst2[0], e.dst2[1],
                        e.arrival()[0], e.arrival()[1]
                    ),
                );
            }
        }

        // Cosmetic bindings: ring (trigger center) ↔ visual gate center + line.
        for b in bindings.iter() {
            let e = &edges[b.edge];
            let (rlx, rly) = entity_to_local_px_f(b.trigger[0], b.trigger[1]);
            let rsx = iox + rlx.round() as i32;
            let rsy = ioy + rly.round() as i32;
            let (glx, gly) = prop_to_local_px(b.gate_grid[0].round() as i32, b.gate_grid[1].round() as i32);
            let gsx = iox + glx;
            let gsy = ioy + gly;
            line(&mut self.frame, fw, fh, gsx, gsy, rsx, rsy, [255, 200, 60, 255]);
            ellipse_outline(&mut self.frame, fw, fh, rsx as f32, rsy as f32, 9.0, 9.0, 2, [120, 255, 120, 255]);
            rect_outline(&mut self.frame, fw, fh, gsx - 4, gsy - 4, 9, 9, [255, 120, 220, 255]);
            if on_screen(rsx, rsy) {
                self.text_clamped(
                    rsx + 8,
                    rsy + 8,
                    0x78_FF78,
                    0.8,
                    format!(
                        "ring rec{} {}->{} gate{} dist={:.2} trig=({:.1},{:.1})",
                        e.rec, e.from, e.to, b.gate, b.dist, b.trigger[0], b.trigger[1]
                    ),
                );
            }
        }

        // Incoming edges: arrival points in this map.
        for e in edges.iter() {
            if e.to == self.current_map {
                let [ax, ay] = e.arrival();
                let (lx, ly) = entity_to_local_px_f(ax as f64, ay as f64);
                let sx = iox + lx.round() as i32;
                let sy = ioy + ly.round() as i32;
                ellipse_outline(&mut self.frame, fw, fh, sx as f32, sy as f32, 11.0, 6.0, 2, [255, 220, 80, 255]);
                if on_screen(sx, sy) {
                    self.text_clamped(
                        sx + 6,
                        sy - 16,
                        0xF0_DC80,
                        0.8,
                        format!("arr rec{} {}->{} @({},{})", e.rec, e.from, e.to, ax, ay),
                    );
                }
            }
        }

        // Visual 244/245 gate props + center; annotate with the matched edge.
        let mut prop_marks: Vec<(i32, i32)> = Vec::new();
        let mut gate_labels: Vec<(i32, i32, String)> = Vec::new();
        if let Some(loc) = self.world.loc(self.current_map) {
            for g in &loc.gates {
                for p in &g.props {
                    prop_marks.push(prop_to_local_px(p.grid_x, p.grid_y));
                }
                let (lx, ly) = prop_to_local_px(g.x.round() as i32, g.y.round() as i32);
                let matched = bindings.iter().find(|b| b.gate == g.index).map(|b| {
                    let e = &edges[b.edge];
                    format!(" matched rec{} {}->{} d={:.2}", e.rec, e.from, e.to, b.dist)
                });
                if on_screen(iox + lx, ioy + ly) {
                    gate_labels.push((
                        iox + lx + 6,
                        ioy + ly + 8,
                        format!(
                            "gate{} center=({:.1},{:.1}){}",
                            g.index,
                            g.x,
                            g.y,
                            matched.unwrap_or_default()
                        ),
                    ));
                }
            }
        }
        for (lx, ly) in prop_marks {
            rect_outline(&mut self.frame, fw, fh, iox + lx - 8, ioy + ly - 8, 16, 16, [255, 120, 220, 255]);
        }
        for (sx, sy, s) in gate_labels {
            self.text_clamped(sx, sy, 0xFF_78DC, 0.8, s);
        }

        // Player raw cell, continuously.
        let (pcx, pcy) = entity_to_local_px_f(self.player_x, self.player_y);
        ellipse_outline(
            &mut self.frame,
            fw,
            fh,
            (iox + pcx.round() as i32) as f32,
            (ioy + pcy.round() as i32) as f32,
            5.0,
            5.0,
            2,
            [255, 255, 255, 255],
        );
        self.text(
            12,
            VIRTUAL_H - 24,
            0x80_C8FF,
            0.9,
            format!(
                "T portal debug | player raw cell=({},{})",
                self.player_x.round() as i32,
                self.player_y.round() as i32
            ),
        );

        // Fixed coordinate panel: the player plus every portal (bound outgoing
        // transition) in the current map, so their coordinates are readable even
        // when the markers are off-screen. Wrapped to the screen width.
        let (plx, ply) = entity_to_local_px_f(self.player_x, self.player_y);
        let mut py = top;
        py = self.text_wrapped(
            12,
            py,
            0xEB_F5EB,
            0.9,
            format!(
                "PLAYER cell=({},{}) local_px=({:.0},{:.0})",
                self.player_x.round() as i32,
                self.player_y.round() as i32,
                plx,
                ply
            ),
        );
        py = self.text_wrapped(
            12,
            py,
            0xAF_F0C3,
            0.9,
            format!("PORTALS in Konung.{} ({})", self.current_map, bindings.len()),
        );
        for b in bindings.iter() {
            if py > VIRTUAL_H - 40 {
                break;
            }
            let e = &edges[b.edge];
            py = self.text_wrapped(
                12,
                py,
                0xAF_F0C3,
                0.8,
                format!(
                    "rec{} {}->{} trig=({:.0},{:.0}) gate{} center=({:.1},{:.1}) d={:.2}",
                    e.rec,
                    e.from,
                    e.to,
                    b.trigger[0],
                    b.trigger[1],
                    b.gate,
                    b.gate_grid[0],
                    b.gate_grid[1],
                    b.dist
                ),
            );
        }
    }

    /// R2 render-layer debug: per-frame draw counts for each static/dynamic
    /// layer (instrumentation only, not normal HUD content).
    fn draw_layer_debug(&mut self) {
        let c = self.layer_counts;
        let lines = {
            let p = &self.player;
            vec![
                format!(
                    "hero={} class={} game={} loc={} player=({},{}) facing={} vi={} pal=0x{:X} name_id={} level={}",
                    p.hero.name,
                    p.class.name(),
                    self.profile,
                    self.current_map,
                    self.player_x as i32,
                    self.player_y as i32,
                    self.player_dir,
                    p.hero.visual_index,
                    p.hero.palette_offset,
                    p.hero.name_id,
                    p.level
                ),
                format!("terrain_records={}", crate::loc::LOCAL_TERRAIN_W * crate::loc::LOCAL_TERRAIN_H),
                format!("terrain_low_drawn={}", c.terrain_low),
                format!("terrain_high_drawn={}", c.terrain_high),
                format!("graph_drawn={}", c.graph),
                format!("props_drawn={}", self.props_drawn),
                format!("actors_drawn={}", self.actors_drawn),
                format!("rejected_visuals={}", self.population_rejected.len()),
                // Player-state block (task §9): persistent character-creation data.
                format!("HERO {}", p.hero.name),
                format!("CLASS {} (entity +04 = {})", p.class.name(), p.entity_class()),
                format!("level={}", p.level),
                format!("STR base={} current={}", p.strength_base, p.strength_current),
                format!("DEX base={} current={}", p.dexterity_base, p.dexterity_current),
                format!("CHA base={} current={}", p.charisma_base, p.charisma_current),
                format!("END base={} current={}", p.endurance_base, p.endurance_current),
                format!(
                    "melee={} archery={} trade={} building={} healing={} identify={} smithing={}",
                    p.melee_mastery,
                    p.archery_mastery,
                    p.trade,
                    p.building,
                    p.healing,
                    p.identification,
                    p.smithing
                ),
            ]
        };
        let mut y = 72i32;
        for l in lines {
            if y > VIRTUAL_H - 16 {
                break;
            }
            y = self.text_wrapped(12, y, 0xC8_E0F0, 0.8, l);
        }
    }

    /// Diagnostics for the currently selected location's C1/C2 population.
    /// Runtime placement, failures and visual-resolution failures are all
    /// listed; nothing with a saved position is silently skipped.
    fn draw_population_debug(&mut self) {
        let counts = self.population_cache.get(&self.current_map).map(|p| {
            (
                p.matching_groups,
                p.reserved_group_zero,
                p.inactive_groups,
                p.invalid_ranges,
                p.enumerated_entities,
                p.persisted_position_actors(),
                p.runtime_candidates,
                p.runtime_placed,
                p.runtime_failed,
            )
        });
        let (matching, reserved, inactive, invalid, enumerated, persisted, candidates, placed, failed) =
            counts.unwrap_or((0, 0, 0, 0, 0, 0, 0, 0, 0));
        let rejected = self.population_rejected.len();

        let mut lines: Vec<String> = vec![
            format!("location={}", self.current_map),
            format!("matching_groups={}", matching),
            format!("reserved_group_zero={}", reserved),
            format!("inactive_groups={}", inactive),
            format!("invalid_ranges={}", invalid),
            format!("enumerated_entities={}", enumerated),
            format!("persisted_position_actors={}", persisted),
            format!("runtime_candidates={}", candidates),
            format!("runtime_placed={}", placed),
            format!("runtime_failed={}", failed),
            format!("rejected_visuals={}", rejected),
        ];

        if let Some(p) = self.population_cache.get(&self.current_map) {
            for a in p.actors.iter().filter(|a| a.is_runtime()).take(16) {
                let (rx, ry) = a.display_raw();
                let (px, py) = entity_to_local_px_f(rx, ry);
                let r = a.spawn_rect().unwrap_or_default();
                lines.push(format!(
                    "runtime entity={} group={} spawn_rect=({},{},{},{}) attempts={} raw_runtime_pos=({},{}) rendered_local_px=({:.1},{:.1}) validation=collision",
                    a.source_entity_id(),
                    a.source_group_id,
                    r.x1,
                    r.x2,
                    r.y1,
                    r.y2,
                    a.attempts().unwrap_or(0),
                    rx as i32,
                    ry as i32,
                    px,
                    py
                ));
            }
            if p.runtime_placed > 16 {
                lines.push(format!("runtime ... {} more", p.runtime_placed - 16));
            }
            for f in p.failed.iter().take(8) {
                lines.push(format!(
                    "runtime entity={} group={} spawn_rect=({},{},{},{}) attempts={} placement_failed_after_100_attempts",
                    f.source_entity_id,
                    f.source_group_id,
                    f.spawn_rect.x1,
                    f.spawn_rect.x2,
                    f.spawn_rect.y1,
                    f.spawn_rect.y2,
                    f.attempts
                ));
            }
        }
        for r in self.population_rejected.iter().take(8) {
            lines.push(r.clone());
        }

        let mut y = 72i32;
        for l in lines {
            if y > VIRTUAL_H - 16 {
                break;
            }
            y = self.text_wrapped(12, y, 0xD8_E8D0, 0.8, l);
        }
    }

    fn graph_positions(&self) -> HashMap<u32, (f64, f64)> {
        self.layout
            .iter()
            .map(|(k, v)| (*k, (v.0 * 330.0, v.1 * 235.0)))
            .collect()
    }

    fn fit_graph(&mut self) {
        let pos = self.graph_positions();
        if pos.is_empty() {
            self.graph_zoom = 1.0;
            self.graph_pan = [0.0, 0.0];
            return;
        }
        let minx = pos.values().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let miny = pos.values().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let maxx = pos.values().map(|p| p.0 + 280.0).fold(f64::NEG_INFINITY, f64::max);
        let maxy = pos.values().map(|p| p.1 + 180.0).fold(f64::NEG_INFINITY, f64::max);
        let sw = VIRTUAL_W as f64;
        let sh = VIRTUAL_H as f64;
        let margin_x = 70.0;
        let top = 70.0;
        let bottom = 35.0;
        let avail_w = (sw - 2.0 * margin_x).max(100.0);
        let avail_h = (sh - top - bottom).max(100.0);
        let z = (avail_w / (maxx - minx).max(1.0)).min(avail_h / (maxy - miny).max(1.0));
        self.graph_zoom = z.clamp(0.04, 1.0);
        let used_w = (maxx - minx) * self.graph_zoom;
        let used_h = (maxy - miny) * self.graph_zoom;
        self.graph_pan = [
            (sw - used_w) / 2.0 - minx * self.graph_zoom - 30.0,
            top + (avail_h - used_h) / 2.0 - miny * self.graph_zoom - 70.0,
        ];
    }

    fn draw_graph(&mut self) {
        self.texts.clear();
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        fill_rect(&mut self.frame, fw, fh, 0, 0, VIRTUAL_W, VIRTUAL_H, [8, 11, 10, 255]);
        let pos = self.graph_positions();
        let z = self.graph_zoom;
        let (px0, py0) = (self.graph_pan[0], self.graph_pan[1]);

        for p in &self.pairs {
            if let (Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(&p.a), pos.get(&p.b)) {
                let x1 = i32::from_f64((ax + 130.0) * z + px0 + 30.0);
                let y1 = i32::from_f64((ay + 75.0) * z + py0 + 70.0);
                let x2 = i32::from_f64((bx + 130.0) * z + px0 + 30.0);
                let y2 = i32::from_f64((by + 75.0) * z + py0 + 70.0);
                let inter = self.edges[p.ab]
                    .profiles
                    .iter()
                    .filter(|x| self.edges[p.ba].profiles.contains(x))
                    .count();
                let col = if inter == 3 { [95, 210, 135, 255] } else { [215, 185, 90, 255] };
                line(&mut self.frame, fw, fh, x1, y1, x2, y2, col);
            }
        }
        for &ei in &self.one_way {
            let e = &self.edges[ei];
            if let (Some(&(ax, ay)), Some(&(bx, by))) = (pos.get(&e.from), pos.get(&e.to)) {
                let x1 = i32::from_f64((ax + 130.0) * z + px0 + 30.0);
                let y1 = i32::from_f64((ay + 75.0) * z + py0 + 70.0);
                let x2 = i32::from_f64((bx + 130.0) * z + px0 + 30.0);
                let y2 = i32::from_f64((by + 75.0) * z + py0 + 70.0);
                line(&mut self.frame, fw, fh, x1, y1, x2, y2, [220, 95, 95, 255]);
            }
        }

        self.node_rects.clear();
        let loc_ids: Vec<u32> = self.world.locs.iter().map(|l| l.loc_id).collect();
        for loc_id in loc_ids {
            let (x, y) = match pos.get(&loc_id) {
                Some(v) => *v,
                None => continue,
            };
            let rx = i32::from_f64(x * z + px0 + 30.0);
            let ry = i32::from_f64(y * z + py0 + 70.0);
            let rw = ((280.0 * z) as i32).max(80);
            let rh = ((180.0 * z) as i32).max(50);
            self.node_rects.insert(loc_id, (rx, ry, rw, rh));
            if rx + rw < 0 || ry + rh < 0 || rx > VIRTUAL_W || ry > VIRTUAL_H {
                continue;
            }
            fill_rect(&mut self.frame, fw, fh, rx, ry, rw, rh, [38, 47, 42, 255]);
            rect_outline(&mut self.frame, fw, fh, rx, ry, rw, rh, [100, 125, 110, 255]);
            if z >= 0.42 {
                let th = self.world.thumbnail(loc_id, 260, 145);
                let tw = ((260.0 * z) as i32).max(1);
                let thh = ((145.0 * z) as i32).max(1);
                let scaled = scale_image(&th, tw as usize, thh as usize);
                blit(
                    &mut self.frame,
                    fw,
                    fh,
                    &scaled.data,
                    scaled.w,
                    scaled.h,
                    rx + ((10.0 * z) as i32),
                    ry + ((26.0 * z) as i32),
                );
            }
            let degree = self
                .edges
                .iter()
                .filter(|e| e.from == loc_id && e.from != e.to)
                .count();
            let local = self.self_edges.iter().filter(|&&i| self.edges[i].from == loc_id).count();
            let name = self.label(loc_id);
            self.text_clamped(
                rx + 6,
                ry + 5,
                0xEB_F0EB,
                (0.7 * z as f32).max(0.5),
                format!("{}  exits:{}  local:{}", name, degree, local),
            );
        }

        let hdr = format!(
            "EXACT FOREST COUNTRY GRAPH | profile:{} | maps:{} directed:{} paired:{} one-way:{} | click open | wheel zoom | drag pan",
            self.profile,
            self.world.locs.len(),
            self.edges.len(),
            self.pairs.len(),
            self.one_way.len()
        );
        let hy = self.text_wrapped(12, 12, 0xF5_F5F5, 1.1, hdr);
        self.text_wrapped(
            12,
            hy + 2,
            0xB9_E1C3,
            0.9,
            "Edges come directly from Game.N transition records. Green=present in all three starts, yellow=variant, red=one-way.".to_string(),
        );
    }

    // --- input ---

    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.host_w = w.max(1);
        self.host_h = h.max(1);
        if !self.mode_graph && self.camera_follow {
            self.center_camera_on_player(true, 0.0);
        }
    }

    /// Index of the party-portrait cell under the pointer (2x5 grid), if any.
    /// Read-only hit test; the original *selects* the member here (region
    /// `0x1e..0x27`) and never opens the character screen from it.
    fn party_cell_at(&self, x: f64, y: f64) -> Option<usize> {
        let (mx, my) = self.window_to_virtual(x, y)?;
        let (mx, my) = (mx.round() as i32, my.round() as i32);
        let l = crate::hud::layout(VIRTUAL_W);
        (0..crate::hud::PARTY_CELLS)
            .find(|&i| l.party_cell(i).map(|r| r.contains(mx, my)).unwrap_or(false))
    }

    pub fn pointer_down(&mut self, button: u32, x: f64, y: f64) {
        // While the character screen is open it consumes every click: no world
        // targeting and no click-through. It is closed with Escape only.
        if self.character_screen {
            return;
        }
        // A left-click on a party-portrait cell is consumed (the original
        // selects that member; with a one-member party it is a no-op). It must
        // not issue a world movement command. The character screen is not opened
        // by this click (original: opened by a command) — use `C` instead.
        if button == 1 && !self.mode_graph && self.party_cell_at(x, y).is_some() {
            return;
        }
        if button == 3 && !self.mode_graph {
            self.mode_graph = true;
            self.selected = None;
            self.drag = false;
        } else if button == 1 {
            if self.mode_graph {
                let v = self.window_to_virtual(x, y);
                if let Some((vx, vy)) = v {
                    let hit = self
                        .node_rects
                        .iter()
                        .find(|(_, r)| {
                            vx >= r.0 as f64
                                && vx < (r.0 + r.2) as f64
                                && vy >= r.1 as f64
                                && vy < (r.1 + r.3) as f64
                        })
                        .map(|(k, _)| *k);
                    if let Some(lid) = hit {
                        self.current_map = lid;
                        self.selected = Some(lid);
                        self.mode_graph = false;
                        self.target = None;
                        self.fit_detail();
                    }
                }
            } else {
                self.set_target_from_mouse(x, y);
            }
            self.drag = false;
            self.last_mouse = (x, y);
        } else if button == 2 {
            self.camera_follow = false;
            self.drag = true;
            self.last_mouse = (x, y);
        }
    }

    pub fn pointer_up(&mut self, button: u32) {
        if button == 2 {
            self.drag = false;
        }
    }

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        if self.drag {
            let dx = x - self.last_mouse.0;
            let dy = y - self.last_mouse.1;
            self.last_mouse = (x, y);
            self.hovered_slot = None;
            if self.mode_graph {
                self.graph_pan[0] += dx;
                self.graph_pan[1] += dy;
            } else {
                let (_, _, _, _, sc) = self.window_game_rect();
                self.detail_pan[0] += dx / sc.max(1e-6);
                self.detail_pan[1] += dy / sc.max(1e-6);
            }
            return;
        }
        if self.mode_graph {
            self.hovered_slot = None;
        } else {
            self.update_hover(x, y);
        }
    }

    /// Read-only HUD hover: which visible inventory cell is under the cursor.
    /// Does not touch world targeting, movement or portals.
    fn update_hover(&mut self, x: f64, y: f64) {
        let Some((mx, my)) = self.window_to_virtual(x, y) else {
            self.hovered_slot = None;
            return;
        };
        let l = crate::hud::layout(VIRTUAL_W);
        let (mx, my) = (mx.round() as i32, my.round() as i32);
        self.hovered_slot = (0..l.visible_slots).find(|&slot| {
            l.inventory_slot_rect(slot, 70)
                .map(|r| r.contains(mx, my))
                .unwrap_or(false)
        });
    }

    pub fn wheel(&mut self, delta: f64) {
        if self.mode_graph {
            let factor = 1.2f64.powf(delta);
            self.graph_zoom = (self.graph_zoom * factor).clamp(0.04, 3.0);
        }
    }

    pub fn key_down(&mut self, code: u32) {
        match code {
            // Escape closes the character screen when open; otherwise it opens
            // the frontend/menu (the live game is preserved by the host runtime).
            KEY_ESCAPE => {
                if self.character_screen {
                    self.character_screen = false;
                    self.hovered_slot = None;
                } else {
                    self.request_menu = true;
                }
            }
            KEY_H => {
                if !self.mode_graph {
                    self.mode_graph = true;
                    self.selected = None;
                } else {
                    self.mode_graph = false;
                    self.selected = Some(self.current_map);
                    self.fit_detail();
                }
            }
            KEY_BACKSPACE => {
                if !self.mode_graph {
                    self.mode_graph = true;
                    self.selected = None;
                }
            }
            // Tab toggles the protagonist's battle stance (`Entity+0x03` bit
            // `0x04`): peaceful idle/walk (6/7) <-> battle idle/walk (0/1).
            KEY_TAB => {
                if !self.mode_graph {
                    self.player.flags_b ^= crate::anim::FLAG_BATTLE;
                    self.anim_time = 0.0;
                }
            }
            KEY_D => {
                self.debug_ids = !self.debug_ids;
            }
            KEY_P => {
                self.population_debug = !self.population_debug;
            }
            KEY_L => {
                self.layer_debug = !self.layer_debug;
            }
            KEY_T => {
                self.teleport_debug = !self.teleport_debug;
            }
            KEY_F => {
                if !self.mode_graph {
                    self.fit_detail();
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                }
            }
            // `C` toggles the character screen (the original opens it via a UI
            // command/action, not a mouse portrait click; this is the remake's
            // explicit binding). Escape also closes it.
            KEY_C => {
                if !self.mode_graph {
                    self.character_screen = !self.character_screen;
                    self.hovered_slot = None;
                }
            }
            // `V` toggles camera follow (moved off `C`).
            KEY_V => {
                if !self.mode_graph {
                    self.camera_follow = !self.camera_follow;
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                }
            }
            KEY_R => {
                self.world.clear_caches();
            }
            _ => {}
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
mod tests {
    use super::*;

    // Konung.4, Game.1: gates at raw averaged 244/245 grid centers, in gate-index order.
    const K4_GATES: [(f64, f64); 4] =
        [(123.0, 61.5), (115.5, 5.0), (16.0, 6.5), (66.0, 65.0)];
    // Outgoing edges in (rec, to) order: rec10 4->5, rec11 4->15, rec12 4->30, rec13 4->32.
    const K4_TRIGGERS: [(i32, i32, i32, i32); 4] =
        [(63, 64, 63, 64), (119, 61, 119, 61), (112, 4, 112, 4), (12, 5, 12, 5)];
    const K4_RECS: [usize; 4] = [10, 11, 12, 13];

    fn install_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// A minimal playable app (real `Game.res` + `CLIENT.EXE`; synthetic
    /// GRAPH/Objects) for input-routing tests. Gated on files being present.
    fn build_test_app() -> Option<PlayableApp> {
        use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
        use crate::hero::hero;
        use crate::render::World;
        use crate::res::Res;
        let dir = install_dir();
        let game = std::fs::read(dir.join("Game.res")).ok()?;
        let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
        let graph = vec![0u8; 0x30000]; // palettes/terrain unused in this test
        let objects = vec![0u8; 8 + 512 * 8];
        let res = Res::new(game, graph, objects, exe);
        let world = World::new(res, Vec::new());
        let h = *hero(0).unwrap();
        let mut player = PlayerCharacter::from_build(
            h,
            CharacterBuild::from_class(CharacterClass::Warrior),
        );
        player.hp_internal = 1600;
        player.equipment = [0, 0, 1, 0, 0, 0];
        Some(PlayableApp::new(
            world,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            1,
            Vec::new(),
            player,
            1,
            0.0,
            0.0,
            5,
            None,
            Vec::new(),
        ))
    }

    #[test]
    fn character_screen_open_close_and_no_world_clickthrough() {
        let Some(mut app) = build_test_app() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let inv0 = app.player.inventory;
        let eq0 = app.player.equipment;
        let hp0 = app.player.hp_internal;

        // A party-portrait click selects/consumes only: it never opens the
        // screen (original semantics) and never issues a world move.
        assert!(!app.character_screen);
        app.pointer_down(1, 36.0, 36.0);
        assert!(!app.character_screen, "portrait click must not open the screen");
        assert!(app.target.is_none(), "portrait click must not move the world");

        // `C` opens the character screen.
        app.key_down(KEY_C);
        assert!(app.character_screen);
        assert!(app.target.is_none(), "opening must not issue a world move");

        // While open, a world click and an equipment click are consumed.
        app.pointer_down(1, 500.0, 300.0);
        assert!(app.character_screen);
        assert!(app.target.is_none(), "no click-through to the world");
        app.pointer_down(1, 183.0, 103.0);
        assert!(app.character_screen);
        assert!(app.target.is_none());

        // `C` toggles closed; Escape also closes.
        app.key_down(KEY_C);
        assert!(!app.character_screen);
        app.key_down(KEY_C);
        assert!(app.character_screen);
        app.key_down(KEY_ESCAPE);
        assert!(!app.character_screen);

        // World clicks work again after closing.
        app.pointer_down(1, 500.0, 300.0);
        assert!(app.target.is_some(), "world targeting restored after close");

        // `V` toggles camera follow (moved off `C`).
        let cf = app.camera_follow;
        app.key_down(KEY_V);
        assert_eq!(app.camera_follow, !cf);

        // Open/close cycles do not mutate gameplay data.
        assert_eq!(app.player.inventory, inv0);
        assert_eq!(app.player.equipment, eq0);
        assert_eq!(app.player.hp_internal, hp0);
    }

    #[test]
    fn tab_toggles_protagonist_battle_stance() {
        let Some(mut app) = build_test_app() else {
            eprintln!("skipping: original resources not present");
            return;
        };
        let before = app.player.flags_b & crate::anim::FLAG_BATTLE;
        app.key_down(KEY_TAB);
        assert_ne!(
            app.player.flags_b & crate::anim::FLAG_BATTLE,
            before,
            "Tab must flip the battle stance bit"
        );
        app.key_down(KEY_TAB);
        assert_eq!(
            app.player.flags_b & crate::anim::FLAG_BATTLE,
            before,
            "Tab must be a toggle"
        );
    }

    #[test]
    fn konung4_maps_each_trigger_to_its_gate() {
        let a = match_triggers_to_gates(&K4_TRIGGERS, &K4_RECS, &K4_GATES, MAX_GATE_TRIGGER_DIST);
        // rec10 4->5 -> gate3, rec11 4->15 -> gate0, rec12 4->30 -> gate1, rec13 4->32 -> gate2
        assert_eq!(a, vec![Some(3), Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn global_matching_beats_greedy_rec_order() {
        // g0 near e1; e0 is nearer to g1 than greedy (rec order) would allow by
        // taking g0 first and stranding e1.
        let gates = [(0.0, 0.0), (10.0, 0.0)];
        let triggers = [(4, 0, 4, 0), (1, 0, 1, 0)]; // e0 rec0, e1 rec1
        let recs = [0usize, 1];
        let a = match_triggers_to_gates(&triggers, &recs, &gates, 6.0);
        // Global optimum binds both: e0->g1 (6.0), e1->g0 (1.0).
        assert_eq!(a, vec![Some(1), Some(0)]);
    }

    #[test]
    fn threshold_leaves_distant_trigger_unbound() {
        let gates = [(0.0, 0.0)];
        let triggers = [(50, 50, 50, 50)];
        let recs = [7usize];
        assert_eq!(match_triggers_to_gates(&triggers, &recs, &gates, MAX_GATE_TRIGGER_DIST), vec![None]);
    }

    #[test]
    fn point_and_rectangular_trigger_centers() {
        assert_eq!(trigger_center(12, 5, 12, 5), [12.0, 5.0]);
        assert_eq!(trigger_center(136, 10, 146, 14), [141.0, 12.0]);
        assert_eq!(trigger_center(14, 7, 12, 5), [13.0, 6.0]);
    }

    #[test]
    fn dist_point_to_rect_handles_points_and_ranges() {
        // Point trigger.
        assert_eq!(dist_point_to_rect(12.0, 5.0, 12, 5, 12, 5), 0.0);
        assert_eq!(dist_point_to_rect(15.0, 9.0, 12, 5, 12, 5), 5.0);
        // Inside a normalized range.
        assert_eq!(dist_point_to_rect(12.0, 5.0, 14, 7, 0, 0), 0.0);
        // Nearest corner when outside; unnormalized corners are accepted.
        assert_eq!(dist_point_to_rect(0.0, 0.0, 14, 7, 12, 5), 13.0);
    }

    #[test]
    fn tie_break_is_deterministic_by_rec_then_gate() {
        // Two identical gates, two coincident triggers of equal cost: the winner
        // must be the lexicographically smallest (rec, gate) pairing.
        let gates = [(0.0, 0.0), (0.0, 0.0)];
        let triggers = [(0, 0, 0, 0), (0, 0, 0, 0)];
        let recs = [5usize, 3]; // local 0 = rec5, local 1 = rec3
        let a = match_triggers_to_gates(&triggers, &recs, &gates, 1.0);
        // rec3 -> gate0, rec5 -> gate1.
        assert_eq!(a, vec![Some(1), Some(0)]);
    }

    #[test]
    fn wrap_to_px_keeps_every_line_within_width() {
        let s = "OpenKonung v15 | Старый лагерь | hero:Dragomir Game.1 | 800x600 native camera | pal:33 | portals:4 | C follow | H graph | D ids | P pop | L layers | T portals | cell=(72,34)";
        let lines = wrap_to_px(s, 1.0, VIRTUAL_W - 16);
        assert!(lines.len() > 1, "long header must wrap");
        for l in &lines {
            assert!(text_px(l, 1.0) <= VIRTUAL_W - 16, "line too wide: {l:?}");
        }
    }

    #[test]
    fn wrap_to_px_hard_splits_a_single_long_word() {
        let s = "x".repeat(400);
        let lines = wrap_to_px(&s, 1.0, 200);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(text_px(l, 1.0) <= 200);
        }
        assert_eq!(lines.concat().len(), 400);
    }

    #[test]
    fn text_clamped_width_estimate_is_positive_and_monotonic() {
        assert!(text_px("", 1.0) == 0);
        assert!(text_px("ab", 1.0) < text_px("abcd", 1.0));
        assert!(text_px("abcd", 1.0) < text_px("abcd", 2.0));
    }
}
