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
