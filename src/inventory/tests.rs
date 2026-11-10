//! Unit tests for the inventory rules.

use super::*;
use crate::res::Res;

/// Load the original install resources, or `None` when they are absent.
///
/// Attaches the `Game.1` item-instance table so `item_info`/class lookups
/// resolve against real instances.
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

/// The capacity formula is `10000 + endurance_base * 1000 / 3`.
#[test]
fn capacity_matches_original_formula() {
    assert_eq!(capacity(10), 10_000 + 10_000 / 3);
    assert_eq!(capacity(0), 10_000);
}

/// Insertion lands at slot 0 and shifts; a full bag is refused; removal shifts
/// the tail and clears the last slot.
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

/// Main slots accept only their own class; additional slots map classes
/// 6/7/8 to slots 0 and 1-2 and 3-4 only.
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

/// The recipe table has 33 rows and keeps the recovered first, twentieth and
/// last records verbatim.
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

/// A matched recipe rewrites the held subtype, consumes the target and adds the
/// row's healing gain.
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

/// An unknown pair produces «Непонятная смесь»: subtype 4, consumed target, no
/// skill gain.
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

/// Alchemy rejects combining an item with itself and any non-class-9 target.
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

/// A healing potion emits a full-HP set and becomes an empty jar (subtype 3).
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

/// Subtype 9 refuses to stack while a temporary effect timer is running and
/// leaves the item unconsumed.
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

/// Oil clears the ammo modifier index and turns itself into an empty jar.
#[test]
fn oil_turns_arrows_into_plain_ammo_and_is_consumed() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // Class 9 subtype 7 = Масло; class 12 subtype 30 = Кремниевые стрелы.
    let oil = res.alloc_item_instance(9, 7, 0, -1, 1.0, 0, 0).unwrap();
    let arrows = res.alloc_item_instance(12, 30, 0, -1, 30.0, 0, 0).unwrap();
    let snap = OwnerSnapshot::default();
    let ops = use_item_on(&mut res, &snap, oil, arrows).expect("oil applies");
    assert!(ops.is_empty());
    assert_eq!(res.item_instance(arrows).unwrap().modifier_index, 0);
    assert_eq!(res.item_class_subtype(oil), Some((9, 3)), "empty jar");
    assert_eq!(res.item_class_subtype(arrows), Some((12, 30)));
}

/// A rejected oil use returns `NotUsable` and leaves the oil instance intact.
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

/// Poison writes its rounded quality into the target's poison field and is
/// consumed into a jar.
#[test]
fn poison_coats_weapons_and_arrows() {
    let Some(mut res) = test_res() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let poison = res.alloc_item_instance(9, 6, 0, -1, 5.0, 0, 0).unwrap();
    let arrows = res.alloc_item_instance(12, 30, 0, -1, 30.0, 0, 0).unwrap();
    let snap = OwnerSnapshot::default();
    assert!(use_item_on(&mut res, &snap, poison, arrows).is_ok());
    assert_eq!(res.item_instance(arrows).unwrap().poison, 5);
    assert_eq!(res.item_class_subtype(poison), Some((9, 3)));
}

/// Matching class-12 stack keys add quantities and mark the incoming instance
/// `class = 0xFF` so the caller does not insert it.
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

/// Synthetic save: only the 16-byte instances the test needs.
fn synthetic_save(instances: &[(u16, u8, u8)]) -> Vec<u8> {
    let mut save = vec![0u8; 8192 * 16];
    for &(id, class, subtype) in instances {
        let o = id as usize * 16;
        save[o] = class;
        save[o + 3] = subtype;
    }
    save
}

fn permissive_snapshot() -> OwnerSnapshot {
    OwnerSnapshot {
        hp_internal: 60_000,
        dexterity_current: 60_000,
        strength_current: 60_000,
        charisma_current: 60_000,
        endurance_current: 60_000,
        raw_0x52: 60_000,
        ..OwnerSnapshot::default()
    }
}

#[test]
fn auto_equip_takes_the_class_match_and_consumes_the_inventory_slot() {
    let Some(mut res) = test_res() else {
        return;
    };
    res.set_save_game(&synthetic_save(&[(100, 0, 5), (101, 12, 1)]));
    let snap = permissive_snapshot();
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut inventory = [0u16; INVENTORY_SLOTS];
    inventory[0] = 101; // class 12: never fits a main slot
    inventory[1] = 100; // class 0 -> slot 0
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inventory,
        additional: &mut additional,
        special: &mut special,
    };
    assert!(auto_equip_slot(&snap, &res, &mut slots, 0));
    assert_eq!(slots.equipment[0], 100);
    assert_eq!(&slots.inventory[..3], &[101, 0, 0]);
    // An occupied slot never rescans.
    assert!(auto_equip_slot(&snap, &res, &mut slots, 0));
    assert_eq!(slots.equipment[0], 100);
    // No class-3 item: the empty slot stays empty.
    assert!(!auto_equip_slot(&snap, &res, &mut slots, 3));
    assert_eq!(slots.equipment[3], 0);
}

#[test]
fn refresh_equipment_fills_ammo_then_slots_and_recomputes_the_weapon_slot() {
    let Some(mut res) = test_res() else {
        return;
    };
    res.set_save_game(&synthetic_save(&[(100, 0, 5), (101, 12, 1), (102, 1, 5)]));
    let snap = permissive_snapshot();
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut inventory = [0u16; INVENTORY_SLOTS];
    inventory[0] = 101; // ammo
    inventory[1] = 102; // bow (class 1)
    inventory[2] = 100; // axe/slot-0 weapon (class 0)
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inventory,
        additional: &mut additional,
        special: &mut special,
    };

    let out = refresh_equipment(&snap, &res, &mut slots);
    assert!(out.changed);
    assert_eq!(*slots.special, 101, "ammo slot takes the class-12 item");
    assert_eq!(slots.equipment[0], 100);
    assert_eq!(slots.equipment[1], 102);
    assert_eq!(slots.inventory[0], 0);
    // Ammo + bow -> the bow is the active weapon (`Entity+0x12 == 1`).
    assert_eq!(out.weapon_slot, 1);

    // Without the bow the slot-0 weapon wins.
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut inventory = [0u16; INVENTORY_SLOTS];
    inventory[0] = 100;
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inventory,
        additional: &mut additional,
        special: &mut special,
    };
    let out = refresh_equipment(&snap, &res, &mut slots);
    assert_eq!(out.weapon_slot, 0);

    // Nothing equipped at all -> unarmed row 2.
    let mut equipment = [0u16; MAIN_SLOTS];
    let mut inventory = [0u16; INVENTORY_SLOTS];
    let mut additional = [0u16; ADDITIONAL_SLOTS];
    let mut special = 0u16;
    let mut slots = SlotsMut {
        equipment: &mut equipment,
        inventory: &mut inventory,
        additional: &mut additional,
        special: &mut special,
    };
    let out = refresh_equipment(&snap, &res, &mut slots);
    assert!(!out.changed);
    assert_eq!(out.weapon_slot, 2);
}
