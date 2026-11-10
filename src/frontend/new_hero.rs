//! Original New-Hero screen (`NEWH{width}R.RES`) state and helpers.
//!
//! Rendering is image-driven: the screen background (`#0`), the three hero
//! portrait pairs (`#3/#4`, `#5/#6`, `#7/#8`) and the two bottom button pairs
//! (`#9/#10`, `#11/#12`). The hero-selection regions are the confirmed
//! `NEWH_HIT_*` regions `[0, 1, 2]`; the two large bottom controls are regions
//! `[20, 21]`.
//!
//! The first image of each hero pair is the normal state and the second is the
//! selected state; the original has no hover variant (`FUN_0042EA6C`).
//! Both images of every pair are preserved.
//!
//! Bottom control roles are verified from `FUN_0042EA6C` / case 2:
//! **left** (region 20, #11/#12) = Ok; **right** (region 21, #9/#10) = Cancel.
//!
//! # Region map
//!
//! ```text
//! 0..2    hero identity portraits (draw and hit rects are the portrait boxes)
//! 3..6    class boxes, one per CharacterClass (region = Entity+0x04 + 1):
//!         3 Merchant / 4 Hunter / 5 Chief / 6 Warrior
//! 7..10   numeric rows: charisma / strength / dexterity / endurance
//! 11..12  numeric rows: max load / max party
//! 13..14  numeric rows: smithing / building (original click handlers are no-ops)
//! 15..16  combat selector boxes (Fencing / Marksmanship)
//! 17..19  civil selector boxes (Identification / Trade / Healing)
//! 20..21  bottom controls (Ok on the left / Cancel on the right)
//! 22      zero-size point: origin of the 160x160 paper doll
//! 23      big description panel (`0x46331C` records)
//! 24      top free-points box (Entity+0x38, the "PARAMETERS" header)
//! 25..32  primary-stat steppers, (minus, plus) per stat CHA/STR/DEX/END
//! ```
//!
//! Every region has one `NEWH_DRAW_*` placement rect and one `NEWH_HIT_*`
//! interactive rect; images are drawn at the draw rect and hit-tested against
//! the hit rect (they differ for the wide label rows). The module only owns the
//! *selection state*: the painters live in `super::screens`.

use crate::character::{CharacterBuild, CharacterClass, CombatSpecialization};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewHeroState {
    /// Selected hero identity index (0/1/2).
    pub selected_hero: Option<usize>,
    /// Selected class.
    pub selected_class: Option<CharacterClass>,
    /// Character-creation draft (editable).
    pub build: Option<CharacterBuild>,
    /// Currently hovered `NEWH_HIT_*` region index.
    pub hovered_region: Option<usize>,
    /// Big-panel description currently shown (hit region `0..=19`, or a stat
    /// row `7..=10`). The original keeps the last description while the pointer
    /// is over a region that has no description.
    pub description_region: Option<usize>,
    /// `Entity+0x12` active weapon storage slot used by the paper-doll preview
    /// (`FUN_00436214` hero/specialization handlers).
    pub preview_weapon_slot: u8,
}

impl Default for NewHeroState {
    /// Original New-Game init (`FUN_00436214` case 1, menu action 3): the
    /// creation entity starts as Dragomir (`D6 = 1`), class 5 (Warrior),
    /// Fencing + Identification, stats 3/3/15/10.
    fn default() -> Self {
        let class = CharacterClass::Warrior;
        NewHeroState {
            selected_hero: Some(0),
            selected_class: Some(class),
            build: Some(CharacterBuild::from_class(class)),
            hovered_region: None,
            description_region: None,
            preview_weapon_slot: 0,
        }
    }
}

impl NewHeroState {
    /// Select a hero identity and reproduce the original `Entity+0x12` choice:
    /// hero 0 → `0` when melee > archery else `1`; heroes 1/2 → `2` else `1`.
    pub fn select_hero(&mut self, hero: usize) {
        self.selected_hero = Some(hero);
        if let Some(build) = self.build.as_ref() {
            let archery_below_melee = build.archery_mastery < build.melee_mastery;
            self.preview_weapon_slot = match hero {
                0 => {
                    if archery_below_melee {
                        0
                    } else {
                        1
                    }
                }
                _ => {
                    if archery_below_melee {
                        2
                    } else {
                        1
                    }
                }
            };
        }
    }

    /// Select a combat specialization. The original handlers also set the
    /// active weapon slot (`Entity+0x12`): Fencing → `0` for Mikhail else `2`;
    /// Marksmanship → `1`.
    pub fn select_combat(&mut self, spec: CombatSpecialization) {
        if let Some(build) = self.build.as_mut() {
            build.select_combat(spec);
        }
        self.preview_weapon_slot = match spec {
            CombatSpecialization::Fencing => {
                if self.selected_hero == Some(1) {
                    0
                } else {
                    2
                }
            }
            CombatSpecialization::Marksmanship => 1,
        };
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

    /// `NEWH` skill-box regions that receive the image-`#2` marker, in the same
    /// independent order as `FUN_0042EA6C`: one combat box (`archery < melee` →
    /// 15, else 16) and any civil box whose skill exceeds its threshold.
    pub fn skill_marker_regions(&self) -> Option<Vec<usize>> {
        let b = self.build?;
        let mut out = Vec::with_capacity(4);
        out.push(if b.archery_mastery < b.melee_mastery {
            15
        } else {
            16
        });
        if b.identification > 10 {
            out.push(17);
        }
        if b.trade > 10 {
            out.push(18);
        }
        if b.healing > 50 {
            out.push(19);
        }
        Some(out)
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
    /// Implemented: combat/civil skill selector marker box (regions 15..19).
    Skill,
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
    } else if (15..=19).contains(&region) {
        RegionStatus::Skill
    } else if region == NEWH_POINTS_REGION {
        RegionStatus::Points
    } else if BOTTOM_REGIONS.contains(&region) {
        RegionStatus::Bottom
    } else {
        RegionStatus::Unknown
    }
}

impl NewHeroState {
    /// True when the region selects a hero identity (`NEWH_HIT` regions 0/1/2).
    pub fn is_hero_region(region: usize) -> bool {
        HERO_REGIONS.contains(&region)
    }

    /// Hero identity index `0..=2` for one of the three portrait regions, or
    /// `None` for any other region.
    pub fn hero_index_for_region(region: usize) -> Option<usize> {
        HERO_REGIONS.iter().position(|&r| r == region)
    }

    /// Bottom control ordinal (0 = left/Ok, 1 = right/Отмена) for a hit region,
    /// if it is one of the two large buttons.
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
    /// is chosen, otherwise the normal image. The original has no hover variant
    /// (`FUN_0042EA6C` compares only the selected hero index).
    pub fn hero_image(&self, hero: usize, _region: usize) -> usize {
        let (normal, selected) = HERO_IMAGE_PAIRS[hero];
        if self.selected_hero == Some(hero) {
            selected
        } else {
            normal
        }
    }

    /// Normal image index for a bottom control (ordinal 0 = Ok, 1 = Cancel).
    /// The original only swaps in the pressed image during the click itself.
    pub fn bottom_image(&self, bottom: usize) -> usize {
        BOTTOM_IMAGE_PAIRS[bottom].0
    }

    /// Draw position of a region's image (`NEWH_DRAW_*`).
    pub fn draw_rect(res: FrontendRes, region: usize) -> Rect {
        res.newh_draw()[region]
    }
}
