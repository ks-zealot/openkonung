//! Unit tests for the [`crate::item`] module.

use super::*;

fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(install_dir().join(name)).ok()
}

fn db_and_game() -> Option<(ItemDb, Vec<u8>)> {
    let konung = load("konung.res")?;
    // Item instances come from the loaded save file, not Game.res.
    let game = load("Game.1")?;
    Some((ItemDb::from_konung(&konung), game))
}

#[test]
fn instance_decode_reads_class_and_subtype() {
    let mut game = vec![0u8; 16 * 4];
    game[16] = 1; // id 1 class
    game[16 + 3] = 6; // id 1 subtype
    let e = ItemDb::decode_instance(&game, 1).unwrap();
    assert_eq!((e.class, e.subtype), (1, 6));
    // id 0 is the empty slot.
    assert!(ItemDb::decode_instance(&game, 0).is_none());
    // out of range.
    assert!(ItemDb::decode_instance(&game, 99).is_none());
    // truncated.
    assert!(ItemDb::decode_instance(&[0u8; 8], 1).is_none());
}

#[test]
fn table_selection_by_class() {
    assert_eq!(table_for(0).unwrap().1, 64);
    assert_eq!(table_for(5).unwrap().1, 64);
    assert_eq!(table_for(12).unwrap().1, 64);
    assert_eq!(table_for(0).unwrap().2, NAME1_VA);
    assert_eq!(table_for(12).unwrap().2, NAME1_VA);
    assert_eq!(table_for(6).unwrap().1, 128);
    assert_eq!(table_for(8).unwrap().1, 128);
    assert_eq!(table_for(6).unwrap().2, NAME2_VA);
    assert_eq!(table_for(9).unwrap().0, TABLE3_VA);
    assert_eq!(table_for(9).unwrap().2, NAME3_VA);
    assert_eq!(table_for(10).unwrap().0, TABLE4_VA);
    assert_eq!(table_for(11).unwrap().0, TABLE4_VA);
    assert_eq!(table_for(11).unwrap().2, NAME4_VA);
    assert!(table_for(13).is_none());
}

#[test]
fn modifier_and_stat_name_tables_match_the_original() {
    let Some((db, _)) = db_and_game() else { return };
    assert_eq!(db.modifier_name(0).as_deref(), Some(", зажигательные"));
    assert_eq!(db.modifier_name(1).as_deref(), Some(", на оборотня"));
    assert_eq!(db.modifier_name(-1), None);
    assert_eq!(db.stat_name(1).as_deref(), Some(" Хар"));
    assert_eq!(db.stat_name(2).as_deref(), Some(" Лов"));
    assert_eq!(db.stat_name(5).as_deref(), Some(" Сил"));
    assert_eq!(db.stat_name(8).as_deref(), Some(" Удр"));
    assert_eq!(db.stat_name(0), None);
}

#[test]
fn hover_detail_builds_the_original_single_line() {
    let Some((db, game)) = db_and_game() else { return };
    // Dragomir's starting knife (Game.1 instance 1).
    let inst = ItemDb::decode_instance(&game, 1).unwrap();
    let text = db.hover_detail(&inst).unwrap();
    assert!(text.starts_with("Нож"), "{text}");
    assert!(text.contains(", убойная сила "), "{text}");
    assert!(text.contains(", износ "), "{text}");
    assert!(text.contains(", вес "), "{text}");
}

#[test]
fn real_starting_item_mapping() {    let Some((db, game)) = db_and_game() else {
        eprintln!("skipping: konung.res / Game.1 not present");
        return;
    };
    // Dragomir's Game.1 Entity[0] loadout: (id, class, subtype, INTR icon).
    let expected = [
        (1u16, 2u8, 18u8, 150u16),
        (2, 11, 25, 99),
        (3, 9, 5, 71),
        (4, 9, 5, 71),
        (5, 9, 5, 71),
        (6, 9, 5, 71),
        (7, 9, 5, 71),
    ];
    for (id, class, subtype, icon) in expected {
        let info = db.resolve(&game, id).unwrap_or_else(|| panic!("item {id}"));
        assert_eq!(
            (info.class, info.subtype),
            (class, subtype),
            "item {id} type"
        );
        assert_eq!(info.icon, icon, "item {id} icon");
    }
}

#[test]
fn real_starting_item_names() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let n = |id| db.resolve(&game, id).unwrap().name;
    assert_eq!(n(1), "Нож");
    assert_eq!(n(2), "Правая часть амулета Дракона");
    assert_eq!(n(3), "Лечебный бальзам");
    assert_eq!(n(7), "Лечебный бальзам");
}

#[test]
fn real_starting_item_weight_and_rating() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    // (item id, weight @ icon_base+0x00, combat value @ 0x459B7C + subtype*0x40)
    let expected = [(1u16, 200i32, 60u16), (2, 100, 0), (3, 500, 0)];
    for (id, weight, rating) in expected {
        let info = db.resolve(&game, id).unwrap_or_else(|| panic!("item {id}"));
        assert_eq!(info.weight, weight, "item {id} weight");
        assert_eq!(info.rating, rating, "item {id} rating");
    }
}

#[test]
fn equipped_visual_field() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    let ov = |id| db.resolve(&game, id).unwrap().equipped_visual;
    // Game.1: id 1 = Нож (class2/sub18), id 288 = Нож, id 289 = Кольчуга.
    assert_eq!(ov(1), 4);
    assert_eq!(ov(288), 4);
    assert_eq!(ov(289), 8);
    // Non-equipment classes (amulet class 11, balm class 9) have no overlay.
    assert_eq!(ov(2), -1);
    assert_eq!(ov(3), -1);
}

#[test]
fn herb_world_sprites_match_the_original_ground_icons() {
    let Some((db, _)) = db_and_game() else { return };
    // `FUN_00438CC8` reads the world-sprite word at +4 of the class record.
    assert_eq!(db.info(9, 1).unwrap().world_sprite, 160); // Земляной орех
    assert_eq!(db.info(9, 2).unwrap().world_sprite, 168); // Белый корень
                                                          // Generic loot and stackable arrows.
    assert_eq!(db.info(9, 3).unwrap().world_sprite, 163);
    assert_eq!(db.info(12, 30).unwrap().world_sprite, 0x99);
    assert_eq!(db.info(10, 0).unwrap().world_sprite, 0xA3);
}

#[test]
fn unknown_class_is_none() {
    let Some((db, _)) = db_and_game() else { return };
    // Class 13 has no table in the original selector.
    assert!(db.info(13, 0).is_none());
}

#[test]
fn settlement_prop_table_matches_the_original_base_ids() {
    let Some(konung) = load("konung.res") else {
        eprintln!("skipping: konung.res not present");
        return;
    };
    let db = ItemDb::from_konung(&konung);
    // 0x458A02 + key*0x40, u16 at +2. Verified against the shipped table.
    assert_eq!(db.settlement_prop_base(0), Some(7));
    assert_eq!(db.settlement_prop_base(1), Some(53));
    assert_eq!(db.settlement_prop_base(2), Some(416));
    // Keys are signed bytes: 0..=0x7F are valid, 0x80..=0xFF are rejected.
    assert!(db.settlement_prop_base(0x7F).is_some());
    assert_eq!(db.settlement_prop_base(0x80), None);
}

#[test]
fn class9_price_is_signed_and_uses_charge_count() {
    let Some((db, game)) = db_and_game() else {
        return;
    };
    // Game.1 item 3 = "???????? ???????": price field -2, quantity 7.
    // `FUN_0041B2A8`: round(-7 * -2) << 3 = 112.
    let inst = ItemDb::decode_instance(&game, 3).unwrap();
    assert_eq!(inst.current_durability, 7.0);
    assert_eq!(db.instance_trade_value(&inst), 112);
}
