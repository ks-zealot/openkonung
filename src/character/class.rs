//! `class` subsystem of [`crate::character`] (split from `character.rs`).

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

/// The four primary stats (display order: Харизма, Сила, Ловкость, Выносливость).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryStat {
    Charisma,
    Strength,
    Dexterity,
    Endurance,
}

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
impl CombatSpecialization {
    pub fn name(self) -> &'static str {
        match self {
            CombatSpecialization::Fencing => "Fencing",
            CombatSpecialization::Marksmanship => "Marksmanship",
        }
    }
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
