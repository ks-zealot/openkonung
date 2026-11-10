//! `exe` subsystem of [`crate::res`] (split from `res.rs`).
//!
//! The original code and its data tables live in one flat 32-bit image: every
//! recovered table is addressed by a DGROUP-relative virtual address (VA), and
//! `exe_va_to_off` converts it to a file offset with the profile's raw/VA pair.
//! The profile is chosen by executable size, because the shipped 1999
//! `CLIENT.EXE` and the 2003 `LAN_KONG.EXE` place DGROUP differently.

/// DGROUP address map of one supported executable image.
#[derive(Clone, Copy)]
pub struct ExeProfile {
    /// Human-readable build name (used in diagnostics only).
    pub name: &'static str,
    /// Virtual address of the start of DGROUP (`.data`).
    pub dgroup_va: u32,
    /// File offset of the same DGROUP start.
    pub dgroup_raw: u32,
    /// Base VA of the item-template record table.
    pub item_template_va: u32,
    /// VA of the 24-dword human-overlay palette-offset table.
    pub overlay_palette_va: u32,
}

/// Look up the DGROUP profile by executable image size.
///
/// The two supported builds are `LAN_KONG.EXE` (3796992 bytes) and
/// `CLIENT.EXE` v1.03 (939008 bytes). Returns `None` for any other size; there
/// is no heuristic fallback because the table VAs would be wrong.
pub fn exe_profile(len: usize) -> Option<ExeProfile> {
    match len {
        3796992 => Some(ExeProfile {
            name: "LAN_KONG.EXE 2003",
            dgroup_va: 0x470000,
            dgroup_raw: 0x54600,
            item_template_va: 0x48089C,
            overlay_palette_va: 0x478DD4,
        }),
        939008 => Some(ExeProfile {
            name: "CLIENT.EXE 1999 v1.03",
            dgroup_va: 0x440000,
            dgroup_raw: 0x29C00,
            item_template_va: 0x44FEC0,
            overlay_palette_va: 0x44885C,
        }),
        _ => None,
    }
}

/// Convert a DGROUP VA to a file offset: `dgroup_raw + (va - dgroup_va)`.
///
/// The subtraction wraps for a VA below `dgroup_va`, so callers must pass an
/// address that really is inside the image.
pub fn exe_va_to_off(va: u32, p: &ExeProfile) -> usize {
    (p.dgroup_raw + (va - p.dgroup_va)) as usize
}

// --- human animation helpers (free functions over raw graph bytes) ---
