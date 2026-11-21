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

/// Return the authored `Entity` level from the population table.
fn entity_level(app: &PlayableApp, id: usize) -> u8 {
    app.tables
        .as_ref()
        .and_then(|t| t.entities.get(id))
        .map(|e| e.stats.level)
        .unwrap()
}

/// Return the settlement school timer (`Settlement+0x0C`, stored as `SettlementTrade::timer3`).
///
/// The voivode training pass counts it down and reloads `0x2D0` when it reaches
/// zero, so a test can tell whether a pass trained the village.
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

/// The voivode's school pass levels the villagers, moves mastery toward the voivode and resets the `0x2D0` timer.
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
    assert_eq!(
        entity_level(&app, 187),
        6,
        "the voivode never trains itself"
    );

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

/// Without a waypoint-4 owner the training pass changes nothing.
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

/// A recruited apprentice gains `round(elapsed periods * master skill * 0.03)` smithing.
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

/// The apprentice walks to a free neighbour of the master's station and faces the waypoint facing.
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
    let mut collision = app
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
        update_population(pop, &mut collision, settlement, 1.0 / LOGIC_TICK_HZ);
    }

    let motion = app
        .population_cache
        .get(&4)
        .and_then(|p| p.motion.get(&281))
        .expect("apprentice motion");
    let cell = motion.cell;
    assert_ne!(cell, station, "must not stand on the master's cell");
    let adjacent =
        (0..8u8).any(|d| crate::coords::step_by_direction(station.0, station.1, d) == cell);
    assert!(
        adjacent,
        "apprentice cell {cell:?} must be adjacent to {station:?}"
    );
    assert_eq!(motion.facing, waypoint.facing, "faces the station facing");
}

/// Return the `settlements` index of the settlement at `loc_id`.
fn settlement_index(app: &PlayableApp, loc_id: u8) -> usize {
    app.settlements
        .iter()
        .position(|s| s.loc_id == loc_id)
        .expect("settlement")
}

/// Build a settlement target record with the given key, state byte, prop index and timer.
fn target(key: u8, state: u8, prop_index: u16, timer: u16) -> crate::population::SettlementTarget {
    crate::population::SettlementTarget {
        key,
        state,
        prop_index,
        timer,
    }
}

/// The target timer is divided by the free-worker count, and training is skipped while `local_30` is set.
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
        app.settlements[si].targets[0].timer, 180,
        "24 * 60 / 8 workers"
    );
    assert_eq!(
        settlement_timer3(&app, 19),
        0,
        "training blocked by local_30"
    );
    assert_eq!(entity_level(&app, 183), before_level);

    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].timer, 179);
    assert_eq!(settlement_timer3(&app, 19), 0x2D0, "training ran");
    assert!(entity_level(&app, 183) > before_level);
}

/// Key 255 is a sentinel; a ruined target (state 6) rebuilds while workers are
/// free, and a completed target (state 3) is left idle.
#[test]
fn target_pass_ignores_sentinels_and_keeps_completed_targets() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 19);
    app.ensure_population(19);
    let si = settlement_index(&app, 19);
    app.settlements[si].targets = vec![
        target(255, 0, 0, 5),
        target(0, 6, 0, 0),
        target(0, 3, 0, 0),
    ];
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;

    app.tick_settlement_pass(1);
    assert_eq!(app.settlements[si].targets[0].state, 0, "key 255 sentinel");
    assert_eq!(app.settlements[si].targets[0].timer, 5);
    assert_eq!(
        app.settlements[si].targets[1].state, 0,
        "ruined target rebuilds with workers"
    );
    assert_eq!(
        app.settlements[si].targets[1].timer, 180,
        "armed with 24 * 60 / 8 workers"
    );
    assert_eq!(
        app.settlements[si].targets[2].state, 3,
        "completed target stays complete"
    );
    assert_eq!(app.settlements[si].targets[2].timer, 0);
}

/// Regression for the shipped villages: `Game.1` loc 4 authors every main
/// building at state 3 (complete). The periodic pass must leave those targets
/// and their prop sprites alone instead of collapsing the village into
/// foundations on the first 16-frame tick.
#[test]
fn shipped_completed_targets_survive_the_first_pass() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let si = settlement_index(&app, 4);
    let ti = app.settlements[si]
        .targets
        .iter()
        .position(|t| t.key < 0x80 && t.state == 3 && t.timer == 0)
        .expect("loc 4 ships completed targets");
    let prop_index = app.settlements[si].targets[ti].prop_index;
    app.world.loc_mut(4).unwrap().props.push(crate::loc::Prop {
        ri: prop_index as usize,
        object_id: 0,
        grid_x: 10,
        grid_y: 10,
        interactive: true,
        palette: 0,
        frame: 0,
        flame_emitters: Vec::new(),
        flame_frames: Vec::new(),
    });
    app.apply_settlement_props();
    let object_before = app.world.loc(4).unwrap().props[0].object_id;
    let group_id = app.settlements[si].group_id as usize;
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 0;

    app.tick_settlement_pass(1);
    assert_eq!(
        app.settlements[si].targets[ti].state, 3,
        "a completed building stays complete"
    );
    assert_eq!(app.settlements[si].targets[ti].timer, 0);
    assert_eq!(
        app.world.loc(4).unwrap().props[0].object_id,
        object_before,
        "the completed building's prop sprite must not change"
    );
}

/// The temporary-effect countdown reaches every cached world actor, not only the
/// current map's citizen groups (`FUN_0041D048` phase A and `FUN_0041BD3C` have
/// no location or class filter).
#[test]
fn temp_effect_pass_ticks_cached_world_actors() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 19);
    app.ensure_population(19);
    let id = app.population_cache[&19].actors[0].source_entity_id();
    {
        let pop = app.population_cache.get_mut(&19).unwrap();
        let actor = pop
            .actors
            .iter_mut()
            .find(|a| a.source_entity_id() == id)
            .expect("cached actor");
        actor.entity.stats.saved_charisma = 14;
        actor.entity.stats.charisma_current = 9;
        actor.entity.stats.temp_effect_timer = 2;
    }
    app.tick_temp_effect_pass();
    let after_first = app.population_cache[&19]
        .actors
        .iter()
        .find(|a| a.source_entity_id() == id)
        .unwrap()
        .entity
        .stats
        .charisma_current;
    assert_eq!(after_first, 9, "first pass only decrements the timer");
    app.tick_temp_effect_pass();
    let restored = app.population_cache[&19]
        .actors
        .iter()
        .find(|a| a.source_entity_id() == id)
        .unwrap()
        .entity
        .stats
        .clone();
    assert_eq!(restored.charisma_current, 14, "stats restored at zero");
    assert_eq!(restored.temp_effect_timer, 0);
}

/// An armed foreign party inside rectangle A clears `+0x1A`, sets `+0x1D = 1` and targets group 0.
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
        let group = app
            .tables
            .as_mut()
            .unwrap()
            .groups
            .get_mut(group_id)
            .unwrap();
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

/// The `+0x14` event counter resets at `0x2760` and adds the worker/trade-derived tribute.
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

/// Zero workers clear the activity flags and counter, after which the same pass's event increments it once.
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

/// A power-ratio defeat hides the defenders, freezes death rows on their last frame and only idles survivors.
///
/// The hide bit is `Entity+0x04 & 0x80`; the frozen frame is `Entity+0xD7 - 1`,
/// which is what `Entity+0x10` holds for a corpse.
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
    let live = pop
        .actors
        .iter()
        .find(|a| a.entity.index == second)
        .unwrap();
    assert_eq!(live.entity.archetype & 0x80, 0);
    let live_motion = pop.motion.get(&second).unwrap();
    assert!(!live_motion.dead);
    assert_eq!(live_motion.frame, 0);
}

/// Construction states 0..6 step the object ids 37..43, and state 6 rebuilds to the base sprite.
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
    // 3 -> 4 -> 5 -> 6 = damage states; 6 -> 0 rebuilds.
    for (state, expected_obj) in [(4u8, 41usize), (5, 42), (6, 43)] {
        app.tick_settlement_pass(1);
        assert_eq!(app.settlements[si].targets[0].state, state);
        assert_eq!(
            app.world.locs[0].props.last().unwrap().object_id,
            expected_obj
        );
        app.settlements[si].targets[0].timer = 1;
    }
    // A ruined target (state 6) restarts from the base sprite while workers
    // are free; with no workers it stays ruined.
    app.settlements[si].targets[0].timer = 0;
    app.tick_settlement_pass(1);
    assert_eq!(
        app.settlements[si].targets[0].state, 0,
        "state 6 rebuilds with workers"
    );
    assert_eq!(app.world.locs[0].props.last().unwrap().object_id, 37);

    app.settlements[si].targets[0].state = 6;
    app.settlements[si].targets[0].timer = 0;
    app.apply_settlement_props();
    assert_eq!(app.world.locs[0].props.last().unwrap().object_id, 43);
    app.tables.as_mut().unwrap().groups[group_id].activity_1d = 1;
    app.tick_settlement_pass(1);
    assert_eq!(
        app.settlements[si].targets[0].state, 6,
        "a workerless village cannot rebuild"
    );
    assert_eq!(app.world.locs[0].props.last().unwrap().object_id, 43);
}

/// A target state change rewrites the prop's object id on the current map.
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

/// `FUN_00435870`: entering a location releases the dead actors' item instances
/// and clears the settlement waypoint owners that point at them.
#[test]
fn map_entry_releases_dead_actor_items_and_waypoints() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    add_free_location(&mut app, 4);
    let Some(si) = app.settlements.iter().position(|s| s.loc_id == 4) else {
        eprintln!("skipping: no loc 4 settlement");
        return;
    };
    let group = app.settlements[si].group_id as usize;
    let (first, count) = {
        let t = app.tables.as_ref().expect("tables");
        let g = &t.groups[group];
        (g.first_entity as usize, g.entity_count as usize)
    };
    assert!(count > 0, "loc 4 settlement group has members");
    let member = first;
    let item = app
        .world
        .res
        .alloc_item_instance(3, 34, 0, -1, 1.0, 0, 0)
        .expect("armor instance");
    if let Some(t) = app.tables.as_mut() {
        let e = &mut t.entities[member];
        e.flags_b |= 0x80; // dead
        e.equipment[3] = item;
    }
    // `FUN_00435870` itself (the map-entry hook runs it before the stock
    // seeding, which would recycle the freed instance slot).
    app.cleanup_dead_at_location(4);
    assert_eq!(
        app.world.res.item_class(item),
        Some(0xFF),
        "dead actor items released"
    );
    // The map-entry path also clears the settlement waypoint owner.
    app.settlements[si].waypoints[2].owner = member as u16;
    app.population_cache.remove(&4);
    app.ensure_population(4);
    assert_eq!(
        app.settlements[si].waypoints[2].owner, 0,
        "settlement waypoint owner cleared"
    );
}

/// `FUN_004172AC`: the prop start frame and every flame phase come from the
/// game RNG (`rand() % count`, `rand() % (max-min+1) + min`), not a stable hash.
#[test]
fn settlement_prop_phases_come_from_the_game_rng() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.ensure_population(app.current_map);
    let loc = app.current_map;
    let target = crate::population::SettlementTarget {
        key: 0,
        state: 0,
        prop_index: 0,
        timer: 0,
    };
    let base = {
        let db = app.world.res.item_db.as_ref().expect("item db");
        db.settlement_prop_base(0).expect("key 0 base")
    };
    let object_id = base as usize + 30;
    let frame_count = app.world.res.object_frame_count(object_id).max(1);
    let emitters = app.world.res.object_flames(object_id);
    let ranges: Vec<(u8, u8)> = emitters
        .iter()
        .filter_map(|e| crate::res::flame_range(e.selector))
        .collect();

    let mut expect = app.combat_rng;
    let expected_frame = if frame_count < 2 {
        0
    } else {
        (expect.next_u32() % frame_count as u32) as u8
    };
    let mut expected_phases = Vec::new();
    for (min, max) in &ranges {
        let span = (*max - *min) as u32 + 1;
        let off = if span < 2 {
            0
        } else {
            (expect.next_u32() % span) as u8
        };
        expected_phases.push(*min + off);
    }

    app.apply_settlement_prop(loc, &target);
    assert_eq!(
        app.combat_rng, expect,
        "the prop rebuild must consume the game RNG in the original order"
    );
    if let Some(prop) = app
        .world
        .loc(loc)
        .and_then(|l| l.props.iter().find(|p| p.ri == 0))
    {
        assert_eq!(prop.frame, expected_frame);
        assert_eq!(prop.flame_frames, expected_phases);
    }
}
