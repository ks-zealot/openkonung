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
fn real_starting_item_mapping() {
    let Some((db, game)) = db_and_game() else {
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
        assert_eq!((info.class, info.subtype), (class, subtype), "item {id} type");
        assert_eq!(info.icon, icon, "item {id} icon");
    }
}

#[test]
fn real_starting_item_names() {
    let Some((db, game)) = db_and_game() else { return };
    let n = |id| db.resolve(&game, id).unwrap().name;
    assert_eq!(n(1), "Нож");
    assert_eq!(n(2), "Правая часть амулета Дракона");
    assert_eq!(n(3), "Лечебный бальзам");
    assert_eq!(n(7), "Лечебный бальзам");
}

#[test]
fn real_starting_item_weight_and_rating() {
    let Some((db, game)) = db_and_game() else { return };
    // (item id, weight @ icon_base+0x00, combat value @ 0x459B7C + subtype*0x40)
    let expected = [
        (1u16, 200u16, 60u16),
        (2, 100, 0),
        (3, 500, 0),
    ];
    for (id, weight, rating) in expected {
        let info = db.resolve(&game, id).unwrap_or_else(|| panic!("item {id}"));
        assert_eq!(info.weight, weight, "item {id} weight");
        assert_eq!(info.rating, rating, "item {id} rating");
    }
}

#[test]
fn equipped_visual_field() {
    let Some((db, game)) = db_and_game() else { return };
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
fn unknown_class_is_none() {
    let Some((db, _)) = db_and_game() else { return };
    // Class 13 has no table in the original selector.
    assert!(db.info(13, 0).is_none());
}
