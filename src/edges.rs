//! Exact Game.N transition graph.
//!
//! Faithful port of `konung_exact_country.py`.

use std::collections::HashMap;

use crate::loc::Location;
use crate::res::u16;

pub const GAME_SIZE: usize = 0xAD4BE;
pub use crate::constants::game_layout::TRANS_OFF;
pub use crate::constants::game_layout::TRANS_REC;
pub use crate::constants::game_layout::TRANS_COUNT;

pub type Key7 = (u8, u8, u8, u16, u16, u16, u16, u16, u16);

/// One 17-byte `Game.N` transition record (`+0x29600`).
///
/// Canonical field roles (`KONUNG_SOURCE_OF_TRUTH.md` §13.1):
/// `+05/+07` = destination arrival coordinates (`sx/sy`), and
/// `+09/+0B` + `+0D/+0F` = source trigger rectangle (`dx/dy`, `dx2/dy2`), all in
/// the raw runtime/Entity coordinate domain.
#[derive(Clone, Copy)]
pub struct Transition {
    pub rec: usize,
    pub meta: u8,
    pub src: u8,
    pub dst: u8,
    pub sx: u16,
    pub sy: u16,
    pub dx: u16,
    pub dy: u16,
    pub dx2: u16,
    pub dy2: u16,
}

impl Transition {
    pub fn key7(&self) -> Key7 {
        (self.meta, self.src, self.dst, self.sx, self.sy, self.dx, self.dy, self.dx2, self.dy2)
    }
}

pub fn read_transitions(data: &[u8]) -> Vec<Transition> {
    let mut out = Vec::new();
    if data.len() != GAME_SIZE {
        return out;
    }
    for i in 0..TRANS_COUNT {
        let o = TRANS_OFF + i * TRANS_REC;
        let r = &data[o..o + TRANS_REC];
        let src = r[3];
        let dst = r[4];
        if src == 0 && dst == 0 {
            continue;
        }
        out.push(Transition {
            rec: i,
            meta: r[2],
            src,
            dst,
            sx: u16(r, 5) as u16,
            sy: u16(r, 7) as u16,
            dx: u16(r, 9) as u16,
            dy: u16(r, 11) as u16,
            dx2: u16(r, 13) as u16,
            dy2: u16(r, 15) as u16,
        });
    }
    out
}

#[derive(Clone)]
pub struct Edge {
    pub rec: usize,
    pub meta: u8,
    pub from: u32,
    pub to: u32,
    /// Destination arrival point (`+05/+07`), raw runtime/Entity coordinates.
    pub src: [i32; 2],
    /// Source trigger rectangle corner 1 (`+09/+0B`), raw runtime/Entity coords.
    pub dst: [i32; 2],
    /// Source trigger rectangle corner 2 (`+0D/+0F`), raw runtime/Entity coords.
    pub dst2: [i32; 2],
    pub side: &'static str,
    pub profiles: Vec<u32>,
}

impl Edge {
    /// Destination arrival point (`+05/+07`) in raw runtime/Entity coordinates.
    pub fn arrival(&self) -> [i32; 2] {
        self.src
    }

    /// Source trigger rectangle (`+09/+0B`)-(`+0D/+0F`), normalized to
    /// `(min, max)` in raw runtime/Entity coordinates.
    pub fn trigger_rect(&self) -> ([i32; 2], [i32; 2]) {
        let (x1, y1) = (self.dst[0], self.dst[1]);
        let (x2, y2) = (self.dst2[0], self.dst2[1]);
        ([x1.min(x2), y1.min(y2)], [x1.max(x2), y1.max(y2)])
    }

    /// True when a raw runtime/Entity cell lies inside this edge's normalized
    /// source trigger rectangle.
    pub fn trigger_contains(&self, x: i32, y: i32) -> bool {
        let (mn, mx) = self.trigger_rect();
        x >= mn[0] && x <= mx[0] && y >= mn[1] && y <= mx[1]
    }
}

pub struct Pair {
    pub a: u32,
    pub b: u32,
    pub ab: usize,
    pub ba: usize,
    pub coord_error: i64,
}

fn classify_side(x: i32, y: i32, loc: &Location) -> &'static str {
    let gate_max_x = loc.gates.iter().map(|g| g.x).fold(0.0f64, f64::max) as i32;
    let gate_max_y = loc.gates.iter().map(|g| g.y).fold(0.0f64, f64::max) as i32;
    let mx = loc.max_x.max(gate_max_x).max(159);
    let my = loc.max_y.max(gate_max_y).max(79);
    let cx = mx as f64 / 2.0;
    let cy = my as f64 / 2.0;
    let dx = (x as f64 - cx) / (mx as f64).max(1.0);
    let dy = (y as f64 - cy) / (my as f64).max(1.0);
    let dirs = ["E", "SE", "S", "SW", "W", "NW", "N", "NE"];
    let idx = (dy.atan2(dx) / (std::f64::consts::PI / 4.0)).round() as i64;
    dirs[idx.rem_euclid(8) as usize]
}

pub fn build_exact_edges(
    profile: Option<u32>,
    profiles: &HashMap<u32, Vec<Transition>>,
    locs: &[Location],
) -> Vec<Edge> {
    let by_id: HashMap<u32, &Location> = locs.iter().map(|l| (l.loc_id, l)).collect();

    let mut obj: HashMap<Key7, Transition> = HashMap::new();
    let mut pres: HashMap<Key7, Vec<u32>> = HashMap::new();
    for (p, trs) in profiles.iter() {
        for t in trs {
            let k = t.key7();
            obj.insert(k, *t);
            let e = pres.entry(k).or_default();
            if !e.contains(p) {
                e.push(*p);
            }
        }
    }
    for v in pres.values_mut() {
        v.sort_unstable();
    }

    let keys: Vec<Key7> = match profile {
        Some(id) => profiles
            .get(&id)
            .map(|trs| trs.iter().map(|t| t.key7()).collect())
            .unwrap_or_default(),
        None => obj.keys().copied().collect(),
    };

    let mut keys = keys;
    keys.sort_by_key(|z| (z.1, z.2, z.3, z.4));
    keys.dedup();

    let mut edges = Vec::new();
    for k in keys {
        let t = match obj.get(&k) {
            Some(t) => *t,
            None => continue,
        };
        if t.src == 0 || t.dst == 0 {
            continue;
        }
        let side = match by_id.get(&(t.src as u32)) {
            Some(loc) => classify_side(t.sx as i32, t.sy as i32, loc),
            None => "?",
        };
        edges.push(Edge {
            rec: t.rec,
            meta: t.meta,
            from: t.src as u32,
            to: t.dst as u32,
            src: [t.sx as i32, t.sy as i32],
            dst: [t.dx as i32, t.dy as i32],
            dst2: [t.dx2 as i32, t.dy2 as i32],
            side,
            profiles: pres.get(&k).cloned().unwrap_or_default(),
        });
    }
    edges
}

pub fn reciprocal_pairs(edges: &[Edge]) -> (Vec<Pair>, Vec<usize>, Vec<usize>) {
    let mut directed: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        if e.from != e.to {
            directed.entry((e.from, e.to)).or_default().push(i);
        }
    }

    let mut pairs = Vec::new();
    let mut used = vec![false; edges.len()];

    let mut dir_keys: Vec<(u32, u32)> = directed.keys().copied().collect();
    dir_keys.sort();
    for (a, b) in dir_keys {
        if a > b {
            continue;
        }
        let xs = directed.get(&(a, b)).cloned().unwrap_or_default();
        let ys = directed.get(&(b, a)).cloned().unwrap_or_default();
        let mut candidates: Vec<(i64, usize, usize)> = Vec::new();
        for &ix in &xs {
            for &iy in &ys {
                let x = &edges[ix];
                let y = &edges[iy];
                let d = (x.dst[0] - y.src[0]) as i64 * (x.dst[0] - y.src[0]) as i64
                    + (x.dst[1] - y.src[1]) as i64 * (x.dst[1] - y.src[1]) as i64
                    + (y.dst[0] - x.src[0]) as i64 * (y.dst[0] - x.src[0]) as i64
                    + (y.dst[1] - x.src[1]) as i64 * (y.dst[1] - x.src[1]) as i64;
                candidates.push((d, ix, iy));
            }
        }
        candidates.sort();
        for (d, ix, iy) in candidates {
            if used[ix] || used[iy] {
                continue;
            }
            used[ix] = true;
            used[iy] = true;
            pairs.push(Pair {
                a,
                b,
                ab: ix,
                ba: iy,
                coord_error: d,
            });
        }
    }

    let mut one_way = Vec::new();
    let mut self_edges = Vec::new();
    for (i, e) in edges.iter().enumerate() {
        if e.from == e.to {
            self_edges.push(i);
        } else if !used[i] {
            one_way.push(i);
        }
    }
    (pairs, one_way, self_edges)
}

pub fn side_vec(side: &str) -> (f64, f64) {
    match side {
        "N" => (0.0, -1.0),
        "NE" => (1.0, -1.0),
        "E" => (1.0, 0.0),
        "SE" => (1.0, 1.0),
        "S" => (0.0, 1.0),
        "SW" => (-1.0, 1.0),
        "W" => (-1.0, 0.0),
        "NW" => (-1.0, -1.0),
        _ => (0.0, 0.0),
    }
}

pub fn spring_layout(
    locs: &[Location],
    edges: &[Edge],
    pairs: &[Pair],
) -> HashMap<u32, (f64, f64)> {
    let mut ids: Vec<u32> = locs.iter().map(|l| l.loc_id).collect();
    ids.sort_unstable();
    let n = ids.len().max(1);

    let mut pos: HashMap<u32, [f64; 2]> = HashMap::new();
    for (i, &lid) in ids.iter().enumerate() {
        let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        pos.insert(lid, [a.cos() * 8.0, a.sin() * 6.0]);
    }

    let mut links: Vec<(u32, u32, &'static str)> = Vec::new();
    for p in pairs {
        links.push((p.a, p.b, edges[p.ab].side));
    }
    for (i, e) in edges.iter().enumerate() {
        if e.from != e.to && !pairs.iter().any(|p| p.ab == i || p.ba == i) {
            links.push((e.from, e.to, e.side));
        }
    }

    let target = 2.4;
    for it in 0..700 {
        let mut f: HashMap<u32, [f64; 2]> = ids.iter().map(|&id| (id, [0.0, 0.0])).collect();
        // pairwise repulsion
        for i in 0..ids.len() {
            let a = ids[i];
            let (ax, ay) = (pos[&a][0], pos[&a][1]);
            for &b in ids.iter().skip(i + 1) {
                let (bx, by) = (pos[&b][0], pos[&b][1]);
                let dx = ax - bx;
                let dy = ay - by;
                let d2 = dx * dx + dy * dy + 0.2;
                let q = 0.08 / d2;
                let fx = dx * q;
                let fy = dy * q;
                f.get_mut(&a).unwrap()[0] += fx;
                f.get_mut(&a).unwrap()[1] += fy;
                f.get_mut(&b).unwrap()[0] -= fx;
                f.get_mut(&b).unwrap()[1] -= fy;
            }
        }
        for (a, b, side) in &links {
            if !pos.contains_key(a) || !pos.contains_key(b) {
                continue;
            }
            let (ax, ay) = (pos[a][0], pos[a][1]);
            let (bx, by) = (pos[b][0], pos[b][1]);
            let (dx, dy) = (bx - ax, by - ay);
            let (mut vx, mut vy) = side_vec(side);
            let norm = (vx * vx + vy * vy).sqrt();
            if norm != 0.0 {
                vx /= norm;
                vy /= norm;
            }
            let d = (dx * dx + dy * dy).sqrt() + 1e-6;
            let stretch = (d - target) * 0.025;
            let fx = dx / d * stretch;
            let fy = dy / d * stretch;
            f.get_mut(a).unwrap()[0] += fx;
            f.get_mut(a).unwrap()[1] += fy;
            f.get_mut(b).unwrap()[0] -= fx;
            f.get_mut(b).unwrap()[1] -= fy;

            let wantx = ax + vx * target;
            let wanty = ay + vy * target;
            f.get_mut(b).unwrap()[0] += (wantx - bx) * 0.018;
            f.get_mut(b).unwrap()[1] += (wanty - by) * 0.018;
            f.get_mut(a).unwrap()[0] -= (wantx - bx) * 0.006;
            f.get_mut(a).unwrap()[1] -= (wanty - by) * 0.006;
        }
        let step = (0.55 * (1.0 - it as f64 / 700.0)).max(0.05);
        for &lid in &ids {
            let p = pos.get_mut(&lid).unwrap();
            let ff = f[&lid];
            p[0] += ff[0].clamp(-0.25, 0.25) * step;
            p[1] += ff[1].clamp(-0.25, 0.25) * step;
        }
    }

    let minx = pos.values().map(|v| v[0]).fold(f64::INFINITY, f64::min);
    let miny = pos.values().map(|v| v[1]).fold(f64::INFINITY, f64::min);
    pos.into_iter().map(|(k, v)| (k, (v[0] - minx, v[1] - miny))).collect()
}

#[cfg(test)]
mod tests;
