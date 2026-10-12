//! Original New-Hero screen (`NEWH{width}R.RES`) state and helpers.
//!
//! Rendering is image-driven: the screen background (`#0`), the three hero
//! portrait pairs (`#3/#4`, `#5/#6`, `#7/#8`) and the two bottom button pairs
//! (`#9/#10`, `#11/#12`). The hero-selection regions are the confirmed
//! `NEWH_HIT_*` regions `[0, 1, 2]`; the two large bottom controls are regions
//! `[20, 21]`.
//!
//! **WORKING INTERPRETATION** (task §15): the first image of each hero pair is
//! the normal state and the second is the selected/hovered state. Both images
//! of every pair are preserved.
//!
//! Bottom control roles are **DATA-CORRELATED** from the decoded artwork: the
//! left control (`NEWH{width}R.RES` #9/#10) is "Отмена" (Cancel) and the right
//! (#11/#12) is "Ok".

use crate::character::{CharacterBuild, CharacterClass};

use super::layout::{
    FrontendRes, Rect, BOTTOM_IMAGE_PAIRS, BOTTOM_REGIONS, HERO_IMAGE_PAIRS, HERO_REGIONS,
    NEWH_POINTS_REGION, NEWH_STEPPER_FIRST_REGION,
};

/// Mutable New-Hero selection state.
///
/// Hero identity and character class are independent. `build` is the transient
/// character-creation draft; it is copied **once** into the runtime
/// [`crate::character::PlayerCharacter`] when the player confirms, and is not
/// consulted by the game afterwards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewHeroState {
    /// Selected hero identity index (0/1/2), or `None` before a click.
    pub selected_hero: Option<usize>,
    /// Selected class, or `None` before a class box is clicked.
    pub selected_class: Option<CharacterClass>,
    /// Character-creation draft (set when a class is selected; editable).
    pub build: Option<CharacterBuild>,
    /// Currently hovered `NEWH_HIT_*` region index.
    pub hovered_region: Option<usize>,
}

impl NewHeroState {
    /// Select a hero identity.
    pub fn select_hero(&mut self, hero: usize) {
        self.selected_hero = Some(hero);
    }

    /// Select a class. The first selection initializes the draft; subsequent
    /// selections apply the class's primary-stat preset while preserving the
    /// seven secondary-skill values and both specialization selections (the
    /// original class handler never touches `Entity+0x40..+0x4C`).
    pub fn select_class(&mut self, class: CharacterClass) {
        self.selected_class = Some(class);
        match self.build.as_mut() {
            Some(b) => b.apply_class_preset(class),
            None => self.build = Some(CharacterBuild::from_class(class)),
        }
    }

    /// Start Game requires both a hero identity and a class/build.
    pub fn can_confirm(&self) -> bool {
        self.selected_hero.is_some() && self.build.is_some()
    }
}

/// Audit status of a `NEWH_HIT_*` region (task §7).
///
/// Regions 0-2 (hero identity), 3-6 (class boxes) and 20-21 (Ok / Отмена) are
/// implemented. Regions 7-19 are real original hitboxes whose behavior is **not
/// yet reverse engineered**, so they are left explicitly `Unknown` rather than
/// wired to guessed semantics:
///
/// ```text
/// 3..6   class boxes (Купец 3 / Охотник 4 / Вождь 5 / Воин 6)
/// 7..10  four stat steppers (Харизма / Сила / Ловкость / Выносливость)
/// 11..12 Макс. груз / Макс. отряд rows
/// 13..14 unknown rows
/// 15..19 skill/profession checkboxes (Кузнечное дело / Плотницкое дело, …)
/// ```
///
/// Their handlers (routine `0x410B08`, stat fields `0x56..0x70`) are still
/// untraced; see the final report's remaining-frontend-work list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionStatus {
    /// Implemented: selects a hero identity.
    Hero,
    /// Implemented: selects a character class (Warrior/Hunter/Merchant/Chief).
    Class,
    /// Implemented: Ok (confirm) / Отмена (back).
    Bottom,
    /// Implemented: primary-stat stepper `-`/`+` (regions 25..32).
    Stepper,
    /// Implemented (display only): top free-parameter-points box (region 24).
    Points,
    /// Real hitbox, original behavior not yet recovered.
    Unknown,
}

/// Classify a `NEWH_HIT_*` region for the debug overlay and audits.
pub fn region_status(region: usize) -> RegionStatus {
    if HERO_REGIONS.contains(&region) {
        RegionStatus::Hero
    } else if CharacterClass::from_region(region).is_some() {
        RegionStatus::Class
    } else if (NEWH_STEPPER_FIRST_REGION..NEWH_STEPPER_FIRST_REGION + 8).contains(&region) {
        RegionStatus::Stepper
    } else if region == NEWH_POINTS_REGION {
        RegionStatus::Points
    } else if BOTTOM_REGIONS.contains(&region) {
        RegionStatus::Bottom
    } else {
        RegionStatus::Unknown
    }
}

impl NewHeroState {
    pub fn is_hero_region(region: usize) -> bool {
        HERO_REGIONS.contains(&region)
    }

    pub fn hero_index_for_region(region: usize) -> Option<usize> {
        HERO_REGIONS.iter().position(|&r| r == region)
    }

    /// Bottom control ordinal (0 = left/confirm candidate, 1 = right/cancel
    /// candidate) for a hit region, if it is one of the two large buttons.
    pub fn bottom_index_for_region(region: usize) -> Option<usize> {
        BOTTOM_REGIONS.iter().position(|&r| r == region)
    }

    /// First hit region containing the logical point, in original table order.
    pub fn hit_test(res: FrontendRes, mx: i32, my: i32) -> Option<usize> {
        res.newh_hit()
            .iter()
            .position(|r: &Rect| r.contains(mx, my))
    }

    /// Image index to draw for a hero region: the selected image when the hero
    /// is chosen or hovered, otherwise the normal image.
    pub fn hero_image(&self, hero: usize, region: usize) -> usize {
        let (normal, selected) = HERO_IMAGE_PAIRS[hero];
        if self.selected_hero == Some(hero) || self.hovered_region == Some(region) {
            selected
        } else {
            normal
        }
    }

    /// Image index to draw for a bottom control: the alternate image while hovered.
    pub fn bottom_image(&self, bottom: usize, region: usize) -> usize {
        let (normal, alternate) = BOTTOM_IMAGE_PAIRS[bottom];
        if self.hovered_region == Some(region) {
            alternate
        } else {
            normal
        }
    }

    /// Draw position of a region's image (`NEWH_DRAW_*`).
    pub fn draw_rect(res: FrontendRes, region: usize) -> Rect {
        res.newh_draw()[region]
    }
}
