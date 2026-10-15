//! `roles` subsystem of [`crate::intr`] (split from `intr.rs`).

/// Named INTR indices verified against `konung.res` consumers.
pub const HUD: usize = 3;

pub const CHARACTER_WINDOW: usize = 2;

pub const INVENTORY_SLOT: usize = 17;

/// Left / right inventory scroll-arrow caps (scroll == 0 selects these).
pub const ARROW_LEFT: usize = 18;

pub const ARROW_LEFT_ALT: usize = 19;

pub const ARROW_RIGHT: usize = 20;

pub const ARROW_RIGHT_ALT: usize = 21;

/// AUTO button (off / on), selected by `Entity+0x03 & 0x02` (VA `0x42E438`).
pub const AUTO_OFF: usize = 169;

pub const AUTO_ON: usize = 170;

/// First / last portrait index (`hud::portrait_id`).
pub const PORTRAIT_FIRST: usize = 200;

pub const PORTRAIT_LAST: usize = 274;

/// First / last item-icon index (referenced by the `item.rs` template tables).
pub const ITEM_ICON_FIRST: usize = 22;

pub const ITEM_ICON_LAST: usize = 168;


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
    /// Character portrait (`INTR[200..274]`).
    Portrait,
    /// Item / ability icon referenced by the item tables.
    ItemIcon,
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
        200..=274 => IntrRole::Portrait,
        22..=168 => IntrRole::ItemIcon,
        0 | 1 | 4 | 5 | 6 => IntrRole::WindowFrame,
        179 => IntrRole::Panel,
        8 | 9 | 16 | 172 | 178 => IntrRole::Panel,
        180..=183 => IntrRole::Button,
        10..=15 => IntrRole::InterfaceControl,
        171 | 173..=177 | 184..=199 => IntrRole::SmallUi,
        _ => IntrRole::Unclassified,
    }
}


