//! Unit tests for the [`crate::anim`] module.

use super::*;

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
