//! Runtime actor occupancy and the "bump" regression.
//!
//! The original merges a one-based entity marker into the Konung.N collision
//! word and re-checks it at every walk-step commit. The shipped start location
//! (Game.1, loc 4) has Sigmar at `(70,31)` in group 65, whose spawn rectangle A
//! is exactly that single cell. Before this port the player could step onto him,
//! wake group 65 and make him acquire a target; here the player must stop or
//! detour.

use super::common::*;

use crate::actors::occupancy::{marker, occupant};
use crate::combat::CombatTarget;

/// Sigmar's authored cell and source ids.
const SIGMAR: usize = 539;
const SIGMAR_GROUP: usize = 65;
const SIGMAR_CELL: (i32, i32) = (70, 31);

#[test]
fn player_cannot_enter_sigmars_cell_or_wake_his_group() {
    let Some(mut app) = build_party_app_with_free_location() else {
        return;
    };
    app.ensure_population(4);
    app.rebuild_occupancy(4);
    app.debug_set_player_cell(72, 34);

    // The runtime collision carries Sigmar's one-based marker.
    let cell_marker = {
        let collision = app.world.collision(4).expect("loc 4 collision");
        occupant(collision, SIGMAR_CELL)
    };
    assert_eq!(cell_marker, marker(SIGMAR));
    assert!(app.compute_player_path((72, 34), SIGMAR_CELL).is_none());

    // A route past him detours around the occupied cell.
    let path = app
        .compute_player_path((72, 34), (66, 31))
        .expect("reachable detour");
    let mut cur = (72, 34);
    for &dir in &path {
        let next = crate::coords::step_by_direction(cur.0, cur.1, dir);
        assert_ne!(next, SIGMAR_CELL, "route stepped onto Sigmar");
        assert!(crate::actors::is_free(
            app.world.collision(4).unwrap(),
            next.0,
            next.1
        ));
        cur = next;
    }
    assert_eq!(cur, (66, 31));

    // Even a direct move command through/past him must never enter his cell.
    app.set_player_target(66, 31);
    for _ in 0..600 {
        app.update_player(1.0 / crate::actors::LOGIC_TICK_HZ);
        app.update_combat(1.0 / crate::actors::LOGIC_TICK_HZ);
        let cell = (app.player_x.round() as i32, app.player_y.round() as i32);
        assert_ne!(cell, SIGMAR_CELL, "player entered Sigmar's cell");
    }

    // He was never woken and never acquired a target.
    let group = &app.tables.as_ref().unwrap().groups[SIGMAR_GROUP];
    assert_eq!(
        group.activity_1d, 0,
        "Sigmar's group woke without a real attack"
    );
    let motion = &app.population_cache[&4].motion[&539];
    assert_eq!(motion.combat_target, None);
    assert_eq!(motion.target_marker, 0);
}

#[test]
fn blocked_step_into_the_ordered_target_becomes_contact_attack() {
    let Some(mut app) = build_party_app_with_free_location() else {
        return;
    };
    app.ensure_population(4);
    app.rebuild_occupancy(4);
    app.debug_set_player_cell(72, 34);

    // Build a synthetic motion that is already ordered onto Sigmar and whose
    // path's first step is his occupied cell.
    let sigmar_cell = {
        let pop = app.population_cache.get(&4).unwrap();
        pop.motion[&SIGMAR].cell
    };
    let mut chosen = None;
    'outer: for dx in -2..=2 {
        for dy in -2..=2 {
            let start = (sigmar_cell.0 + dx, sigmar_cell.1 + dy);
            if start == sigmar_cell {
                continue;
            }
            let free = crate::actors::is_free(app.world.collision(4).unwrap(), start.0, start.1);
            if !free {
                continue;
            }
            for d in 0..8u8 {
                if crate::coords::step_by_direction(start.0, start.1, d) == sigmar_cell {
                    chosen = Some((start, d));
                    break 'outer;
                }
            }
        }
    }
    let Some((start, dir)) = chosen else {
        panic!("no free neighbour steps onto Sigmar");
    };
    let mut m = crate::actors::ActorMotion::default();
    m.cell = start;
    m.render_cell = start;
    m.combat_target = Some(CombatTarget::World(SIGMAR));
    m.target_marker = marker(SIGMAR);
    m.path = vec![dir];
    m.walking = true;
    // Aligned: the contact/branch under test happens after the turn gate.
    m.facing = dir;
    let mut occupied = std::collections::HashSet::new();
    occupied.insert(start);
    let Some((collision, _)) = app.world.collision_and_res_mut(4) else {
        return;
    };
    let moved = crate::actors::begin_actor_segment(&mut m, 0, &mut occupied, collision);
    assert!(!moved, "contact step must not enter the target cell");
    assert_eq!(m.cell, start);
    assert!(m.path.is_empty());
    assert_eq!(m.combat_target, Some(CombatTarget::World(SIGMAR)));
    assert_eq!(m.target_marker, marker(SIGMAR));
}
