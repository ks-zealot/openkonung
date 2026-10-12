//! Character creation: class presets, the final build, and the persistent
//! runtime player.
//!
//! This module separates **hero identity** ([`crate::hero::HeroTemplate`]:
//! Game.N / start location / name / visual / palette) from **character class**
//! ([`CharacterClass`]: `Entity+0x04` and the stat preset). The four class
//! presets and the initial skill values are the original character-creation
//! values (`KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** for
//! the values; the UI controls that edit them are still unimplemented).
//!
//! These values are **persistent data only** in this milestone: nothing here
//! affects damage, armor, movement, trading, dialogue, healing, crafting, item
//! requirements, XP or level-up.

use crate::hero::HeroTemplate;

/// Character class. Controls `Entity+0x04` and the stat preset. Independent of
/// the selected hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterClass {
    Warrior,
    Hunter,
    Merchant,
    Chief,
}

/// The four primary stat values a class preset initializes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassPreset {
    pub dexterity: u16,
    pub charisma: u16,
    pub strength: u16,
    pub endurance: u16,
}

impl CharacterClass {
    /// All classes in enum order.
    pub const ALL: [CharacterClass; 4] = [
        CharacterClass::Warrior,
        CharacterClass::Hunter,
        CharacterClass::Merchant,
        CharacterClass::Chief,
    ];

    /// `Entity+0x04` class value.
    pub fn entity_class(self) -> u8 {
        match self {
            CharacterClass::Warrior => 5,
            CharacterClass::Hunter => 3,
            CharacterClass::Merchant => 4,
            CharacterClass::Chief => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CharacterClass::Warrior => "Warrior",
            CharacterClass::Hunter => "Hunter",
            CharacterClass::Merchant => "Merchant",
            CharacterClass::Chief => "Chief",
        }
    }

    /// Canonical original stat preset.
    pub fn preset(self) -> ClassPreset {
        match self {
            CharacterClass::Warrior => ClassPreset {
                dexterity: 3,
                charisma: 3,
                strength: 15,
                endurance: 10,
            },
            CharacterClass::Hunter => ClassPreset {
                dexterity: 15,
                charisma: 3,
                strength: 3,
                endurance: 10,
            },
            CharacterClass::Merchant => ClassPreset {
                dexterity: 3,
                charisma: 25,
                strength: 7,
                endurance: 7,
            },
            CharacterClass::Chief => ClassPreset {
                dexterity: 3,
                charisma: 3,
                strength: 10,
                endurance: 12,
            },
        }
    }

    /// Map a `NEWH_HIT_*` region to its class.
    ///
    /// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** — the
    /// original class-screen renderer (VA 0x42EA6C) reads `Entity+0x04`,
    /// `inc eax`, and indexes the draw table with `entity_class + 1`. Hence:
    ///
    /// ```text
    /// class 2 (Chief)    -> region 3
    /// class 3 (Hunter)   -> region 4
    /// class 4 (Merchant) -> region 5
    /// class 5 (Warrior)  -> region 6
    /// ```
    pub fn from_region(region: usize) -> Option<CharacterClass> {
        match region {
            3 => Some(CharacterClass::Chief),
            4 => Some(CharacterClass::Hunter),
            5 => Some(CharacterClass::Merchant),
            6 => Some(CharacterClass::Warrior),
            _ => None,
        }
    }

    /// The `NEWH_HIT_*` region that selects this class: `entity_class + 1`.
    pub fn region(self) -> usize {
        self.entity_class() as usize + 1
    }

    /// The primary stat this class is specialized in (cheaper to raise).
    ///
    /// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED**
    /// (Chief→Endurance, Hunter→Dexterity, Merchant→Charisma, Warrior→Strength).
    pub fn specialization(self) -> PrimaryStat {
        match self {
            CharacterClass::Chief => PrimaryStat::Endurance,
            CharacterClass::Hunter => PrimaryStat::Dexterity,
            CharacterClass::Merchant => PrimaryStat::Charisma,
            CharacterClass::Warrior => PrimaryStat::Strength,
        }
    }
}

/// The four primary stats (display order: Харизма, Сила, Ловкость, Выносливость).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryStat {
    Charisma,
    Strength,
    Dexterity,
    Endurance,
}

impl PrimaryStat {
    /// Display/`NEWH` order (regions 7..10 and stepper pairs 25..32).
    pub const ORDER: [PrimaryStat; 4] = [
        PrimaryStat::Charisma,
        PrimaryStat::Strength,
        PrimaryStat::Dexterity,
        PrimaryStat::Endurance,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PrimaryStat::Charisma => "Charisma",
            PrimaryStat::Strength => "Strength",
            PrimaryStat::Dexterity => "Dexterity",
            PrimaryStat::Endurance => "Endurance",
        }
    }
}

/// Original primary-stat point cost tiers: `(cost, specialized_threshold,
/// normal_threshold)`. For a current value `v`, the first row whose applicable
/// threshold is `> v` gives the cost to raise the stat by one.
pub const STAT_TIERS: [(u32, u16, u16); 12] = [
    (2, 13, 9),
    (5, 19, 14),
    (11, 26, 20),
    (14, 34, 28),
    (18, 43, 36),
    (23, 53, 44),
    (29, 63, 52),
    (35, 74, 60),
    (72, 80, 65),
    (100, 90, 70),
    (150, 95, 75),
    (200, 100, 80),
];

/// Cost to raise `stat` by one at `value` (`999` when past the last finite tier).
pub fn stat_cost(class: CharacterClass, stat: PrimaryStat, value: u16) -> u32 {
    let specialized = class.specialization() == stat;
    for &(cost, spec, normal) in STAT_TIERS.iter() {
        let threshold = if specialized { spec } else { normal };
        if threshold > value {
            return cost;
        }
    }
    999
}

/// Original maximum-load calculation (`Entity+0x62` = `endurance_base`).
/// Rendered with one decimal (`%4.1f`).
pub fn max_load(endurance_base: u16) -> f64 {
    let raw = 10_000 + (endurance_base as i32 * 1000) / 3;
    raw as f64 * 0.001
}

/// Original maximum-party-size calculation (`Entity+0x04` = class,
/// `Entity+0x58` = `charisma_base`).
pub fn max_party(class: CharacterClass, charisma: u16) -> u8 {
    let thresholds: [u16; 10] = match class {
        CharacterClass::Chief | CharacterClass::Merchant => {
            [15, 21, 28, 36, 45, 55, 65, 74, 83, 95]
        }
        CharacterClass::Hunter => [17, 23, 30, 38, 47, 57, 67, 76, 85, 97],
        CharacterClass::Warrior => [20, 28, 36, 45, 55, 65, 74, 83, 95, 110],
    };
    let mut i = 0usize;
    while i < 10 && thresholds[i] <= charisma {
        i += 1;
    }
    if i < 10 {
        i += 1;
    }
    i as u8
}

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

/// Combat specialization: controls `melee_mastery`, `archery_mastery` and the
/// derived `building`/`smithing` values.
///
/// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** (New-Hero
/// handlers #15 VA 0x4371EE / #16 VA 0x43725C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatSpecialization {
    Fencing,
    Marksmanship,
}

impl CombatSpecialization {
    pub fn name(self) -> &'static str {
        match self {
            CombatSpecialization::Fencing => "Fencing",
            CombatSpecialization::Marksmanship => "Marksmanship",
        }
    }
}

/// Civil specialization: controls `trade`, `healing`, `identification`.
///
/// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** (New-Hero
/// handlers #17/#18/#19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CivilSpecialization {
    Identification,
    Trade,
    Healing,
}

impl CivilSpecialization {
    pub fn name(self) -> &'static str {
        match self {
            CivilSpecialization::Identification => "Identification",
            CivilSpecialization::Trade => "Trade",
            CivilSpecialization::Healing => "Healing",
        }
    }
}

/// Final state of the character-creation screen: the class plus the configured
/// skills and primary stats. This is what Start Game copies into the runtime
/// player — the class preset is **not** regenerated at start.
///
/// The seven numeric skill fields are the authoritative runtime values; the two
/// specialization enums describe how the initial values were chosen and are
/// never recomputed into the player after start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterBuild {
    pub class: CharacterClass,
    /// `Entity+0x38` — free parameter points (spent/refunded by the stat
    /// steppers). Presets initialize this to 0.
    pub available_xp: u32,
    pub combat_specialization: CombatSpecialization,
    pub civil_specialization: CivilSpecialization,
    pub melee_mastery: u16,
    pub trade: u16,
    pub archery_mastery: u16,
    pub building: u16,
    pub healing: u16,
    pub identification: u16,
    pub smithing: u16,
    pub dexterity: u16,
    pub charisma: u16,
    pub strength: u16,
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

    fn apply_combat(&mut self, spec: CombatSpecialization) {
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

    fn apply_civil(&mut self, spec: CivilSpecialization) {
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

    fn add_stat(&mut self, stat: PrimaryStat, delta: i32) {
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

/// The one authoritative runtime player: hero identity + class + persistent
/// character-creation values. Field offsets correspond to the original `Entity`
/// fields (`+04`, `+40..+4C`, `+54/+56`, `+58/+5A`, `+5E/+60`, `+62/+64`) but
/// this is not a full 0x100 `Entity` record — the runtime player is not an
/// `Entity` in the remake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerCharacter {
    pub hero: HeroTemplate,
    pub class: CharacterClass,
    pub level: u8,
    pub money: i32,
    pub available_xp: u32,
    pub invested_xp: u32,

    pub melee_mastery: u16,
    pub trade: u16,
    pub archery_mastery: u16,
    pub building: u16,
    pub healing: u16,
    pub identification: u16,
    pub smithing: u16,

    pub dexterity_base: u16,
    pub dexterity_current: u16,
    pub charisma_base: u16,
    pub charisma_current: u16,
    pub strength_base: u16,
    pub strength_current: u16,
    pub endurance_base: u16,
    pub endurance_current: u16,
}

impl PlayerCharacter {
    /// Copy a [`CharacterBuild`] once into the runtime player. Base == current
    /// for every primary stat, and the class preset is **not** re-applied.
    pub fn from_build(hero: HeroTemplate, build: CharacterBuild) -> PlayerCharacter {
        PlayerCharacter {
            hero,
            class: build.class,
            level: 1,
            money: 0,
            available_xp: build.available_xp,
            invested_xp: 0,
            melee_mastery: build.melee_mastery,
            trade: build.trade,
            archery_mastery: build.archery_mastery,
            building: build.building,
            healing: build.healing,
            identification: build.identification,
            smithing: build.smithing,
            dexterity_base: build.dexterity,
            dexterity_current: build.dexterity,
            charisma_base: build.charisma,
            charisma_current: build.charisma,
            strength_base: build.strength,
            strength_current: build.strength,
            endurance_base: build.endurance,
            endurance_current: build.endurance,
        }
    }

    /// `Entity+0x04` renderer class value.
    pub fn entity_class(&self) -> u8 {
        self.class.entity_class()
    }

    /// Resource palette id from the hero identity.
    pub fn palette_id(&self) -> i32 {
        self.hero.palette_id()
    }

    /// Human-readable player state block (diagnostic only, task §9).
    pub fn debug_block(&self) -> String {
        format!(
            "HERO {}\nCLASS {} (entity +04 = {})\n\nlevel={}\npoints={}\n\nSTR base={} current={}\nDEX base={} current={}\nCHA base={} current={}\nEND base={} current={}\n\nmax_load={:.1} max_party={}\n\nmelee={}\narchery={}\ntrade={}\nbuilding={}\nhealing={}\nidentify={}\nsmithing={}\n",
            self.hero.name,
            self.class.name(),
            self.entity_class(),
            self.level,
            self.available_xp,
            self.strength_base,
            self.strength_current,
            self.dexterity_base,
            self.dexterity_current,
            self.charisma_base,
            self.charisma_current,
            self.endurance_base,
            self.endurance_current,
            max_load(self.endurance_base),
            max_party(self.class, self.charisma_base),
            self.melee_mastery,
            self.archery_mastery,
            self.trade,
            self.building,
            self.healing,
            self.identification,
            self.smithing,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::hero;

    #[test]
    fn exact_class_presets() {
        assert_eq!(CharacterClass::Warrior.entity_class(), 5);
        assert_eq!(CharacterClass::Hunter.entity_class(), 3);
        assert_eq!(CharacterClass::Merchant.entity_class(), 4);
        assert_eq!(CharacterClass::Chief.entity_class(), 2);

        let w = CharacterClass::Warrior.preset();
        assert_eq!((w.dexterity, w.charisma, w.strength, w.endurance), (3, 3, 15, 10));
        let h = CharacterClass::Hunter.preset();
        assert_eq!((h.dexterity, h.charisma, h.strength, h.endurance), (15, 3, 3, 10));
        let m = CharacterClass::Merchant.preset();
        assert_eq!((m.dexterity, m.charisma, m.strength, m.endurance), (3, 25, 7, 7));
        let c = CharacterClass::Chief.preset();
        assert_eq!((c.dexterity, c.charisma, c.strength, c.endurance), (3, 3, 10, 12));
    }

    #[test]
    fn from_class_sets_base_equal_current_for_every_stat() {
        for class in CharacterClass::ALL {
            let pc = PlayerCharacter::from_build(*hero(0).unwrap(), CharacterBuild::from_class(class));
            assert_eq!(pc.dexterity_base, pc.dexterity_current, "{class:?} dex");
            assert_eq!(pc.charisma_base, pc.charisma_current, "{class:?} cha");
            assert_eq!(pc.strength_base, pc.strength_current, "{class:?} str");
            assert_eq!(pc.endurance_base, pc.endurance_current, "{class:?} end");
        }
    }

    #[test]
    fn initial_skill_values() {
        let pc = PlayerCharacter::from_build(
            *hero(0).unwrap(),
            CharacterBuild::from_class(CharacterClass::Warrior),
        );
        assert_eq!(pc.melee_mastery, 100);
        assert_eq!(pc.trade, 10);
        assert_eq!(pc.archery_mastery, 0);
        assert_eq!(pc.building, 1);
        assert_eq!(pc.healing, 50);
        assert_eq!(pc.identification, 20);
        assert_eq!(pc.smithing, 10);
    }

    #[test]
    fn modified_build_survives_without_preset_regeneration() {
        // §10: choose Merchant, then edit charisma 25 -> 24 and trade 10 -> 11.
        let mut build = CharacterBuild::from_class(CharacterClass::Merchant);
        build.charisma = 24;
        build.trade = 11;
        let pc = PlayerCharacter::from_build(*hero(1).unwrap(), build);
        assert_eq!(pc.class, CharacterClass::Merchant);
        assert_eq!(pc.entity_class(), 4);
        assert_eq!(pc.charisma_base, 24);
        assert_eq!(pc.charisma_current, 24);
        assert_eq!(pc.trade, 11);
        // The class preset (charisma 25) must not have been regenerated.
        assert_ne!(pc.charisma_current, CharacterClass::Merchant.preset().charisma);
    }

    #[test]
    fn class_region_mapping() {
        // Original: region = entity_class + 1.
        assert_eq!(CharacterClass::from_region(3), Some(CharacterClass::Chief));
        assert_eq!(CharacterClass::from_region(4), Some(CharacterClass::Hunter));
        assert_eq!(CharacterClass::from_region(5), Some(CharacterClass::Merchant));
        assert_eq!(CharacterClass::from_region(6), Some(CharacterClass::Warrior));
        assert_eq!(CharacterClass::from_region(0), None);
        assert_eq!(CharacterClass::from_region(7), None);
        assert_eq!(CharacterClass::Chief.region(), 3);
        assert_eq!(CharacterClass::Hunter.region(), 4);
        assert_eq!(CharacterClass::Merchant.region(), 5);
        assert_eq!(CharacterClass::Warrior.region(), 6);
        for class in CharacterClass::ALL {
            assert_eq!(class.region(), class.entity_class() as usize + 1, "{class:?}");
            assert_eq!(CharacterClass::from_region(class.region()), Some(class));
        }
    }

    #[test]
    fn default_hero_build_copies_identity_and_level() {
        let h = *hero(2).unwrap();
        let pc = PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
        assert_eq!(pc.hero.name, "Wolf");
        assert_eq!(pc.level, 1);
        assert_eq!(pc.money, 0);
        assert_eq!(pc.palette_id(), h.palette_id());
    }

    #[test]
    fn debug_block_contains_expected_fields() {
        let pc = PlayerCharacter::from_build(
            *hero(0).unwrap(),
            CharacterBuild::from_class(CharacterClass::Warrior),
        );
        let b = pc.debug_block();
        assert!(b.contains("HERO Dragomir"), "{b}");
        assert!(b.contains("CLASS Warrior (entity +04 = 5)"), "{b}");
        assert!(b.contains("STR base=15 current=15"), "{b}");
        assert!(b.contains("DEX base=3 current=3"), "{b}");
        assert!(b.contains("CHA base=3 current=3"), "{b}");
        assert!(b.contains("END base=10 current=10"), "{b}");
        assert!(b.contains("melee=100"), "{b}");
    }

    #[test]
    fn specialization_mapping() {
        assert_eq!(CharacterClass::Chief.specialization(), PrimaryStat::Endurance);
        assert_eq!(CharacterClass::Hunter.specialization(), PrimaryStat::Dexterity);
        assert_eq!(CharacterClass::Merchant.specialization(), PrimaryStat::Charisma);
        assert_eq!(CharacterClass::Warrior.specialization(), PrimaryStat::Strength);
    }

    #[test]
    fn stat_cost_worked_examples() {
        // The four documented refund/cost examples.
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 14), 5);
        assert_eq!(stat_cost(CharacterClass::Hunter, PrimaryStat::Dexterity, 14), 5);
        assert_eq!(stat_cost(CharacterClass::Merchant, PrimaryStat::Charisma, 24), 11);
        assert_eq!(stat_cost(CharacterClass::Chief, PrimaryStat::Endurance, 11), 2);
    }

    #[test]
    fn stat_cost_tier_boundaries() {
        // Specialized (Warrior/STR): threshold row applies while spec_thr > v.
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 0), 2);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 12), 2);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 13), 5);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 18), 5);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 19), 11);
        // Non-specialized (Warrior/CHA uses the normal thresholds).
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 8), 2);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 9), 5);
        // Past the last finite tier -> 999.
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 100), 999);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Charisma, 80), 999);
        // The last finite specialized tier (150/95/75).
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 94), 150);
        assert_eq!(stat_cost(CharacterClass::Warrior, PrimaryStat::Strength, 95), 200);
    }

    #[test]
    fn max_load_examples() {
        assert_eq!(format!("{:4.1}", max_load(7)), "12.3");
        assert_eq!(format!("{:4.1}", max_load(10)), "13.3");
        assert_eq!(format!("{:4.1}", max_load(12)), "14.0");
    }

    #[test]
    fn max_party_examples_and_edges() {
        assert_eq!(max_party(CharacterClass::Chief, 3), 1);
        assert_eq!(max_party(CharacterClass::Hunter, 3), 1);
        assert_eq!(max_party(CharacterClass::Merchant, 25), 3);
        assert_eq!(max_party(CharacterClass::Warrior, 3), 1);
        // Threshold edges.
        assert_eq!(max_party(CharacterClass::Chief, 14), 1);
        assert_eq!(max_party(CharacterClass::Chief, 15), 2);
        assert_eq!(max_party(CharacterClass::Chief, 94), 10);
        assert_eq!(max_party(CharacterClass::Chief, 95), 10);
        assert_eq!(max_party(CharacterClass::Warrior, 19), 1);
        assert_eq!(max_party(CharacterClass::Warrior, 20), 2);
        assert_eq!(max_party(CharacterClass::Warrior, 110), 10);
    }

    #[test]
    fn stat_stepper_is_exactly_reversible() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior); // STR 15, points 0
        assert_eq!(b.available_xp, 0);
        b.decrease(PrimaryStat::Strength);
        assert_eq!(b.strength, 14);
        assert_eq!(b.available_xp, 5);
        assert!(b.try_increase(PrimaryStat::Strength));
        assert_eq!(b.strength, 15);
        assert_eq!(b.available_xp, 0);
    }

    #[test]
    fn insufficient_points_is_a_noop() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior); // points 0
        assert!(!b.try_increase(PrimaryStat::Charisma)); // cost 2 > 0
        assert_eq!(b.charisma, 3);
        assert_eq!(b.available_xp, 0);
    }

    #[test]
    fn decrease_at_zero_is_a_noop() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
        b.charisma = 0;
        b.decrease(PrimaryStat::Charisma);
        assert_eq!(b.charisma, 0);
        assert_eq!(b.available_xp, 0);
    }

    #[test]
    fn available_xp_persists_into_the_runtime_player() {
        let mut b = CharacterBuild::from_class(CharacterClass::Merchant);
        b.decrease(PrimaryStat::Charisma); // 25 -> 24, refund 11
        assert_eq!((b.charisma, b.available_xp), (24, 11));
        let pc = PlayerCharacter::from_build(*hero(0).unwrap(), b);
        assert_eq!(pc.available_xp, 11);
        assert_eq!(pc.charisma_base, 24);
        assert_eq!(pc.charisma_current, 24);
    }

    #[test]
    fn secondary_skill_defaults_match_initial_skills() {
        let b = CharacterBuild::from_class(CharacterClass::Warrior);
        assert_eq!(b.combat_specialization, CombatSpecialization::Fencing);
        assert_eq!(b.civil_specialization, CivilSpecialization::Identification);
        assert_eq!(b.skills(), INITIAL_SKILLS);
    }

    fn combat4(b: &CharacterBuild) -> (u16, u16, u16, u16) {
        (b.melee_mastery, b.archery_mastery, b.building, b.smithing)
    }

    fn civil3(b: &CharacterBuild) -> (u16, u16, u16) {
        (b.trade, b.healing, b.identification)
    }

    #[test]
    fn combat_specialization_switch() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
        b.select_combat(CombatSpecialization::Marksmanship);
        assert_eq!(combat4(&b), (0, 100, 2, 5));
        assert_eq!(civil3(&b), (10, 50, 20), "civil fields must be untouched");
        b.select_combat(CombatSpecialization::Fencing);
        assert_eq!(combat4(&b), (100, 0, 1, 10));
        assert_eq!(civil3(&b), (10, 50, 20));
    }

    #[test]
    fn civil_specialization_switch() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
        b.select_civil(CivilSpecialization::Trade);
        assert_eq!(civil3(&b), (20, 50, 10));
        assert_eq!(combat4(&b), (100, 0, 1, 10), "combat fields must be untouched");
        b.select_civil(CivilSpecialization::Healing);
        assert_eq!(civil3(&b), (10, 100, 10));
        assert_eq!(combat4(&b), (100, 0, 1, 10));
        b.select_civil(CivilSpecialization::Identification);
        assert_eq!(civil3(&b), (10, 50, 20));
        assert_eq!(combat4(&b), (100, 0, 1, 10));
    }

    #[test]
    fn class_change_preserves_secondary_skills_and_specializations() {
        let mut b = CharacterBuild::from_class(CharacterClass::Warrior);
        b.select_combat(CombatSpecialization::Marksmanship);
        b.select_civil(CivilSpecialization::Trade);
        let skills = b.skills();
        b.apply_class_preset(CharacterClass::Merchant);
        assert_eq!(b.class, CharacterClass::Merchant);
        assert_eq!(b.combat_specialization, CombatSpecialization::Marksmanship);
        assert_eq!(b.civil_specialization, CivilSpecialization::Trade);
        assert_eq!(b.skills(), skills, "class change must not touch the skills");
        assert_eq!(b.available_xp, 0, "class change resets points");
    }
}

