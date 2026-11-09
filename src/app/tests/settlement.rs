//! Settlement producer/training tests (`FUN_00417F84`, `FUN_004168A4`).

use super::super::*;

use super::common::*;

/// Add a synthetic, fully-free location and make it current.
fn add_free_location(app: &mut PlayableApp, loc_id: u32) {
    use crate::loc::Location;
    if app.world.loc(loc_id).is_none() {
        let loc = Location {
            loc_id,
            data: Vec::new(),
            props: Vec::new(),
            gates: Vec::new(),
            graph_placements: Vec::new(),
            collision: vec![0u16; 320 * 160],
            max_x: 319,
            max_y: 159,
        };
        let index = app.world.locs.len();
        app.world.locs.push(loc);
        app.world.loc_index.insert(loc_id, index);
    }
    app.current_map = loc_id;
    app.selected = Some(loc_id);
}

fn entity_level(app: &PlayableApp, id: usize) -> u8 {
    app.tables
        .as_ref()
        .and_then(|t| t.entities.get(id))
        .map(|e| e.stats.level)
        .unwrap()
}

fn settlement_timer3(app: &PlayableApp, loc_id: u8) -> i32 {
    let si = app
        .settlements
        .iter()
        .position(|s| s.loc_id == loc_id)
        .expect("settlement");
    app.settlement_trade
        .iter()
        .find(|t| t.index == si)
        .map(|t| t.timer3)
        .unwrap()
}

#[test]
fn voivode_training_levels_up_the_village() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // loc 19 has voivode `waypoint[4].owner = 187` (level 6).
    add_free_location(&mut app, 19);
    app.ensure_population(19);
    assert_eq!(
        app.settlements
            .iter()
            .find(|s| s.loc_id == 19)
            .map(|s| s.voivode()),
        Some(187)
    );
    assert_eq!(settlement_timer3(&app, 19), 0, "school timer starts at 0");
    let mastery_before = app
        .tables
        .as_ref()
        .and_then(|t| t.entities.get(184))
        .map(|e| e.stats.melee_mastery)
        .unwrap();

    // First pass fires immediately (`timer < 1`), then resets to 0x2D0.
    app.tick_settlement_training(1, &HashSet::new());
    assert_eq!(settlement_timer3(&app, 19), 0x2D0);
    assert_eq!(entity_level(&app, 183), 9, "high-stat villager levels up");
    assert_eq!(entity_level(&app, 184), 7, "low villager levels up");
    let mastery_after = app
        .tables
        .as_ref()
        .and_then(|t| t.entities.get(184))
        .map(|e| e.stats.melee_mastery)
        .unwrap();
    assert!(
        mastery_after > mastery_before,
        "mastery moves toward the voivode ({mastery_before} -> {mastery_after})"
    );
    assert_eq!(entity_level(&app, 187), 6, "the voivode never trains itself");

    // The cached current-location actor is mirrored, not rebuilt.
    let pop = app.population_cache.get(&19).expect("population");
    if let Some(actor) = pop.actors.iter().find(|a| a.source_entity_id() == 183) {
        assert_eq!(actor.entity.stats.level, 9);
    }

    // A second pass before the period elapses does nothing.
    app.tick_settlement_training(1, &HashSet::new());
    assert_eq!(entity_level(&app, 183), 9);
    assert_eq!(settlement_timer3(&app, 19), 0x2CF);
}

#[test]
fn training_requires_a_voivode() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    // loc 4 (settlement 5) has `waypoint[4].owner = 0`.
    app.ensure_population(4);
    assert_eq!(
        app.settlements
            .iter()
            .find(|s| s.loc_id == 4)
            .map(|s| s.voivode()),
        Some(0)
    );
    let before = entity_level(&app, 281);
    app.tick_settlement_training(100, &HashSet::new());
    assert_eq!(entity_level(&app, 281), before);
}

#[test]
fn recruited_apprentice_gains_the_elapsed_smith_skill() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.debug_set_player_cell(60, 27);
    app.player.money = 100;
    app.ensure_population(4);
    app.open_dialog(280, 102);
    let deal = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("к тебе дело"))
        .expect("smith deal option");
    app.dialog_click_option(deal);
    let apprentice = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("ученика"))
        .expect("apprentice option");
    app.dialog_click_option(apprentice);
    let start = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.record == 2087)
        .expect("Приступай option");
    app.dialog_click_option(start);
    // Entity 281 is the smith's apprentice (role bits 0x18 -> role 3).
    // Pretend ten training periods have elapsed: gain = 10.
    let si = app
        .settlements
        .iter()
        .position(|s| s.loc_id == 4)
        .expect("settlement");
    app.settlements[si].apprentice_start[2] = -(16 * 0x5A0 * 10);

    app.player.charisma_base = 20;
    app.open_dialog(281, 143);
    let join = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("пойдем со мной"))
        .expect("finish training option");
    app.dialog_click_option(join);
    assert_eq!(app.party.len(), 1);
    // Smith 280 has smithing 9: round(0 + (9 - 0) * 0.03 * 10) = 3.
    assert_eq!(app.party[0].entity.stats.smithing, 3);
}

#[test]
fn apprentice_stands_next_to_the_master_not_on_him() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.debug_set_player_cell(60, 27);
    app.player.money = 100;
    app.ensure_population(4);
    // Smith (entity 280, dialogue 102) -> "У меня есть к делу" -> page 2073.
    app.open_dialog(280, 102);
    let deal = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("к тебе дело"))
        .expect("smith deal option");
    app.dialog_click_option(deal);
    let apprentice = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.label().contains("ученика"))
        .expect("apprentice option");
    app.dialog_click_option(apprentice);
    let start = app
        .dialog
        .as_ref()
        .unwrap()
        .options
        .iter()
        .position(|o| o.record == 2087)
        .expect("Приступай option");
    app.dialog_click_option(start);
    // The quest context invalidates the cache; rebuild with the new role bits.
    app.ensure_population(4);

    let waypoint = app
        .settlements
        .iter()
        .find(|s| s.loc_id == 4)
        .and_then(|s| s.waypoints.get(3))
        .copied()
        .expect("smith station");
    let station = (waypoint.x as i32, waypoint.y as i32);
    let collision = app
        .world
        .loc(4)
        .map(|l| l.collision.clone())
        .unwrap_or_default();
    for _ in 0..2000 {
        let PlayableApp {
            population_cache,
            settlements,
            ..
        } = &mut app;
        let Some(pop) = population_cache.get_mut(&4) else {
            break;
        };
        let settlement = settlements.iter().find(|s| s.loc_id == 4);
        update_population(pop, &collision, settlement, 1.0 / LOGIC_TICK_HZ);
    }

    let motion = app
        .population_cache
        .get(&4)
        .and_then(|p| p.motion.get(&281))
        .expect("apprentice motion");
    let cell = motion.cell;
    assert_ne!(cell, station, "must not stand on the master's cell");
    let adjacent = (0..8u8)
        .any(|d| crate::coords::step_by_direction(station.0, station.1, d) == cell);
    assert!(adjacent, "apprentice cell {cell:?} must be adjacent to {station:?}");
    assert_eq!(motion.facing, waypoint.facing, "faces the station facing");
}

fn settlement_index(app: &PlayableApp, loc_id: u8) -> usize {
    app.settlements
        .iter()
        .position(|s| s.loc_id == loc_id)
        .expect("settlement")
}

fn target(key: u8, state: u8, prop_index: u16, timer: u16) -> crate::population::SettlementTarget {
    crate::population::SettlementTarget {
        key,
        state,
        prop_index,
        timer,
    }
}

#[test]
fn target_pass_scales_by_workers_and_gates_training() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 19);
    app.ensure_population(19);
    let si = settlement_index(&app, 19);
    // loc 19: 13 free villagers, 5 waypoint owners -> 8 workers.
    app.settlements[si].targets = vec![target(0, 0, 1, 1)];
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;
    let before_level = entity_level(&app, 183);

    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].state, 1);
    assert_eq!(
        app.settlements[si].targets[0].timer,
        180,
        "24 * 60 / 8 workers"
    );
    assert_eq!(settlement_timer3(&app, 19), 0, "training blocked by local_30");
    assert_eq!(entity_level(&app, 183), before_level);

    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].timer, 179);
    assert_eq!(settlement_timer3(&app, 19), 0x2D0, "training ran");
    assert!(entity_level(&app, 183) > before_level);
}

#[test]
fn target_pass_ignores_sentinels_and_resets_completed_targets() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 19);
    app.ensure_population(19);
    let si = settlement_index(&app, 19);
    app.settlements[si].targets = vec![target(255, 0, 0, 5), target(0, 6, 0, 0)];
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;

    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].state, 0, "key 255 sentinel");
    assert_eq!(app.settlements[si].targets[0].timer, 5);
    assert_eq!(app.settlements[si].targets[1].state, 0, "state 6 resets");
    assert_eq!(app.settlements[si].targets[1].timer, 180);
}

#[test]
fn settlement_claim_wakes_the_group_for_a_hostile_party() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let si = settlement_index(&app, 4);
    let group_id = app.settlements[si].group_id as usize;
    app.settlements[si].flags_4b2 |= 1;
    {
        let group = app.tables.as_mut().unwrap().groups.get_mut(group_id).unwrap();
        group.activity_1d = 0;
        group.rect_a = crate::population::Rect {
            x1: 0,
            x2: 319,
            y1: 0,
            y2: 159,
        };
    }
    app.debug_set_player_cell(50, 50);
    app.tick_settlement_pass(1);
    let group = &app.tables.as_ref().unwrap().groups[group_id];
    assert_eq!(group.activity_1d, 1);
    assert_eq!(group.target_group, 0);
}

#[test]
fn settlement_random_event_adds_the_money_pool() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let si = settlement_index(&app, 4);
    let group_id = app.settlements[si].group_id as usize;
    app.settlements[si].flags_4b2 &= !0x10;
    app.settlements[si].progress_14 = 0x275F;
    app.settlements[si].target_base0 = 1;
    app.player.trade = 36;
    app.settlements[si].money_pool_10 = 0;
    let count = app.tables.as_ref().unwrap().groups[group_id].entity_count as f64;

    app.tick_settlement_pass(1);
    let expected = (count * 5.0 + 20.0 * 6.0 + 1.0).round_ties_even() as i32;
    assert_eq!(app.settlements[si].progress_14, 0);
    assert_eq!(app.settlements[si].money_pool_10, expected);
}

#[test]
fn no_worker_clear_resets_activity_flags_and_counter() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let si = settlement_index(&app, 4);
    let group_id = app.settlements[si].group_id as usize;
    app.settlements[si].flags_4b2 = 0x15;
    app.settlements[si].progress_14 = 5;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[group_id].entity_count = 0;

    app.tick_settlement_pass(1);
    let group = &app.tables.as_ref().unwrap().groups[group_id];
    assert_eq!(group.activity_1d, 0);
    assert_eq!(app.settlements[si].flags_4b2 & 0x15, 0);
    // The clear zeroes the counter; the same pass's random event then
    // increments it once (flag 0x10 was cleared).
    assert_eq!(app.settlements[si].progress_14, 1);
}

#[test]
fn defeat_freezes_runtime_death_rows_on_the_last_frame() {
    use crate::actors::{ActorMotion, ActorPlacement, PopulationBuild, RuntimeActor};
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let si = settlement_index(&app, 4);
    let group_id = app.settlements[si].group_id as usize;
    let (first, second) = {
        let tables = app.tables.as_mut().unwrap();
        let group = tables.groups.get_mut(group_id).unwrap();
        group.activity_1d = 1;
        group.entity_count = 2;
        let first = group.first_entity as usize;
        for id in [first, first + 1] {
            let entity = tables.entities.get_mut(id).unwrap();
            entity.kind = 6;
            entity.archetype &= 0x7F;
            entity.visual_index = 0;
            entity.stats = Default::default();
            entity.stats.hp_internal = 1600;
            entity.equipment = [0; 6];
            entity.inventory = [0; 32];
            entity.additional_equipment = [0; 5];
            entity.special_equipment = 0;
        }
        (first, first + 1)
    };
    // `first` is a runtime death row (`Entity+0x00 = 3`) with a known 5-frame
    // death; `second` is a live zero-power defender so the free-worker count is
    // non-zero (otherwise the no-worker clear runs before the defeat).
    let mut pop = PopulationBuild::default();
    for id in [first, second] {
        let entity = app.tables.as_ref().unwrap().entities[id].clone();
        pop.actors.push(RuntimeActor {
            entity,
            source_group_id: group_id,
            placement: ActorPlacement::Persisted,
        });
    }
    let mut dead_motion = ActorMotion::default();
    dead_motion.action = 3;
    dead_motion.dead = true;
    dead_motion.death_frames = 5;
    dead_motion.frame = 0;
    pop.motion.insert(first, dead_motion);
    let mut live_motion = ActorMotion::default();
    live_motion.action = 6;
    pop.motion.insert(second, live_motion);
    app.population_cache.insert(4, pop);
    app.world_frame = 9;

    app.tick_settlement_pass(1);

    // The authored table and the cached actor are hidden corpses.
    assert_ne!(
        app.tables.as_ref().unwrap().entities[first].archetype & 0x80,
        0
    );
    let pop = &app.population_cache[&4];
    let dead = pop.actors.iter().find(|a| a.entity.index == first).unwrap();
    assert_ne!(dead.entity.archetype & 0x80, 0);
    // `Entity+0x10 = Entity+0xD7 - 1`.
    let motion = pop.motion.get(&first).unwrap();
    assert!(motion.dead);
    assert_eq!(motion.action, 3);
    assert_eq!(motion.death_frames, 5);
    assert_eq!(motion.frame, 4);
    // The live defender is idled, not hidden.
    let live = pop.actors.iter().find(|a| a.entity.index == second).unwrap();
    assert_eq!(live.entity.archetype & 0x80, 0);
    let live_motion = pop.motion.get(&second).unwrap();
    assert!(!live_motion.dead);
    assert_eq!(live_motion.frame, 0);
}

#[test]
fn building_progresses_construction_damage_and_rebuild_sprites() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let prop = crate::loc::Prop {
        ri: 1,
        object_id: 0,
        grid_x: 10,
        grid_y: 10,
        interactive: true,
        palette: 0,
        frame: 0,
        flame_emitters: Vec::new(),
        flame_frames: Vec::new(),
    };
    app.world.loc_mut(4).unwrap().props.push(prop);
    let si = settlement_index(&app, 4);
    app.settlements[si].targets = vec![target(0, 0, 1, 1)];
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;

    // 0 -> 1 -> 2 -> 3 = construction; object id = base(7) + state + 30.
    for (state, expected_obj) in [(1u8, 38usize), (2, 39), (3, 40)] {
        app.tick_settlement_pass(1);
        assert_eq!(app.settlements[si].targets[0].state, state);
        assert_eq!(
            app.world.locs[0].props.last().unwrap().object_id,
            expected_obj
        );
        app.settlements[si].targets[0].timer = 1;
    }
    // 3 -> 4 -> 5 -> 6 = damage states; 6 -> 0 resets/rebuilds.
    for (state, expected_obj) in [(4u8, 41usize), (5, 42), (6, 43)] {
        app.tick_settlement_pass(1);
        assert_eq!(app.settlements[si].targets[0].state, state);
        assert_eq!(
            app.world.locs[0].props.last().unwrap().object_id,
            expected_obj
        );
        app.settlements[si].targets[0].timer = 1;
    }
    app.settlements[si].targets[0].timer = 0;
    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].state, 0, "state 6 rebuilds");
    assert_eq!(app.world.locs[0].props.last().unwrap().object_id, 37);
}

#[test]
fn target_state_change_updates_the_current_map_prop() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let prop = crate::loc::Prop {
        ri: 1,
        object_id: 0,
        grid_x: 10,
        grid_y: 10,
        interactive: true,
        palette: 0,
        frame: 0,
        flame_emitters: Vec::new(),
        flame_frames: Vec::new(),
    };
    app.world.loc_mut(4).unwrap().props.push(prop);
    let si = settlement_index(&app, 4);
    app.settlements[si].targets = vec![target(0, 0, 1, 1)];
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;

    app.tick_settlement_pass(1);
    // key 0 base = 7; state 1 -> object id 7 + 1 + 30 = 38.
    assert_eq!(app.world.locs[0].props[0].object_id, 38);
}
