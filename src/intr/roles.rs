//! `roles` subsystem of [`crate::intr`] (split from `intr.rs`).

/// Named INTR indices verified against `konung.res` consumers.
pub const HUD: usize = 3;

/// Character-window frame (`INTR[2]`), opened by interface mode 3.
pub const CHARACTER_WINDOW: usize = 2;

/// Empty inventory slot background (`INTR[17]`, 70x70 in both variants).
pub const INVENTORY_SLOT: usize = 17;

/// Left / right inventory scroll-arrow caps (scroll == 0 selects these).
pub const ARROW_LEFT: usize = 18;

/// Alternate left scroll-arrow cap (second scroll state).
pub const ARROW_LEFT_ALT: usize = 19;

/// Right scroll-arrow cap.
pub const ARROW_RIGHT: usize = 20;

/// Alternate right scroll-arrow cap.
pub const ARROW_RIGHT_ALT: usize = 21;

/// AUTO button, off state (`Entity+0x03 & 0x02` clear), VA `0x42E438`.
pub const AUTO_OFF: usize = 169;

/// AUTO button, on state (`Entity+0x03 & 0x02` set): auto-spends experience
/// while active.
pub const AUTO_ON: usize = 170;

/// Party-panel selection marker (four corner gems) drawn over a selected
/// member's portrait (`FUN_0042C730` selection branch, descriptor `_DAT_0061F940`
/// = slot 173). Verified visually against the decoded sprite.
pub const SELECTION_MARKER: usize = 173;

/// HP-indicator bar fills, selected by health tier and poisoning (`Entity+0x50`)
/// in `FUN_0042C730`'s bar branch (`0xAE..0xB1` = descriptor slots 174..177).
pub const HP_BAR_HEALTHY: usize = 174;

/// Poisoned fill (`Entity+0x50 != 0`), drawn regardless of health tier.
pub const HP_BAR_POISONED: usize = 175;

/// Deeply wounded fill (`health < 401`).
pub const HP_BAR_DEEP_WOUNDED: usize = 176;

/// Wounded fill (`401 <= health < 801`).
pub const HP_BAR_WOUNDED: usize = 177;

/// First portrait index (`hud::portrait_id`).
pub const PORTRAIT_FIRST: usize = 200;

/// Last portrait index; `200..=274` are the 75 character portraits.
pub const PORTRAIT_LAST: usize = 274;

/// First item-icon index (referenced by the `item.rs` template tables).
pub const ITEM_ICON_FIRST: usize = 22;

/// Last item-icon index; `22..=168` are the inventory/ground icons.
pub const ITEM_ICON_LAST: usize = 168;

/// Lower-left HUD button strip (`0x81CF68[0..7]`, drawn by `FUN_0042CA44`).
///
/// Slot 0 selects sword (`16`) / axe (`8`) / bow (`9`) by `Entity+0x12`; the six
/// remaining buttons are stance, map, halt, engage, quest log and character.
pub const HUD_ICON_AXE: usize = 8;
/// Active-weapon plaque: bow (`Entity+0x12` selects the bow slot).
pub const HUD_ICON_BOW: usize = 9;
/// Battle-stance button (`FUN_00421554`).
pub const HUD_ICON_STANCE_BATTLE: usize = 10;
/// Halt button (`FUN_0042246C`).
pub const HUD_ICON_HALT: usize = 11;
/// Quest-log button (`FUN_004242BC`, interface mode 2).
pub const HUD_ICON_QUEST: usize = 12;
/// Character-screen button (`FUN_00424330`, interface mode 3).
pub const HUD_ICON_CHARACTER: usize = 13;
/// World-map button (`FUN_0042CBD0(4)`, interface mode 5).
pub const HUD_ICON_MAP: usize = 14;
/// Peace-stance button (same handler as the battle button).
pub const HUD_ICON_STANCE_PEACE: usize = 15;
/// Active-weapon plaque: sword.
pub const HUD_ICON_SWORD: usize = 16;

/// World-map parchment frame (`INTR[4]`, `FUN_00428390`).
pub const MAP_PARCHMENT: usize = 4;
/// 100x100 stone tile used to fill the world-map viewport (`INTR[5]`).
pub const MAP_STONE_TILE: usize = 5;
/// World-map discovered-region overlay (`INTR[179]`, 502x361, masked through the
/// `0x450EBC` run table).
pub const MAP_OVERLAY: usize = 179;

/// Semantic role of one occupied INTR index (verified where noted; the rest are
/// descriptive classifications, `Unclassified`/`SmallUi` where evidence is thin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrRole {
    /// Full-screen gameplay interface (`INTR[3]`).
    HudOverlay,
    /// Character window (`INTR[2]`).
    CharacterWindow,
    /// Empty inventory slot background (`INTR[17]`).
    InventorySlot,
    /// Inventory scroll-arrow caps (`INTR[18..21]`).
    InventoryArrow,
    /// AUTO combat button, off/on (`INTR[169]/[170]`).
    AutoButton,
    /// Party-panel selection marker (`INTR[173]`).
    SelectionMarker,
    /// Party-panel HP-indicator fill (`INTR[174..177]`).
    HealthBar,
    /// Character portrait (`INTR[200..274]`).
    Portrait,
    /// Item / ability icon referenced by the item tables.
    ItemIcon,
    /// Lower-left HUD button-strip sprite (`INTR[8..16]`).
    HudIcon,
    /// World-map parchment frame (`INTR[4]`).
    MapParchment,
    /// World-map discovered-region overlay (`INTR[179]`).
    MapOverlay,
    /// Large window / dialog frame.
    WindowFrame,
    /// Panel background.
    Panel,
    /// Push button.
    Button,
    /// Generic interface control.
    InterfaceControl,
    /// Small UI ornament / gauge.
    SmallUi,
    /// Not classified (usage unresolved).
    Unclassified,
}

/// The verified/classified role of an occupied INTR index. Indices outside the
/// occupied range return [`IntrRole::Unclassified`].
pub fn role_of(id: usize) -> IntrRole {
    match id {
        HUD => IntrRole::HudOverlay,
        CHARACTER_WINDOW => IntrRole::CharacterWindow,
        INVENTORY_SLOT => IntrRole::InventorySlot,
        18..=21 => IntrRole::InventoryArrow,
        169 | 170 => IntrRole::AutoButton,
        SELECTION_MARKER => IntrRole::SelectionMarker,
        174..=177 => IntrRole::HealthBar,
        200..=274 => IntrRole::Portrait,
        22..=168 => IntrRole::ItemIcon,
        MAP_PARCHMENT => IntrRole::MapParchment,
        MAP_STONE_TILE => IntrRole::Panel,
        MAP_OVERLAY => IntrRole::MapOverlay,
        8..=16 => IntrRole::HudIcon,
        0 | 1 | 6 => IntrRole::WindowFrame,
        172 | 178 => IntrRole::Panel,
        180..=183 => IntrRole::Button,
        171 | 184..=199 => IntrRole::SmallUi,
        _ => IntrRole::Unclassified,
    }
}
