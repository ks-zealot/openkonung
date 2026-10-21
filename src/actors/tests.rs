//! Unit tests for the [`crate::actors`] module.

use super::*;
use crate::population::{Entity, EntityStats, Group};

/// Inclusive rectangle containment in the runtime/Entity grid. Used by tests;
/// the production wander path relies on `wander_target`'s strict-interior rule.
fn rect_contains(rect: &Rect, c: (i32, i32)) -> bool {
    let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
    let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
    c.0 >= x1 && c.0 <= x2 && c.1 >= y1 && c.1 <= y2
}

fn group(index: usize, first: u16, count: u8, loc: u8, class: u8, rect_a: Rect) -> Group {
    Group {
        index,
        first_entity: first,
        rect_a,
        rect_b: Rect::default(),
        count_18: count,
        capacity: count,
        loc_id: loc,
        entity_count: count,
        class_1e: class,
    }
}

fn entity(index: usize, kind: u8, archetype: u8, x: u16, y: u16) -> Entity {
    Entity {
        index,
        kind,
        facing: 3,
        flags_a: 0,
        flags_b: 0,
        archetype,
        group_id: 0,
        raw_0x12: 0,
        x,
        y,
        stats: EntityStats::default(),
        equipment: [0; 6],
        inventory: [0; 32],
        dialogue_id: 255,
        name_id: 0,
        nickname_id: 0,
        visual_index: 0,
        palette_offset: 0,
        additional_equipment: [0; 5],
        special_equipment: 0,
    }
}

fn tables(groups: Vec<Group>, entities: Vec<Entity>) -> GameTables {
    GameTables { groups, entities }
}

fn free_collision() -> Vec<u16> {
    vec![0u16; 320 * 160]
}

#[test]
fn spawn_center_half_matches_original_formula() {
    // Real g27 rect A.
    let g = group(1, 0, 0, 1, 65, Rect { x1: 36, x2: 136, y1: 24, y2: 50 });
    assert_eq!(spawn_center_half(&g.rect_a), (86, 37, 50, 13));
    // Degenerate rect: half < 2 => candidate is exactly the center.
    let g2 = group(1, 0, 0, 1, 65, Rect { x1: 134, x2: 134, y1: 21, y2: 21 });
    assert_eq!(spawn_center_half(&g2.rect_a), (134, 21, 0, 0));
    let mut rng = location_rng(1);
    assert_eq!(generate_spawn_candidate(&g2, &mut rng), RawSpawnCoord { x: 134, y: 21 });
}

#[test]
fn candidates_stay_within_center_and_half() {
    let g = group(1, 0, 0, 1, 65, Rect { x1: 36, x2: 136, y1: 24, y2: 50 });
    let (cx, cy, hw, hh) = spawn_center_half(&g.rect_a);
    let mut rng = location_rng(7);
    for _ in 0..1000 {
        let c = generate_spawn_candidate(&g, &mut rng);
        assert!(c.x >= cx - hw && c.x <= cx + hw, "x {} out of range", c.x);
        assert!(c.y >= cy - hh && c.y <= cy + hh, "y {} out of range", c.y);
    }
}

#[test]
fn collision_validator_accepts_first_free_candidate() {
    let g = group(1, 0, 0, 1, 65, Rect { x1: 10, x2: 30, y1: 4, y2: 8 });
    let free = free_collision();
    let mut rng = location_rng(1);
    let (raw, attempts) = place_entity(&g, &mut rng, &free).unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(raw, generate_spawn_candidate(&g, &mut location_rng(1)));
}

#[test]
fn collision_validator_bounds_and_index() {
    let mut c = free_collision();
    // x=268 is a valid runtime X (source §3/§23.4).
    assert!(validate_spawn_candidate(268, 5, &c));
    assert!(validate_spawn_candidate(319, 159, &c));
    assert!(!validate_spawn_candidate(320, 0, &c));
    assert!(!validate_spawn_candidate(0, 160, &c));
    assert!(!validate_spawn_candidate(-1, 0, &c));
    assert!(!validate_spawn_candidate(0, -1, &c));
    // index = x*160 + y.
    c[268 * 160 + 5] = 0x0001; // low 12 bits non-zero => blocked
    assert!(!validate_spawn_candidate(268, 5, &c));
    c[268 * 160 + 5] = 0x1000; // high bits only => free
    assert!(validate_spawn_candidate(268, 5, &c));
    c[319 * 160 + 159] = 0xFF;
    assert!(!validate_spawn_candidate(319, 159, &c));
}

#[test]
fn placement_fails_when_rect_area_is_blocked() {
    let g = group(1, 0, 0, 1, 65, Rect { x1: 0, x2: 40, y1: 0, y2: 40 });
    let mut blocked = free_collision();
    for cell in blocked.iter_mut() {
        *cell = 0x0001;
    }
    let mut rng = location_rng(3);
    assert!(place_entity(&g, &mut rng, &blocked).is_err());
}

#[test]
fn place_with_fails_after_exactly_100_attempts() {
    let g = group(1, 0, 0, 1, 65, Rect { x1: 0, x2: 40, y1: 0, y2: 40 });
    let mut rng = location_rng(3);
    let err = place_with(&g, &mut rng, |_| false).unwrap_err();
    assert_eq!(err, PlacementError::NoCandidateAfterAttempts { attempts: 100 });
    assert_eq!(err.to_string(), "placement_failed_after_100_attempts");
}

#[test]
fn skips_group_zero_and_inactive_groups() {
    let groups = vec![
        group(0, 0, 1, 1, 32, Rect::default()), // reserved
        group(1, 1, 1, 1, 0, Rect::default()),  // inactive
        group(2, 2, 1, 1, 65, Rect::default()), // active
    ];
    let entities = vec![
        entity(0, 6, 0x00, 100, 60), // player start: never an actor
        entity(1, 6, 0x00, 5, 6),
        entity(2, 6, 0x00, 7, 8),
    ];
    let t = tables(groups, entities);
    let b = build_population(&t, 1, &free_collision());
    assert_eq!(b.matching_groups, 3);
    assert_eq!(b.reserved_group_zero, 1);
    assert_eq!(b.inactive_groups, 1);
    assert_eq!(b.enumerated_entities, 1);
    assert_eq!(b.actors.len(), 1);
    assert_eq!(b.actors[0].source_entity_id(), 2);
}

#[test]
fn enumerates_first_plus_count_not_capacity_nor_one_per_group() {
    let mut g = group(1, 10, 3, 1, 65, Rect { x1: 0, x2: 8, y1: 0, y2: 8 });
    g.capacity = 99;
    g.count_18 = 99;
    let mut entities: Vec<Entity> = (0..14).map(|i| entity(i, 0, 0, 1, 1)).collect();
    entities[13] = entity(13, 0, 0, 4, 4);
    let t = tables(vec![g], entities);
    let b = build_population(&t, 1, &free_collision());
    assert_eq!(b.enumerated_entities, 3);
    assert_eq!(b.persisted_position_actors(), 3);
    let ids: Vec<usize> = b.actors.iter().map(|a| a.source_entity_id()).collect();
    assert_eq!(ids, vec![10, 11, 12]);
}

#[test]
fn zero_position_is_runtime_placed_and_source_unchanged() {
    let rect = Rect { x1: 10, x2: 30, y1: 4, y2: 8 };
    let groups = vec![group(1, 0, 2, 1, 65, rect)];
    let entities = vec![entity(0, 0, 0x4F, 0, 0), entity(1, 0, 0x4F, 0, 0)];
    let t = tables(groups, entities);

    let b = build_population(&t, 1, &free_collision());
    assert_eq!(b.runtime_candidates, 2);
    assert_eq!(b.runtime_placed, 2);
    assert_eq!(b.runtime_failed, 0);
    assert!(b.failed.is_empty());
    assert_eq!(b.enumerated_entities, 2);
    assert_eq!(b.runtime_positioned(), 2);

    for a in &b.actors {
        assert!(a.is_runtime());
        assert_eq!(a.attempts(), Some(1));
        // Original parsed fields are preserved untouched.
        assert_eq!((a.entity.x, a.entity.y), (0, 0));
        let (rx, ry) = a.display_raw();
        let (cx, cy, hw, hh) = (20, 6, 10, 2);
        assert!(rx >= (cx - hw) as f64 && rx <= (cx + hw) as f64);
        assert!(ry >= (cy - hh) as f64 && ry <= (cy + hh) as f64);
    }
}

#[test]
fn deterministic_repeatability_per_location() {
    let rect = Rect { x1: 10, x2: 60, y1: 5, y2: 25 };
    let groups = vec![group(1, 0, 6, 1, 65, rect)];
    let entities: Vec<Entity> = (0..6).map(|i| entity(i, 0, 0x4F, 0, 0)).collect();
    let t = tables(groups, entities);

    let a = build_population(&t, 1, &free_collision());
    let b = build_population(&t, 1, &free_collision());
    let coords = |p: &PopulationBuild| {
        p.actors.iter().map(|x| x.display_raw()).collect::<Vec<_>>()
    };
    assert_eq!(coords(&a), coords(&b));
}

#[test]
fn family_is_selected_by_archetype_bit_not_kind() {
    assert_eq!(visual_family(0x52), VisualFamily::Object);
    assert_eq!(visual_family(0x01), VisualFamily::Human);
    assert_eq!(visual_family(0x40), VisualFamily::Object);
    assert_eq!(visual_family(0x3F), VisualFamily::Human);
}

#[test]
fn projected_sort_key_uses_entity_projection() {
    let a = RuntimeActor {
        entity: entity(1, 0, 0, 5, 10),
        source_group_id: 1,
        placement: ActorPlacement::Persisted,
    };
    let b = RuntimeActor {
        entity: entity(1, 0, 0, 200, 1),
        ..a.clone()
    };
    // local_x = y*58 + (2-(x&1))*29, local_y = x*16+16.
    assert_eq!(
        projected_sort_key(&a),
        (a.entity.x as i64 * 16 + 16, 10 * 58 + 29)
    );
    assert_eq!(
        projected_sort_key(&b),
        (b.entity.x as i64 * 16 + 16, 1 * 58 + 58)
    );

    let mut actors = vec![b.clone(), a.clone()];
    sort_actors(&mut actors);
    assert_eq!(actors[0].entity.x, 5);
    assert_eq!(actors[1].entity.x, 200);
}

#[test]
fn invalid_range_is_counted_not_silently_dropped() {
    let groups = vec![group(1, 1999, 5, 1, 65, Rect::default())];
    let entities: Vec<Entity> = (0..2000).map(|i| entity(i, 0, 0, 0, 0)).collect();
    let t = tables(groups, entities);
    let b = build_population(&t, 1, &free_collision());
    assert_eq!(b.invalid_ranges, 1);
    assert_eq!(b.enumerated_entities, 0);
}

#[test]
fn wanders_classification_matches_dispatch_gate() {
    let mut e = entity(0, 6, 0x05, 10, 10);
    e.flags_a = FLAG_WANDER;
    assert!(wanders(&e));
    e.flags_a = 0x00;
    assert!(!wanders(&e));
    e.flags_a = FLAG_WANDER;
    e.archetype = 0x85; // bit 0x80 set -> not civilian AI
    assert!(!wanders(&e));
    e.archetype = 0x05;
    e.kind = 3;
    assert!(!wanders(&e));
    e.kind = 6;
    assert!(wanders(&e));
}

#[test]
fn wander_target_stays_strictly_inside_rect_and_free() {
    // Rect large enough for the original 800x600 radii (x=20, y=9 => offsets up
    // to 2*radius-1), so a candidate is always found.
    let rect = Rect { x1: 0, x2: 120, y1: 0, y2: 60 };
    let free = free_collision();
    let mut rng = location_rng(1);
    for _ in 0..500 {
        let t = wander_target(&rect, (60, 30), &mut rng, &free).expect("candidate");
        assert!(t.0 > rect.x1 as i32 && t.0 < rect.x2 as i32, "x {t:?}");
        assert!(t.1 > rect.y1 as i32 && t.1 < rect.y2 as i32, "y {t:?}");
        assert!(is_free(&free, t.0, t.1));
    }
}

#[test]
fn wander_target_none_when_rect_blocked() {
    let rect = Rect { x1: 0, x2: 40, y1: 0, y2: 40 };
    let mut blocked = free_collision();
    for c in blocked.iter_mut() {
        *c = 0x0001;
    }
    let mut rng = location_rng(3);
    assert!(wander_target(&rect, (20, 20), &mut rng, &blocked).is_none());
}

#[test]
fn find_path_routes_around_obstacles() {
    let mut c = free_collision();
    // Vertical wall at x=5 for y 0..=9, gap at y>=10.
    for y in 0..10 {
        c[5 * 160 + y] = 0x0001;
    }
    let occ = std::collections::HashSet::new();
    let start = (0, 0);
    let goal = (10, 0);
    let dirs = find_path(&c, &occ, start, goal, (0, 0, 20, 20)).expect("path");
    assert!(!dirs.is_empty());
    // Replay the direction codes and verify the goal is reached without
    // crossing the wall or stepping onto a blocked cell.
    let mut cell = start;
    for &d in &dirs {
        let next = step_by_direction(cell.0, cell.1, d);
        assert!(!(next.0 == 5 && next.1 < 10), "path crosses the wall at {next:?}");
        assert!(is_free(&c, next.0, next.1), "path steps onto blocked {next:?}");
        cell = next;
    }
    assert_eq!(cell, goal);
}

#[test]
fn find_path_rejects_blocked_goal() {
    // The destination must be an "acceptable" (passable) point; the original
    // aborts the command for a blocked click.
    let mut c = free_collision();
    c[10 * 160 + 0] = 0x0001;
    let occ = std::collections::HashSet::new();
    assert!(find_path(&c, &occ, (0, 0), (10, 0), (0, 0, 20, 20)).is_none());
}

#[test]
fn dup_column_and_row_walks_keep_constant_direction() {
    let c = free_collision();
    let occ = std::collections::HashSet::new();
    // Column move (vary x) is a straight screen line: one constant code.
    let col = find_path(&c, &occ, (0, 0), (6, 0), (0, 0, 20, 20)).expect("col");
    assert!(!col.is_empty());
    assert!(col.iter().all(|&d| d == col[0]), "column zigzags: {col:?}");
    // Row move (vary y) likewise.
    let row = find_path(&c, &occ, (0, 0), (0, 4), (0, 0, 20, 20)).expect("row");
    assert!(!row.is_empty());
    assert!(row.iter().all(|&d| d == row[0]), "row zigzags: {row:?}");
}

#[test]
fn update_population_moves_only_wanderers() {
    let rect = Rect { x1: 10, x2: 60, y1: 5, y2: 25 };
    let groups = vec![group(1, 0, 2, 1, 0x81, rect)];
    let mut citizen = entity(0, 6, 0x05, 30, 15);
    citizen.flags_a = FLAG_WANDER;
    let mob = entity(1, 6, 0x05, 40, 15); // flags_a 0 -> stays put
    let t = tables(groups, vec![citizen, mob]);
    let collision = free_collision();
    let mut b = build_population(&t, 1, &collision);

    let start_c = b.motion[&0].cell;
    let start_m = b.motion[&1].cell;
    for _ in 0..600 {
        update_population(&mut b, &collision, None, 1.0 / LOGIC_TICK_HZ);
    }
    assert_ne!(b.motion[&0].cell, start_c, "citizen must wander");
    assert_eq!(b.motion[&1].cell, start_m, "mob must stay at spawn");
    assert!(rect_contains(&rect, b.motion[&0].cell));
}

#[test]
fn object_group_mobs_hold_position() {
    // Group class `0x41` (bit 0x40) selects the object/enemy AI `FUN_00412e00`,
    // which idles in place while `Group+0x1D == 0`. Such actors must not wander
    // even though every active-population entity carries `flags_a = 0x40`.
    let rect = Rect { x1: 30, x2: 70, y1: 8, y2: 24 };
    let groups = vec![group(1, 0, 1, 1, 0x41, rect)];
    let mut mob = entity(0, 6, 0x41, 50, 16);
    mob.flags_a = FLAG_WANDER;
    let t = tables(groups, vec![mob]);
    let collision = free_collision();
    let mut b = build_population(&t, 1, &collision);
    assert!(!b.motion[&0].citizen, "class 0x41 is not a citizen group");

    let start = b.motion[&0].cell;
    for _ in 0..900 {
        update_population(&mut b, &collision, None, 1.0 / LOGIC_TICK_HZ);
    }
    assert_eq!(b.motion[&0].cell, start, "object-group mob must stay put");
    assert_eq!(b.motion[&0].frame, 0, "idle frame must be held");
}

#[test]
fn citizen_group_wanders() {
    // Group class `0x81` (bit 0x80, no 0x40) selects `FUN_004168a4`.
    let rect = Rect { x1: 30, x2: 70, y1: 8, y2: 24 };
    let groups = vec![group(1, 0, 1, 1, 0x81, rect)];
    let mut c = entity(0, 6, 0x05, 50, 16);
    c.flags_a = FLAG_WANDER;
    let t = tables(groups, vec![c]);
    let collision = free_collision();
    let mut b = build_population(&t, 1, &collision);
    assert!(b.motion[&0].citizen);

    let start = b.motion[&0].cell;
    for _ in 0..900 {
        update_population(&mut b, &collision, None, 1.0 / LOGIC_TICK_HZ);
    }
    assert_ne!(b.motion[&0].cell, start, "citizen group member must wander");
    assert!(rect_contains(&rect, b.motion[&0].cell));
}

#[test]
fn idle_wanderer_holds_its_frame_not_loops() {
    use crate::population::{Settlement, Waypoint};
    let rect = Rect { x1: 40, x2: 60, y1: 10, y2: 30 };
    // Entity index 1 owns the home waypoint (owner 0 means "unassigned").
    let groups = vec![group(1, 1, 1, 1, 0x81, rect)];
    let mut mob = entity(1, 6, 0x45, 50, 20);
    mob.flags_a = FLAG_WANDER;
    let t = tables(groups, vec![entity(0, 6, 0x45, 0, 0), mob]);
    let collision = free_collision();
    let mut b = build_population(&t, 1, &collision);
    // Home waypoint owned by entity 1 at its own cell -> the actor idles there.
    let settlement = Settlement {
        index: 0,
        loc_id: 1,
        group_id: 0,
        flags_4b2: 0,
        targets: Vec::new(),
        waypoints: vec![Waypoint {
            facing: 3,
            owner: 1,
            x: 50,
            y: 20,
        }],
    };
    for _ in 0..120 {
        update_population(&mut b, &collision, Some(&settlement), 1.0 / LOGIC_TICK_HZ);
    }
    let m = &b.motion[&1];
    assert_eq!(m.cell, (50, 20), "home owner must stay at its waypoint");
    assert_eq!(m.frame, 0, "idle frame must be held, not looped");
    assert_eq!(m.action, crate::anim::idle_action(false) as u8);
}

#[test]
fn actor_commits_the_first_step_at_segment_start() {
    // A wandering citizen: the first cell of every route is committed at the
    // START of its segment (`FUN_0041435C`), so the stored cell leads the drawn
    // sprite (`render_cell`) by one step.
    let rect = Rect { x1: 30, x2: 70, y1: 8, y2: 24 };
    let groups = vec![group(1, 0, 1, 1, 0x81, rect)];
    let mut c = entity(0, 6, 0x05, 50, 16);
    c.flags_a = FLAG_WANDER;
    let t = tables(groups, vec![c]);
    let collision = free_collision();
    let mut b = build_population(&t, 1, &collision);

    // Idle at spawn: no segment in flight, draw cell == stored cell.
    assert!(!b.motion[&0].walking);
    assert_eq!(b.motion[&0].render_cell, b.motion[&0].cell);
    let start = b.motion[&0].cell;

    let mut issued = false;
    for _ in 0..600 {
        update_population(&mut b, &collision, None, 1.0 / LOGIC_TICK_HZ);
        let m = &b.motion[&0];
        if m.walking {
            // The first committed step advanced the stored cell, while the draw
            // cell still holds the pre-step (spawn) cell.
            assert_eq!(m.render_cell, start, "draw cell is the pre-step cell");
            assert_ne!(m.cell, start, "stored cell leads the sprite by one step");
            assert_eq!(m.cell, step_by_direction(start.0, start.1, m.facing));
            issued = true;
            break;
        }
    }
    assert!(issued, "citizen should begin walking");
}

#[test]
fn real_game1_group23_owners_stay_non_owners_wander() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let (Ok(game), Ok(k4)) = (
        std::fs::read(dir.join("Game.1")),
        std::fs::read(dir.join("Konung.4")),
    ) else {
        eprintln!("skipping: Game.1 / Konung.4 not present");
        return;
    };
    let tables = crate::population::parse_tables(&game).unwrap();
    let settlements = crate::population::parse_settlements(&game).unwrap();
    let collision = crate::loc::parse_collision(&k4).unwrap();
    let settlement = settlements
        .iter()
        .find(|s| s.loc_id == 4)
        .expect("loc 4 settlement");

    // Home waypoint owners (`FUN_0041182C`, first five slots).
    let owner_home: HashMap<usize, (i32, i32)> = settlement
        .waypoints
        .iter()
        .take(5)
        .filter(|w| w.owner != 0)
        .map(|w| (w.owner as usize, (w.x as i32, w.y as i32)))
        .collect();
    assert!(
        owner_home.contains_key(&277),
        "Гилли (277) must own a loc-4 home waypoint"
    );

    let mut b = build_population(&tables, 4, &collision);
    let movers: Vec<usize> = b
        .actors
        .iter()
        .filter(|a| wanders(&a.entity) && !owner_home.contains_key(&a.source_entity_id()))
        .map(|a| a.source_entity_id())
        .collect();
    assert!(!movers.is_empty(), "loc 4 must have wandering non-owners");
    let mover_starts: Vec<(i32, i32)> = movers.iter().map(|id| b.motion[id].cell).collect();

    for _ in 0..3600 {
        update_population(&mut b, &collision, Some(settlement), 1.0 / LOGIC_TICK_HZ);
    }

    // Waypoint owners stay exactly at their home waypoint.
    for (id, home) in &owner_home {
        if let Some(m) = b.motion.get(id) {
            assert_eq!(m.cell, *home, "home-waypoint owner {id} must stay put");
        }
    }
    // Non-owners wander within rect A.
    let moved = movers
        .iter()
        .zip(&mover_starts)
        .filter(|(id, s)| b.motion[id].cell != **s)
        .count();
    assert!(moved > 0, "non-owner citizens must wander");
    for id in &movers {
        let m = &b.motion[id];
        assert!(rect_contains(&m.rect_a, m.cell), "left rect A: {id} {:?}", m.cell);
    }
}

#[test]
fn follow_distance_matches_original_grid_metric() {
    // FUN_0043881c
    assert_eq!(follow_distance((0, 0), (0, 0)), 0);
    assert_eq!(follow_distance((0, 0), (9, 0)), 9);
    assert_eq!(follow_distance((0, 0), (10, 0)), 10);
    assert_eq!(follow_distance((0, 0), (3, 5)), 6);
    assert_eq!(follow_distance((0, 0), (1, 5)), 5);
}

#[test]
fn followers_walk_toward_a_distant_leader_and_stop_when_close() {
    let collision = free_collision();
    let leader = (100, 80);
    let mut member = PartyMember {
        entity: entity(539, 6, 0x05, 80, 80),
        motion: ActorMotion::default(),
    };
    member.motion.cell = (80, 80);
    member.motion.render_cell = (80, 80);
    member.motion.facing = 5;
    member.motion.walk_frames = 4;
    member.motion.action = 0;

    // 20 cells away (>= FOLLOW_DISTANCE): issues a route and starts walking.
    update_followers(std::slice::from_mut(&mut member), &collision, leader, 1);
    assert!(member.motion.walking, "companion starts following");
    let start = member.motion.cell;

    for _ in 0..600 {
        update_followers(std::slice::from_mut(&mut member), &collision, leader, 1);
    }
    assert_ne!(member.motion.cell, start, "companion moved toward the leader");
    assert!(
        follow_distance(member.motion.cell, leader) < FOLLOW_DISTANCE,
        "companion arrived within follow distance (cell {:?})",
        member.motion.cell
    );
    assert!(!member.motion.walking, "companion settled");
}
