//! `player` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    /// Route to an acceptable (passable) point using the original pathfinder
    /// (`FUN_0043DA1F`). Returns `None` when the goal is blocked, out of bounds
    /// or unreachable — the original issues no movement command then.
    pub(super) fn compute_player_path(
        &self,
        start: (i32, i32),
        goal: (i32, i32),
    ) -> Option<Vec<u8>> {
        let l = self.world.loc(self.current_map)?;
        find_path(
            &l.collision,
            &HashSet::new(),
            start,
            goal,
            (
                0,
                0,
                crate::coords::RUNTIME_W - 1,
                crate::coords::RUNTIME_H - 1,
            ),
        )
    }

    /// Begin the next walk segment exactly like the original `FUN_0041435C`:
    /// take the draw projection (`+0xE8`) from the CURRENT cell at entry, then
    /// commit one cell step into `+0x08/+0x0A` and reset the frame counter. The
    /// stored cell therefore leads the drawn sprite by one step.
    pub(super) fn begin_player_segment(&mut self) {
        let Some(&dir) = self.player_path.first() else {
            return;
        };
        self.player_render_x = self.player_x;
        self.player_render_y = self.player_y;
        self.player_dir = dir as i32;
        let (nx, ny) = step_by_direction(
            self.player_x.round() as i32,
            self.player_y.round() as i32,
            dir,
        );
        self.player_x = nx as f64;
        self.player_y = ny as f64;
        self.player_path.remove(0);
        self.player_step_t = 0.0;
        self.player_active = true;
    }

    /// The runtime/entity cell under a window-space pointer, or `None` when the
    /// pointer is outside the play view.
    pub(super) fn mouse_to_cell(&self, x: f64, y: f64) -> Option<(i32, i32)> {
        let (mx, my) = self.window_to_virtual(x, y)?;
        let (ox, oy) = (self.detail_pan[0], self.detail_pan[1]);
        let (lx, ly) = (mx - ox, my - oy);
        // Nearest runtime/entity cell, clamped only to the proven runtime
        // bounds (x 0..319, y 0..159) — not to the terrain surface.
        Some(local_px_to_entity(lx.round() as i32, ly.round() as i32))
    }

    /// Store the destination and (re)start the protagonist's walk to `(tx, ty)`.
    /// Origin is the stored runtime/Entity cell `+0x08/+0x0A`. Because a
    /// segment's step is committed at its START (`FUN_0041435C`), this is the
    /// cell the hero is walking INTO while a walk is in flight — so a mid-walk
    /// click re-plans from there and never rewinds to the cell being left.
    pub(super) fn set_player_target(&mut self, tx: i32, ty: i32) {
        let start = (self.player_x.round() as i32, self.player_y.round() as i32);
        let path = self.compute_player_path(start, (tx, ty));
        // The original `FUN_004142D4`/`FUN_0041ECC8` store the destination before
        // running the pathfinder, so it is recorded even when the route fails;
        // only a successful route starts/restarts the walk.
        self.target = Some((tx as f64, ty as f64));
        match path {
            Some(p) if !p.is_empty() => {
                self.player_path = p;
                self.begin_player_segment();
            }
            _ => {
                // Blocked/unreachable (or same cell): abort to idle on the
                // committed cell (no walk command). The interaction flag is
                // consumed on arrival/failure like the original `0x422075`.
                self.player_path.clear();
                self.player_interact = false;
                self.player_active = false;
                self.player_step_t = 0.0;
                self.player_render_x = self.player_x;
                self.player_render_y = self.player_y;
            }
        }
    }

    /// Test-only: place the protagonist on a runtime cell and settle the camera.
    pub fn debug_set_player_cell(&mut self, x: i32, y: i32) {        self.player_x = x as f64;
        self.player_y = y as f64;
        self.player_render_x = self.player_x;
        self.player_render_y = self.player_y;
        self.player_path.clear();
        self.player_active = false;
        self.player_step_t = 0.0;
        self.target = None;
        self.player_attack_target = None;
        self.player_attack_ranged = false;
        self.center_camera_on_player(true, 0.0);
        self.fit_graph();
    }

    /// Diagnostic: protagonist has an active walk segment or queued route.
    pub fn debug_player_active(&self) -> bool {
        self.player_active || !self.player_path.is_empty()
    }

    // --- update ---

    /// Advance the current location's non-player actors (citizen wander) at the
    /// fixed logic rate. Uses split borrows so the world collision table and the
    /// mutable population cache are accessed simultaneously.
    pub(super) fn update_actors(&mut self, dt: f64) {
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
        let collision = world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let settlement = settlements.iter().find(|s| s.loc_id as u32 == loc);
        update_population(pop, collision, settlement, dt);
    }

    /// Seconds per player cell step: one collision cell per completed
    /// walk-animation cycle (`FUN_0041435c` state 1/7 + `FUN_00413de4`). Cycle
    /// length is the hero's walk-animation frame count from the GRAPH tables.
    pub(super) fn player_cell_period(&self) -> f64 {
        (self.player_walk_frames() / LOGIC_TICK_HZ).max(1.0 / LOGIC_TICK_HZ)
    }

    pub(super) fn player_walk_frames(&self) -> f64 {
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

    pub(super) fn update_player(&mut self, dt: f64) {
        if self.player.dead {
            return;
        }
        self.anim_time += dt;
        self.teleport_cooldown = (self.teleport_cooldown - dt).max(0.0);

        // Movement is driven by the direction queue (`FUN_0043DA1F` →
        // `Entity+0x13`), not by the raw target. One completed walk-animation
        // cycle consumes one direction code; the next cell is committed at the
        // START of its segment (`FUN_0041435C`), so `player_x/player_y` leads the
        // drawn sprite (`player_render_*`) by one step. Facing is the pending
        // direction (`FUN_004123BC` gate `facing == path[0]`), so a straight walk
        // never zigzags.
        if self.player_active {
            self.player_step_t += dt;
            if self.player_step_t >= self.player_cell_period() {
                if !self.player_path.is_empty() {
                    // Commit the next cell at cycle start.
                    self.begin_player_segment();
                } else {
                    // Cycle finished with no queued direction: arrived, or a
                    // >MAX_PATH_STEPS route prefix was consumed. Park the sprite
                    // on the committed cell (never carry a backlog) and re-route
                    // while the stored goal is still ahead.
                    self.player_active = false;
                    self.player_step_t = 0.0;
                    self.player_render_x = self.player_x;
                    self.player_render_y = self.player_y;
                    if let Some((tx, ty)) = self.target {
                        let goal = (tx.round() as i32, ty.round() as i32);
                        let cur = (self.player_x.round() as i32, self.player_y.round() as i32);
                        if cur == goal {
                            // Right-click interaction arrival: `FUN_00421B50`
                            // only calls the pickup handler when `+0x03 & 0x40`
                            // is set; a plain move never collects.
                            if self.player_interact {
                                self.try_pickup_here();
                                self.player_interact = false;
                            }
                            self.target = None;
                        } else {
                            match self.compute_player_path(cur, goal) {
                                Some(p) if !p.is_empty() => {
                                    self.player_path = p;
                                    self.begin_player_segment();
                                }
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
                let facing = self.player_dir;
                if self.travel_to_map(e.from, dst, e.arrival()[0], e.arrival()[1], facing, e.rec) {
                    self.last_teleport = Some((e.from, dst, e.rec));
                    self.teleport_error = None;
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

    /// Shared destination application for portals and world-map travel. The
    /// original travel command finds the transition record whose
    /// `destination_location (+0x04)` matches the clicked map region and uses
    /// its arrival cell (`+05/+07`) and meta/facing (`+02`).
    pub(super) fn travel_to_map(
        &mut self,
        from: u32,
        dst: u32,
        ax: i32,
        ay: i32,
        facing: i32,
        rec: usize,
    ) -> bool {
        if !self.world.loc_index.contains_key(&dst) {
            return false;
        }
        self.current_map = dst;
        self.selected = Some(dst);
        // `FUN_0043ADF0` clears the torch-highlight global on map entry.
        self.torch_reveal = false;
        // Original mode-0 map entry rebuilds the population and applies growth;
        // cached motion from the previous visit must not suppress that pass.
        self.population_cache.remove(&dst);
        self.player_x = ax as f64;
        self.player_y = ay as f64;
        self.player_render_x = ax as f64;
        self.player_render_y = ay as f64;
        self.player_dir = facing & 7;
        // Companions travel with the party: snap them onto the arrival cell
        // (their previous map cells are meaningless here) and clear routes.
        for m in self.party.iter_mut() {
            m.motion.cell = (ax, ay);
            m.motion.render_cell = (ax, ay);
            m.motion.path.clear();
            m.motion.target = None;
            m.motion.walking = false;
            m.motion.frame = 0;
        }
        self.target = None;
        self.player_path.clear();
        self.player_active = false;
        self.player_step_t = 0.0;
        self.player_interact = false;
        self.reset_object_group_activity();
        self.apply_settlement_props();
        self.rebuild_world_objects();
        self.dialog = None;
        self.teleport_cooldown = 0.9;
        self.last_teleport = Some((from, dst, rec));
        self.teleport_error = None;
        self.fit_detail();
        if self.camera_follow {
            self.center_camera_on_player(true, 0.0);
        }
        true
    }

    /// Resolve the protagonist through the canonical Entity visual path with
    /// movement animation. The active action is the original stance pair
    /// (`FUN_004142D4`/`FUN_00414A34`): walk `1`/`7` while moving, idle `0`/`6`
    /// otherwise, chosen by `Entity+0x03 & 0x04` (see [`crate::anim`]). The
    /// renderer family/class comes from `PlayerCharacter.class` (`Entity+0x04`);
    /// the visual index and palette come from the hero identity. Equipped items
    /// composite their GRAPH overlays via the original `FUN_00427e9c` rules,
    /// including the active-weapon "in hand" variant (`Entity+0x12`). No Game.res
    /// variant fallback. One walk cycle == one cell step (`FUN_00413de4`).
    /// `(action, frame index, battle)` for the protagonist's current pose. The
    /// same values drive [`player_frame`] and the ground-shadow lookup.
    pub(super) fn player_pose(&self, moving: bool) -> (usize, usize, bool) {
        let battle = crate::anim::battle_from_flags(self.player.flags_b);
        if self.player.dead {
            return (
                crate::anim::death_action(battle),
                self.player_death_frame as usize,
                battle,
            );
        }
        if !moving && !self.player_active && self.player_attack_ranged {
            return (crate::anim::ACT_RANGED, self.player_ranged_frame as usize, battle);
        }
        if !moving && !self.player_active && self.player_attack_target.is_some() {
            return (crate::anim::ACT_MELEE, self.player_attack_frame as usize, battle);
        }
        let action = if moving {
            crate::anim::walk_action(battle)
        } else {
            crate::anim::idle_action(battle)
        };
        (action, self.player_frame_index(moving, action), battle)
    }

    pub(super) fn player_frame(&mut self, moving: bool) -> Result<(Img, i32, i32), String> {
        let battle = crate::anim::battle_from_flags(self.player.flags_b);
        let (action, fi, _) = self.player_pose(moving);
        self.world.res.actor_frame_state(
            &self.player.equipment,
            self.player.raw_0x12,
            battle,
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
    pub(super) fn player_frame_index(&self, moving: bool, action: usize) -> usize {
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
}
