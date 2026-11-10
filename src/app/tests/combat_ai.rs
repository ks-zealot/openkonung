//! Group-activity / autonomous target-acquisition tests (`FUN_00412C30`,
//! `FUN_00412E00`, `FUN_004245B4`).

use super::super::*;
use super::common::*;
use crate::combat::CombatTarget;
use crate::population::{Entity, EntityStats, GameTables, Group, Rect};

fn entity(index: usize, kind: u8, archetype: u8, group: u8, x: u16, y: u16) -> Entity {
    Entity {
        index,
        kind,
        facing: 0,
        // Every shipped active actor has this flag; `FUN_004245B4` uses bit 0
        // only for its queue-clear gate.
        flags_a: 0x40,
        flags_b: 0,
        archetype,
        group_id: group,
        raw_0x12: 0,
        x,
        y,
        stats: EntityStats {
            hp_internal: 1600,
            strength_base: 10,
            endurance_base: 10,
            ..EntityStats::default()
        },
        equipment: [0; 6],
        inventory: [0; 32],
        additional_equipment: [0; 5],
        special_equipment: 0,
        dialogue_id: 0xFF,
        state_d2: 0,
        name_id: 0,
        nickname_id: 0,
        visual_index: 0,
        palette_offset: 0,
    }
}

fn group(index: usize, first: u16, count: u8, class: u8, rect: Rect) -> Group {
    Group {
        index,
        first_entity: first,
        rect_a: rect,
        rect_b: Rect::default(),
        growth_timer: 0,
        growth_countdown: 0,
        count_18: count,
        capacity: count,
        loc_id: 1,
        target_group: 0,
        entity_count: count,
        class_1e: class,
        activity_1d: 0,
        visited: false,
    }
}

fn synthetic_app(actor: Entity, class: u8) -> Option<PlayableApp> {
    let mut app = build_party_move_app()?;
    let rect = Rect {
        x1: 10,
        x2: 20,
        y1: 10,
        y2: 20,
    };
    let mut groups = vec![Group::default(); 200];
    for (i, g) in groups.iter_mut().enumerate() {
        g.index = i;
    }
    groups[1] = group(1, actor.index as u16, 1, class, rect);
    let mut entities = vec![entity(0, 0, 0, 0, 0, 0); 2000];
    for (i, e) in entities.iter_mut().enumerate() {
        e.index = i;
    }
    let actor_index = actor.index;
    entities[actor_index] = actor;
    app.tables = Some(GameTables { groups, entities });
    app.population_cache.clear();
    app.current_map = 1;
    app.ensure_population(1);
    app.player_x = 16.0;
    app.player_y = 15.0;
    app.player_render_x = 16.0;
    app.player_render_y = 15.0;
    app.player.dead = false;
    app.party.clear();
    Some(app)
}

#[test]
fn party_inside_group_rect_wakes_and_acquires() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.update_group_relations();
    let g = &app.tables.as_ref().unwrap().groups[1];
    assert_eq!(g.activity_1d, 1);
    assert_eq!(g.target_group, 0);

    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(m.combat_target, Some(CombatTarget::Player));

    app.update_world_combat();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(m.action, crate::anim::ACT_MELEE as u8);
    assert!(m.attack_frames >= 1);
}

#[test]
fn wake_sets_activity_without_assigning_a_target() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.wake_group(1, CombatTarget::Player);
    let g = &app.tables.as_ref().unwrap().groups[1];
    assert_eq!(g.activity_1d, 1);
    assert_eq!(g.target_group, 0);
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(m.combat_target, None);
}

#[test]
fn group_with_party_outside_and_member_inside_stays_idle() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.player_x = 100.0;
    app.player_y = 80.0;
    app.update_group_relations();
    assert_eq!(app.tables.as_ref().unwrap().groups[1].activity_1d, 0);
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(m.combat_target, None);
}

#[test]
fn stray_member_sets_recall_and_starts_return_path() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 30, 30), 0x41) else {
        return;
    };
    app.player_x = 100.0;
    app.player_y = 80.0;
    app.update_group_relations();
    assert_eq!(app.tables.as_ref().unwrap().groups[1].activity_1d, 0x20);
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert!(m.walking || !m.path.is_empty());
}

#[test]
fn citizen_combat_target_is_not_reset_by_wander_pass() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x81, 1, 15, 15), 0x81) else {
        return;
    };
    // Synthetic helper always uses group 1; use a citizen class/archetype.
    app.wake_group(1, CombatTarget::Player);
    app.update_world_ai();
    {
        let m = app
            .population_cache
            .get_mut(&1)
            .unwrap()
            .motion
            .get_mut(&10)
            .unwrap();
        assert_eq!(m.combat_target, Some(CombatTarget::Player));
        m.action = crate::anim::ACT_MELEE as u8;
        m.frame = 3;
    }
    app.update_actors(0.1);
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(m.action, crate::anim::ACT_MELEE as u8);
    assert_eq!(m.frame, 3);
}

#[test]
fn group_recall_uses_live_cells_not_authored_positions() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    // Authored position is inside rectangle A, but the runtime cell moved out.
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .cell = (30, 30);
    app.player_x = 100.0;
    app.player_y = 80.0;
    app.update_group_relations();
    assert_eq!(app.tables.as_ref().unwrap().groups[1].activity_1d, 0x20);
}

#[test]
fn inactive_object_group_is_forced_idle() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let m = pop.motion.get_mut(&10).unwrap();
        m.action = crate::anim::ACT_MELEE as u8;
        m.frame = 3;
        m.combat_target = None;
    }
    app.tables.as_mut().unwrap().groups[1].activity_1d = 0;
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).unwrap();
    assert_eq!(m.action, crate::anim::idle_action(false) as u8);
    assert_eq!(m.frame, 0);
}

#[test]
fn ranged_world_actor_switches_to_action_row_four() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x21, 1, 18, 15), 0x41) else {
        return;
    };
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let a = pop
            .actors
            .iter_mut()
            .find(|a| a.entity.index == 10)
            .unwrap();
        a.entity.equipment[1] = 23;
        a.entity.special_equipment = 27;
        a.entity.raw_0x12 = 1;
    }
    if let Ok(save) = std::fs::read(install_dir().join("Game.1")) {
        app.world.res.set_save_game(&save);
    }
    app.world.res.set_item_quantity(27, 30);
    app.world.res.set_item_durability(23, 56.0);
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    app.update_world_combat();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert!(m.ranged_order);
    assert_eq!(m.action, crate::anim::ACT_RANGED as u8);
}

#[test]
fn mo_walk_completion_seeds_entity_0x12_delay() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x4D, 1, 15, 15), 0x4D) else {
        return;
    };
    app.player_x = 25.0;
    app.player_y = 15.0;
    app.player_render_x = 25.0;
    app.player_render_y = 15.0;
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    assert_eq!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target),
        Some(CombatTarget::Player)
    );
    let mut saw_delay = false;
    for _ in 0..400 {
        app.update_world_combat();
        let delay = app.population_cache[&1]
            .actors
            .iter()
            .find(|a| a.entity.index == 10)
            .map(|a| a.entity.raw_0x12)
            .unwrap_or(0);
        if delay > 0 {
            saw_delay = true;
            break;
        }
    }
    assert!(saw_delay, "M/O walk completion never seeded Entity+0x12");
}

#[test]
fn engage_button_orders_nearest_awake_hostile() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.update_group_relations();
    app.update_world_ai();
    assert_eq!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target),
        Some(CombatTarget::Player)
    );
    app.player_attack_target = None;
    app.party_engage();
    assert_eq!(
        app.player_attack_target,
        Some(CombatTarget::World(10)),
        "engage did not order the nearest awake hostile"
    );
    assert!(
        crate::anim::battle_from_flags(app.player.flags_b),
        "engage must put the leader in battle stance"
    );
}

#[test]
fn idle_party_retaliates_against_adjacent_attacker() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.update_group_relations();
    app.update_world_ai();
    assert_eq!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target),
        Some(CombatTarget::Player)
    );
    app.player_attack_target = None;
    app.player_path.clear();
    app.player_active = false;
    app.update_player_combat();
    assert_eq!(
        app.player_attack_target,
        Some(CombatTarget::World(10)),
        "idle leader did not retaliate against an adjacent attacker"
    );
}
