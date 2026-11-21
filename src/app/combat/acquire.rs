//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).
//!
//! This file: the persistent engage order's re-acquisition pass
//! (`FUN_0041E2D4`, `FUN_0041FB64`, `FUN_00420190`, `FUN_00420098`,
//! `FUN_00410E84`).
//!
//! The original melee order (`Entity+0x02 & 0x01`, written by `FUN_00421458` and
//! the right-click attack branch of `FUN_0042260C`) is **persistent**. Every idle
//! tick the party dispatcher `FUN_00421B50` runs `FUN_0041ECC8`, which calls
//! `FUN_0041E2D4` while the order bit is set:
//!
//! * a still-valid current target is kept (chase/attack);
//! * a dead/invalid target is cleared and the unit re-acquires through
//!   `FUN_0041FB64(own group)` (nearest hostile group around the party leader),
//!   then `FUN_00420190` (nearest member no engaged party unit targets) with the
//!   `FUN_00420098` plain-nearest fallback;
//! * no candidate clears the order bit (`+0x02 &= 0xFE`).
//!
//! That is why a companion ordered onto a mob group keeps killing the next
//! aggressive mob after each kill instead of stopping after one.
//!
//! `FUN_0041FB64` selectivity (`class` = `Group+0x1E`, `activity` =
//! `Group+0x1D`): same location, not the party group, class != `0x53`, and
//! either an object/monster group (`class & 0x80 == 0`) or a citizen group
//! woken to activity `1`. The search area is the party's rectangle A or - for
//! the party group (`class & 0x20`) - the leader +/- `(r + r/2)` with
//! `r = 20/9` at 800x600 (`_DAT_0081D6E0`/`_DAT_0081D6A4`, 17/7 at 640x480).
//!
//! Archer orders (`0x44`, target/approach bit `0x04`) re-pick through
//! `FUN_00410E84` instead: the first visible member of the nearest hostile group
//! whose `FUN_0041130C` line-of-fire probe is clear.

use super::super::*;
use crate::combat::CombatTarget;

/// `_DAT_0081D6E0`: hostile-group search radius in x around the leader at
/// 800x600 (`17` at 640x480).
const HOSTILE_GROUP_RADIUS_X: i32 = 20;
/// `_DAT_0081D6A4`: hostile-group search radius in y around the leader at
/// 800x600 (`7` at 640x480).
const HOSTILE_GROUP_RADIUS_Y: i32 = 9;

/// `FUN_0041FB64` search area around the party leader, as inclusive cell bounds.
///
/// The original expands the radius by half of itself again
/// (`_DAT_0081D6E0 / 2`), so 800x600 gives `x +/- 30`, `y +/- 13`.
fn hostile_group_area(leader: (i32, i32)) -> (i32, i32, i32, i32) {
    let rx = HOSTILE_GROUP_RADIUS_X;
    let ry = HOSTILE_GROUP_RADIUS_Y;
    (
        leader.0 - rx - rx / 2,
        leader.1 - ry - ry / 2,
        leader.0 + rx + rx / 2,
        leader.1 + ry + ry / 2,
    )
}

impl PlayableApp {
    /// `FUN_0041E2D4` + `FUN_0041FB64` + `FUN_00420190`/`FUN_00420098`: pick the
    /// next hostile for an ordered unit that has no valid target.
    ///
    /// `unit_cell` is the acquiring unit's live runtime cell; melee candidates
    /// are ranked by the game distance `FUN_0043881C`. With `ranged` set the
    /// original takes the `FUN_00410E84` line-of-fire pick instead (first
    /// visible member of the nearest hostile group the probe reaches).
    ///
    /// # Returns
    ///
    /// `None` when the party leader has no hostile group in range, which is the
    /// original's `+0x02 &= 0xFE` branch (the engage order ends).
    pub(in crate::app) fn acquire_next_hostile(
        &self,
        unit: CombatTarget,
        unit_cell: (i32, i32),
        ranged: bool,
    ) -> Option<CombatTarget> {
        let leader = (self.player_x.round() as i32, self.player_y.round() as i32);
        let group = self.nearest_hostile_group(leader)?;
        let id = match (ranged, self.probe_range(unit)) {
            (true, Some(range)) => self.first_group_los_enemy(group, unit_cell, range),
            _ => self.nearest_group_enemy(group, unit_cell),
        }?;
        Some(CombatTarget::World(id))
    }

    /// `FUN_0041FB64(own group)`: nearest hostile group around the party leader.
    ///
    /// A group qualifies when it is not the party group, is on the current
    /// location, is not class `0x53`, and is either an object/monster group
    /// (`class & 0x80 == 0`) or a citizen group woken to activity `1`. The group
    /// distance is the `FUN_0043881C` distance from the leader to its nearest
    /// visible living member inside the search area.
    fn nearest_hostile_group(&self, leader: (i32, i32)) -> Option<usize> {
        let loc = self.current_map;
        let (x1, y1, x2, y2) = hostile_group_area(leader);
        let tables = self.tables.as_ref()?;
        let pop = self.population_cache.get(&loc)?;
        let mut best: Option<(i32, usize)> = None;
        for g in tables.groups.iter() {
            if g.index == 0 || g.loc_id as u32 != loc {
                continue;
            }
            if g.class_1e == 0x53 {
                continue;
            }
            // `class & 0x80` = citizen group: hostile only once woken (`0x1D == 1`).
            if g.class_1e & 0x80 != 0 && g.activity_1d != 1 {
                continue;
            }
            for a in pop.actors.iter().filter(|a| a.source_group_id == g.index) {
                if a.entity.archetype & 0x80 != 0 {
                    continue;
                }
                let Some(m) = pop.motion.get(&a.entity.index) else {
                    continue;
                };
                if m.dead || m.cell.0 < x1 || m.cell.0 > x2 || m.cell.1 < y1 || m.cell.1 > y2 {
                    continue;
                }
                let d = crate::actors::follow_distance(leader, m.cell);
                if best.map_or(true, |(best_d, _)| d < best_d) {
                    best = Some((d, g.index));
                }
            }
        }
        best.map(|(_, index)| index)
    }

    /// `FUN_00420190` then `FUN_00420098`: nearest visible living member of
    /// `group` that no **engaged** party unit already targets, falling back to
    /// the plain nearest member when every one is engaged.
    fn nearest_group_enemy(&self, group: usize, unit_cell: (i32, i32)) -> Option<usize> {
        let pop = self.population_cache.get(&self.current_map)?;
        let mut taken: HashSet<usize> = HashSet::new();
        if self.player_engaged {
            if let Some(CombatTarget::World(id)) = self.player_attack_target {
                taken.insert(id);
            }
        }
        for m in self.party.iter() {
            if !m.motion.engaged {
                continue;
            }
            if let Some(CombatTarget::World(id)) = m.motion.combat_target {
                taken.insert(id);
            }
        }
        let mut best_unengaged: Option<(i32, usize)> = None;
        let mut best_any: Option<(i32, usize)> = None;
        for a in pop.actors.iter().filter(|a| a.source_group_id == group) {
            if a.entity.archetype & 0x80 != 0 {
                continue;
            }
            let Some(m) = pop.motion.get(&a.entity.index) else {
                continue;
            };
            if m.dead {
                continue;
            }
            let d = crate::actors::follow_distance(unit_cell, m.cell);
            if best_any.map_or(true, |(best_d, _)| d < best_d) {
                best_any = Some((d, a.entity.index));
            }
            if !taken.contains(&a.entity.index)
                && best_unengaged.map_or(true, |(best_d, _)| d < best_d)
            {
                best_unengaged = Some((d, a.entity.index));
            }
        }
        best_unengaged.or(best_any).map(|(_, id)| id)
    }

    /// `FUN_00410E84`: first visible living member of `group` that the
    /// `FUN_0041130C` line-of-fire probe reaches (authored entity order, not
    /// nearest; the original walks the group's member list in place).
    fn first_group_los_enemy(
        &self,
        group: usize,
        unit_cell: (i32, i32),
        range: u16,
    ) -> Option<usize> {
        let loc = self.current_map;
        let collision = self.world.collision(loc)?;
        let pop = self.population_cache.get(&loc)?;
        for a in pop.actors.iter().filter(|a| a.source_group_id == group) {
            if a.entity.archetype & 0x80 != 0 {
                continue;
            }
            let Some(m) = pop.motion.get(&a.entity.index) else {
                continue;
            };
            if m.dead {
                continue;
            }
            if crate::combat::los::probe(collision, unit_cell, m.cell, range).is_clear() {
                return Some(a.entity.index);
            }
        }
        None
    }

    /// `FUN_00410E84` gate: active weapon is the bow (`Entity+0x12 == 1`), a bow
    /// is equipped (`equipment[1]`, `Entity+0x68`) and ammo is loaded
    /// (`Entity+0xCC`). Returns the line-of-fire sample count of the **bow's**
    /// subtype (`u16@0x459B84 + subtype*0x40`, read by `FUN_0041130C`).
    fn probe_range(&self, unit: CombatTarget) -> Option<u16> {
        let (slot, equipment, ammo) = match unit {
            CombatTarget::Player => (
                self.player.raw_0x12,
                self.player.equipment,
                self.player.special_equipment,
            ),
            CombatTarget::Party(i) => {
                let m = self.party.get(i)?;
                (m.entity.raw_0x12, m.entity.equipment, m.entity.special_equipment)
            }
            CombatTarget::World(id) => {
                let e = self.combatant_entity(id)?;
                (e.raw_0x12, e.equipment, e.special_equipment)
            }
            CombatTarget::Prop(_) => return None,
        };
        let bow = super::armed_bow(slot, &equipment, ammo)?;
        let subtype = self.world.res.item_class_subtype(bow)?.1;
        self.world
            .res
            .item_db
            .as_ref()
            .map(|db| db.probe_range(subtype))
    }
}

/// `FUN_0041130C` between two runtime cells for an armed archer.
///
/// `None` when the unit fails the `FUN_00410E84` gate (`Entity+0x12 == 1`, a bow
/// in `equipment[1]`, ammo in `+0xCC`) or the resource tables cannot resolve the
/// bow subtype. The sample count is the bow's
/// `u16@0x459B84 + subtype*0x40`.
pub(in crate::app::combat) fn armed_los(
    res: &crate::res::Res,
    collision: &[u16],
    slot: u8,
    equipment: &[u16; 6],
    ammo: u16,
    from: (i32, i32),
    to: (i32, i32),
) -> Option<crate::combat::los::LineOfFire> {
    let bow = super::armed_bow(slot, equipment, ammo)?;
    let subtype = res.item_class_subtype(bow)?.1;
    let range = res.item_db.as_ref()?.probe_range(subtype);
    Some(crate::combat::los::probe(collision, from, to, range))
}
