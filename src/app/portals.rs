//! `portals` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

/// Cosmetic binding between one outgoing transition edge and its nearest visual
/// 244/245 gate. The canonical gameplay trigger is the source of truth: the
/// normal portal ring/label is drawn at `trigger` (raw runtime/Entity), while the
/// associated visual gate center (`gate_grid`, raw runtime-grid) is retained
/// only for the `T` debug overlay. `dist` is the gate-center → trigger-rectangle
/// distance in grid units.
#[derive(Clone)]
pub(super) struct PortalBinding {
    pub(super) edge: usize,
    pub(super) gate: usize,
    /// Canonical trigger rectangle center in raw runtime/Entity coordinates.
    pub(super) trigger: [f64; 2],
    /// Associated visual gate center in raw runtime-grid coordinates (grid_x, grid_y).
    pub(super) gate_grid: [f64; 2],
    /// Gate-center → trigger-rectangle distance (grid units).
    pub(super) dist: f64,
}


/// Maximum gate-center → trigger-rectangle distance (grid units) for a cosmetic
/// binding. Empirical Game.1 distribution: 100/108 outgoing transitions have a
/// nearest gate within 8.0, then a clear gap to 18.34; 13.0 sits in that gap, so
/// unrelated 244/245 objects stay unbound rather than drawing a misleading ring.
pub(super) const MAX_GATE_TRIGGER_DIST: f64 = 13.0;


/// Center of a trigger rectangle given its raw (possibly unnormalized) corners.
/// A point trigger yields its exact coordinate.
pub(super) fn trigger_center(x1: i32, y1: i32, x2: i32, y2: i32) -> [f64; 2] {
    [(x1 + x2) as f64 * 0.5, (y1 + y2) as f64 * 0.5]
}


/// Distance from a point to the rectangle with the given raw corners (0 if the
/// point lies inside the normalized rectangle).
pub(super) fn dist_point_to_rect(px: f64, py: f64, x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
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
pub(super) struct GateMatchBest {
    count: usize,
    cost: f64,
    /// Sorted `(rec, gate)` pairs: the deterministic tie-break key.
    key: Vec<(usize, usize)>,
    assign: Vec<Option<usize>>,
}


pub(super) fn gate_match_better(a: &GateMatchBest, b: &GateMatchBest) -> bool {
    if a.count != b.count {
        return a.count > b.count;
    }
    if (a.cost - b.cost).abs() > 1e-9 {
        return a.cost < b.cost;
    }
    a.key < b.key
}


pub(super) fn gate_match_key(assign: &[Option<usize>], recs: &[usize]) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = assign
        .iter()
        .enumerate()
        .filter_map(|(e, g)| g.map(|gi| (recs[e], gi)))
        .collect();
    v.sort_unstable();
    v
}


#[allow(clippy::too_many_arguments)]
pub(super) fn gate_match_dfs(
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
pub(super) fn match_triggers_to_gates(
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


impl PlayableApp {
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
    pub(super) fn rebuild_portal_bindings(&mut self) {
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


    pub(super) fn outgoing(&self, lid: u32) -> Vec<PortalBinding> {
        self.portal_bindings.get(&lid).cloned().unwrap_or_default()
    }

    // --- Phase C1 population ---


}
