//! `world_draw` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    pub(super) fn draw_detail(&mut self) {
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


}
