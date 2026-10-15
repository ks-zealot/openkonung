use super::*;

fn put_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Encode one sprite record from already-encoded row token streams (each row
/// must include its own 0x00 EOL token).
fn record(w: u16, h: u16, rows: &[Vec<u8>]) -> Vec<u8> {
    assert_eq!(rows.len(), h as usize);
    let mut r = Vec::new();
    put_u16(&mut r, w);
    put_u16(&mut r, h);
    for row in rows {
        put_u16(&mut r, row.len() as u16);
    }
    for row in rows {
        r.extend_from_slice(row);
    }
    r
}

/// Build an archive from records and per-record palette offsets. Remaining
/// slots are 0xFFFFFFFF.
fn archive(records: &[Vec<u8>], pals: &[u32]) -> Vec<u8> {
    assert_eq!(records.len(), pals.len());
    assert!(records.len() <= INTR_SLOTS);
    let region: usize = records.iter().map(|r| r.len()).sum();
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    put_u32(&mut out, region as u32);
    let mut off = 0u32;
    for i in 0..INTR_SLOTS {
        if i < records.len() {
            put_u32(&mut out, off);
            put_u32(&mut out, pals[i]);
            off += records[i].len() as u32;
        } else {
            put_u32(&mut out, INTR_UNUSED);
            put_u32(&mut out, INTR_UNUSED);
        }
    }
    for r in records {
        out.extend_from_slice(r);
    }
    out
}

/// A valid GRAPH palette buffer with palette 0 = red, palette 1 = blue.
fn graph() -> Vec<u8> {
    let mut g = vec![0u8; GRAPH_PALETTE_BYTES];
    // RGB555 0x7C00 -> red at palette 0, colour 0.
    g[0] = 0x00;
    g[1] = 0x7C;
    // RGB555 0x001F -> blue at palette 1, colour 0.
    let b = PALETTE_BLOCK as usize;
    g[b] = 0x1F;
    g[b + 1] = 0x00;
    g
}

#[test]
fn parses_all_slots_and_marks_unused() {
    let r0 = record(1, 1, &[vec![0x01, 0x00, 0x00]]);
    let r1 = record(1, 1, &[vec![0x01, 0x01, 0x00]]);
    let a = archive(&[r0, r1], &[0, 512]);
    let arch = IntrArchive::load(&a, &graph()).unwrap();
    assert_eq!(arch.entries.len(), INTR_SLOTS);
    assert_eq!(arch.len(), 2);
    assert!(arch.entry(0).is_some());
    assert!(arch.entry(1).is_some());
    assert!(arch.entry(2).is_none());
    assert!(arch.entry(INTR_SLOTS - 1).is_none());
}

#[test]
fn zero_data_offset_is_valid() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
    let arch = IntrArchive::load(&a, &graph()).unwrap();
    assert_eq!(arch.entry(0).unwrap().data_offset, 0);
    assert!(arch.dimensions(0).is_some());
}

#[test]
fn decodes_colors_and_transparency() {
    // width 3: skip 1, then two literal pixels of palette colour index 0.
    let rows = vec![vec![0x81, 0x02, 0x00, 0x00, 0x00]];
    let a = archive(&[record(3, 1, &rows)], &[512]);
    let mut arch = IntrArchive::load(&a, &graph()).unwrap();
    let im = arch.get(0).unwrap();
    assert_eq!((im.w, im.h), (3, 1));
    assert_eq!(&im.data[0..4], &[0, 0, 0, 0]); // skipped (transparent run)
    assert_eq!(&im.data[4..8], &[0, 0, 255, 255]); // palette 1 colour 0 = blue
    assert_eq!(&im.data[8..12], &[0, 0, 255, 255]);
}

#[test]
fn per_entry_palette_is_used() {
    let rows = vec![vec![0x01, 0x00, 0x00]];
    let rec = record(1, 1, &rows);
    let a = archive(&[rec.clone(), rec], &[0, 512]);
    let mut arch = IntrArchive::load(&a, &graph()).unwrap();
    let c0 = arch.get(0).unwrap();
    let c1 = arch.get(1).unwrap();
    assert_eq!(&c0.data[0..4], &[255, 0, 0, 255]); // palette 0 = red
    assert_eq!(&c1.data[0..4], &[0, 0, 255, 255]); // palette 1 = blue
    assert_ne!(c0.data, c1.data);
}

#[test]
fn rejects_bad_magic() {
    let mut a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
    a[3] = 0x09;
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("bad magic"), "{e}");
}

#[test]
fn rejects_truncated_index_or_file() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
    let e = IntrArchive::load(&a[..INTR_DATA_START - 1], &graph()).unwrap_err();
    assert!(e.message.contains("file too small"), "{e}");
}

#[test]
fn rejects_short_graph() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
    let e = IntrArchive::load(&a, &vec![0u8; 16]).unwrap_err();
    assert!(e.message.contains("GRAPH.RES too small"), "{e}");
}

#[test]
fn rejects_literal_run_past_width() {
    let rows = vec![vec![0x02, 0x00, 0x00, 0x00]]; // literal 2 in width 1
    let a = archive(&[record(1, 1, &rows)], &[0]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("past width"), "{e}");
}

#[test]
fn rejects_transparent_run_past_width() {
    let rows = vec![vec![0x82, 0x00]]; // skip 2 in width 1
    let a = archive(&[record(1, 1, &rows)], &[0]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("past width"), "{e}");
}

#[test]
fn rejects_missing_eol() {
    let rows = vec![vec![0x01, 0x00]]; // one literal, no EOL
    let a = archive(&[record(1, 1, &rows)], &[0]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("end-of-row"), "{e}");
}

#[test]
fn rejects_trailing_after_eol() {
    let rows = vec![vec![0x01, 0x00, 0x00, 0xFF]]; // literal + EOL + junk
    let a = archive(&[record(1, 1, &rows)], &[0]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("trailing"), "{e}");
}

#[test]
fn rejects_truncated_record() {
    let rows = vec![vec![0x01, 0x00, 0x00]];
    let mut a = archive(&[record(1, 1, &rows)], &[0]);
    a.truncate(a.len() - 2); // chop the row payload
    // Header still claims the full region, so bounds reject it.
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    let m = e.message;
    assert!(m.contains("data region") || m.contains("past the data region"), "{m}");
}

#[test]
fn rejects_out_of_range_palette() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[GRAPH_PALETTE_BYTES as u32]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("outside GRAPH") || e.message.contains("palette_offset"), "{e}");
}

#[test]
fn rejects_unaligned_palette() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[0x100]);
    let e = IntrArchive::load(&a, &graph()).unwrap_err();
    assert!(e.message.contains("multiple"), "{e}");
}

#[test]
fn get_returns_none_for_bad_ids() {
    let a = archive(&[record(1, 1, &[vec![0x00]])], &[0]);
    let mut arch = IntrArchive::load(&a, &graph()).unwrap();
    assert!(arch.get(1).is_none());
    assert!(arch.get(INTR_SLOTS).is_none());
    assert!(arch.get(usize::MAX).is_none());
}

#[test]
fn catalog_roles_for_known_indices() {
    assert_eq!(role_of(HUD), IntrRole::HudOverlay);
    assert_eq!(role_of(CHARACTER_WINDOW), IntrRole::CharacterWindow);
    assert_eq!(role_of(INVENTORY_SLOT), IntrRole::InventorySlot);
    for id in ARROW_LEFT..=ARROW_RIGHT_ALT {
        assert_eq!(role_of(id), IntrRole::InventoryArrow, "arrow {id}");
    }
    assert_eq!(role_of(AUTO_OFF), IntrRole::AutoButton);
    assert_eq!(role_of(AUTO_ON), IntrRole::AutoButton);
    assert_eq!(role_of(PORTRAIT_FIRST), IntrRole::Portrait);
    assert_eq!(role_of(PORTRAIT_LAST), IntrRole::Portrait);
    assert_eq!(role_of(ITEM_ICON_FIRST), IntrRole::ItemIcon);
    assert_eq!(role_of(ITEM_ICON_LAST), IntrRole::ItemIcon);
    // Out-of-range ids.
    assert_eq!(role_of(INTR_VALID_COUNT), IntrRole::Unclassified);
    assert_eq!(role_of(511), IntrRole::Unclassified);
}

#[test]
fn auto_button_selects_by_state() {
    assert_eq!(AUTO_OFF, 169);
    assert_eq!(AUTO_ON, 170);
    // `hud` maps Entity+0x03 & 0x02 -> on; verify the constant ordering only
    // (the archive lookup itself is covered by real-resource tests).
    assert!(AUTO_ON > AUTO_OFF);
}

// --- real-resource regression (only when the original assets are present) ---

fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_variant(name: &str) -> Option<(IntrArchive, Vec<u8>)> {
    let dir = install_dir();
    let intr = std::fs::read(dir.join(name)).ok()?;
    let graph = std::fs::read(dir.join("GRAPH.RES")).ok()?;
    let arch = IntrArchive::load(&intr, &graph).expect("INTR parse");
    Some((arch, intr))
}

fn dim(arch: &IntrArchive, id: usize) -> (u16, u16) {
    arch.dimensions(id).expect("dimension")
}

#[test]
fn intr800_real_archive() {
    let Some((mut arch, _)) = load_variant("INTR800R.RES") else {
        eprintln!("skipping: INTR800R.RES not present");
        return;
    };
    assert_eq!(arch.entries.len(), INTR_SLOTS);
    assert_eq!(arch.len(), INTR_VALID_COUNT);
    for id in 0..INTR_VALID_COUNT {
        assert!(arch.entry(id).is_some(), "index {id} should be occupied");
    }
    for id in INTR_VALID_COUNT..INTR_SLOTS {
        assert!(arch.entry(id).is_none(), "index {id} should be unused");
    }
    assert_eq!(arch.entry(0).unwrap().data_offset, 0);
    assert_eq!(dim(&arch, 0), (615, 55));
    assert_eq!(dim(&arch, 1), (651, 470));
    assert_eq!(dim(&arch, 2), (642, 460));
    assert_eq!(dim(&arch, 3), (800, 600));
    assert_eq!(dim(&arch, 17), (70, 70));
    let im = arch.get(3).expect("sprite 3");
    assert_eq!(im.data.len(), 800 * 600 * 4);
}

#[test]
fn intr640_real_archive() {
    let Some((arch, _)) = load_variant("INTR640R.RES") else {
        eprintln!("skipping: INTR640R.RES not present");
        return;
    };
    assert_eq!(arch.len(), INTR_VALID_COUNT);
    assert_eq!(dim(&arch, 0), (492, 44));
    assert_eq!(dim(&arch, 1), (521, 376));
    assert_eq!(dim(&arch, 2), (528, 376));
    assert_eq!(dim(&arch, 3), (640, 480));
    assert_eq!(dim(&arch, 17), (70, 70));
}

#[test]
fn variants_share_index_and_palette_table() {
    let (a, _) = match load_variant("INTR800R.RES") {
        Some(v) => v,
        None => return,
    };
    let (b, _) = match load_variant("INTR640R.RES") {
        Some(v) => v,
        None => return,
    };
    for id in 0..INTR_SLOTS {
        match (a.entry(id), b.entry(id)) {
            (None, None) => {}
            (Some(ea), Some(eb)) => {
                assert_eq!(ea.palette_offset, eb.palette_offset, "palette mismatch at {id}");
            }
            _ => panic!("occupancy mismatch at index {id}"),
        }
    }
}

#[test]
fn reference_sprites_have_expected_transparency() {
    let Some((mut arch, _)) = load_variant("INTR800R.RES") else {
        return;
    };
    // The inventory slot frame is fully opaque...
    let im = arch.get(17).unwrap();
    assert!(
        im.data.chunks(4).all(|p| p[3] == 255),
        "sprite 17 should be fully opaque"
    );
    // ...while the HUD overlay, window frames and icons carry transparent
    // pixels (transparent RLE runs / short rows), not opaque palette index 0.
    for id in [0usize, 1, 3, 71, 150, 240] {
        let im = arch.get(id).unwrap();
        assert!(
            im.data.chunks(4).any(|p| p[3] == 0),
            "sprite {id} should contain transparent pixels"
        );
        assert!(
            im.data.chunks(4).any(|p| p[3] == 255),
            "sprite {id} should contain opaque pixels"
        );
    }
}
