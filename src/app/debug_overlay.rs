//! `debug_overlay` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
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
    pub(super) fn draw_portal_debug(&mut self, top: i32) {
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
            rect_outline(
                &mut self.frame,
                fw,
                fh,
                rx - 3,
                ry - 2,
                rw,
                rh,
                [80, 200, 255, 255],
            );
            if on_screen(rx, ry) {
                self.text_clamped(
                    rx + 4,
                    ry - 16,
                    0x80_C8FF,
                    0.8,
                    format!(
                        "T rec{} {}->{} trig({},{})-({},{}) arr({},{})",
                        e.rec,
                        e.from,
                        e.to,
                        e.dst[0],
                        e.dst[1],
                        e.dst2[0],
                        e.dst2[1],
                        e.arrival()[0],
                        e.arrival()[1]
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
            let (glx, gly) =
                prop_to_local_px(b.gate_grid[0].round() as i32, b.gate_grid[1].round() as i32);
            let gsx = iox + glx;
            let gsy = ioy + gly;
            line(
                &mut self.frame,
                fw,
                fh,
                gsx,
                gsy,
                rsx,
                rsy,
                [255, 200, 60, 255],
            );
            ellipse_outline(
                &mut self.frame,
                fw,
                fh,
                rsx as f32,
                rsy as f32,
                9.0,
                9.0,
                2,
                [120, 255, 120, 255],
            );
            rect_outline(
                &mut self.frame,
                fw,
                fh,
                gsx - 4,
                gsy - 4,
                9,
                9,
                [255, 120, 220, 255],
            );
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
                ellipse_outline(
                    &mut self.frame,
                    fw,
                    fh,
                    sx as f32,
                    sy as f32,
                    11.0,
                    6.0,
                    2,
                    [255, 220, 80, 255],
                );
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
            rect_outline(
                &mut self.frame,
                fw,
                fh,
                iox + lx - 8,
                ioy + ly - 8,
                16,
                16,
                [255, 120, 220, 255],
            );
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
            format!(
                "PORTALS in Konung.{} ({})",
                self.current_map,
                bindings.len()
            ),
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
    pub(super) fn draw_layer_debug(&mut self) {
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
    pub(super) fn draw_population_debug(&mut self) {
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
        let (
            matching,
            reserved,
            inactive,
            invalid,
            enumerated,
            persisted,
            candidates,
            placed,
            failed,
        ) = counts.unwrap_or((0, 0, 0, 0, 0, 0, 0, 0, 0));
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
}
