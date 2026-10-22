//! Centralised diagnostic-tooling constants (extracted from `abi/debug.rs`).

/// HUD preview checkerboard cell size in pixels.
pub const HUD_PREVIEW_CHECKER_CELL: usize = 16;
/// HUD preview checkerboard dark shade.
pub const HUD_PREVIEW_CHECKER_DARK: u8 = 0x30;
/// HUD preview checkerboard light shade.
pub const HUD_PREVIEW_CHECKER_LIGHT: u8 = 0x50;

/// HUD preview dimensions for the 640 profile.
pub const HUD_PREVIEW_640: (u32, u32) = (640, 480);
/// HUD preview dimensions for the 800 profile.
pub const HUD_PREVIEW_800: (u32, u32) = (800, 600);

/// INTR variant selector value for the 640 profile (anything else = 800).
pub const INTR_VARIANT_640: u32 = 640;

