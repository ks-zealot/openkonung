//! Motion/formation and shipped-data tests.

use super::super::*;

use super::common::*;

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
