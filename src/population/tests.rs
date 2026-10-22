//! Unit tests for the [`crate::population`] module.

use super::*;

fn empty_game() -> Vec<u8> {
    vec![0u8; ENTITY_END]
}

fn set_u16(data: &mut [u8], o: usize, v: u16) {
    data[o] = (v & 0xFF) as u8;
    data[o + 1] = (v >> 8) as u8;
}

fn set_u32(data: &mut [u8], o: usize, v: u32) {
    data[o] = (v & 0xFF) as u8;
    data[o + 1] = ((v >> 8) & 0xFF) as u8;
    data[o + 2] = ((v >> 16) & 0xFF) as u8;
    data[o + 3] = ((v >> 24) & 0xFF) as u8;
}

fn write_group(data: &mut [u8], index: usize, first: u16, count: u8, loc: u8, class: u8) {
    let o = GROUP_OFF + index * GROUP_STRIDE;
    set_u16(data, o, first);
    data[o + 0x1B] = loc;
    data[o + 0x1C] = count;
    data[o + 0x1E] = class;
}

fn write_entity(data: &mut [u8], index: usize, kind: u8, grp: u8, x: u16, y: u16) {
    let o = ENTITY_OFF + index * ENTITY_STRIDE;
    data[o] = kind;
    data[o + 0x05] = grp;
    set_u16(data, o + 0x08, x);
    set_u16(data, o + 0x0A, y);
}

#[test]
fn group_field_offsets_are_exact() {
    let mut data = empty_game();
    let o = GROUP_OFF + 5 * GROUP_STRIDE;
    set_u16(&mut data, o + 0x00, 0x1234);
    set_u16(&mut data, o + 0x08, 0xA001);
    set_u16(&mut data, o + 0x0A, 0xB001);
    set_u16(&mut data, o + 0x0C, 0xA002);
    set_u16(&mut data, o + 0x0E, 0xB002);
    set_u16(&mut data, o + 0x10, 0xA003);
    set_u16(&mut data, o + 0x12, 0xB003);
    set_u16(&mut data, o + 0x14, 0xA004);
    set_u16(&mut data, o + 0x16, 0xB004);
    data[o + 0x18] = 0x18;
    data[o + 0x19] = 0x19;
    data[o + 0x1B] = 0x1B;
    data[o + 0x1C] = 0x1C;
    data[o + 0x1E] = 0x1E;

    let groups = parse_groups(&data).unwrap();
    let g = &groups[5];
    assert_eq!(g.index, 5);
    assert_eq!(g.first_entity, 0x1234);
    assert_eq!(
        g.rect_a,
        Rect {
            x1: 0xA001,
            x2: 0xA002,
            y1: 0xA003,
            y2: 0xA004
        }
    );
    assert_eq!(
        g.rect_b,
        Rect {
            x1: 0xB001,
            x2: 0xB002,
            y1: 0xB003,
            y2: 0xB004
        }
    );
    assert_eq!(g.count_18, 0x18);
    assert_eq!(g.capacity, 0x19);
    assert_eq!(g.loc_id, 0x1B);
    assert_eq!(g.entity_count, 0x1C);
    assert_eq!(g.class_1e, 0x1E);
}

#[test]
fn entity_field_offsets_are_exact_including_unaligned_base() {
    let mut data = empty_game();
    let o = ENTITY_OFF + 7 * ENTITY_STRIDE;
    data[o + 0x00] = 0x01;
    data[o + 0x01] = 0x02;
    data[o + 0x02] = 0x03;
    data[o + 0x03] = 0x04;
    data[o + 0x04] = 0x05;
    data[o + 0x05] = 0x06;
    data[o + 0x12] = 0x12;
    set_u16(&mut data, o + 0x08, 0x0A0B);
    set_u16(&mut data, o + 0x0A, 0x0C0D);
    data[o + 0xD0] = 0xD0;
    data[o + 0xD4] = 0xD4;
    data[o + 0xD5] = 0xD5;
    data[o + 0xD6] = 0xD6;
    set_u32(&mut data, o + 0xD8, 0xDEAD_BEEF);

    let entities = parse_entities(&data).unwrap();
    let e = &entities[7];
    assert_eq!(e.index, 7);
    assert_eq!(e.kind, 0x01);
    assert_eq!(e.facing, 0x02);
    assert_eq!(e.flags_a, 0x03);
    assert_eq!(e.flags_b, 0x04);
    assert_eq!(e.archetype, 0x05);
    assert_eq!(e.group_id, 0x06);
    assert_eq!(e.raw_0x12, 0x12);
    assert_eq!(e.x, 0x0A0B);
    assert_eq!(e.y, 0x0C0D);
    assert_eq!(e.dialogue_id, 0xD0);
    assert_eq!(e.name_id, 0xD4);
    assert_eq!(e.nickname_id, 0xD5);
    assert_eq!(e.visual_index, 0xD6);
    assert_eq!(e.palette_offset, 0xDEAD_BEEF);
}

#[test]
fn entity_range_boundaries() {
    // Empty range at the very end is valid.
    let g = Group {
        index: 1,
        first_entity: ENTITY_COUNT as u16,
        entity_count: 0,
        ..empty_group(1)
    };
    assert_eq!(g.entity_range().unwrap(), ENTITY_COUNT..ENTITY_COUNT);

    // Last valid range.
    let g = Group {
        first_entity: 1999,
        entity_count: 1,
        ..empty_group(1)
    };
    assert_eq!(g.entity_range().unwrap(), 1999..2000);

    // One past the end is not.
    let g = Group {
        first_entity: 1999,
        entity_count: 2,
        ..empty_group(1)
    };
    assert!(matches!(
        g.entity_range(),
        Err(RangeError::EndOutOfRange { .. })
    ));

    // Start beyond the table.
    let g = Group {
        first_entity: 2001,
        entity_count: 0,
        ..empty_group(1)
    };
    assert!(matches!(
        g.entity_range(),
        Err(RangeError::StartOutOfRange { .. })
    ));
}

fn empty_group(index: usize) -> Group {
    Group {
        index,
        first_entity: 0,
        rect_a: Rect::default(),
        rect_b: Rect::default(),
        count_18: 0,
        capacity: 0,
        loc_id: 0,
        entity_count: 0,
        class_1e: 0,
    }
}

#[test]
fn diagnose_filters_location_and_preserves_ids() {
    let mut data = empty_game();
    // Group 3 -> entities 10..12 on location 1.
    write_group(&mut data, 3, 10, 2, 1, 79);
    write_entity(&mut data, 10, 0x11, 3, 100, 60);
    write_entity(&mut data, 11, 0x12, 3, 101, 61);
    // Another group on a different location.
    write_group(&mut data, 4, 20, 1, 2, 65);
    write_entity(&mut data, 20, 0x21, 4, 7, 8);

    let tables = parse_tables(&data).unwrap();
    let reports = diagnose_location(&tables, 1);
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert_eq!(r.group.index, 3);
    assert_eq!(r.entities.len(), 2);
    assert_eq!(r.entities[0].index, 10);
    assert_eq!(r.entities[1].index, 11);
    assert_eq!(r.entities[0].group_id, 3);
    assert_eq!((r.entities[0].x, r.entities[0].y), (100, 60));
    assert!(!r.reserved_group_zero);
    assert!(r.valid_class);
}

#[test]
fn diagnose_reports_invalid_range_without_clipping() {
    let mut data = empty_game();
    // Group 5 claims 3000..3003, beyond the 2000-entry table.
    write_group(&mut data, 5, 3000, 3, 1, 65);

    let tables = parse_tables(&data).unwrap();
    let reports = diagnose_location(&tables, 1);
    assert_eq!(reports.len(), 1);
    assert!(reports[0].range.is_err());
    assert!(reports[0].entities.is_empty());
}

#[test]
fn reserved_group_zero_is_flagged() {
    let mut data = empty_game();
    write_group(&mut data, 0, 0, 1, 1, 32);
    write_entity(&mut data, 0, 0x06, 0, 100, 60);

    let tables = parse_tables(&data).unwrap();
    let reports = diagnose_location(&tables, 1);
    assert_eq!(reports.len(), 1);
    assert!(reports[0].reserved_group_zero);
    assert_eq!(reports[0].entities.len(), 1);
}

#[test]
fn active_entities_excludes_group_zero_and_inactive_class() {
    let mut data = empty_game();
    write_group(&mut data, 0, 0, 1, 1, 32); // reserved
    write_entity(&mut data, 0, 0x06, 0, 100, 60);
    write_group(&mut data, 1, 10, 1, 1, 0); // inactive class
    write_entity(&mut data, 10, 0x11, 1, 1, 1);
    write_group(&mut data, 2, 20, 1, 1, 65); // active
    write_entity(&mut data, 20, 0x12, 2, 2, 2);

    let tables = parse_tables(&data).unwrap();
    let active = active_entities(&tables, 1);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].index, 20);
    assert_eq!(active[0].group_id, 2);
}

#[test]
fn truncated_buffers_error() {
    let short = vec![0u8; 16];
    assert!(parse_groups(&short).is_err());
    assert!(parse_entities(&short).is_err());
    assert!(parse_tables(&short).is_err());

    // Long enough for Group but not Entity.
    let mid = vec![0u8; ENTITY_OFF];
    assert!(parse_groups(&mid).is_ok());
    assert!(parse_entities(&mid).is_err());
}

#[test]
fn last_entity_is_readable() {
    let mut data = empty_game();
    set_u16(&mut data, ENTITY_OFF + 1999 * ENTITY_STRIDE + 0x08, 0xBEEF);
    set_u32(
        &mut data,
        ENTITY_OFF + 1999 * ENTITY_STRIDE + 0xD8,
        0xCAFE_F00D,
    );
    let entities = parse_entities(&data).unwrap();
    let e = &entities[1999];
    assert_eq!(e.x, 0xBEEF);
    assert_eq!(e.palette_offset, 0xCAFE_F00D);
}

#[test]
fn entity_stats_equipment_inventory_offsets() {
    let mut data = empty_game();
    let o = ENTITY_OFF + 3 * ENTITY_STRIDE;
    data[o + 0x11] = 0x11;
    data[o + 0x33] = 7;
    set_u32(&mut data, o + 0x34, 1234); // money i32
    set_u32(&mut data, o + 0x38, 55);
    set_u32(&mut data, o + 0x3C, 66);
    set_u16(&mut data, o + 0x40, 100);
    set_u16(&mut data, o + 0x42, 32);
    set_u16(&mut data, o + 0x44, 100);
    set_u16(&mut data, o + 0x46, 1);
    set_u16(&mut data, o + 0x48, 50);
    set_u16(&mut data, o + 0x4A, 20);
    set_u16(&mut data, o + 0x4C, 10);
    set_u16(&mut data, o + 0x4E, 10);
    set_u16(&mut data, o + 0x50, 5);
    set_u16(&mut data, o + 0x52, 0x52);
    set_u16(&mut data, o + 0x54, 10);
    set_u16(&mut data, o + 0x56, 10);
    set_u16(&mut data, o + 0x58, 10);
    set_u16(&mut data, o + 0x5A, 10);
    set_u16(&mut data, o + 0x5C, 1600);
    set_u16(&mut data, o + 0x5E, 15);
    set_u16(&mut data, o + 0x60, 15);
    set_u16(&mut data, o + 0x62, 10);
    set_u16(&mut data, o + 0x64, 10);
    for i in 0..6 {
        set_u16(&mut data, o + 0x66 + i * 2, 200 + i as u16);
    }
    for i in 0..32 {
        set_u16(&mut data, o + 0x72 + i * 2, 300 + i as u16);
    }

    let entities = parse_entities(&data).unwrap();
    let e = &entities[3];
    let s = &e.stats;
    assert_eq!(s.raw_0x11, 0x11);
    assert_eq!(s.level, 7);
    assert_eq!(s.money, 1234);
    assert_eq!((s.available_xp, s.invested_xp), (55, 66));
    assert_eq!(
        (s.melee_mastery, s.trade, s.archery_mastery),
        (100, 32, 100)
    );
    assert_eq!(
        (s.building, s.healing, s.identification, s.smithing),
        (1, 50, 20, 10)
    );
    assert_eq!((s.base_armor, s.periodic_damage, s.raw_0x52), (10, 5, 0x52));
    assert_eq!((s.dexterity_base, s.dexterity_current), (10, 10));
    assert_eq!((s.charisma_base, s.charisma_current), (10, 10));
    assert_eq!((s.strength_base, s.strength_current), (15, 15));
    assert_eq!((s.endurance_base, s.endurance_current), (10, 10));
    assert_eq!(s.hp_internal, 1600);
    assert_eq!(e.equipment, [200, 201, 202, 203, 204, 205]);
    assert_eq!(e.inventory[0], 300);
    assert_eq!(e.inventory[31], 331);
}

#[test]
fn entity_additional_and_special_equipment_offsets() {
    let mut data = empty_game();
    let o = ENTITY_OFF + 9 * ENTITY_STRIDE;
    for i in 0..5 {
        set_u16(&mut data, o + 0xB2 + i * 2, 400 + i as u16);
    }
    set_u16(&mut data, o + 0xCC, 0x1234);
    let entities = parse_entities(&data).unwrap();
    assert_eq!(entities[9].additional_equipment, [400, 401, 402, 403, 404]);
    assert_eq!(entities[9].special_equipment, 0x1234);
}

fn game1() -> Option<Vec<u8>> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Game.1");
    std::fs::read(&p).ok()
}

#[test]
fn real_game1_entity_277_and_539_regression() {
    let Some(data) = game1() else {
        eprintln!("skipping: Game.1 not present");
        return;
    };
    let t = parse_tables(&data).unwrap();

    let e = &t.entities[277];
    assert_eq!(e.group_id, 23);
    assert_eq!((e.x, e.y), (74, 39));
    assert_eq!(e.archetype, 5);
    assert_eq!(e.stats.level, 1);
    assert_eq!(
        (e.stats.dexterity_base, e.stats.dexterity_current),
        (10, 10)
    );
    assert_eq!((e.stats.charisma_base, e.stats.charisma_current), (10, 10));
    assert_eq!((e.stats.strength_base, e.stats.strength_current), (15, 15));
    assert_eq!(
        (e.stats.endurance_base, e.stats.endurance_current),
        (10, 10)
    );
    assert_eq!(e.stats.hp_internal, 1600);
    assert_eq!(e.stats.melee_mastery, 100);
    assert_eq!(e.stats.archery_mastery, 100);
    assert_eq!(e.stats.trade, 32);
    assert_eq!(e.equipment, [0, 0, 288, 289, 0, 0]);

    let e = &t.entities[539];
    assert_eq!(e.stats.level, 3);
    assert_eq!(
        (e.stats.dexterity_base, e.stats.dexterity_current),
        (28, 28)
    );
    assert_eq!((e.stats.charisma_base, e.stats.charisma_current), (25, 25));
    assert_eq!((e.stats.strength_base, e.stats.strength_current), (29, 29));
    assert_eq!(
        (e.stats.endurance_base, e.stats.endurance_current),
        (25, 25)
    );
    assert_eq!(e.stats.melee_mastery, 700);
    assert_eq!(e.stats.archery_mastery, 700);
    assert_eq!(e.stats.building, 5);
    assert_eq!(e.equipment, [0, 0, 745, 746, 747, 748]);
}

#[test]
fn real_game1_entity_debug_block() {
    let Some(data) = game1() else { return };
    let t = parse_tables(&data).unwrap();
    let b = t.entities[277].debug_block();
    for needle in [
        "Entity 277",
        "group=23",
        "pos=(74,39)",
        "class=5",
        "level=1",
        "HP=1600",
        "DEX=10/10",
        "CHA=10/10",
        "STR=15/15",
        "END=10/10",
        "melee=100",
        "archery=100",
        "trade=32",
    ] {
        assert!(b.contains(needle), "missing {needle:?} in:\n{b}");
    }
}

#[test]
fn real_game1_settlement_waypoints_are_homes() {
    let Some(data) = game1() else {
        eprintln!("skipping: Game.1 not present");
        return;
    };
    let settlements = parse_settlements(&data).unwrap();
    assert_eq!(settlements.len(), SETTLEMENT_COUNT);
    let s = settlements
        .iter()
        .find(|s| s.loc_id == 4)
        .expect("loc 4 settlement");
    // Owners 0x0115..=0x0118 (277..280) map to their authored positions.
    let by_owner = |o: u16| s.waypoints.iter().find(|w| w.owner == o).copied();
    assert_eq!(by_owner(277).map(|w| (w.x, w.y)), Some((74, 39)));
    assert_eq!(by_owner(278).map(|w| (w.x, w.y)), Some((50, 30)));
    assert_eq!(by_owner(279).map(|w| (w.x, w.y)), Some((94, 22)));
    assert_eq!(by_owner(280).map(|w| (w.x, w.y)), Some((60, 26)));
    // Settlement target list / group used by the projectile prop branch.
    assert_eq!(s.group_id, 23);
    assert_eq!(s.flags_4b2 & 0x10, 0x10);
    let t = s
        .targets
        .iter()
        .find(|t| t.prop_index == 33)
        .expect("prop 33 target");
    assert_eq!(t.state, 3);
}
