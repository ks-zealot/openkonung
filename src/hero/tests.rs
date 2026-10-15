use super::*;

#[test]
fn explicit_mapping() {
    let d = hero(0).unwrap();
    assert_eq!((d.name, d.game_suffix, d.start_location), ("Dragomir", 1, 4));
    assert_eq!((d.visual_index, d.name_id), (1, 124));
    assert_eq!(d.palette_offset, 0x4200);

    let m = hero(1).unwrap();
    assert_eq!((m.name, m.game_suffix, m.start_location), ("Mikhail", 2, 7));
    assert_eq!((m.visual_index, m.name_id), (2, 151));
    assert_eq!(m.palette_offset, 0x4E00);

    let w = hero(2).unwrap();
    assert_eq!((w.name, w.game_suffix, w.start_location), ("Wolf", 0, 1));
    assert_eq!((w.visual_index, w.name_id), (0, 10));
    assert_eq!(w.palette_offset, 0x9400);
}

#[test]
fn game_suffix_is_not_the_hero_index() {
    assert_eq!(hero(0).unwrap().game_suffix, 1);
    assert_eq!(hero(1).unwrap().game_suffix, 2);
    assert_eq!(hero(2).unwrap().game_suffix, 0);
}

#[test]
fn start_location_formula_and_palette_range() {
    for h in &HEROES {
        assert_eq!(h.start_location, h.visual_index as u32 * 3 + 1, "{}", h.name);
        assert!((0..256).contains(&h.palette_id()), "{} palette", h.name);
    }
}

#[test]
fn unknown_hero_is_none() {
    assert!(hero(3).is_none());
}

#[test]
fn preset_identity_matches_hero_fields() {
    for h in &HEROES {
        assert_eq!(h.preset.field_d4 as u8, h.name_id, "{} D4/name_id", h.name);
        assert_eq!(h.preset.field_d6 as u8, h.visual_index, "{} D6/visual_index", h.name);
        assert_eq!(h.preset.field_d8 as u32, h.palette_offset, "{} D8/palette", h.name);
    }
}

#[test]
fn original_presets_match_documented_values() {
    let p0 = HEROES[0].preset;
    assert_eq!(
        (p0.field_d4, p0.field_d6, p0.field_d8),
        (0x7C, 1, 0x4200)
    );
    assert_eq!(
        (p0.field_66, p0.field_68, p0.field_6a, p0.field_6c, p0.field_6e, p0.field_70),
        (Some(3), Some(4), None, Some(1), Some(8), Some(11))
    );

    let p1 = HEROES[1].preset;
    assert_eq!(
        (p1.field_d4, p1.field_d6, p1.field_d8),
        (0x97, 2, 0x4E00)
    );
    assert_eq!(
        (p1.field_66, p1.field_68, p1.field_6a, p1.field_6c, p1.field_6e, p1.field_70),
        (None, Some(4), Some(5), None, Some(7), Some(10))
    );

    let p2 = HEROES[2].preset;
    assert_eq!(
        (p2.field_d4, p2.field_d6, p2.field_d8),
        (0x0A, 0, 0x9400)
    );
    assert_eq!(
        (p2.field_66, p2.field_68, p2.field_6a, p2.field_6c, p2.field_6e, p2.field_70),
        (None, Some(4), Some(5), Some(2), Some(6), Some(9))
    );
}
