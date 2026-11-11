//! `population` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    /// Build (once) the runtime actor list for a location from the parsed Game.N
    /// tables. Actors are pre-sorted by projected draw position at build time.
    pub(super) fn ensure_population(&mut self, loc_id: u32) {
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
        // `FUN_0043ADF0(mode 0)` population growth: consume every negative
        // `Group+0x02` accumulator and fill the target count before actors are
        // sorted. Save mode 1 never reaches this path because its cache exists.
        let grown = {
            let collision = self.world.loc(loc_id).map(|l| l.collision.as_slice());
            match (self.tables.as_mut(), collision) {
                (Some(t), Some(c)) => crate::actors::grow_map(t, loc_id as u8, c, &mut build.rng),
                _ => Vec::new(),
            }
        };
        for a in grown {
            let rect = self
                .tables
                .as_ref()
                .and_then(|t| t.groups.get(a.source_group_id))
                .map(|g| g.rect_a)
                .unwrap_or_default();
            let cell = (a.entity.x as i32, a.entity.y as i32);
            build.motion.insert(
                a.entity.index,
                ActorMotion {
                    cell,
                    render_cell: cell,
                    facing: a.entity.facing,
                    rect_a: rect,
                    ..Default::default()
                },
            );
            build.actors.push(a);
        }
        // A recruited companion left its location group (original `0x123` moves
        // the entity into the party group 0), so it must not be rebuilt as a
        // world NPC at its location.
        if !self.party.is_empty() {
            let party_ids: std::collections::HashSet<usize> =
                self.party.iter().map(|m| m.entity.index).collect();
            build
                .actors
                .retain(|a| !party_ids.contains(&a.source_entity_id()));
            build.motion.retain(|k, _| !party_ids.contains(k));
        }
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

    /// Resolve every already-positioned non-player actor (live action/frame from
    /// [`ActorMotion`], same Entity projection as the protagonist) into `out` for
    /// the dynamic depth pass.
    pub(super) fn collect_actor_sprites(&mut self, out: &mut Vec<WorldSprite>) {
        let loc_id = self.current_map;
        if let Some(pop) = self.population_cache.get(&loc_id) {
            for a in &pop.actors {
                // Live motion (draw cell + facing + action) when present, else the
                // placement with the authored stance's idle action at frame 0.
                let (facing, cell, action, frame) = pop
                    .motion
                    .get(&a.source_entity_id())
                    .map(|m| (m.facing, m.render_cell, m.action as usize, m.frame as usize))
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
                    a.entity.raw_0x12,
                    crate::anim::battle_from_flags(a.entity.flags_b),
                    a.entity.archetype,
                    a.entity.visual_index,
                    a.entity.palette_offset,
                    facing,
                    action,
                    frame,
                ) {
                    Ok((im, gx, gy)) => {
                        let (lx, ly) = entity_to_local_px_f(cell.0 as f64, cell.1 as f64);
                        out.push(WorldSprite {
                            img: im,
                            gx,
                            gy,
                            lx,
                            ly,
                            shadow: Some(crate::render::ShadowKey {
                                equipment: a.entity.equipment,
                                weapon_slot: a.entity.raw_0x12,
                                battle: crate::anim::battle_from_flags(a.entity.flags_b),
                                archetype: a.entity.archetype,
                                visual_index: a.entity.visual_index,
                                action,
                                facing,
                                frame,
                            }),
                        });
                    }
                    Err(_) => {}
                }
            }
        }
    }

    /// The non-player actor whose drawn sprite covers virtual `(mx, my)`, as
    /// `(source entity id, archetype, dialogue_id, group id)`. Uses the same
    /// projection and frame as [`Self::collect_actor_sprites`]; the front-most
    /// hit wins. `None` when no actor is under the cursor.
    pub(super) fn actor_at(&mut self, mx: i32, my: i32) -> Option<(usize, u8, u8, u8)> {
        let loc = self.current_map;
        let iox = self.detail_pan[0].round() as i32;
        let ioy = self.detail_pan[1].round() as i32;
        let mut best: Option<(usize, u8, u8, u8)> = None;
        let PlayableApp {
            world,
            population_cache,
            party,
            ..
        } = self;
        if let Some(pop) = population_cache.get(&loc) {
            for a in &pop.actors {
                if pop
                    .motion
                    .get(&a.source_entity_id())
                    .is_some_and(|m| m.dead)
                {
                    continue;
                }
                let (facing, cell, action, frame) = pop
                    .motion
                    .get(&a.source_entity_id())
                    .map(|m| (m.facing, m.render_cell, m.action as usize, m.frame as usize))
                    .unwrap_or_else(|| {
                        let battle = crate::anim::battle_from_flags(a.entity.flags_b);
                        (
                            a.entity.facing,
                            (a.entity.x as i32, a.entity.y as i32),
                            crate::anim::idle_action(battle),
                            0,
                        )
                    });
                if let Ok((im, gx, gy)) = world.res.actor_frame_state(
                    &a.entity.equipment,
                    a.entity.raw_0x12,
                    crate::anim::battle_from_flags(a.entity.flags_b),
                    a.entity.archetype,
                    a.entity.visual_index,
                    a.entity.palette_offset,
                    facing,
                    action,
                    frame,
                ) {
                    let (lx, ly) = entity_to_local_px_f(cell.0 as f64, cell.1 as f64);
                    let x = iox + lx.round() as i32 + gx;
                    let y = ioy + ly.round() as i32 + gy;
                    if sprite_opaque_at(&im, x, y, mx, my) {
                        best = Some((
                            a.source_entity_id(),
                            a.entity.archetype,
                            a.entity.dialogue_id,
                            a.entity.group_id,
                        ));
                    }
                }
            }
        }
        // Recruited companions are no longer world NPCs but remain clickable at
        // their followed position (party-mode dialogue, `0x15E`).
        for m in party.iter() {
            if m.motion.dead {
                continue;
            }
            if let Ok((im, gx, gy)) = world.res.actor_frame_state(
                &m.entity.equipment,
                m.entity.raw_0x12,
                crate::anim::battle_from_flags(m.entity.flags_b),
                m.entity.archetype,
                m.entity.visual_index,
                m.entity.palette_offset,
                m.motion.facing,
                m.motion.action as usize,
                m.motion.frame as usize,
            ) {
                let (lx, ly) = entity_to_local_px_f(
                    m.motion.render_cell.0 as f64,
                    m.motion.render_cell.1 as f64,
                );
                let x = iox + lx.round() as i32 + gx;
                let y = ioy + ly.round() as i32 + gy;
                if sprite_opaque_at(&im, x, y, mx, my) {
                    best = Some((
                        m.entity.index,
                        m.entity.archetype,
                        m.entity.dialogue_id,
                        m.entity.group_id,
                    ));
                }
            }
        }
        best
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
}

/// Original `FUN_0043E4EA` hit test: inside the sprite box **and** on a
/// non-transparent pixel. Used by the actor hover (`_81D578`) and click target.
pub(super) fn sprite_opaque_at(im: &Img, x: i32, y: i32, mx: i32, my: i32) -> bool {
    if mx < x || my < y || mx >= x + im.w as i32 || my >= y + im.h as i32 {
        return false;
    }
    let px = (mx - x) as usize;
    let py = (my - y) as usize;
    im.data[((py * im.w + px) * 4) + 3] != 0
}
