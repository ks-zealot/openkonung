//! Unit tests for the [`crate::edges`] module.

use super::*;

fn edge(src: [i32; 2], dst: [i32; 2], dst2: [i32; 2]) -> Edge {
    Edge {
        rec: 0,
        meta: 0,
        from: 4,
        to: 32,
        src,
        dst,
        dst2,
        side: "NW",
        profiles: vec![0, 1, 2],
    }
}

#[test]
fn read_transitions_parses_canonical_fields() {
    let mut data = vec![0u8; GAME_SIZE];
    let o = TRANS_OFF;
    // record 0: meta=3, src=4, dst=32,
    // +05/+07 arrival=(196,8), +09/+0B=(12,5), +0D/+0F=(12,5)
    data[o + 2] = 3;
    data[o + 3] = 4;
    data[o + 4] = 32;
    data[o + 5] = 196;
    data[o + 6] = 0;
    data[o + 7] = 8;
    data[o + 8] = 0;
    data[o + 9] = 12;
    data[o + 10] = 0;
    data[o + 11] = 5;
    data[o + 12] = 0;
    data[o + 13] = 12;
    data[o + 14] = 0;
    data[o + 15] = 5;
    data[o + 16] = 0;

    let ts = read_transitions(&data);
    assert_eq!(ts.len(), 1);
    let t = &ts[0];
    assert_eq!((t.rec, t.meta, t.src, t.dst), (0, 3, 4, 32));
    assert_eq!((t.sx, t.sy), (196, 8), "arrival +05/+07");
    assert_eq!((t.dx, t.dy), (12, 5), "trigger corner 1 +09/+0B");
    assert_eq!((t.dx2, t.dy2), (12, 5), "trigger corner 2 +0D/+0F");
}

#[test]
fn read_transitions_rejects_wrong_size() {
    assert!(read_transitions(&vec![0u8; 16]).is_empty());
}

#[test]
fn arrival_is_plus05_plus07() {
    let e = edge([196, 8], [12, 5], [12, 5]);
    assert_eq!(e.arrival(), [196, 8]);
}

#[test]
fn trigger_rect_normalizes_corners() {
    // Reversed corners must yield the same normalized rectangle.
    let a = edge([0, 0], [12, 5], [40, 30]);
    let b = edge([0, 0], [40, 30], [12, 5]);
    assert_eq!(a.trigger_rect(), ([12, 5], [40, 30]));
    assert_eq!(b.trigger_rect(), ([12, 5], [40, 30]));
}

#[test]
fn trigger_contains_boundaries_and_outside() {
    let e = edge([0, 0], [12, 5], [14, 7]);
    // inclusive corners
    assert!(e.trigger_contains(12, 5));
    assert!(e.trigger_contains(14, 7));
    assert!(e.trigger_contains(13, 6));
    // just outside
    assert!(!e.trigger_contains(11, 6));
    assert!(!e.trigger_contains(13, 8));
}

#[test]
fn point_trigger_matches_only_its_cell() {
    // Most Game.N records store a single-point trigger rect.
    let e = edge([196, 8], [12, 5], [12, 5]);
    assert!(e.trigger_contains(12, 5));
    assert!(!e.trigger_contains(12, 6));
    assert!(!e.trigger_contains(13, 5));
}
