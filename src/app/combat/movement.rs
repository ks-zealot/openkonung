//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

use std::collections::HashSet;

use super::super::*;
use crate::actors::ActorMotion;

impl PlayableApp {
    pub(super) fn advance_motion_walk(
        m: &mut ActorMotion,
        collision: &[u16],
        occupied: &mut HashSet<(i32, i32)>,
    ) -> bool {
        if !m.walking {
            return false;
        }
        let wf = if m.walk_frames == 0 {
            crate::actors::FALLBACK_WALK_FRAMES
        } else {
            m.walk_frames
        };
        m.frame = m.frame.saturating_add(1);
        if m.frame >= wf {
            m.frame = 0;
            if !m.path.is_empty() {
                crate::actors::begin_actor_segment(m, occupied, collision);
            } else {
                m.walking = false;
                m.render_cell = m.cell;
                m.target = None;
            }
        }
        m.walking
    }

    pub(crate) fn start_motion_walk(
        m: &mut ActorMotion,
        collision: &[u16],
        occupied: &mut HashSet<(i32, i32)>,
        goal: (i32, i32),
    ) {
        m.target = Some(goal);
        let bounds = (
            0,
            0,
            crate::coords::RUNTIME_W - 1,
            crate::coords::RUNTIME_H - 1,
        );
        match crate::actors::find_path(collision, occupied, m.cell, goal, bounds) {
            Some(p) if !p.is_empty() => {
                m.path = p;
                crate::actors::begin_actor_segment(m, occupied, collision);
            }
            _ => {
                m.path.clear();
                m.walking = false;
                m.render_cell = m.cell;
            }
        }
    }

    /// Advance one walk cycle but stop at the cycle boundary instead of starting
    /// the next queue segment. Used by the M/O AI delay (`FUN_004123BC`).
    pub(super) fn advance_motion_walk_cycle(
        m: &mut ActorMotion,
        _collision: &[u16],
        _occupied: &mut HashSet<(i32, i32)>,
    ) -> bool {
        if !m.walking {
            return false;
        }
        let wf = if m.walk_frames == 0 {
            crate::actors::FALLBACK_WALK_FRAMES
        } else {
            m.walk_frames
        };
        m.frame = m.frame.saturating_add(1);
        if m.frame >= wf {
            m.frame = 0;
            m.walking = false;
            m.render_cell = m.cell;
            if m.path.is_empty() {
                m.target = None;
            }
        }
        m.walking
    }
}

