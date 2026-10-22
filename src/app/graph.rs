//! `graph` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    pub(super) fn graph_positions(&self) -> HashMap<u32, (f64, f64)> {
        self.layout
            .iter()
            .map(|(k, v)| (*k, (v.0 * 330.0, v.1 * 235.0)))
            .collect()
    }

    pub(super) fn fit_graph(&mut self) {
        let pos = self.graph_positions();
        if pos.is_empty() {
            self.graph_zoom = 1.0;
            self.graph_pan = [0.0, 0.0];
            return;
        }
        let minx = pos.values().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let miny = pos.values().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let maxx = pos
            .values()
            .map(|p| p.0 + 280.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let maxy = pos
            .values()
            .map(|p| p.1 + 180.0)
            .fold(f64::NEG_INFINITY, f64::max);
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

    pub(super) fn draw_graph(&mut self) {
        self.texts.clear();
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        fill_rect(
            &mut self.frame,
            fw,
            fh,
            0,
            0,
            VIRTUAL_W,
            VIRTUAL_H,
            [8, 11, 10, 255],
        );
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
                let col = if inter == 3 {
                    [95, 210, 135, 255]
                } else {
                    [215, 185, 90, 255]
                };
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
            rect_outline(
                &mut self.frame,
                fw,
                fh,
                rx,
                ry,
                rw,
                rh,
                [100, 125, 110, 255],
            );
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
            let local = self
                .self_edges
                .iter()
                .filter(|&&i| self.edges[i].from == loc_id)
                .count();
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
}
