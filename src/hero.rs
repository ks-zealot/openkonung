//! Hero selection ↔ world/Game.N mapping.
//!
//! Reverse-engineered mapping (do **not** assume `hero_index == Game.N suffix`):
//!
//! ```text
//! hero_selection_index  hero      Game.N  visual_index(D6)  palette   name_id  start
//! 0                     Dragomir  Game.1  1                 0x4200    124      4
//! 1                     Mikhail   Game.2  2                 0x4E00    151      7
//! 2                     Wolf      Game.0  0                 0x9400     10      1
//! ```
//!
//! The original chooses the `Game.N` file from `Entity+D6` (`visual_index`), and
//! the start-location formula is `location_id = visual_index * 3 + 1`.
//!
//! For a **new game** the original does not reuse the saved `Entity` facing; it
//! initializes `facing = 5`, `level = 1`. The start coordinates come from the
//! selected file's `Entity[0]` (+0x08/+0x0A).
//!
//! Each template also carries the raw New-Hero handler output
//! ([`OriginalHeroPreset`]) with the `0x56..0x70` fields preserved under their
//! original names (semantics **UNKNOWN**).

/// Facing set by new-game initialization (the saved `Entity+0x01` is ignored).
pub const NEW_GAME_FACING: i32 = 5;
/// Level set by new-game initialization (no stats system yet; display only).
pub const NEW_GAME_LEVEL: u8 = 1;

/// Original New-Hero handler output, preserved with **original field names**.
///
/// `KONUNG_SOURCE_OF_TRUTH.md` §0 label: **ORIGINAL-CODE VERIFIED** for the
/// values (traced from the New-Hero selection handlers); the *semantics* of the
/// `0x56..0x70` fields are **UNKNOWN** and deliberately not named after RPG
/// concepts. They are stored as `original_field_*` until their consumers are
/// traced. `D4/D6/D8` are the mapped identity fields and must stay equal to
/// [`HeroTemplate::name_id`] / [`HeroTemplate::visual_index`] /
/// [`HeroTemplate::palette_offset`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginalHeroPreset {
    /// `Entity+0xD4` (name id).
    pub field_d4: u16,
    /// `Entity+0xD6` (visual index).
    pub field_d6: u16,
    /// `Entity+0xD8` (palette / palette-base offset).
    pub field_d8: u16,
    /// `Entity+0x66` — set by hero selection 0 only.
    pub field_66: Option<u16>,
    /// `Entity+0x68` — set by all three hero selections.
    pub field_68: Option<u16>,
    /// `Entity+0x6A` — set by hero selections 1 and 2.
    pub field_6a: Option<u16>,
    /// `Entity+0x6C` — set by hero selections 0 and 2.
    pub field_6c: Option<u16>,
    /// `Entity+0x6E` — set by all three hero selections.
    pub field_6e: Option<u16>,
    /// `Entity+0x70` — set by all three hero selections.
    pub field_70: Option<u16>,
}

/// One selectable hero: identity fields, `Game.N` file, start location and the
/// raw New-Hero handler output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeroTemplate {
    /// Position in [`HEROES`] (0 = Dragomir, 1 = Mikhail, 2 = Wolf).
    pub hero_index: u32,
    /// Display name used by diagnostics.
    pub name: &'static str,
    /// `Game.N` suffix selected for this hero (from `Entity+D6` semantics).
    pub game_suffix: u32,
    /// `Entity+D6` visual index (human base-graphics index; the renderer family
    /// comes from the selected [`crate::character::CharacterClass`] `Entity+0x04`).
    pub visual_index: u8,
    /// `Entity+0xD8` palette/palette-base offset.
    pub palette_offset: u32,
    /// `Entity+0xD4` name id.
    pub name_id: u8,
    /// `location_id = visual_index * 3 + 1`.
    pub start_location: u32,
    /// Raw New-Hero handler output (see [`OriginalHeroPreset`]).
    pub preset: OriginalHeroPreset,
}

impl HeroTemplate {
    /// Resource palette id used by the canonical Entity visual path.
    pub fn palette_id(&self) -> i32 {
        (self.palette_offset / 512) as i32
    }
}

/// Explicit `hero_selection_index -> world` table.
pub const HEROES: [HeroTemplate; 3] = [
    HeroTemplate {
        hero_index: 0,
        name: "Dragomir",
        game_suffix: 1,
        visual_index: 1,
        palette_offset: 0x4200,
        name_id: 124,
        start_location: 1 * 3 + 1,
        // Hero selection 0: D4=0x7C, D6=1, D8=0x4200; 66=3, 68=4, 6C=1, 6E=8, 70=11.
        preset: OriginalHeroPreset {
            field_d4: 0x7C,
            field_d6: 1,
            field_d8: 0x4200,
            field_66: Some(3),
            field_68: Some(4),
            field_6a: None,
            field_6c: Some(1),
            field_6e: Some(8),
            field_70: Some(11),
        },
    },
    HeroTemplate {
        hero_index: 1,
        name: "Mikhail",
        game_suffix: 2,
        visual_index: 2,
        palette_offset: 0x4E00,
        name_id: 151,
        start_location: 2 * 3 + 1,
        // Hero selection 1: D4=0x97, D6=2, D8=0x4E00; 68=4, 6A=5, 6E=7, 70=10.
        preset: OriginalHeroPreset {
            field_d4: 0x97,
            field_d6: 2,
            field_d8: 0x4E00,
            field_66: None,
            field_68: Some(4),
            field_6a: Some(5),
            field_6c: None,
            field_6e: Some(7),
            field_70: Some(10),
        },
    },
    HeroTemplate {
        hero_index: 2,
        name: "Wolf",
        game_suffix: 0,
        visual_index: 0,
        palette_offset: 0x9400,
        name_id: 10,
        start_location: 0 * 3 + 1,
        // Hero selection 2: D4=0x0A, D6=0, D8=0x9400; 68=4, 6A=5, 6C=2, 6E=6, 70=9.
        preset: OriginalHeroPreset {
            field_d4: 0x0A,
            field_d6: 0,
            field_d8: 0x9400,
            field_66: None,
            field_68: Some(4),
            field_6a: Some(5),
            field_6c: Some(2),
            field_6e: Some(6),
            field_70: Some(9),
        },
    },
];

/// Look up a hero by selection index.
pub fn hero(hero_index: u32) -> Option<&'static HeroTemplate> {
    HEROES.get(hero_index as usize)
}

#[cfg(test)]
mod tests;
