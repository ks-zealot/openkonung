//! Portal/gate matching tests.

use super::super::*;


// Konung.4, Game.1: gates at raw averaged 244/245 grid centers, in gate-index order.
const K4_GATES: [(f64, f64); 4] =
    [(123.0, 61.5), (115.5, 5.0), (16.0, 6.5), (66.0, 65.0)];
// Outgoing edges in (rec, to) order: rec10 4->5, rec11 4->15, rec12 4->30, rec13 4->32.
const K4_TRIGGERS: [(i32, i32, i32, i32); 4] =
    [(63, 64, 63, 64), (119, 61, 119, 61), (112, 4, 112, 4), (12, 5, 12, 5)];
const K4_RECS: [usize; 4] = [10, 11, 12, 13];

#[test]
fn konung4_maps_each_trigger_to_its_gate() {
    let a = match_triggers_to_gates(&K4_TRIGGERS, &K4_RECS, &K4_GATES, MAX_GATE_TRIGGER_DIST);
    // rec10 4->5 -> gate3, rec11 4->15 -> gate0, rec12 4->30 -> gate1, rec13 4->32 -> gate2
    assert_eq!(a, vec![Some(3), Some(0), Some(1), Some(2)]);
}

#[test]
fn global_matching_beats_greedy_rec_order() {
    // g0 near e1; e0 is nearer to g1 than greedy (rec order) would allow by
    // taking g0 first and stranding e1.
    let gates = [(0.0, 0.0), (10.0, 0.0)];
    let triggers = [(4, 0, 4, 0), (1, 0, 1, 0)]; // e0 rec0, e1 rec1
    let recs = [0usize, 1];
    let a = match_triggers_to_gates(&triggers, &recs, &gates, 6.0);
    // Global optimum binds both: e0->g1 (6.0), e1->g0 (1.0).
    assert_eq!(a, vec![Some(1), Some(0)]);
}

#[test]
fn threshold_leaves_distant_trigger_unbound() {
    let gates = [(0.0, 0.0)];
    let triggers = [(50, 50, 50, 50)];
    let recs = [7usize];
    assert_eq!(match_triggers_to_gates(&triggers, &recs, &gates, MAX_GATE_TRIGGER_DIST), vec![None]);
}

#[test]
fn point_and_rectangular_trigger_centers() {
    assert_eq!(trigger_center(12, 5, 12, 5), [12.0, 5.0]);
    assert_eq!(trigger_center(136, 10, 146, 14), [141.0, 12.0]);
    assert_eq!(trigger_center(14, 7, 12, 5), [13.0, 6.0]);
}

#[test]
fn dist_point_to_rect_handles_points_and_ranges() {
    // Point trigger.
    assert_eq!(dist_point_to_rect(12.0, 5.0, 12, 5, 12, 5), 0.0);
    assert_eq!(dist_point_to_rect(15.0, 9.0, 12, 5, 12, 5), 5.0);
    // Inside a normalized range.
    assert_eq!(dist_point_to_rect(12.0, 5.0, 14, 7, 0, 0), 0.0);
    // Nearest corner when outside; unnormalized corners are accepted.
    assert_eq!(dist_point_to_rect(0.0, 0.0, 14, 7, 12, 5), 13.0);
}

#[test]
fn tie_break_is_deterministic_by_rec_then_gate() {
    // Two identical gates, two coincident triggers of equal cost: the winner
    // must be the lexicographically smallest (rec, gate) pairing.
    let gates = [(0.0, 0.0), (0.0, 0.0)];
    let triggers = [(0, 0, 0, 0), (0, 0, 0, 0)];
    let recs = [5usize, 3]; // local 0 = rec5, local 1 = rec3
    let a = match_triggers_to_gates(&triggers, &recs, &gates, 1.0);
    // rec3 -> gate0, rec5 -> gate1.
    assert_eq!(a, vec![Some(1), Some(0)]);
}

