//! Party follow formation and commands.
//!
//! Recruited companions belong to the original reserved group 0, so the
//! per-frame dispatcher `FUN_00421B50` routes them through the inline AI
//! `FUN_0041ECC8` -> `FUN_0041F3C8`: one cell per walk cycle toward a slot of
//! the authored formation table, only when the original `FUN_0043881C` distance
//! reaches [`FOLLOW_DISTANCE`]. The slot is chosen with `rand() % 12` out of
//! [`FOLLOW_SLOTS`] (see [`follow_goal`]), so the follower also consumes the
//! shared RNG exactly like the original. An explicit world click overrides this
//! by commanding the same destination to every selected unit.

use super::super::*;

use crate::combat::OriginalRng;
use crate::constants::motion::*;
use crate::coords::direction_from_cells;

/// `FUN_0043881c`: grid distance between two runtime cells (Chebyshev, with a
/// `+1` when the smaller axis exceeds `1`).
pub fn follow_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    if dx < dy {
        if dx > 1 {
            dy + 1
        } else {
            dy
        }
    } else if dy > 1 {
        dx + 1
    } else {
        dx
    }
}

/// A free cell in the original follow formation (`FUN_0041f3c8`).
///
/// `FUN_0043897C(member, leader)` gives the direction *from the leader toward
/// the member*; the table is indexed by that direction and by the leader's
/// x-parity. The original draws `rand() % 12` once per attempt to pick the
/// starting slot, walks all twelve slots (wrapping) and tries the facing
/// direction plus the previous one (`dir-1`) for each, repeating four times.
/// It accepts `0 < x < 0x13F`, `0 < y < 0x9F` and `collision & 0xFFF == 0`,
/// i.e. a free cell 1–7 cells from the leader.
pub(crate) fn follow_goal(
    leader: (i32, i32),
    member: (i32, i32),
    collision: &[u16],
    occupied: &HashSet<(i32, i32)>,
    rng: &mut OriginalRng,
) -> Option<(i32, i32)> {
    let dir = direction_from_cells(member.0, member.1, leader.0, leader.1) & 7;
    let alt = (dir + 7) & 7;
    let parity = (leader.0 & 1) as usize;
    for _attempt in 0..4 {
        let start = (rng.roll() % 12) as usize;
        for i in 0..12 {
            let slot = (start + i) % 12;
            for d in [dir as usize, alt as usize] {
                let (dx, dy) = FOLLOW_SLOTS[parity][d][slot];
                let cell = (leader.0 + dx as i32, leader.1 + dy as i32);
                if cell.0 > 0
                    && cell.0 < 0x13F
                    && cell.1 > 0
                    && cell.1 < 0x9F
                    && is_free(collision, cell.0, cell.1)
                    && !occupied.contains(&cell)
                {
                    return Some(cell);
                }
            }
        }
    }
    None
}

/// Advance recruited companions toward the leader, one collision cell per
/// completed walk cycle (original party **group 0**, class `0x20` -> the
/// `FUN_00421b50` inline AI -> `FUN_0041ecc8` -> `FUN_0041f3c8`). A companion
/// re-targets a free cell beside the leader only when it is at least
/// [`FOLLOW_DISTANCE`] cells away; it then walks there and holds position.
pub fn update_followers(
    members: &mut [PartyMember],
    collision: &mut [u16],
    leader: (i32, i32),
    ticks: i64,
    rng: &mut OriginalRng,
) {
    if ticks <= 0 || members.is_empty() {
        return;
    }
    let ticks = ticks.min(MAX_MOTION_TICKS_PER_FRAME);
    let mut occupied: HashSet<(i32, i32)> = members
        .iter()
        .filter(|m| !m.motion.dead)
        .map(|m| m.motion.cell)
        .collect();
    occupied.insert(leader);
    for _ in 0..ticks {
        for m in members.iter_mut() {
            // `FUN_0041EFD8`: follow is the *last* option of the idle branch -
            // a unit with a live target or a held engage order (armed pick or
            // retaliation already handled it) never follows.
            if m.motion.dead || m.motion.combat_target.is_some() || m.motion.engaged {
                continue;
            }
            let battle = crate::anim::battle_from_flags(m.entity.flags_b);
            if !m.motion.walking {
                if follow_distance(m.motion.cell, leader) >= FOLLOW_DISTANCE {
                    if let Some(goal) = follow_goal(
                        leader,
                        m.motion.cell,
                        collision,
                        &occupied,
                        rng,
                    ) {
                        if goal != m.motion.cell {
                            let bounds = (0, 0, RUNTIME_W - 1, RUNTIME_H - 1);
                            if let Some(p) =
                                find_path(collision, &occupied, m.motion.cell, goal, bounds)
                            {
                                if !p.is_empty() {
                                    m.motion.path = p;
                                    m.motion.target = Some(goal);
                                }
                            }
                        }
                    }
                } else {
                    m.motion.path.clear();
                    m.motion.target = None;
                }
                if !m.motion.path.is_empty() {
                    begin_actor_segment(&mut m.motion, m.entity.index, &mut occupied, collision);
                }
            }
            let next_action = if m.motion.walking {
                crate::anim::walk_action(battle) as u8
            } else {
                crate::anim::idle_action(battle) as u8
            };
            if m.motion.action != next_action {
                m.motion.action = next_action;
                m.motion.frame = 0;
            }
            if m.motion.walking {
                let wf = if m.motion.walk_frames == 0 {
                    FALLBACK_WALK_FRAMES
                } else {
                    m.motion.walk_frames
                };
                m.motion.frame = m.motion.frame.saturating_add(1);
                if m.motion.frame >= wf {
                    m.motion.frame = 0;
                    if !m.motion.path.is_empty() {
                        begin_actor_segment(
                            &mut m.motion,
                            m.entity.index,
                            &mut occupied,
                            collision,
                        );
                    } else {
                        m.motion.walking = false;
                        m.motion.render_cell = m.motion.cell;
                        m.motion.target = None;
                    }
                }
            } else {
                m.motion.frame = 0;
            }
        }
    }
}

/// Issue a move command to one companion toward `goal`, mirroring the original
/// left-click command 0 (`FUN_0042260C`), which writes the clicked cell to
/// `Entity+0x0C/+0x0E` on **every** unit in the selection array `0x81A814`.
///
/// The destination is stored even when the route fails (`FUN_004142D4` records
/// the target before pathfinding and simply does not move); on success the first
/// step is committed immediately (`FUN_0041435C`) so the companion starts
/// walking and the straggler AI does not clear the commanded route.
pub fn command_follower(
    member: &mut PartyMember,
    collision: &mut [u16],
    occupied: &mut HashSet<(i32, i32)>,
    goal: (i32, i32),
) {
    member.motion.target = Some(goal);
    if goal == member.motion.cell {
        member.motion.path.clear();
        member.motion.walking = false;
        member.motion.render_cell = member.motion.cell;
        return;
    }
    let bounds = (0, 0, RUNTIME_W - 1, RUNTIME_H - 1);
    match find_path(collision, occupied, member.motion.cell, goal, bounds) {
        Some(p) if !p.is_empty() => {
            member.motion.path = p;
            begin_actor_segment(&mut member.motion, member.entity.index, occupied, collision);
        }
        _ => {
            // Destination recorded but unreachable/blocked: no walk (original
            // keeps the stored target and does not start moving).
            member.motion.path.clear();
            member.motion.walking = false;
            member.motion.render_cell = member.motion.cell;
        }
    }
}
