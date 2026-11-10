//! `class` subsystem of [`crate::character`] (split from `character.rs`).
//!
//! Maps a class to its `Entity+0x04` value, its four primary-stat preset and
//! its specialized stat, and carries the recovered DGROUP tables: the XP cost
//! tiers (`0x45AFB0`), the per-class growth caps/minima (`0x45B0AB`), the
//! specialized-stat bytes (`0x45AE28`) and the class-name base (`0x45ADC0`).

/// Character class. Controls `Entity+0x04` and the stat preset. Independent of
/// the selected hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CharacterClass {
    /// Class 5 (`Entity+0x04 = 5`, the Warrior); strength-specialized.
    Warrior,
    /// Class 3 (`Entity+0x04 = 3`, the Hunter); dexterity-specialized.
    Hunter,
    /// Class 2 (`Entity+0x04 = 2`, the Merchant); endurance-specialized.
    Merchant,
    /// Class 4 (`Entity+0x04 = 4`, the Chief); charisma-specialized.
    Chief,
}

/// The four primary stat values a class preset initializes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassPreset {
    /// `Entity+0x54` dexterity.
    pub dexterity: u16,
    /// `Entity+0x58` charisma.
    pub charisma: u16,
    /// `Entity+0x5E` strength.
    pub strength: u16,
    /// `Entity+0x62` endurance.
    pub endurance: u16,
}

/// The four primary stats (display order: Харизма, Сила, Ловкость, Выносливость).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryStat {
    /// Charisma (`Entity+0x58`).
    Charisma,
    /// Strength (`Entity+0x5E`).
    Strength,
    /// Dexterity (`Entity+0x54`).
    Dexterity,
    /// Endurance (`Entity+0x62`).
    Endurance,
}

/// Combat specialization: controls `melee_mastery`, `archery_mastery` and the
/// derived `building`/`smithing` values.
///
/// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** (New-Hero
/// handlers #15 VA 0x4371EE / #16 VA 0x43725C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatSpecialization {
    /// Fencing: melee 100, archery 0, building 1, smithing 10.
    Fencing,
    /// Marksmanship: melee 0, archery 100, building 2, smithing 5.
    Marksmanship,
}

/// Civil specialization: controls `trade`, `healing`, `identification`.
///
/// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** (New-Hero
/// handlers #17/#18/#19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CivilSpecialization {
    /// Identification: trade 10, healing 50, identification 20.
    Identification,
    /// Trade: trade 20, healing 50, identification 10.
    Trade,
    /// Healing: trade 10, healing 100, identification 10.
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

/// `0x45B0AB`: per-class stat-growth caps, indexed
/// `[class - 1][stat block][level]` where the block order is the original
/// record order `dexterity, charisma, strength, endurance`. `FUN_00410B08`
/// returns `0` (no raise shown/allowed) when the cap is `<=` the current value.
pub const STAT_GROWTH_CAPS: [[[u8; 13]; 4]; 5] = [
    [
        [255, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 19, 26, 34, 43, 53, 63, 74, 80, 90, 95, 97, 100],
        [150, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
    ],
    [
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 19, 26, 34, 43, 53, 63, 74, 80, 90, 95, 97, 100],
    ],
    [
        [150, 19, 26, 34, 43, 53, 63, 74, 80, 90, 95, 97, 100],
        [150, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
    ],
    [
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 19, 26, 34, 43, 53, 63, 74, 80, 90, 95, 97, 100],
        [150, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
    ],
    [
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
        [120, 19, 26, 34, 43, 53, 63, 74, 80, 90, 95, 97, 100],
        [150, 7, 10, 14, 18, 22, 26, 30, 32, 35, 37, 39, 41],
    ],
];

/// `0x45B0AB + 1`: per-class minimum stat for the "raise anything" gate
/// (`FUN_00410B08(entity, 0)`), same indexing as [`STAT_GROWTH_CAPS`].
pub const STAT_GROWTH_MINS: [[[u8; 13]; 4]; 5] = [
    [
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [10, 16, 22, 30, 38, 48, 58, 68, 77, 85, 92, 96, 98],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
    ],
    [
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [10, 16, 22, 30, 38, 48, 58, 68, 77, 85, 92, 96, 98],
    ],
    [
        [10, 16, 22, 30, 38, 48, 58, 68, 77, 85, 92, 96, 98],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
    ],
    [
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [10, 16, 22, 30, 38, 48, 58, 68, 77, 85, 92, 96, 98],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
    ],
    [
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
        [10, 16, 22, 30, 38, 48, 58, 68, 77, 85, 92, 96, 98],
        [3, 5, 8, 12, 16, 20, 24, 28, 31, 34, 36, 38, 40],
    ],
];

/// Original block index of one primary stat inside a `0x45B0AB` class record.
pub fn stat_block(stat: PrimaryStat) -> usize {
    match stat {
        PrimaryStat::Dexterity => 0,
        PrimaryStat::Charisma => 1,
        PrimaryStat::Strength => 2,
        PrimaryStat::Endurance => 3,
    }
}

/// `FUN_00410B08` class index: `(Entity+0x04 & 7) - 1`. All four playable
/// classes map to 1..=4; index 0 is the unused record at `0x45B0AB`.
fn growth_index(class: CharacterClass) -> usize {
    (class.entity_class() as usize).saturating_sub(1).min(4)
}

/// Per-level growth cap (`0x45B0AB` low byte) for `stat`.
pub fn stat_growth_cap(class: CharacterClass, stat: PrimaryStat, level: u8) -> u8 {
    let lvl = (level as usize).min(12);
    STAT_GROWTH_CAPS[growth_index(class)][stat_block(stat)][lvl]
}

/// Per-level minimum (`0x45B0AB` high byte) for `stat` in the "raise anything"
/// gate.
pub fn stat_growth_min(class: CharacterClass, stat: PrimaryStat, level: u8) -> u8 {
    let lvl = (level as usize).min(12);
    STAT_GROWTH_MINS[growth_index(class)][stat_block(stat)][lvl]
}

/// `FUN_00410B08(entity, 0)`: whether the character may raise anything / level
/// up. Level `0xD` is refused outright; otherwise every one of the four
/// current stats must be at least that level's minimum.
pub fn can_raise_any(
    class: CharacterClass,
    level: u8,
    dexterity: u16,
    charisma: u16,
    strength: u16,
    endurance: u16,
) -> bool {
    if level == 0xD {
        return false;
    }
    let checks = [
        (PrimaryStat::Dexterity, dexterity),
        (PrimaryStat::Charisma, charisma),
        (PrimaryStat::Strength, strength),
        (PrimaryStat::Endurance, endurance),
    ];
    checks
        .iter()
        .all(|&(stat, value)| value >= stat_growth_min(class, stat, level) as u16)
}

/// `FUN_00410B08` raw class index for an `Entity+0x04` archetype:
/// `(archetype & 7) - 1`, or `1` when `archetype & 0x40` is set. Shipped
/// villagers use `&7 == 1..5`, so index `0` (the record at `0x45B0AB`) is a
/// real, reachable record; `&7 == 0` / `> 5` are rejected rather than indexing
/// outside the five records.
pub fn growth_index_raw(archetype: u8) -> Option<usize> {
    if archetype & 0x40 != 0 {
        return Some(1);
    }
    let value = (archetype & 7) as usize;
    if (1..=5).contains(&value) {
        Some(value - 1)
    } else {
        None
    }
}

/// `0x45AE28`: per-class-index specialization stat (`[3, 6, 2, 3, 5]` =
/// charisma, endurance, dexterity, charisma, strength). Index 0 is the unused
/// record used by `archetype & 7 == 1`.
pub const SPECIALIZED_STAT: [PrimaryStat; 5] = [
    PrimaryStat::Charisma,
    PrimaryStat::Endurance,
    PrimaryStat::Dexterity,
    PrimaryStat::Charisma,
    PrimaryStat::Strength,
];

/// Per-level growth cap for a raw `FUN_00410B08` class index.
pub fn stat_growth_cap_index(index: usize, stat: PrimaryStat, level: u8) -> u8 {
    let lvl = (level as usize).min(12);
    STAT_GROWTH_CAPS[index.min(4)][stat_block(stat)][lvl]
}

/// Per-level minimum for a raw `FUN_00410B08` class index.
pub fn stat_growth_min_index(index: usize, stat: PrimaryStat, level: u8) -> u8 {
    let lvl = (level as usize).min(12);
    STAT_GROWTH_MINS[index.min(4)][stat_block(stat)][lvl]
}

/// `FUN_00410B08(entity, 0)` for a raw class index (NPC training). Level `0xD`
/// is refused; every one of the four stored stats must meet the level minimum.
pub fn can_raise_any_index(
    index: usize,
    level: u8,
    dexterity: u16,
    charisma: u16,
    strength: u16,
    endurance: u16,
) -> bool {
    if level == 0xD {
        return false;
    }
    let checks = [
        (PrimaryStat::Dexterity, dexterity),
        (PrimaryStat::Charisma, charisma),
        (PrimaryStat::Strength, strength),
        (PrimaryStat::Endurance, endurance),
    ];
    checks
        .iter()
        .all(|&(stat, value)| value >= stat_growth_min_index(index, stat, level) as u16)
}

/// `FUN_00410B08(entity, stat)` for a raw class index: the XP cost to raise
/// `stat` from `value`, or `None` when the per-level cap is reached or the
/// character cannot afford it. Mirrors the `0x45AFB0` tier table (the final
/// sentinel tier costs `999`).
pub fn raise_cost_index(
    index: usize,
    stat: PrimaryStat,
    level: u8,
    value: u16,
    available_xp: u32,
) -> Option<u32> {
    let index = index.min(4);
    if stat_growth_cap_index(index, stat, level) as u16 <= value {
        return None;
    }
    let specialized = SPECIALIZED_STAT[index] == stat;
    for &(cost, spec, normal) in STAT_TIERS.iter() {
        let threshold = if specialized { spec } else { normal };
        if threshold > value {
            return if cost <= available_xp {
                Some(cost)
            } else {
                None
            };
        }
    }
    if 999 <= available_xp {
        Some(999)
    } else {
        None
    }
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

    /// Map a raw `Entity+0x04` class value (`2..=5`).
    pub fn from_entity_class(value: u8) -> Option<CharacterClass> {
        match value {
            2 => Some(CharacterClass::Merchant),
            3 => Some(CharacterClass::Hunter),
            4 => Some(CharacterClass::Chief),
            5 => Some(CharacterClass::Warrior),
            _ => None,
        }
    }

    /// `Entity+0x04` class value. Verified against the class-name table
    /// (`0x45ADC0`, record `Entity+0x04 - 1`): 2 = Купец (Merchant),
    /// 3 = Охотник (Hunter), 4 = Вождь (Chief), 5 = Воин (Warrior).
    pub fn entity_class(self) -> u8 {
        match self {
            CharacterClass::Warrior => 5,
            CharacterClass::Hunter => 3,
            CharacterClass::Merchant => 2,
            CharacterClass::Chief => 4,
        }
    }

    /// English class name (diagnostic/debug output only; the game shows CP866
    /// names from `0x45ADC0`).
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
            // Class 2 (Купец): `FUN_00436214` case 3 / region 3.
            CharacterClass::Merchant => ClassPreset {
                dexterity: 3,
                charisma: 3,
                strength: 10,
                endurance: 12,
            },
            // Class 4 (Вождь): `FUN_00436214` case 5 / region 5.
            CharacterClass::Chief => ClassPreset {
                dexterity: 3,
                charisma: 25,
                strength: 7,
                endurance: 7,
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
    /// class 2 (Merchant / Купец)   -> region 3
    /// class 3 (Hunter / Охотник)   -> region 4
    /// class 4 (Chief / Вождь)      -> region 5
    /// class 5 (Warrior / Воин)     -> region 6
    /// ```
    pub fn from_region(region: usize) -> Option<CharacterClass> {
        match region {
            3 => Some(CharacterClass::Merchant),
            4 => Some(CharacterClass::Hunter),
            5 => Some(CharacterClass::Chief),
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
    /// Original specialized-stat byte table `0x45AE28 + (Entity+0x04 - 1)`
    /// (`FUN_00410B08`): class 2 → Endurance, class 3 → Dexterity,
    /// class 4 → Charisma, class 5 → Strength.
    pub fn specialization(self) -> PrimaryStat {
        match self {
            CharacterClass::Merchant => PrimaryStat::Endurance,
            CharacterClass::Hunter => PrimaryStat::Dexterity,
            CharacterClass::Chief => PrimaryStat::Charisma,
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

    /// English stat name (diagnostic/debug output only; the game shows the
    /// CP866 names from `0x465E04`).
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
    /// English specialization name (diagnostic/debug output only).
    pub fn name(self) -> &'static str {
        match self {
            CombatSpecialization::Fencing => "Fencing",
            CombatSpecialization::Marksmanship => "Marksmanship",
        }
    }
}
impl CivilSpecialization {
    /// English specialization name (diagnostic/debug output only).
    pub fn name(self) -> &'static str {
        match self {
            CivilSpecialization::Identification => "Identification",
            CivilSpecialization::Trade => "Trade",
            CivilSpecialization::Healing => "Healing",
        }
    }
}
