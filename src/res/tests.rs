//! Unit tests for the [`crate::res`] module.

use super::{human_frame_count, human_mapped_frame, Res};

#[test]
fn human_frames_wrap_by_graph_sequence_count() {
    // Synthetic GRAPH block at position 0: action 0 / direction 0 has a
    // 3-frame sequence with frame ids 7, 9, 11 (table-driven, not hardcoded).
    let mut g = vec![0u8; 0x200];
    g[0x08] = 3; // count (low byte)
    g[0x0A] = 7;
    g[0x0C] = 9;
    g[0x0E] = 11;

    assert_eq!(human_frame_count(&g, 0, 0, 0), 3);
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 0), (7, 3));
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 1), (9, 3));
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 2), (11, 3));
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 3), (7, 3)); // wraps
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 7), (9, 3)); // 7 % 3 == 1
}

#[test]
fn zero_count_sequence_is_single_frame() {
    let g = vec![0u8; 0x200];
    assert_eq!(human_frame_count(&g, 0, 0, 0), 0);
    assert_eq!(human_mapped_frame(&g, 0, 0, 0, 5), (0, 1));
}

#[test]
fn cursor_block_decodes_16bit_color_and_transparency() {
    // One cursor block at 0x20000: `[u32 type][u32 size][u16 w][u16 h]
    // [row-size table][rows]`. Row 0 = skip 1 px + literal 1 px 0x7C00 (red) + end.
    let payload: Vec<u8> = {
        let mut p = vec![2u8, 0, 1, 0]; // w=2, h=1
        p.extend_from_slice(&[5u8, 0]); // row byte length = 5
        p.extend_from_slice(&[0x81, 0x01, 0x00, 0x7C, 0x00]); // skip1, literal1, px, end
        p
    };
    let mut g = vec![0u8; crate::res::CURSOR_GRAPH_OFF];
    g.extend_from_slice(&0x0500_0000u32.to_le_bytes()); // type
    g.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // size
    g.extend_from_slice(&payload);
    let img = crate::res::cursor_image(&g, 0).expect("decodes");
    assert_eq!((img.w, img.h), (2, 1));
    assert_eq!(img.data[3], 0, "skipped pixel is transparent");
    assert_eq!(&img.data[4..8], &[255, 0, 0, 255], "literal pixel 0x7C00 = red");
    assert!(crate::res::cursor_image(&g, 1).is_none(), "second block absent");
}

#[test]
fn human_action_rows_do_not_fold_modulo_six() {
    // Each of the 10 action rows must read its own table slice. Row 6/7
    // (peaceful idle/walk) must NOT alias rows 0/1 (battle idle/walk).
    let mut g = vec![0u8; 0x2000];
    for a in 0..10usize {
        let o = a * 0xD0 + 8;
        g[o] = 1; // count = 1
        g[o + 2] = (100 + a) as u8; // frame id == 100 + action
        g[o + 3] = 0;
    }
    for a in 0..10usize {
        assert_eq!(human_frame_count(&g, 0, a, 0), 1);
        assert_eq!(human_mapped_frame(&g, 0, a, 0, 0), (100 + a, 1));
    }
    assert_ne!(
        human_mapped_frame(&g, 0, 0, 0, 0),
        human_mapped_frame(&g, 0, 6, 0, 0)
    );
    assert_ne!(
        human_mapped_frame(&g, 0, 1, 0, 0),
        human_mapped_frame(&g, 0, 7, 0, 0)
    );
}

#[test]
fn object_actions_six_to_eleven_alias_zero_to_five() {
    for a in 0..6usize {
        assert_eq!(Res::object_row(a), a);
        assert_eq!(Res::object_row(a + 6), a);
    }
}

#[test]
fn object_anchor_is_indexed_by_row_dir_frame_not_frame_id() {
    use super::objects::monster_anchor_offset;

    // Original object anchor table: `0x1728 + row*0x480 + dir*0x90 + fi*8`
    // (18 slots per direction, 144 per row). `FUN_00414078` 0x41410a-0x414143.
    assert_eq!(monster_anchor_offset(0, 0, 0), 0x1728);
    assert_eq!(monster_anchor_offset(1, 0, 0), 0x1728 + 0x480);
    assert_eq!(monster_anchor_offset(0, 1, 0), 0x1728 + 0x90);
    assert_eq!(monster_anchor_offset(0, 0, 1), 0x1728 + 8);
    assert_eq!(monster_anchor_offset(5, 7, 17), 0x1728 + 5 * 0x480 + 7 * 0x90 + 17 * 8);

    // Regression: a walk frame id (e.g. 132 for row 1 / dir 0) must NOT be used
    // as the table index; that was the "jumping" object-anchor bug.
    assert_ne!(monster_anchor_offset(1, 0, 0), 0x1728 + 132 * 8);
}

/// End-to-end: equipped items composite their GRAPH overlays onto a Game.N
/// humanoid, and the frame cache distinguishes different loadouts. Gated on
/// the original files being present.
#[test]
fn human_frame_from_base_applies_equipment_overlays() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |n: &str| std::fs::read(dir.join(n)).ok();
    let (Some(game), Some(graph), Some(objects), Some(exe), Some(konung), Some(save)) = (
        read("Game.res"),
        read("GRAPH.RES"),
        read("Objects.res"),
        read("CLIENT.EXE"),
        read("konung.res"),
        read("Game.1"),
    ) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    res.set_save_game(&save);
    // Гилли (Game.1 entity 277): visual_index 1, palette 30, equipment
    // [0,0,288,289,0,0] = Нож + Кольчуга (overlays 4 and 8).
    let bare = res.human_frame_from_base(&[0u16; 6], 1, 30, 0, 0, 0);
    let dressed = res.human_frame_from_base(&[0, 0, 288, 289, 0, 0], 1, 30, 0, 0, 0);
    assert!(bare.0.w > 0 && dressed.0.w > 0, "frames must decode");
    assert_ne!(
        bare.0.data, dressed.0.data,
        "equipment must change the composited frame"
    );
}

/// Real-object check for the per-frame anchor table: the walk row must be a
/// monotonic sweep and must NOT alias the `flags` idle region or the animation
/// frame id. Gated on the original files being present.
#[test]
fn monster_anchor_uses_row_dir_frame_index() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |n: &str| std::fs::read(dir.join(n)).ok();
    let (Some(game), Some(graph), Some(objects), Some(exe)) = (
        read("Game.res"),
        read("GRAPH.RES"),
        read("Objects.res"),
        read("CLIENT.EXE"),
    ) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let res = Res::new(game, graph, objects, exe);

    // Object 0's walk row (row 1, dir 0) sweeps its gx monotonically in the
    // original table; the buggy frame-id indexing produced (0,0) mid-cycle.
    let walk: Vec<i32> = (0..12)
        .map(|fi| res.monster_anchor(0, 1, 0, fi).0)
        .collect();
    assert!(
        walk.windows(2).all(|w| w[1] < w[0]),
        "walk anchors must be a strict sweep: {walk:?}"
    );

    // Idle (row 0) and walk (row 1) are distinct table regions.
    assert_ne!(res.monster_anchor(0, 0, 0, 0), res.monster_anchor(0, 1, 0, 0));

    // The anchor must be the bytes at `0x1728 + row*0x480 + dir*0x90 + fi*8`.
    let blk = res.object_block(0);
    let off = 0x1728 + 0x480; // row 1, dir 0, fi 0
    let expected = (super::i16(blk, off + 4), super::i16(blk, off + 6));
    assert_eq!(res.monster_anchor(0, 1, 0, 0), expected);
}
