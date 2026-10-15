//! Centralised constants for `exe_tables` (extracted from the module sources).

/// Size of a save-file item-instance record (`Game.0/1/2`, offset 0).
pub const ITEM_INSTANCE_SIZE: usize = 16;
/// `konung.res` DGROUP mapping: VA `0x450000` == file offset `0x36200`.
pub const ITEM_DGROUP_VA: u32 = 0x450000;
pub const ITEM_DGROUP_FILE: usize = 0x36200;
/// Icon/weight table bases used by the original selector `FUN_0042ba40`
/// (`icon = u16@+0x02`, `weight = u16@+0x00`).
pub const TABLE1_VA: u32 = 0x459B88;
pub const TABLE2_VA: u32 = 0x45812A;
pub const TABLE3_VA: u32 = 0x4594CA;
pub const TABLE4_VA: u32 = 0x460996;
/// Parallel CP866 **name** table bases read by `FUN_0042f5f8` at `+0x00`.
pub const NAME1_VA: u32 = 0x459B50;
pub const NAME2_VA: u32 = 0x4580B0;
pub const NAME3_VA: u32 = 0x459450;
pub const NAME4_VA: u32 = 0x46091C;
/// Combat-value field of the stride-`0x40` prototype record (`name[44]@+00`
/// makes combat value `0x459B50 + 0x2C`). Read by `FUN_0041ab7c` (armor
/// contribution) and `FUN_0041aea8` (weapon base) as `u16@0x459B7C +
/// subtype*0x40`; only defined for classes `0..=5|12`.
pub const ARMOR_TABLE_VA: u32 = 0x459B7C;
/// Stride of the `0x459B7C` combat-value table.
pub const ARMOR_STRIDE: usize = 64;
pub const SMALL_FONT_OFF: usize = 0x171E34;
pub const LARGE_FONT_OFF: usize = 0x16193C;
/// Highlight/selected variant of the small font (VA `0x58B8F0`), used by the
/// dialog renderer (`FUN_0042CBD0` case 1) for the selected option.
pub const HIGHLIGHT_FONT_OFF: usize = 0x171AF0;
/// `konung.res` DGROUP mapping: VA 0x450000 == file offset 0x36200.
/// Glyph pointers in the font tables are VAs into this segment.
pub const FONT_DGROUP_VA: u32 = 0x450000;
pub const FONT_DGROUP_FILE: usize = 0x36200;
