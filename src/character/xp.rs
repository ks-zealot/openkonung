//! `FUN_0041079C`: the experience grant and the AUTO auto-spend loop.
//!
//! Both kill paths call it with `power / 4` (`FUN_004123BC` case 5 for melee,
//! `FUN_00420F30` for arrows), and the quest/item ops use it for flat grants.
//! When the entity's AUTO bit (`Entity+0x03 & 0x02`) is set the grant is spent
//! immediately through `FUN_00410B08` and the class tables:
//!
//! ```text
//! available_xp(+0x38) += amount
//! if (Entity+0x03 & 2) {
//!     do {
//!         if (FUN_00410B08(e, 0)) level(+0x33) += 1          // four stat minima
//!         r = FUN_00410B08(e, 6); if (r) { xp -= r; invested += r; +0x64 += 1 }  // endurance
//!         r = FUN_00410B08(e, 5); ... +0x60 += 1              // strength
//!         r = FUN_00410B08(e, 2); ... +0x56 += 1              // dexterity
//!         r = FUN_00410B08(e, 3); ... +0x5A += 1              // charisma
//!     } while (any row succeeded)
//! }
//! ```
//!
//! `FUN_00410B08` refuses while `Entity+0xCE` (temporary effect) is non-zero,
//! for level `0xD`, and when the per-level stat cap is reached.

use super::class::{can_raise_any_index, raise_cost_index, PrimaryStat};

/// The fields [`grant_xp`] touches, borrowed from a `PlayerCharacter` or a
/// `population::Entity`.
pub struct XpView<'a> {
    /// `FUN_00410B08` class index (`(Entity+0x04 & 7) - 1`, `1` when `& 0x40`).
    /// `None` when the archetype cannot resolve a class record; the grant is
    /// still added, only the AUTO loop is skipped.
    pub class_index: Option<usize>,
    /// `Entity+0xCE`: a running temporary effect disables spending.
    pub temp_effect: u16,
    /// `Entity+0x03`; bit `0x02` is the AUTO button.
    pub flags_b: u8,
    /// `Entity+0x33`.
    pub level: &'a mut u8,
    /// `Entity+0x38`.
    pub available_xp: &'a mut u32,
    /// `Entity+0x3C`.
    pub invested_xp: &'a mut u32,
    /// `Entity+0x56` (dexterity current).
    pub dexterity: &'a mut u16,
    /// `Entity+0x5A` (charisma current).
    pub charisma: &'a mut u16,
    /// `Entity+0x60` (strength current).
    pub strength: &'a mut u16,
    /// `Entity+0x64` (endurance current).
    pub endurance: &'a mut u16,
}

/// `FUN_0041079C(entity, amount)`.
///
/// Adds a signed `amount` to `available_xp` and, while AUTO is set, spends it in
/// the original row order. Returns whether anything was raised.
pub fn grant_xp(v: XpView<'_>, amount: i32) -> bool {
    if amount >= 0 {
        *v.available_xp = v.available_xp.saturating_add(amount as u32);
    } else {
        *v.available_xp = v.available_xp.saturating_sub(amount.unsigned_abs());
    }
    if v.flags_b & 0x02 == 0 || v.temp_effect != 0 {
        return false;
    }
    let Some(index) = v.class_index else {
        return false;
    };
    let mut changed = false;
    loop {
        let mut any = false;
        if can_raise_any_index(
            index,
            *v.level,
            *v.dexterity,
            *v.charisma,
            *v.strength,
            *v.endurance,
        ) {
            *v.level = v.level.saturating_add(1);
            any = true;
        }
        // `FUN_00410B08` rows 6, 5, 2, 3 — endurance, strength, dexterity, charisma.
        for (stat, slot) in [
            (PrimaryStat::Endurance, 3usize),
            (PrimaryStat::Strength, 2),
            (PrimaryStat::Dexterity, 0),
            (PrimaryStat::Charisma, 1),
        ] {
            let value = match slot {
                0 => *v.dexterity,
                1 => *v.charisma,
                2 => *v.strength,
                _ => *v.endurance,
            };
            if let Some(cost) =
                raise_cost_index(index, stat, *v.level, value, *v.available_xp)
            {
                *v.available_xp -= cost;
                *v.invested_xp = v.invested_xp.saturating_add(cost);
                match slot {
                    0 => *v.dexterity = v.dexterity.saturating_add(1),
                    1 => *v.charisma = v.charisma.saturating_add(1),
                    2 => *v.strength = v.strength.saturating_add(1),
                    _ => *v.endurance = v.endurance.saturating_add(1),
                }
                any = true;
            }
        }
        if !any {
            break;
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sheet {
        level: u8,
        available: u32,
        invested: u32,
        dex: u16,
        cha: u16,
        str_: u16,
        end: u16,
    }

    impl Sheet {
        fn new() -> Self {
            Sheet {
                level: 1,
                available: 0,
                invested: 0,
                dex: 10,
                cha: 10,
                str_: 10,
                end: 10,
            }
        }

        fn view(&mut self, flags: u8, temp: u16) -> XpView<'_> {
            XpView {
                class_index: Some(0),
                temp_effect: temp,
                flags_b: flags,
                level: &mut self.level,
                available_xp: &mut self.available,
                invested_xp: &mut self.invested,
                dexterity: &mut self.dex,
                charisma: &mut self.cha,
                strength: &mut self.str_,
                endurance: &mut self.end,
            }
        }
    }

    /// Without AUTO the grant only adds experience.
    #[test]
    fn xp_is_added_and_not_spent_without_auto() {
        let mut s = Sheet::new();
        assert!(!grant_xp(s.view(0, 0), 500));
        assert_eq!(s.available, 500);
        assert_eq!(s.level, 1);
        assert_eq!((s.dex, s.cha, s.str_, s.end), (10, 10, 10, 10));
    }

    /// AUTO spends the grant through the class tables (level row first).
    #[test]
    fn auto_spends_experience_into_level_and_stats() {
        let mut s = Sheet::new();
        // Class index 0 with the minimum stats: the level row succeeds, then the
        // stat rows only while `raise_cost_index` reports them affordable.
        let changed = grant_xp(s.view(0x02, 0), 5000);
        assert!(changed, "AUTO must raise something");
        assert!(s.level > 1, "level row runs first");
        assert!(s.invested > 0, "costs move into invested_xp");
        assert_eq!(s.invested, 5000 - s.available);
        assert!(
            s.end >= 10 && s.str_ >= 10 && s.dex >= 10 && s.cha >= 10,
            "stats never decrease"
        );
    }

    /// A running temporary effect blocks `FUN_00410B08`, so AUTO idles.
    #[test]
    fn a_temporary_effect_blocks_auto_spending() {
        let mut s = Sheet::new();
        assert!(!grant_xp(s.view(0x02, 3), 5000));
        assert_eq!(s.available, 5000);
        assert_eq!(s.level, 1);
    }

    /// A negative grant subtracts (the quest/item ops can both add and remove).
    #[test]
    fn a_negative_grant_subtracts() {
        let mut s = Sheet::new();
        s.available = 100;
        assert!(!grant_xp(s.view(0, 0), -40));
        assert_eq!(s.available, 60);
    }
}
