//! Centralised constants for `exe_tables` (extracted from the module sources).
//!
//! Recovery anchors for the original DGROUP tables: VA -> file-offset mapping,
//! item prototype/name table bases, field offsets inside the stride-`0x40` and
//! stride-`0x80` records, and the font table offsets in `konung.res`.

/// Size of a save-file item-instance record (`Game.0/1/2`, offset 0).
pub const ITEM_INSTANCE_SIZE: usize = 16;
/// `konung.res` DGROUP mapping: VA `0x450000` == file offset `0x36200`.
pub const ITEM_DGROUP_VA: u32 = 0x450000;
/// File offset of the `0x450000` DGROUP inside `konung.res`.
pub const ITEM_DGROUP_FILE: usize = 0x36200;
/// Icon/weight table bases used by the original selector `FUN_0042ba40`
/// (`icon = u16@+0x02`, `weight = u16@+0x00`).
pub const TABLE1_VA: u32 = 0x459B88;
/// Icon/weight base for classes `6..=8` (`0x45812A`, stride `0x80`).
pub const TABLE2_VA: u32 = 0x45812A;
/// Icon/weight base for class `9` (`0x4594CA`, stride `0x80`).
pub const TABLE3_VA: u32 = 0x4594CA;
/// Icon/weight base for classes `10..=11` (`0x460996`, stride `0x80`).
pub const TABLE4_VA: u32 = 0x460996;
/// Parallel CP866 **name** table bases read by `FUN_0042f5f8` at `+0x00`.
pub const NAME1_VA: u32 = 0x459B50;
/// CP866 name base for classes `6..=8` (`0x4580B0`).
pub const NAME2_VA: u32 = 0x4580B0;
/// CP866 name base for class `9` (`0x459450`).
pub const NAME3_VA: u32 = 0x459450;
/// CP866 name base for classes `10..=11` (`0x46091C`).
pub const NAME4_VA: u32 = 0x46091C;
/// Combat-value field of the stride-`0x40` prototype record (`name[44]@+00`
/// makes combat value `0x459B50 + 0x2C`). Read by `FUN_0041ab7c` (armor
/// contribution) and `FUN_0041aea8` (weapon base) as `u16@0x459B7C +
/// subtype*0x40`; only defined for classes `0..=5|12`.
pub const ARMOR_TABLE_VA: u32 = 0x459B7C;
/// Stride of the `0x459B7C` combat-value table.
pub const ARMOR_STRIDE: usize = 64;
/// Line-of-fire sample range of the same stride-`0x40` record (`u16@+0x08`,
/// VA `0x459B84`). Read by `FUN_0041130C`; the probe takes `8 * value` DDA
/// steps. Shipped values: `1` for melee subtypes, `18`/`25` for the ranged
/// ones.
pub const ITEM_PROBE_RANGE_OFF: usize = 0x08;
/// Prototype durability (`u16@+0x2E`) and reach (`u16@+0x34`) of the
/// stride-`0x40` item record.
pub const ITEM_DURABILITY_OFF: usize = 0x2E;
/// Requirement stat id and value (`+0x30`, `+0x32`) of the stride-`0x40`
/// prototype record.
pub const ITEM_REQUIREMENT_STAT_OFF: usize = 0x30;
/// Requirement value (`u16@+0x32`) paired with the stat id at `+0x30`.
pub const ITEM_REQUIREMENT_VALUE_OFF: usize = 0x32;
/// Reach field (`u16@+0x34`); `0` means the item is not ranged.
pub const ITEM_RANGE_OFF: usize = 0x34;
/// Trade-value (`price`) field of the stride-`0x40` prototype record
/// (`0x459B50 + 0x36`, read by `FUN_0041B2A8`).
pub const ITEM_VALUE_OFF_64: usize = 0x36;
/// Trade-value field of the stride-`0x80` prototype record
/// (`0x4580B0/0x459450/0x46091C + 0x78`).
pub const ITEM_VALUE_OFF_128: usize = 0x78;
/// Per-condition trade-value bonus table (`FUN_0041B2A8`, stride `0x10`).
pub const ITEM_CONDITION_VALUE_VA: u32 = 0x458530;
/// Stride of one condition-value record (`0x10` bytes).
pub const ITEM_CONDITION_VALUE_STRIDE: usize = 0x10;
/// Projectile lifetime byte (`+0x08`) read by `FUN_0041BF44` for bows.
pub const ITEM_LIFETIME_OFF: usize = 0x08;
/// Weapon-vs-creature modifier damage table (`FUN_0041aea8`); 18 records of
/// `0x40` bytes. This VA is the damage field at name-record `+0x2C`:
/// `+0x00 u16 value`, `+0x02 u16 target_archetype`. The CP866 name suffix begins
/// at the record base [`ITEM_MODIFIER_NAME_VA`].
pub const ITEM_MODIFIER_TABLE_VA: u32 = 0x462798;
/// Stride of one modifier record (`0x40` bytes).
pub const ITEM_MODIFIER_STRIDE: usize = 0x40;
/// Number of modifier records in the `0x462798` table (`18`).
pub const ITEM_MODIFIER_COUNT: usize = 18;
/// Modifier **name** suffix table (`FUN_0042F5F8`): the same 18 × `0x40`
/// records, starting at the CP866 name at `+0x00` (`", зажигательные"`, ...).
pub const ITEM_MODIFIER_NAME_VA: u32 = 0x46276C;
/// Primary-stat display names used by `FUN_0042F5F8` (`0x465E04`): eight
/// 5-byte CP866 records (` Хар, Лов, Хар, Здр, Сил, Вын, Брн, Удр`).
pub const ITEM_STAT_NAME_VA: u32 = 0x465E04;
/// Stride of one CP866 stat-name record (`5` bytes).
pub const ITEM_STAT_NAME_STRIDE: usize = 5;
/// Number of stat-name records at `0x465E04` (`8`).
pub const ITEM_STAT_NAME_COUNT: usize = 8;
/// Settlement target → prop object table (`FUN_0043adf0`), stride `0x40`;
/// the base object record index is the `u16` at `+0x02`.
pub const SETTLEMENT_PROP_TABLE_VA: u32 = 0x458A02;
/// Stride of one settlement prop record (`0x40` bytes).
pub const SETTLEMENT_PROP_STRIDE: usize = 0x40;
/// Settlement target records `0x458A02 + key*0x40`: `(required max building
/// skill, timer value)`. Field `+0` is the requirement; `+4`/`+6` are the two
/// timer columns (identical for all 16 shipped keys). Used by quest
/// `0x151..0x157`/`0x163..0x169` and the `FUN_0041D048` timer pass.
pub const SETTLEMENT_TARGET_TABLE: [(u16, u16); 16] = [
    (1, 24),
    (1, 24),
    (1, 12),
    (1, 12),
    (1, 6),
    (1, 18),
    (1, 10),
    (3, 24),
    (3, 24),
    (3, 24),
    (1, 24),
    (1, 24),
    (1, 24),
    (1, 24),
    (5, 1),
    (5, 1),
];
/// Small-font table file offset inside `konung.res` (`0x171E34`, VA `0x58BC34`).
pub const SMALL_FONT_OFF: usize = 0x171E34;
/// Large-font table file offset inside `konung.res` (`0x16193C`).
pub const LARGE_FONT_OFF: usize = 0x16193C;
/// Highlight/selected variant of the small font (VA `0x58B8F0`), used by the
/// dialog renderer (`FUN_0042CBD0` case 1) for the selected option.
pub const HIGHLIGHT_FONT_OFF: usize = 0x171AF0;
/// `konung.res` DGROUP mapping: VA 0x450000 == file offset 0x36200.
/// Glyph pointers in the font tables are VAs into this segment.
pub const FONT_DGROUP_VA: u32 = 0x450000;
/// File offset of the `0x450000` DGROUP inside `konung.res`.
pub const FONT_DGROUP_FILE: usize = 0x36200;
