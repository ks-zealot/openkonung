//! `build` subsystem of [`crate::character`] (split from `character.rs`).
//!
//! Holds the New-Hero screen's final state: the [`Skills`] record (`Entity+0x40`
//! ..=`+0x4C`), the [`CharacterBuild`] copied once into the runtime player, and
//! the two specialization selectors that produce those skill values. The
//! stat-stepper cost tiers live in [`super::class`].

use super::*;

/// Original initial skill values applied when the character-creation screen is
/// initialized/reset. If the UI later allows editing, the final UI values
/// override these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skills {
    /// `Entity+0x40`
    pub melee_mastery: u16,
    /// `Entity+0x42`
    pub trade: u16,
    /// `Entity+0x44`
    pub archery_mastery: u16,
    /// `Entity+0x46`
    pub building: u16,
    /// `Entity+0x48`
    pub healing: u16,
    /// `Entity+0x4A`
    pub identification: u16,
    /// `Entity+0x4C`
    pub smithing: u16,
}

/// The original initial skill values (equal to Fencing + Identification).
pub const INITIAL_SKILLS: Skills = Skills {
    melee_mastery: 100,
    trade: 10,
    archery_mastery: 0,
    building: 1,
    healing: 50,
    identification: 20,
    smithing: 10,
};

/// Final state of the character-creation screen: the class plus the configured
/// skills and primary stats. This is what Start Game copies into the runtime
/// player — the class preset is **not** regenerated at start.
///
/// The seven numeric skill fields are the authoritative runtime values; the two
/// specialization enums describe how the initial values were chosen and are
/// never recomputed into the player after start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterBuild {
    /// Selected class; becomes `Entity+0x04` at game start.
    pub class: CharacterClass,
    /// `Entity+0x38` — free parameter points (spent/refunded by the stat
    /// steppers). Presets initialize this to 0.
    pub available_xp: u32,
    /// Which combat specialization produced the four combat skills.
    pub combat_specialization: CombatSpecialization,
    /// Which civil specialization produced the three civil skills.
    pub civil_specialization: CivilSpecialization,
    /// `Entity+0x40` melee mastery.
    pub melee_mastery: u16,
    /// `Entity+0x42` trade skill.
    pub trade: u16,
    /// `Entity+0x44` archery mastery.
    pub archery_mastery: u16,
    /// `Entity+0x46` building skill.
    pub building: u16,
    /// `Entity+0x48` healing skill.
    pub healing: u16,
    /// `Entity+0x4A` identification skill.
    pub identification: u16,
    /// `Entity+0x4C` smithing skill.
    pub smithing: u16,
    /// `Entity+0x54` dexterity base.
    pub dexterity: u16,
    /// `Entity+0x58` charisma base.
    pub charisma: u16,
    /// `Entity+0x5E` strength base.
    pub strength: u16,
    /// `Entity+0x62` endurance base.
    pub endurance: u16,
}

impl CharacterBuild {
    /// A fresh build for a class: preset stats + the default secondary-skill
    /// specializations (Fencing + Identification == [`INITIAL_SKILLS`]).
    pub fn from_class(class: CharacterClass) -> CharacterBuild {
        let p = class.preset();
        let mut b = CharacterBuild {
            class,
            available_xp: 0,
            combat_specialization: CombatSpecialization::Fencing,
            civil_specialization: CivilSpecialization::Identification,
            melee_mastery: 0,
            trade: 0,
            archery_mastery: 0,
            building: 0,
            healing: 0,
            identification: 0,
            smithing: 0,
            dexterity: p.dexterity,
            charisma: p.charisma,
            strength: p.strength,
            endurance: p.endurance,
        };
        b.apply_combat(b.combat_specialization);
        b.apply_civil(b.civil_specialization);
        b
    }

    /// Apply a class: the primary-stat preset and `available_xp = 0`, preserving
    /// the seven secondary-skill values and both specialization selections.
    ///
    /// Mirrors the original class handler (e.g. Chief VA 0x436F9E), which writes
    /// `+0x54..+0x64` and `+0x38` but never `+0x40..+0x4C`.
    pub fn apply_class_preset(&mut self, class: CharacterClass) {
        self.class = class;
        let p = class.preset();
        self.dexterity = p.dexterity;
        self.charisma = p.charisma;
        self.strength = p.strength;
        self.endurance = p.endurance;
        self.available_xp = 0;
    }

    /// Select a combat specialization and apply only its four fields; the civil
    /// fields are untouched.
    pub fn select_combat(&mut self, spec: CombatSpecialization) {
        self.combat_specialization = spec;
        self.apply_combat(spec);
    }

    /// Select a civil specialization and apply only its three fields; the
    /// combat-derived fields are untouched.
    pub fn select_civil(&mut self, spec: CivilSpecialization) {
        self.civil_specialization = spec;
        self.apply_civil(spec);
    }

    /// Apply the four combat-specialization skill values (melee/archery and the
    /// derived building/smithing), leaving the civil fields untouched.
    pub(super) fn apply_combat(&mut self, spec: CombatSpecialization) {
        match spec {
            CombatSpecialization::Fencing => {
                self.melee_mastery = 100;
                self.archery_mastery = 0;
                self.building = 1;
                self.smithing = 10;
            }
            CombatSpecialization::Marksmanship => {
                self.melee_mastery = 0;
                self.archery_mastery = 100;
                self.building = 2;
                self.smithing = 5;
            }
        }
    }

    /// Apply the three civil-specialization skill values (trade/healing/
    /// identification), leaving the combat fields untouched.
    pub(super) fn apply_civil(&mut self, spec: CivilSpecialization) {
        match spec {
            CivilSpecialization::Identification => {
                self.trade = 10;
                self.healing = 50;
                self.identification = 20;
            }
            CivilSpecialization::Trade => {
                self.trade = 20;
                self.healing = 50;
                self.identification = 10;
            }
            CivilSpecialization::Healing => {
                self.trade = 10;
                self.healing = 100;
                self.identification = 10;
            }
        }
    }

    /// Snapshot the seven skill fields as a [`Skills`] record.
    pub fn skills(&self) -> Skills {
        Skills {
            melee_mastery: self.melee_mastery,
            trade: self.trade,
            archery_mastery: self.archery_mastery,
            building: self.building,
            healing: self.healing,
            identification: self.identification,
            smithing: self.smithing,
        }
    }

    /// Current value of a primary stat.
    pub fn stat_value(&self, stat: PrimaryStat) -> u16 {
        match stat {
            PrimaryStat::Charisma => self.charisma,
            PrimaryStat::Strength => self.strength,
            PrimaryStat::Dexterity => self.dexterity,
            PrimaryStat::Endurance => self.endurance,
        }
    }

    /// Add `delta` to one primary stat, clamping at `0` (a decrease below zero
    /// is not possible through the UI).
    pub(super) fn add_stat(&mut self, stat: PrimaryStat, delta: i32) {
        let cur = self.stat_value(stat) as i32;
        let v = (cur + delta).max(0) as u16;
        match stat {
            PrimaryStat::Charisma => self.charisma = v,
            PrimaryStat::Strength => self.strength = v,
            PrimaryStat::Dexterity => self.dexterity = v,
            PrimaryStat::Endurance => self.endurance = v,
        }
    }

    /// Original `+`: spend points to raise a stat. No-op if insufficient.
    pub fn try_increase(&mut self, stat: PrimaryStat) -> bool {
        let cost = stat_cost(self.class, stat, self.stat_value(stat));
        if self.available_xp >= cost {
            self.available_xp -= cost;
            self.add_stat(stat, 1);
            true
        } else {
            false
        }
    }

    /// Original `-`: lower a stat and refund the cost of the step just removed.
    /// Exactly reversible with [`CharacterBuild::try_increase`].
    pub fn decrease(&mut self, stat: PrimaryStat) {
        let v = self.stat_value(stat);
        if v > 0 {
            let refund = stat_cost(self.class, stat, v - 1);
            self.available_xp += refund;
            self.add_stat(stat, -1);
        }
    }
}
