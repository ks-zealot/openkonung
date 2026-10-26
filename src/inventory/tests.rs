//! Unit tests for the inventory rules.

use super::*;
use crate::res::Res;

fn test_res() -> Option<Res> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |n: &str| std::fs::read(dir.join(n)).ok();
    let (game, graph, objects, exe, konung, save) = (
        read("Game.res")?,
        read("GRAPH.RES")?,
        read("Objects.res")?,
        read("CLIENT.EXE")?,
        read("konung.res")?,
        read("Game.1")?,
    );
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    res.set_save_game(&save);
    Some(res)
}

#[test]
fn capacity_matches_original_formula() {
    assert_eq!(capacity(10), 10_000 + 10_000 / 3);
    assert_eq!(capacity(0), 10_000);
}

#[test]
fn inventory_insert_is_front_and_requires_a_free_slot() {
    let mut inv = [0u16; INVENTORY_SLOTS];
    inv[0] = 7;
    assert!(insert_inventory_front(&mut inv, 9));
    assert_eq!(&inv[..3], &[9, 7, 0]);
    inv[0] = 1;
    for slot in inv.iter_mut().skip(1) {
        *slot = 2;
    }
    assert!(!insert_inventory_front(&mut inv, 3));
    assert!(remove_inventory_id(&mut inv, 1));
    assert_eq!(inv[0], 2);
    assert_eq!(inv[INVENTORY_SLOTS - 1], 0);
}

#[test]
fn main_and_additional_class_gates_match_original() {
    for slot in 0..MAIN_SLOTS {
        assert!(main_slot_accepts(slot as u8, slot));
        assert!(!main_slot_accepts(
            (slot as u8 + 1) % MAIN_SLOTS as u8,
            slot
        ));
    }
    assert_eq!(additional_slot_for_class(6, 0x0E), Some(0));
    assert_eq!(additional_slot_for_class(7, 0x0F), Some(1));
    assert_eq!(additional_slot_for_class(7, 0x10), Some(2));
    assert_eq!(additional_slot_for_class(8, 0x11), Some(3));
    assert_eq!(additional_slot_for_class(8, 0x12), Some(4));
    assert_eq!(additional_slot_for_class(6, 0x0F), None);
}

#[test]
fn craft_table_is_the_verified_33_records() {
    assert_eq!(RECIPES.len(), 33);
    assert_eq!(
        RECIPES[0],
        Recipe {
            a: 3,
            b: 0,
            out_a: 6,
            out_b: 0xFF,
            skill_gain: 3,
            op: 0
        }
    );
    assert_eq!(
        RECIPES[19],
        Recipe {
            a: 7,
            b: 8,
            out_a: 13,
            out_b: 3,
            skill_gain: 4,
            op: 3
        }
    );
    assert_eq!(
        RECIPES[32],
        Recipe {
            a: 8,
            b: 7,
            out_a: 13,
            out_b: 3,
            skill_gain: 4,
            op: 3
        }
    );
}

#[test]
fn craft_produces_the_table_result_and_consumes_the_target() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let a = res
        .alloc_item_instance(9, 3, 0, -1, 1.0, 0, 0)
        .expect("held");
    let b = res
        .alloc_item_instance(9, 0, 0, -1, 1.0, 0, 0)
        .expect("target");
    let mut healing = 50u16;
    let result = craft(&mut res, &mut healing, a, b).expect("recipe");
    assert!(!result.default_mix);
    assert_eq!(result.result, a);
    assert_eq!(result.container, b);
    assert_eq!(res.item_class_subtype(a), Some((9, 6)));
    assert_eq!(res.item_class(b), Some(0xFF));
    assert_eq!(healing, 53);
    assert!(res.item_quality(a).unwrap() <= 15.0);
}

#[test]
fn craft_unknown_mix_uses_subtype_four() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let a = res
        .alloc_item_instance(9, 4, 0, -1, 1.0, 0, 0)
        .expect("held");
    let b = res
        .alloc_item_instance(9, 13, 0, -1, 1.0, 0, 0)
        .expect("target");
    let mut healing = 50u16;
    let result = craft(&mut res, &mut healing, a, b).expect("default mix");
    assert!(result.default_mix);
    assert_eq!(res.item_class_subtype(a), Some((9, 4)));
    assert_eq!(res.item_class(b), Some(0xFF));
    assert_eq!(healing, 50);
}

#[test]
fn craft_rejects_non_class_nine_and_self_combination() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let a = res.alloc_item_instance(9, 3, 0, -1, 1.0, 0, 0).unwrap();
    let _b = res.alloc_item_instance(9, 0, 0, -1, 1.0, 0, 0).unwrap();
    let mut healing = 50u16;
    assert!(craft(&mut res, &mut healing, a, a).is_none());
    let arrow = res.alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0).unwrap();
    assert!(craft(&mut res, &mut healing, a, arrow).is_none());
}

#[test]
fn potion_use_heals_and_leaves_an_empty_jar() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let id = res.alloc_item_instance(9, 5, 0, -1, 10.0, 0, 0).unwrap();
    let snap = OwnerSnapshot {
        hp_internal: 1000,
        ..OwnerSnapshot::default()
    };
    let ops = use_item(&mut res, &snap, id).expect("heal");
    assert!(ops.contains(&StatOp::SetHp(1600)));
    assert_eq!(res.item_class_subtype(id), Some((9, 3)));
}

#[test]
fn potion_use_blocks_when_a_timed_effect_is_active() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let id = res.alloc_item_instance(9, 9, 0, -1, 5.0, 0, 0).unwrap();
    let snap = OwnerSnapshot {
        temp_effect_timer: 30,
        ..OwnerSnapshot::default()
    };
    assert_eq!(use_item(&mut res, &snap, id), Err(UseError::Blocked));
}

#[test]
fn oil_turns_arrows_into_plain_ammo_and_is_consumed() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Class 9 subtype 7 = Масло; class 12 subtype 30 = Кремниевые стрелы.
    let oil = res.alloc_item_instance(9, 7, 0, -1, 1.0, 0, 0).unwrap();
    let arrows = res
        .alloc_item_instance(12, 30, 0, -1, 30.0, 0, 0)
        .unwrap();
    let snap = OwnerSnapshot::default();
    let ops = use_item_on(&mut res, &snap, oil, arrows).expect("oil applies");
    assert!(ops.is_empty());
    assert_eq!(res.item_instance(arrows).unwrap().modifier_index, 0);
    assert_eq!(res.item_class_subtype(oil), Some((9, 3)), "empty jar");
    assert_eq!(res.item_class_subtype(arrows), Some((12, 30)));
}

#[test]
fn oil_rejects_non_ammo_without_consuming() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let oil = res.alloc_item_instance(9, 7, 0, -1, 1.0, 0, 0).unwrap();
    let sword = res.alloc_item_instance(2, 0, 0, -1, 1.0, 0, 0).unwrap();
    let snap = OwnerSnapshot::default();
    assert_eq!(
        use_item_on(&mut res, &snap, oil, sword),
        Err(UseError::NotUsable)
    );
    assert_eq!(res.item_class_subtype(oil), Some((9, 7)));
}

#[test]
fn poison_coats_weapons_and_arrows() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let poison = res.alloc_item_instance(9, 6, 0, -1, 5.0, 0, 0).unwrap();
    let arrows = res
        .alloc_item_instance(12, 30, 0, -1, 30.0, 0, 0)
        .unwrap();
    let snap = OwnerSnapshot::default();
    assert!(use_item_on(&mut res, &snap, poison, arrows).is_ok());
    assert_eq!(res.item_instance(arrows).unwrap().poison, 5);
    assert_eq!(res.item_class_subtype(poison), Some((9, 3)));
}

#[test]
fn ammo_stacks_merge_by_type_and_consume_the_incoming_instance() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let existing = res.alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0).unwrap();
    let incoming = res.alloc_item_instance(12, 0, 0, -1, 0.0, 0, 0).unwrap();
    res.set_item_quantity(existing, 30);
    res.set_item_quantity(incoming, 12);
    let mut inv = [0u16; INVENTORY_SLOTS];
    inv[0] = existing;
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inv,
        additional: &mut additional,
        special: &mut special,
    };
    assert!(try_stack_ammo(&mut res, &mut slots, incoming));
    assert_eq!(res.item_quantity(existing), Some(42));
    assert_eq!(res.item_class(incoming), Some(0xFF));
}

#[test]
fn non_ammo_does_not_stack() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let existing = res.alloc_item_instance(9, 1, 0, -1, 1.0, 0, 0).unwrap();
    let incoming = res.alloc_item_instance(9, 1, 0, -1, 1.0, 0, 0).unwrap();
    let mut inv = [0u16; INVENTORY_SLOTS];
    inv[0] = existing;
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inv,
        additional: &mut additional,
        special: &mut special,
    };
    assert!(!try_stack_ammo(&mut res, &mut slots, incoming));
}
