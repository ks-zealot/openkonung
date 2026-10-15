use super::*;

fn collision_buffer() -> Vec<u8> {
    vec![0u8; COLLISION_OFF + COLLISION_BYTES]
}

fn cell_idx(x: usize, y: usize) -> usize {
    COLLISION_OFF + x * RUNTIME_H as usize + y
}

#[test]
fn collision_expansion_rules() {
    let mut data = collision_buffer();
    data[cell_idx(10, 20)] = 0xFF;
    data[cell_idx(5, 7)] = 0x12;
    data[cell_idx(300, 150)] = 0x1F;

    let c = parse_collision(&data).unwrap();
    assert_eq!(c[10 * RUNTIME_H as usize + 20], 0x0FFF);
    assert_eq!(c[5 * RUNTIME_H as usize + 7], 0x1200);
    assert_eq!(c[300 * RUNTIME_H as usize + 150], 0x1FFF);
}

#[test]
fn collision_decode_canonical_bytes() {
    // (input byte, expanded u16, passable).
    let cases: [(u8, u16, bool); 5] = [
        (0x00, 0x0000, true),
        // Low nibble 0x0F => (b << 8) | 0x0FFF, whose low 12 bits are 0xFFF.
        (0x0F, 0x0FFF, false),
        (0x10, 0x1000, true),
        (0x1F, 0x1FFF, false),
        (0xFF, 0x0FFF, false),
    ];

    let mut data = collision_buffer();
    for (i, &(b, _, _)) in cases.iter().enumerate() {
        data[cell_idx(i, i)] = b;
    }

    let c = parse_collision(&data).unwrap();
    for (i, &(b, expanded, passable)) in cases.iter().enumerate() {
        let got = c[i * RUNTIME_H as usize + i];
        assert_eq!(got, expanded, "byte 0x{b:02X} expanded to 0x{got:04X}");
        assert_eq!(
            collision_is_free(&c, i as i32, i as i32),
            passable,
            "byte 0x{b:02X} passability"
        );
    }
}

#[test]
fn collision_index_is_x_major_not_transposed() {
    let mut data = collision_buffer();
    data[cell_idx(10, 20)] = 0x11;
    data[cell_idx(20, 10)] = 0x22;

    let c = parse_collision(&data).unwrap();
    assert_eq!(c[10 * RUNTIME_H as usize + 20], 0x1100);
    assert_eq!(c[20 * RUNTIME_H as usize + 10], 0x2200);
}

#[test]
fn collision_truncated_file_errors() {
    let data = vec![0xFFu8; 16];
    assert!(parse_collision(&data).is_err());
}

#[test]
fn collision_free_checks_low_12_bits_and_bounds() {
    let mut data = collision_buffer();
    // 0x10 -> 0x1000: low nibble 0 => low 12 bits clear => free.
    data[cell_idx(1, 1)] = 0x10;
    // 0x20 -> 0x2000: free.
    data[cell_idx(2, 2)] = 0x20;
    // 0x00 -> 0: free.
    data[cell_idx(3, 3)] = 0x00;
    // 0x01 -> 0x0100: low 12 bits non-zero => blocked.
    data[cell_idx(4, 4)] = 0x01;
    // 0x12 -> 0x1200: blocked.
    data[cell_idx(5, 5)] = 0x12;
    // 0xFF -> 0x0FFF: blocked.
    data[cell_idx(6, 6)] = 0xFF;

    let c = parse_collision(&data).unwrap();
    assert!(collision_is_free(&c, 1, 1));
    assert!(collision_is_free(&c, 2, 2));
    assert!(collision_is_free(&c, 3, 3));
    assert!(!collision_is_free(&c, 4, 4));
    assert!(!collision_is_free(&c, 5, 5));
    assert!(!collision_is_free(&c, 6, 6));
    assert!(!collision_is_free(&c, -1, 0));
    assert!(!collision_is_free(&c, 0, -1));
    assert!(!collision_is_free(&c, RUNTIME_W, 0));
    assert!(!collision_is_free(&c, 0, RUNTIME_H));
}

#[test]
fn prop_projection_exact_cases() {
    // Source-of-truth §23.2 (grid_x = +0x20, grid_y = +0x1E).
    assert_eq!(prop_to_local_px(0, 0), (0, 0));
    assert_eq!(prop_to_local_px(1, 0), (-29, 16));
    assert_eq!(prop_to_local_px(2, 0), (0, 32));
    assert_eq!(prop_to_local_px(5, 7), (377, 80));
}

#[test]
fn terrain_layers_decode_both_bytes() {
    assert_eq!(terrain_layers(0x0201), (Some(0), Some(1)));
    assert_eq!(terrain_layers(0x0000), (None, None));
    assert_eq!(terrain_layers(0x0100), (None, Some(0)));
    assert_eq!(terrain_layers(0x0001), (Some(0), None));
    assert_eq!(terrain_layers(0xFFFF), (Some(254), Some(254)));
}

#[test]
fn graph_anchor_is_identity() {
    assert_eq!(graph_anchor_to_local_px(0, 0), (0, 0));
    assert_eq!(graph_anchor_to_local_px(1624, 320), (1624, 320));
    assert_eq!(graph_anchor_to_local_px(3654, 3646), (3654, 3646));
}

#[test]
fn terrain_lattice_exact_cases() {
    assert_eq!(terrain_to_local_px(0, 0), (0, 0));
    assert_eq!(terrain_to_local_px(1, 0), (116, 0));
    assert_eq!(terrain_to_local_px(0, 1), (58, 32));
    assert_eq!(terrain_to_local_px(1, 1), (174, 32));
    assert_eq!(terrain_to_local_px(2, 1), (2 * 116 + 58, 32));
}

#[test]
fn surface_dimensions_double_vs_half_scale() {
    // Old half-scale surface was 4725 x 2608; corrected lattice roughly
    // doubles both dimensions.
    assert!(LOCAL_PIXEL_W > 9000 && LOCAL_PIXEL_W < 9600, "W={LOCAL_PIXEL_W}");
    assert!(LOCAL_PIXEL_H > 5000 && LOCAL_PIXEL_H < 5300, "H={LOCAL_PIXEL_H}");
    // Entity maxima still fit inside the surface (source §3 / CHECK F).
    let (mx, _) = crate::coords::entity_to_local_px(0, 159);
    let (_, my) = crate::coords::entity_to_local_px(319, 0);
    assert!(mx < LOCAL_PIXEL_W, "entity max x {mx} >= {LOCAL_PIXEL_W}");
    assert!(my < LOCAL_PIXEL_H, "entity max y {my} >= {LOCAL_PIXEL_H}");
}
