//! `exe` subsystem of [`crate::res`] (split from `res.rs`).

#[derive(Clone, Copy)]
pub struct ExeProfile {
    pub name: &'static str,
    pub dgroup_va: u32,
    pub dgroup_raw: u32,
    pub item_template_va: u32,
    pub overlay_palette_va: u32,
}


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


pub fn exe_va_to_off(va: u32, p: &ExeProfile) -> usize {
    (p.dgroup_raw + (va - p.dgroup_va)) as usize
}

// --- human animation helpers (free functions over raw graph bytes) ---


