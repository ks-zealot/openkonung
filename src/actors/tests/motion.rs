//! Motion/formation and shipped-data tests.

use super::super::*;

use super::common::*;

/// On the shipped `Game.1`/`Konung.4` data, loc-4 home-waypoint owners (e.g. the
/// waypoint owner Entity 277) stay put while their unowned group-mates wander
/// inside rectangle A.
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
    let mut collision = crate::loc::parse_collision(&k4).unwrap();
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

    let mut b = build_population(&tables, 4, &mut collision);
    let movers: Vec<usize> = b
        .actors
        .iter()
        .filter(|a| wanders(&a.entity) && !owner_home.contains_key(&a.source_entity_id()))
        .map(|a| a.source_entity_id())
        .collect();
    assert!(!movers.is_empty(), "loc 4 must have wandering non-owners");
    let mover_starts: Vec<(i32, i32)> = movers.iter().map(|id| b.motion[id].cell).collect();

    for _ in 0..3600 {
        update_population(
            &mut b,
            &mut collision,
            Some(settlement),
            1.0 / LOGIC_TICK_HZ,
        );
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
        assert!(
            rect_contains(&m.rect_a, m.cell),
            "left rect A: {id} {:?}",
            m.cell
        );
    }
}

/// `FUN_0043881C` is a Chebyshev distance with a `+1` on the larger axis when
/// the smaller axis exceeds 1.
#[test]
fn follow_distance_matches_original_grid_metric() {
    // FUN_0043881c
    assert_eq!(follow_distance((0, 0), (0, 0)), 0);
    assert_eq!(follow_distance((0, 0), (9, 0)), 9);
    assert_eq!(follow_distance((0, 0), (10, 0)), 10);
    assert_eq!(follow_distance((0, 0), (3, 5)), 6);
    assert_eq!(follow_distance((0, 0), (1, 5)), 5);
}

/// A companion at least `FOLLOW_DISTANCE` cells away issues a route and settles
/// once it is closer than that; a follower nearer than the threshold never
/// starts walking.
#[test]
fn followers_walk_toward_a_distant_leader_and_stop_when_close() {
    let mut collision = free_collision();
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
    let mut rng = crate::combat::OriginalRng::new(1);
    update_followers(
        std::slice::from_mut(&mut member),
        &mut collision,
        leader,
        1,
        &mut rng,
    );
    assert!(member.motion.walking, "companion starts following");
    let start = member.motion.cell;

    for _ in 0..600 {
        update_followers(
            std::slice::from_mut(&mut member),
            &mut collision,
            leader,
            1,
            &mut rng,
        );
    }
    assert_ne!(
        member.motion.cell, start,
        "companion moved toward the leader"
    );
    assert!(
        follow_distance(member.motion.cell, leader) < FOLLOW_DISTANCE,
        "companion arrived within follow distance (cell {:?})",
        member.motion.cell
    );
    assert!(!member.motion.walking, "companion settled");
}

/// `0x45FF1C` formation table: 2 parity blocks x 8 directions x 12 slots.
#[test]
fn follow_slots_match_the_authored_table() {
    use crate::constants::motion::FOLLOW_SLOTS;

    assert_eq!(FOLLOW_SLOTS.len(), 2);
    assert_eq!(FOLLOW_SLOTS[0].len(), 8);
    assert_eq!(FOLLOW_SLOTS[0][0].len(), 12);
    // Spot checks against the recovered bytes at `0x45FF1C`.
    assert_eq!(FOLLOW_SLOTS[0][4][0], (1, 1));
    assert_eq!(FOLLOW_SLOTS[0][4][11], (1, 4));
    assert_eq!(FOLLOW_SLOTS[1][1][4], (-6, 0));
    // Every slot is a non-zero offset inside the authored 1..7 cell reach.
    for block in &FOLLOW_SLOTS {
        for row in block {
            for &(dx, dy) in row {
                assert!(dx != 0 || dy != 0, "empty slot");
                assert!(dx.abs() <= 7 && dy.abs() <= 7, "slot ({dx},{dy})");
            }
        }
    }
}

/// `FUN_0041f3c8` consumes exactly one `rand() % 12` draw per attempt and takes
/// the first free slot from that start index.
#[test]
fn follow_goal_draws_a_random_start_slot() {
    use crate::combat::OriginalRng;
    use crate::constants::motion::FOLLOW_SLOTS;
    use crate::coords::direction_from_cells;

    let collision = free_collision();
    let leader = (100, 80);
    let member = (110, 80); // distance 10; direction from the leader is 6
    let dir = direction_from_cells(member.0, member.1, leader.0, leader.1) as usize;
    assert_eq!(dir, 6);

    let mut rng = OriginalRng::new(7);
    let mut reference = OriginalRng::new(7);
    let start = (reference.roll() % 12) as usize;

    let goal = follow_goal(
        leader,
        member,
        &collision,
        &std::collections::HashSet::new(),
        &mut rng,
    )
    .expect("a free slot");
    let (dx, dy) = FOLLOW_SLOTS[(leader.0 & 1) as usize][dir][start];
    assert_eq!(goal, (leader.0 + dx as i32, leader.1 + dy as i32));
    assert!(follow_distance(goal, leader) >= 1);
    // Exactly one draw was taken (the slot was free on the first try).
    assert_eq!(rng.roll(), reference.roll());
}

/// Every slot blocked: four attempts, four draws, no goal.
#[test]
fn follow_goal_retries_four_times_and_consumes_four_draws() {
    use crate::combat::OriginalRng;

    let mut collision = free_collision();
    for cell in collision.iter_mut() {
        *cell = 1; // occupied low bits: `cell & 0xFFF != 0`
    }
    let leader = (100, 80);
    let member = (110, 80);
    let mut rng = OriginalRng::new(3);
    let mut reference = OriginalRng::new(3);
    assert_eq!(
        follow_goal(
            leader,
            member,
            &collision,
            &std::collections::HashSet::new(),
            &mut rng
        ),
        None
    );
    // Exactly four attempts = four draws: the next value matches a reference
    // generator advanced by four.
    for _ in 0..4 {
        reference.roll();
    }
    assert_eq!(rng.roll(), reference.roll());
}

/// `FUN_0041EFD8` runs follow only after the armed pick and the retaliation
/// check: a member that still holds an engage order (`Entity+0x02 & 1`) never
/// follows, even when it is far from the leader.
#[test]
fn engaged_companions_do_not_follow() {
    let mut collision = free_collision();
    let leader = (100, 80);
    let mut member = PartyMember {
        entity: entity(539, 6, 0x05, 80, 80),
        motion: ActorMotion::default(),
    };
    member.motion.cell = (80, 80);
    member.motion.render_cell = (80, 80);
    member.motion.engaged = true;
    member.motion.walk_frames = 4;
    let mut rng = crate::combat::OriginalRng::new(1);
    for _ in 0..10 {
        update_followers(
            std::slice::from_mut(&mut member),
            &mut collision,
            leader,
            1,
            &mut rng,
        );
    }
    assert!(!member.motion.walking, "an engaged companion must not follow");
    assert_eq!(member.motion.cell, (80, 80));
}
