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
    let bare = res.human_frame_from_base(&[0u16; 6], 0, false, 1, 30, 6, 0, 0);
    let dressed =
        res.human_frame_from_base(&[0, 0, 288, 289, 0, 0], 0, false, 1, 30, 6, 0, 0);
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

#[test]
fn battle_overlay_order_tables_match_original_dgroup() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let Ok(konung) = std::fs::read(dir.join("konung.res")) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let word = |va: u32| -> usize {
        let o = 0x36200 + (va as usize - 0x450000);
        i32::from_le_bytes([konung[o], konung[o + 1], konung[o + 2], konung[o + 3]]) as usize
    };

    for dir in 0..8 {
        for slot in 0..6 {
            let va = 0x4631FC + ((dir * 6 + slot) * 4) as u32;
            assert_eq!(
                super::HUMAN_DRAW_ORDER_PEACE[dir][slot],
                word(va),
                "peace dir {dir} slot {slot}"
            );
        }
    }
    for (name, base, table) in [
        (
            "battle A",
            0x462BFCu32,
            &super::HUMAN_DRAW_ORDER_BATTLE_A,
        ),
        (
            "battle B",
            0x462EFCu32,
            &super::HUMAN_DRAW_ORDER_BATTLE_B,
        ),
    ] {
        for bucket in 0..4 {
            for dir in 0..8 {
                for slot in 0..6 {
                    let index = (bucket * 8 + dir) * 6 + slot;
                    let va = base + (index * 4) as u32;
                    assert_eq!(
                        table[bucket][dir][slot],
                        word(va),
                        "{name} bucket {bucket} dir {dir} slot {slot}"
                    );
                }
            }
        }
    }
}

#[test]
fn overlay_plan_matches_fun_00427e9c() {
    use super::{battle_overlay_bucket, overlay_plan};

    assert_eq!(battle_overlay_bucket(0), 0);
    assert_eq!(battle_overlay_bucket(1), 3);
    assert_eq!(battle_overlay_bucket(2), 0);
    assert_eq!(battle_overlay_bucket(3), 0);
    assert_eq!(battle_overlay_bucket(4), 1);
    assert_eq!(battle_overlay_bucket(5), 2);

    let (order, variant) = overlay_plan(false, 2, 0, 5);
    assert_eq!(order, &super::HUMAN_DRAW_ORDER_PEACE[5]);
    assert!(variant.iter().all(|v| !*v));

    // Active slot 2 in battle: order A, slots 2 and 5 get the +1 variant.
    let (order, variant) = overlay_plan(true, 2, 0, 5);
    assert_eq!(order, &super::HUMAN_DRAW_ORDER_BATTLE_A[0][5]);
    assert!(variant[2] && variant[5]);
    assert_eq!(variant.iter().filter(|v| **v).count(), 2);

    // Active slot 1, action 1 (bucket 3): order B, only slot 1 gets +1.
    let (order, variant) = overlay_plan(true, 1, 1, 3);
    assert_eq!(order, &super::HUMAN_DRAW_ORDER_BATTLE_B[3][3]);
    assert!(variant[1] && !variant[5]);

    // Active slot 1, action 5 (bucket 2): order A, no slot-1 variant, slot 5 +1.
    let (order, variant) = overlay_plan(true, 1, 5, 3);
    assert_eq!(order, &super::HUMAN_DRAW_ORDER_BATTLE_A[2][3]);
    assert!(!variant[1] && variant[5]);
}

#[test]
fn battle_stance_uses_active_weapon_variant_and_paper_doll() {
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

    // Dragomir (Game.1 entity 0): visual 1, palette 33, knife id 1 in slot 2,
    // active weapon slot 2. In battle the knife must switch from overlay 4
    // (sheathed) to overlay 5 (in hand).
    let equipment = [0u16, 0, 1, 0, 0, 0];
    let no_variant = res.human_frame_from_base(&equipment, 0, true, 1, 33, 0, 5, 0);
    let held = res.human_frame_from_base(&equipment, 2, true, 1, 33, 0, 5, 0);
    assert!(no_variant.0.w > 0 && held.0.w > 0);
    assert_ne!(
        no_variant.0.data, held.0.data,
        "the active weapon slot must select the battle overlay variant"
    );

    let doll = res
        .paper_doll_frame(&equipment, 2, true, 1, 33, 0, 5, 0)
        .expect("paper doll frame");
    assert_eq!((doll.w, doll.h), (super::PAPER_DOLL_W, super::PAPER_DOLL_H));
    assert!(doll.data.chunks(4).any(|p| p[3] != 0));
    let doll_no_variant = res
        .paper_doll_frame(&equipment, 0, true, 1, 33, 0, 5, 0)
        .expect("paper doll frame");
    assert_ne!(doll.data, doll_no_variant.data);
}

#[test]
fn paper_doll_peace_frame_is_nonempty() {
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
    let equipment = [0u16, 0, 1, 0, 0, 0];
    for action in [0usize, 6] {
        for direction in 0..8u8 {
            let doll = res
                .paper_doll_frame(&equipment, 2, action == 0, 1, 33, action, direction as usize, 0)
                .expect("doll");
            let opaque = doll.data.chunks(4).filter(|p| p[3] != 0).count();
            assert!(
                opaque > 100,
                "action {action} dir {direction} produced only {opaque} opaque pixels"
            );
        }
    }
}
