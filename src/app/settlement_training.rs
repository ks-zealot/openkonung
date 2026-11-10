//! Barracks/voivode training (`FUN_00417F84`).
//!
//! `FUN_0041D048` runs the settlement branch once per 16 original frames
//! (`(frame + 7) & 0xF == 0`) for **all 20 settlements**. After the role-2/role-4
//! producer passes, `FUN_00417F84` trains the settlement's low-visual villagers
//! against the voivode — `Settlement+0x3F2`, which is `waypoint[4].owner`
//! (quest predicate `0x130`, installed by dialogue opcode `0x159`).
//!
//! Each pass decrements `Settlement+0x0C` (the unused role-3 timer,
//! [`crate::population::SettlementTrade::timer3`]); when it fires it resets to `0x2D0`
//! (≈11.5 real minutes, first pass immediate because the field starts at 0).
//!
//! On a fired pass every visible group member with `visual < 3` (excluding the
//! voivode) gets:
//!
//! 1. `+0x33` level += 1 when `FUN_00410B08(ent, 0)` allows it;
//! 2. each of dex/cha/str/end below the voivode raised by one through the
//!    `0x45AFB0` cost table, paid from a temporary `+1000` XP grant;
//! 3. level += 1 again;
//! 4. melee/archery mastery moved toward the voivode:
//!    `own += (master - own) / deficit + 1`;
//! 5. `FUN_0041C69C` recalculation (stored stat → derived stat).
//!
//! The stored stats are `+0x56/+0x5A/+0x60/+0x64` (`EntityStats::*_current` in
//! this codebase); `+0x54/+0x58/+0x5E/+0x62` (`*_base`) are the derived values.

use super::*;
use crate::character::{can_raise_any_index, growth_index_raw, raise_cost_index, PrimaryStat};
use crate::population::{Entity, EntityStats};

/// `Settlement+0x0C` reset value (`FUN_00417F84`).
pub(super) const TRAINING_PERIOD: i32 = 0x2D0;

/// Temporary XP grant used to pay the stat cost (`FUN_00417F84`).
const TRAINING_XP: u32 = 1000;

/// The stat fields the training pass may change. Only these are mirrored into
/// the cached [`PopulationBuild`] actors so combat HP and temporary effects are
/// never overwritten by the authored table.
#[derive(Clone, Copy)]
struct Trained {
    level: u8,
    available_xp: u32,
    invested_xp: u32,
    dexterity_base: u16,
    dexterity_current: u16,
    charisma_base: u16,
    charisma_current: u16,
    strength_base: u16,
    strength_current: u16,
    endurance_base: u16,
    endurance_current: u16,
    melee_mastery: u16,
    archery_mastery: u16,
}

impl Trained {
    /// Snapshot the stat fields the training pass may change.
    ///
    /// Only these are mirrored back into the cached population actor, so combat
    /// health, poison and temporary-effect fields are never overwritten by the
    /// authored table.
    fn capture(s: &EntityStats) -> Trained {
        Trained {
            level: s.level,
            available_xp: s.available_xp,
            invested_xp: s.invested_xp,
            dexterity_base: s.dexterity_base,
            dexterity_current: s.dexterity_current,
            charisma_base: s.charisma_base,
            charisma_current: s.charisma_current,
            strength_base: s.strength_base,
            strength_current: s.strength_current,
            endurance_base: s.endurance_base,
            endurance_current: s.endurance_current,
            melee_mastery: s.melee_mastery,
            archery_mastery: s.archery_mastery,
        }
    }

    /// Copy the trained fields into a cached actor. `preserve_stats` keeps the
    /// live primary-stat pair untouched when the actor has an active temporary
    /// effect (a potion writes the cached `*_current`; the authored table must
    /// not clobber it).
    fn apply(&self, s: &mut EntityStats, preserve_stats: bool) {
        s.level = self.level;
        s.available_xp = self.available_xp;
        s.invested_xp = self.invested_xp;
        if !preserve_stats {
            s.dexterity_base = self.dexterity_base;
            s.dexterity_current = self.dexterity_current;
            s.charisma_base = self.charisma_base;
            s.charisma_current = self.charisma_current;
            s.strength_base = self.strength_base;
            s.strength_current = self.strength_current;
            s.endurance_base = self.endurance_base;
            s.endurance_current = self.endurance_current;
        }
        s.melee_mastery = self.melee_mastery;
        s.archery_mastery = self.archery_mastery;
    }
}

/// Stored value of one primary stat (`+0x56/+0x5A/+0x60/+0x64`).
///
/// The original keeps a stored ("current") and a derived ("base") pair; the
/// `FUN_0041C69C` recalculation copies stored into derived while no temporary
/// effect is active.
fn stored(stat: PrimaryStat, s: &EntityStats) -> u16 {
    match stat {
        PrimaryStat::Dexterity => s.dexterity_current,
        PrimaryStat::Charisma => s.charisma_current,
        PrimaryStat::Strength => s.strength_current,
        PrimaryStat::Endurance => s.endurance_current,
    }
}

/// Write the stored value of one primary stat (see [`stored`]).
fn set_stored(stat: PrimaryStat, s: &mut EntityStats, value: u16) {
    match stat {
        PrimaryStat::Dexterity => s.dexterity_current = value,
        PrimaryStat::Charisma => s.charisma_current = value,
        PrimaryStat::Strength => s.strength_current = value,
        PrimaryStat::Endurance => s.endurance_current = value,
    }
}

/// `own += (master - own) / deficit + 1` with C-style truncating division.
fn add_share(own: u16, master: u16, deficit: i32) -> u16 {
    let delta = (master as i32 - own as i32) / deficit + 1;
    (own as i32 + delta).clamp(0, u16::MAX as i32) as u16
}

/// Apply one training tick to `ent` (`FUN_00417F84` inner body).
fn train_member(ent: &mut Entity, master: &EntityStats) {
    let Some(index) = growth_index_raw(ent.archetype) else {
        return;
    };
    // `FUN_00410B08` refuses every raise while a temporary effect is active.
    let blocked = ent.stats.temp_effect_timer != 0;

    if !blocked
        && can_raise_any_index(
            index,
            ent.stats.level,
            ent.stats.dexterity_current,
            ent.stats.charisma_current,
            ent.stats.strength_current,
            ent.stats.endurance_current,
        )
    {
        ent.stats.level = ent.stats.level.saturating_add(1);
    }

    let mut deficit: i32 = 0;
    for stat in [
        PrimaryStat::Dexterity,
        PrimaryStat::Charisma,
        PrimaryStat::Strength,
        PrimaryStat::Endurance,
    ] {
        let own = stored(stat, &ent.stats);
        let master_value = stored(stat, master);
        if own >= master_value {
            continue;
        }
        deficit += (master_value - own) as i32;
        if blocked {
            continue;
        }
        // The original grants 1000 XP, pays the cost, then removes the grant:
        // the stat raise is effectively free but `invested_xp` records it.
        let available = ent.stats.available_xp.saturating_add(TRAINING_XP);
        if let Some(cost) = raise_cost_index(index, stat, ent.stats.level, own, available) {
            ent.stats.invested_xp = ent.stats.invested_xp.saturating_add(cost);
            set_stored(stat, &mut ent.stats, own.saturating_add(1));
        }
    }

    if !blocked
        && can_raise_any_index(
            index,
            ent.stats.level,
            ent.stats.dexterity_current,
            ent.stats.charisma_current,
            ent.stats.strength_current,
            ent.stats.endurance_current,
        )
    {
        ent.stats.level = ent.stats.level.saturating_add(1);
    }

    if deficit != 0 {
        ent.stats.melee_mastery = add_share(ent.stats.melee_mastery, master.melee_mastery, deficit);
        ent.stats.archery_mastery =
            add_share(ent.stats.archery_mastery, master.archery_mastery, deficit);
        // `FUN_0041C69C` no-effect path: derived = stored.
        if ent.stats.temp_effect_timer == 0 {
            ent.stats.dexterity_base = ent.stats.dexterity_current;
            ent.stats.charisma_base = ent.stats.charisma_current;
            ent.stats.strength_base = ent.stats.strength_current;
            ent.stats.endurance_base = ent.stats.endurance_current;
        }
    }
}

impl PlayableApp {
    /// Advance the voivode training pass by `passes` settlement ticks (one per
    /// 16 original frames). Runs for every settlement, not only the current
    /// location (`FUN_0041D048`). Settlements whose target pass set `local_30`
    /// are skipped entirely (`blocked`), including the timer decrement.
    pub(super) fn tick_settlement_training(&mut self, passes: u32, blocked: &HashSet<usize>) {
        if passes == 0 || self.tables.is_none() {
            return;
        }
        let jobs: Vec<(usize, usize, usize, usize, usize)> = self
            .settlements
            .iter()
            .enumerate()
            .filter_map(|(si, s)| {
                if blocked.contains(&si) {
                    return None;
                }
                let voivode = s.voivode() as usize;
                if voivode == 0 {
                    return None;
                }
                let ti = self.settlement_trade.iter().position(|t| t.index == si)?;
                let group = self.tables.as_ref()?.groups.get(s.group_id as usize)?;
                Some((
                    si,
                    ti,
                    voivode,
                    group.first_entity as usize,
                    group.entity_count as usize,
                ))
            })
            .collect();
        if jobs.is_empty() {
            return;
        }
        let party_ids: HashSet<usize> = self.party.iter().map(|m| m.entity.index).collect();
        for (si, ti, voivode, first, count) in jobs {
            let mut fires = 0u32;
            {
                let t = &mut self.settlement_trade[ti];
                for _ in 0..passes {
                    t.timer3 -= 1;
                    if t.timer3 < 1 {
                        t.timer3 = TRAINING_PERIOD;
                        fires += 1;
                    }
                }
            }
            for _ in 0..fires {
                self.train_settlement(si, voivode, first, count, &party_ids);
            }
        }
    }

    /// Run one fired training pass for one settlement.
    ///
    /// The voivode (`waypoint[4].owner`) is the stat and mastery ceiling. Members
    /// are filtered like `FUN_00417F84`: visible, not in a death row, `visual < 3`,
    /// not the voivode and not a recruited party member. Each changed stat set is
    /// then mirrored into that location's cached actor.
    ///
    /// `first`/`count` are the settlement group's authored entity window.
    fn train_settlement(
        &mut self,
        si: usize,
        voivode: usize,
        first: usize,
        count: usize,
        party_ids: &HashSet<usize>,
    ) {
        let Some(master) = self
            .tables
            .as_ref()
            .and_then(|t| t.entities.get(voivode))
            .map(|e| e.stats)
        else {
            return;
        };
        let loc = self.settlements.get(si).map(|s| s.loc_id).unwrap_or(0) as u32;
        // The live action row (`Entity+0x00` death rows 3/9) is in the location
        // cache; the authored tables only carry the initial value.
        let runtime: HashMap<usize, (u8, bool)> = self
            .population_cache
            .get(&loc)
            .map(|p| {
                p.motion
                    .iter()
                    .map(|(&id, m)| (id, (m.action, m.dead)))
                    .collect()
            })
            .unwrap_or_default();
        let mut changed: Vec<(usize, Trained)> = Vec::new();
        if let Some(tables) = self.tables.as_mut() {
            for id in first..first.saturating_add(count) {
                if id == voivode || party_ids.contains(&id) {
                    continue;
                }
                let Some(ent) = tables.entities.get_mut(id) else {
                    continue;
                };
                let (action, dead) = runtime
                    .get(&id)
                    .copied()
                    .unwrap_or((ent.kind, matches!(ent.kind, 3 | 9)));
                // `FUN_00417F84` filters: visible, not in a death row, and one
                // of the three low visual indices.
                if dead
                    || matches!(action, 3 | 9)
                    || ent.archetype & 0x80 != 0
                    || ent.visual_index >= 3
                {
                    continue;
                }
                let before = Trained::capture(&ent.stats);
                train_member(ent, &master);
                let after = Trained::capture(&ent.stats);
                if before.level != after.level
                    || before.invested_xp != after.invested_xp
                    || before.dexterity_current != after.dexterity_current
                    || before.charisma_current != after.charisma_current
                    || before.strength_current != after.strength_current
                    || before.endurance_current != after.endurance_current
                    || before.melee_mastery != after.melee_mastery
                    || before.archery_mastery != after.archery_mastery
                {
                    changed.push((id, after));
                }
            }
        }
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            for (id, trained) in changed {
                if let Some(actor) = pop.actors.iter_mut().find(|a| a.source_entity_id() == id) {
                    let preserve = actor.entity.stats.temp_effect_timer != 0;
                    trained.apply(&mut actor.entity.stats, preserve);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an `EntityStats` fixture with the four primary stats and health set.
    fn stats(level: u8, dex: u16, cha: u16, str_: u16, end: u16) -> EntityStats {
        EntityStats {
            level,
            dexterity_current: dex,
            charisma_current: cha,
            strength_current: str_,
            endurance_current: end,
            dexterity_base: dex,
            charisma_base: cha,
            strength_base: str_,
            endurance_base: end,
            hp_internal: 1600,
            ..Default::default()
        }
    }

    /// Build a low-visual `Entity` fixture for the training tests.
    fn entity(archetype: u8, stats: EntityStats) -> Entity {
        Entity {
            index: 0,
            kind: 6,
            facing: 0,
            flags_a: 0x40,
            flags_b: 0,
            archetype,
            group_id: 1,
            raw_0x12: 0,
            x: 0,
            y: 0,
            stats,
            equipment: [0; 6],
            inventory: [0; 32],
            additional_equipment: [0; 5],
            special_equipment: 0,
            dialogue_id: 0xFF,
            state_d2: 0,
            name_id: 0,
            nickname_id: 0,
            visual_index: 1,
            palette_offset: 0,
        }
    }

    /// Training levels the villager up before and after the stat pass and moves
    /// both masteries toward the voivode.
    #[test]
    fn training_raises_stats_toward_the_voivode_and_levels_up() {
        // `archetype & 7 == 5` -> raw class index 4 (Warrior record).
        // Level-1 minima (dex 5, cha 5, str 16, end 5) are met, so the first
        // level-up succeeds; the level-2 caps then allow all four raises.
        let mut ent = entity(0x5, stats(1, 5, 5, 16, 5));
        ent.stats.melee_mastery = 100;
        ent.stats.archery_mastery = 100;
        let mut master = stats(6, 35, 35, 35, 35);
        master.melee_mastery = 1500;
        master.archery_mastery = 1500;
        train_member(&mut ent, &master);
        assert_eq!(ent.stats.level, 2, "level-up before the stat pass");
        assert_eq!(ent.stats.dexterity_current, 6, "level-2 cap is 10");
        assert_eq!(ent.stats.charisma_current, 6, "level-2 cap is 10");
        assert_eq!(ent.stats.strength_current, 17, "level-2 cap is 26");
        assert_eq!(ent.stats.endurance_current, 6, "level-2 cap is 10");
        assert_eq!(ent.stats.strength_base, 17, "derived stat recalculated");
        assert!(ent.stats.invested_xp > 0, "stat cost recorded");
        assert!(
            ent.stats.melee_mastery > 100,
            "mastery moves toward the voivode"
        );
        assert_eq!(ent.stats.melee_mastery, ent.stats.archery_mastery);
    }

    /// A member already at or above the voivode gains neither stats nor mastery.
    #[test]
    fn training_does_not_exceed_the_voivode_stats() {
        let mut ent = entity(0x5, stats(3, 40, 40, 40, 40));
        let master = stats(6, 35, 35, 35, 35);
        train_member(&mut ent, &master);
        assert_eq!(ent.stats.dexterity_current, 40);
        assert_eq!(ent.stats.melee_mastery, 0, "no deficit, no mastery gain");
    }

    /// An active temporary effect blocks the level-up and every stat raise.
    #[test]
    fn training_is_blocked_by_a_temporary_effect() {
        let mut ent = entity(0x5, stats(1, 10, 10, 10, 10));
        ent.stats.temp_effect_timer = 5;
        let master = stats(6, 35, 35, 35, 35);
        train_member(&mut ent, &master);
        assert_eq!(ent.stats.level, 1, "no level-up under a temp effect");
        assert_eq!(ent.stats.dexterity_current, 10, "no stat raise");
    }
}
