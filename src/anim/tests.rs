//! Unit tests for the [`crate::anim`] module.

use super::*;

/// Each stance selects the paired row: idle 0/6, walk 1/7, attack 2/8, death 3/9.
#[test]
fn stance_actions_are_paired() {
    assert_eq!(idle_action(false), 6);
    assert_eq!(idle_action(true), 0);
    assert_eq!(walk_action(false), 7);
    assert_eq!(walk_action(true), 1);
    assert_eq!(attack_action(false), 8);
    assert_eq!(attack_action(true), 2);
    assert_eq!(death_action(false), 9);
    assert_eq!(death_action(true), 3);
    assert!(battle_from_flags(0x04));
    assert!(!battle_from_flags(0x00));
}
