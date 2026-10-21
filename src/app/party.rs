//! `party` subsystem of [`crate::app`] (split from `app.rs`).
//!
//! Runtime behaviour for recruited companions (original reserved **group 0**):
//! they follow the leader at the fixed logic rate and render in the dynamic
//! depth pass. The follower AI is `FUN_00421b50` (group-0 inline branch) ->
//! `FUN_0041ecc8` -> `FUN_0041f3c8`: re-target a free cell beside the group's
//! first entity (the protagonist) once the companion is `>= 10` cells away
//! (`FUN_0043881c`).

use super::*;

impl PlayableApp {
    /// Advance recruited companions toward the leader at the fixed logic rate.
    pub(super) fn update_party(&mut self, dt: f64) {
        if self.party.is_empty() {
            return;
        }
        self.party_step_t += dt.max(0.0) * LOGIC_TICK_HZ;
        let ticks = self.party_step_t.floor() as i64;
        self.party_step_t -= ticks as f64;
        if ticks <= 0 {
            return;
        }
        let leader = (self.player_x.round() as i32, self.player_y.round() as i32);
        let PlayableApp {
            world,
            party,
            current_map,
            ..
        } = self;
        let collision = world
            .loc(*current_map)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        crate::actors::update_followers(party, collision, leader, ticks);
    }

    /// Original `FUN_00421554` battle/peace stance toggle over the current
    /// selection array. The remake binds this to TAB (the original uses SPACE).
    ///
    /// The direction comes from the **first selected** unit. Every selected
    /// living unit is moved into that stance; unselected members are untouched.
    /// Companion action rows and walk/idle cycle lengths are updated immediately
    /// so the pose changes on the same frame (`FUN_00414078`/`FUN_0041435C`).
    pub(super) fn toggle_selected_battle_stance(&mut self) {
        let leader_selected = self.party_selected.get(0).copied().unwrap_or(false);
        let first_battle = if leader_selected {
            crate::anim::battle_from_flags(self.player.flags_b)
        } else {
            let mut found = None;
            for (i, m) in self.party.iter().enumerate() {
                if self.party_selected.get(i + 1).copied().unwrap_or(false) {
                    found = Some(crate::anim::battle_from_flags(m.entity.flags_b));
                    break;
                }
            }
            match found {
                Some(b) => b,
                None => return,
            }
        };
        let enter = !first_battle;

        if leader_selected && !self.player.dead {
            let cur = crate::anim::battle_from_flags(self.player.flags_b);
            if cur != enter {
                if enter {
                    self.player.flags_b |= crate::anim::FLAG_BATTLE;
                } else {
                    self.player.flags_b &= !crate::anim::FLAG_BATTLE;
                }
                self.anim_time = 0.0;
            }
        }

        let PlayableApp {
            world,
            party,
            party_selected,
            ..
        } = self;
        for (i, m) in party.iter_mut().enumerate() {
            if !party_selected.get(i + 1).copied().unwrap_or(false) {
                continue;
            }
            let action_row = m.motion.action as usize;
            if m.entity.archetype & 0x80 != 0
                || action_row == crate::anim::ACT_BATTLE_DEATH
                || action_row == crate::anim::ACT_PEACE_DEATH
            {
                continue;
            }
            let cur = crate::anim::battle_from_flags(m.entity.flags_b);
            if cur == enter {
                continue;
            }
            if enter {
                m.entity.flags_b |= crate::anim::FLAG_BATTLE;
            } else {
                m.entity.flags_b &= !crate::anim::FLAG_BATTLE;
            }
            let battle = crate::anim::battle_from_flags(m.entity.flags_b);
            let action = if action_row == crate::anim::ACT_BATTLE_ATTACK
                || action_row == crate::anim::ACT_PEACE_ATTACK
            {
                crate::anim::attack_action(battle)
            } else if m.motion.walking {
                crate::anim::walk_action(battle)
            } else {
                crate::anim::idle_action(battle)
            };
            if m.motion.action as usize != action {
                m.motion.action = action as u8;
                if !m.motion.walking {
                    m.motion.frame = 0;
                }
            }
            let dir = (m.motion.facing & 7) as usize;
            if let Some(frames) =
                world
                    .res
                    .walk_cycle_frames(m.entity.archetype, m.entity.visual_index, dir, battle)
            {
                m.motion.walk_frames = frames.min(u8::MAX as usize) as u8;
            }
            m.motion.idle_frames = world
                .res
                .action_cycle_frames(
                    m.entity.archetype,
                    m.entity.visual_index,
                    dir,
                    crate::anim::idle_action(battle),
                )
                .unwrap_or(0)
                .min(u8::MAX as usize) as u8;
        }
    }

    /// Store a world-click destination on every **selected** companion and start
    /// its walk, mirroring the original left-click command 0 (`FUN_0042260C`),
    /// which writes the clicked cell `(tx,ty)` to `Entity+0x0C/+0x0E` on every
    /// unit in the selection array `0x81A814`. Companions path independently to
    /// the same clicked cell; collision keeps them from stacking.
    pub(super) fn command_selected_companions(&mut self, tx: i32, ty: i32) {
        if self.party.is_empty() {
            return;
        }
        let loc = self.current_map;
        let leader = (self.player_x.round() as i32, self.player_y.round() as i32);
        let PlayableApp {
            world,
            party,
            party_selected,
            ..
        } = self;
        let collision = world
            .loc(loc)
            .map(|l| l.collision.as_slice())
            .unwrap_or(&[]);
        let mut occupied: HashSet<(i32, i32)> = party.iter().map(|m| m.motion.cell).collect();
        occupied.insert(leader);
        for (i, m) in party.iter_mut().enumerate() {
            if !party_selected.get(i + 1).copied().unwrap_or(false) {
                continue;
            }
            crate::actors::command_follower(m, collision, &mut occupied, (tx, ty));
        }
    }

    /// Resolve every recruited companion for the dynamic depth pass, using the
    /// same Entity projection as the other actors. Visual-resolution failures
    /// are returned with an explicit reason.
    pub(super) fn collect_party_sprites(&mut self, out: &mut Vec<WorldSprite>) -> Vec<String> {        let mut rejected: Vec<String> = Vec::new();
        let PlayableApp { world, party, .. } = self;
        for m in party.iter() {
            match world.res.actor_frame_state(
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
                Ok((im, gx, gy)) => {
                    let (lx, ly) =
                        entity_to_local_px_f(m.motion.render_cell.0 as f64, m.motion.render_cell.1 as f64);
                    out.push(WorldSprite {
                        img: im,
                        gx,
                        gy,
                        lx,
                        ly,
                    });
                }
                Err(reason) => rejected.push(format!(
                    "companion entity={} archetype=0x{:02X} visual_index={} palette_offset=0x{:X} reason={}",
                    m.entity.index,
                    m.entity.archetype,
                    m.entity.visual_index,
                    m.entity.palette_offset,
                    reason
                )),
            }
        }
        rejected
    }
}
