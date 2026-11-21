//! Group-activity / autonomous target-acquisition tests (`FUN_00412C30`,
//! `FUN_00412E00`, `FUN_004245B4`).

use super::super::*;
use super::common::*;
use crate::actors::{ActorMotion, PartyMember};
use crate::combat::CombatTarget;
use crate::population::{Entity, EntityStats, GameTables, Group, Rect};

/// Build a complete synthetic `Entity` for one group member.
///
/// Sets the given index, authored action row `kind`, archetype, group id and
/// authored cell; `flags_a = 0x40` (the shipped active-actor flag used by the
/// group dispatcher), full health (`hp_internal = 1600`), strength/endurance 10,
/// no gear and no dialogue (`dialogue_id = 0xFF`).
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

/// Build a synthetic `Group` holding `count` entities starting at `first`.
///
/// Fills the index, entity range, rectangle A and class byte (`Group+0x1E`), with
/// activity `+0x1D = 0`, no rectangle B and location 1.
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

/// Build a party app whose only population table is one synthetic group 1.
///
/// Replaces `app.tables` with 200 zeroed groups and 2000 zeroed entities, installs
/// `actor` at its own index inside group 1 (with rectangle A 10..20), clears the
/// population cache and re-enters map 1, then parks the leader at cell (16, 15)
/// with no companions so the relation pass sees exactly one party unit.
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

/// A party inside rectangle A sets `Group+0x1D = 1`, and the nearest member then orders an attack on the player.
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

/// `FUN_004245B4` only raises group activity; it never assigns a combat target.
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

/// A group whose own member stands inside rectangle A stays idle when the party is outside.
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

/// `FUN_00412E00` returns 1 for archetypes `0x51`/`0x52`/`0x53` before any
/// target logic: prop-like actors never acquire.
#[test]
fn prop_archetypes_never_acquire_targets() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x52, 1, 15, 15), 0x41) else {
        return;
    };
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    assert_eq!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target),
        None,
        "archetype 0x52 must not acquire"
    );
}

/// `FUN_00412E00` citizen branch: a woken citizen is forced into battle stance
/// and idles for that frame before it may pick a target.
#[test]
fn woken_citizen_enters_battle_stance_before_acquiring() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x81, 1, 15, 15), 0x81) else {
        return;
    };
    app.wake_group(1, CombatTarget::Player);
    app.update_world_ai();
    let battle = app.population_cache[&1]
        .actors
        .iter()
        .find(|a| a.entity.index == 10)
        .map(|a| crate::anim::battle_from_flags(a.entity.flags_b))
        .unwrap_or(false);
    assert!(battle, "the citizen must be forced into battle stance");
    assert_eq!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target),
        None,
        "no target on the stance-entry frame"
    );
}

/// `LAB_004134b3`: a stray member without an engage order walks to a random
/// cell **inside** rectangle A, not to its clamped corner.
#[test]
fn recall_member_wanders_to_a_random_cell_inside_the_rectangle() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 30, 30), 0x41) else {
        return;
    };
    app.player_x = 200.0;
    app.player_y = 100.0;
    app.combat_rng = crate::combat::OriginalRng::new(0x1234);
    app.update_group_relations();
    assert_eq!(app.tables.as_ref().unwrap().groups[1].activity_1d, 0x20);
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    let goal = m.target.expect("recall route");
    let r = app.tables.as_ref().unwrap().groups[1].rect_a;
    assert!(
        goal.0 >= r.x1 as i32
            && goal.0 <= r.x2 as i32
            && goal.1 >= r.y1 as i32
            && goal.1 <= r.y2 as i32,
        "goal {goal:?} outside rect {r:?}"
    );
    assert!(m.walking, "the member starts walking");
    assert_ne!(
        goal,
        (r.x2 as i32, r.y2 as i32),
        "the recall goal must be a random point, not the clamped corner"
    );
}

/// A stray member that still holds its engage order chases the party **inside
/// its rectangle A** (`FUN_0041FB64` object-group branch) instead of wandering.
///
/// Recall (`0x20`) is normally set while the party is outside the rectangle, so
/// this models the 16-frame window where the party entered it after the relation
/// pass: the recorded activity is still `0x20`.
#[test]
fn engaged_stray_chases_the_party_in_recall_mode() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 30, 30), 0x41) else {
        return;
    };
    app.player_x = 18.0;
    app.player_y = 18.0;
    app.tables.as_mut().unwrap().groups[1].activity_1d = 0x20;
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .engaged = true;
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(
        m.combat_target,
        Some(CombatTarget::Player),
        "an engaged stray must chase a party member inside rect A"
    );
}

/// With no party member inside rectangle A, `FUN_0041FB64` finds no group: an
/// engaged stray does not chase (it stays on its recall/wander path).
#[test]
fn engaged_stray_outside_the_rect_finds_no_hostile_group() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 30, 30), 0x41) else {
        return;
    };
    app.player_x = 200.0;
    app.player_y = 100.0;
    app.tables.as_mut().unwrap().groups[1].activity_1d = 0x20;
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .engaged = true;
    app.update_world_ai();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert_eq!(
        m.combat_target, None,
        "no party member inside rect A means no hostile group"
    );
}

/// `FUN_0041FF68` shared branch: the acquirer adopts the target of the **first
/// engaged member** of its own group (member order), not the first engaged
/// entry of the enemy list.
#[test]
fn shared_target_follows_member_order() {
    let Some(mut app) = ant_pair_app((20, 15), Some((17, 15))) else {
        return;
    };
    // A third ant is the acquirer; the other two are already engaged.
    {
        let t = app.tables.as_mut().unwrap();
        t.entities[12] = entity(12, 0, 0x41, 1, 19, 15);
        t.groups[1].entity_count = 3;
    }
    app.population_cache.clear();
    app.ensure_population(1);
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let first = pop.motion.get_mut(&10).unwrap();
        first.engaged = true;
        first.combat_target = Some(CombatTarget::Party(0));
        let second = pop.motion.get_mut(&11).unwrap();
        second.engaged = true;
        second.combat_target = Some(CombatTarget::Player);
    }
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    let target = app.population_cache[&1]
        .motion
        .get(&12)
        .and_then(|m| m.combat_target);
    // The enemy list is [leader, companion], so an enemy-order search would have
    // returned `Player`; member order returns ant 10's target.
    assert_eq!(
        target,
        Some(CombatTarget::Party(0)),
        "the shared target must come from the first engaged member"
    );
}

/// `FUN_004123BC` case 5 tail: archetype `0x49` alone draws one quadrant and
/// walks to `cell + offset` in mode 8 (which drops its target and engage bit).
#[test]
fn post_attack_reposition_issues_a_far_step_aside() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x49, 1, 100, 80), 0x41) else {
        return;
    };
    app.combat_rng = crate::combat::OriginalRng::new(1234);
    let cell = app.population_cache[&1].motion.get(&10).unwrap().cell;
    let mut reference = app.combat_rng.clone();
    let quadrant = reference.roll() % 4;
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let m = pop.motion.get_mut(&10).unwrap();
        m.combat_target = Some(CombatTarget::Player);
        m.engaged = true;
        m.target_marker = 1;
    }
    app.post_attack_reposition(10);
    let expected = match quadrant {
        0 => (cell.0 - 40, cell.1 - 9),
        1 => (cell.0 + 40, cell.1 - 9),
        2 => (cell.0 - 20, cell.1 - 18),
        _ => (cell.0 - 20, cell.1 + 18),
    };
    let m = app.population_cache[&1].motion.get(&10).unwrap();
    assert_eq!(
        m.target,
        Some(expected),
        "the step-aside goal is cell + quadrant offset"
    );
    assert_eq!(m.combat_target, None, "mode 8 drops the target");
    assert!(!m.engaged, "mode 8 clears the engage bit");
    assert_eq!(app.combat_rng.roll(), reference.roll(), "one draw only");
}

/// Every other archetype short-circuits on `archetype != 0x49`: it draws nothing
/// and keeps its action.
#[test]
fn post_attack_reposition_skips_other_archetypes() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 100, 80), 0x41) else {
        return;
    };
    app.combat_rng = crate::combat::OriginalRng::new(1234);
    let mut reference = app.combat_rng.clone();
    app.post_attack_reposition(10);
    let m = app.population_cache[&1].motion.get(&10).unwrap();
    assert_eq!(m.target, None, "0x41 never repositions");
    assert_eq!(app.combat_rng.roll(), reference.roll(), "no draw either");
}

/// A kill pays `power / 4` **experience** and the matching mastery — not money
/// (`FUN_0041079C` + `Entity+0x40`/`+0x44`).
#[test]
fn kill_rewards_pay_experience_and_mastery_not_money() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 20, 15), 0x41) else {
        return;
    };
    let xp = app.player.available_xp;
    let money = app.player.money;
    let melee = app.player.melee_mastery;
    let archery = app.player.archery_mastery;

    app.award_kill_reward(CombatTarget::Player, 400, false);
    assert_eq!(app.player.available_xp, xp + 100, "power/4 experience");
    assert_eq!(app.player.money, money, "a kill pays no money");
    assert_eq!(app.player.archery_mastery, archery + 400);
    assert_eq!(app.player.melee_mastery, melee);

    app.award_kill_reward(CombatTarget::Player, 400, true);
    assert_eq!(app.player.available_xp, xp + 200);
    assert_eq!(app.player.melee_mastery, melee + 400);
    assert_eq!(app.player.archery_mastery, archery + 400);
}

/// With AUTO set the reward is spent immediately (`FUN_00410B08` rows).
#[test]
fn auto_spends_the_kill_reward() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 20, 15), 0x41) else {
        return;
    };
    app.player.flags_b |= 0x02;
    let level = app.player.level;
    app.award_kill_reward(CombatTarget::Player, 4000, false);
    assert!(
        app.player.available_xp < 1000 || app.player.level > level,
        "AUTO must spend part of the grant"
    );
}

/// An approaching world actor commits at most one **direction step** per logic
/// tick. The isometric tables (`0x48010C`/`0x480110`) can move two cells on one
/// axis (direction 6 = `(+2, 0)`), so the invariant is one table delta, not one
/// cell of Chebyshev distance.
#[test]
fn approach_moves_at_most_one_direction_step_per_tick() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 20, 15), 0x41) else {
        return;
    };
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let m = pop.motion.get_mut(&10).unwrap();
        m.combat_target = Some(CombatTarget::Player);
        m.target_marker = 1;
        m.engaged = true;
    }
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    let mut cell = app.population_cache[&1].motion.get(&10).unwrap().cell;
    for tick in 0..6 {
        app.update_world_ai();
        app.update_player_combat();
        app.update_party_combat();
        app.update_world_combat();
        let now = app.population_cache[&1].motion.get(&10).unwrap().cell;
        let single_step = (0..8u8).any(|d| crate::coords::step_by_direction(cell.0, cell.1, d) == now);
        assert!(
            now == cell || single_step,
            "tick {tick}: {cell:?} -> {now:?} is not one direction step"
        );
        cell = now;
    }
}

/// A member outside rectangle A sets the `0x20` return flag and starts walking back.
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

/// The citizen wander pass leaves a live combat target and its attack frame untouched.
#[test]
fn citizen_combat_target_is_not_reset_by_wander_pass() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x81, 1, 15, 15), 0x81) else {
        return;
    };
    // Synthetic helper always uses group 1; use a citizen class/archetype.
    app.wake_group(1, CombatTarget::Player);
    // `FUN_00412E00` first forces a woken citizen into battle stance and idles
    // for that frame; the target is picked on a later AI turn.
    assert!(
        tick_until(
            &mut app,
            |a| a.population_cache[&1]
                .motion
                .get(&10)
                .and_then(|m| m.combat_target)
                == Some(CombatTarget::Player),
            5
        ),
        "the citizen never acquired the player"
    );
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

/// The relation pass tests `ActorMotion.cell`, not the authored `Entity+0x08/+0x0A` position.
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

/// An object group with activity `0` holds its members on the idle row at frame 0.
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

/// `FUN_00410E84` reads the bow from `equipment[1]` (`Entity+0x68`), never from
/// the melee slot 0, and requires the bow to be the active slot with ammo.
#[test]
fn armed_gate_reads_the_bow_slot() {
    let mut equipment = [0u16; 6];
    equipment[0] = 11; // a melee weapon alone is not an archer
    assert_eq!(crate::app::combat::armed_bow(1, &equipment, 27), None);
    equipment[1] = 23; // bow in slot 1
    assert_eq!(crate::app::combat::armed_bow(1, &equipment, 27), Some(23));
    // The active slot must be the bow (`Entity+0x12 == 1`).
    assert_eq!(crate::app::combat::armed_bow(0, &equipment, 27), None);
    assert_eq!(crate::app::combat::armed_bow(2, &equipment, 27), None);
    // Ammo must be loaded (`Entity+0xCC`).
    assert_eq!(crate::app::combat::armed_bow(1, &equipment, 0), None);
}

/// An armed archer (bow 23 in slot 1, ammo 27) in group 1 at `cell`, with the
/// item tables attached so `FUN_0041130C` can resolve the bow subtype.
fn armed_archer_app(cell: (u16, u16)) -> Option<PlayableApp> {
    let mut app = synthetic_app(entity(10, 0, 0x21, 1, cell.0, cell.1), 0x41)?;
    {
        let pop = app.population_cache.get_mut(&1)?;
        let a = pop.actors.iter_mut().find(|a| a.entity.index == 10)?;
        a.entity.equipment[1] = 23;
        a.entity.special_equipment = 27;
        a.entity.raw_0x12 = 1;
        // The wake frame (`FUN_00412E00`: refresh + forced battle stance) has
        // already happened; these tests exercise the ranged cycle itself.
        a.entity.flags_b |= crate::anim::FLAG_BATTLE;
    }
    if let Ok(save) = std::fs::read(install_dir().join("Game.1")) {
        app.world.res.set_save_game(&save);
    }
    app.world.res.set_item_quantity(27, 30);
    app.world.res.set_item_durability(23, 56.0);
    app.tables.as_mut()?.groups[1].activity_1d = 1;
    app.tables.as_mut()?.groups[1].target_group = 0;
    Some(app)
}

/// `FUN_0041ECC8` `+0x02 & 4`: the archer gate is re-checked every cycle. Ammo
/// gone toggles the ranged bit off and leaves the stored target alone.
#[test]
fn ranged_cycle_drops_the_order_when_the_bow_gate_fails() {
    let Some(mut app) = armed_archer_app((20, 15)) else {
        return;
    };
    app.update_world_ai();
    app.update_world_combat();
    assert!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target)
            .is_some(),
        "precondition: the archer acquired a target"
    );
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let a = pop
            .actors
            .iter_mut()
            .find(|a| a.entity.index == 10)
            .unwrap();
        a.entity.special_equipment = 0; // ammo gone
    }
    app.update_world_combat();
    let m = app.population_cache[&1].motion.get(&10).unwrap();
    assert!(!m.ranged_order, "the gate must clear bit 4");
    assert!(
        m.combat_target.is_some(),
        "the gate branch keeps the stored target"
    );
}

/// `FUN_0041ECC8`: a probe result of `-1` (target closer than three cells)
/// clears the target and the ranged order.
#[test]
fn ranged_cycle_clears_a_too_close_target() {
    let Some(mut app) = armed_archer_app((17, 15)) else {
        return;
    };
    app.update_world_ai();
    assert!(
        app.population_cache[&1]
            .motion
            .get(&10)
            .and_then(|m| m.combat_target)
            .is_some(),
        "precondition: the nearest-target fallback acquired the player"
    );
    app.update_world_combat();
    let m = app.population_cache[&1].motion.get(&10).unwrap();
    assert_eq!(m.combat_target, None, "a too-close target is dropped");
    assert!(!m.ranged_order, "the ranged bit is cleared");
}

/// An armed archer that acquires the player switches to action row 4 and latches a ranged order.
#[test]
fn ranged_world_actor_switches_to_action_row_four() {    let Some(mut app) = synthetic_app(entity(10, 0, 0x21, 1, 20, 15), 0x41) else {
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
        // Already past the wake frame (see `armed_archer_app`).
        a.entity.flags_b |= crate::anim::FLAG_BATTLE;
    }
    if let Ok(save) = std::fs::read(install_dir().join("Game.1")) {
        app.world.res.set_save_game(&save);
    }
    app.world.res.set_item_quantity(27, 30);
    app.world.res.set_item_durability(23, 56.0);
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    assert!(
        app.ai_schedule.actions() >= 1,
        "the armed pick must charge the FUN_004116CC budget"
    );
    app.update_world_combat();
    let m = app.population_cache[&1].motion.get(&10).expect("motion");
    assert!(m.ranged_order);
    assert_eq!(m.action, crate::anim::ACT_RANGED as u8);
}

/// `FUN_004105A0` runs from the wake path (`FUN_00412E00`): a peaceful
/// non-object member of a woken group equips its bag gear, enters battle stance,
/// charges **one** AI action and idles for that frame.
#[test]
fn woken_actor_refreshes_gear_and_charges_one_action() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x21, 1, 20, 15), 0x41) else {
        return;
    };
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let a = pop.actors.iter_mut().find(|a| a.entity.index == 10).unwrap();
        a.entity.equipment[1] = 0;
        a.entity.special_equipment = 0;
        a.entity.raw_0x12 = 2;
        a.entity.flags_b &= !crate::anim::FLAG_BATTLE;
        a.entity.inventory[0] = 23; // bow
        a.entity.inventory[1] = 27; // ammo
        a.entity.stats.dexterity_current = 100;
        a.entity.stats.dexterity_base = 100;
        a.entity.stats.strength_current = 100;
        a.entity.stats.strength_base = 100;
    }
    if let Ok(save) = std::fs::read(install_dir().join("Game.1")) {
        app.world.res.set_save_game(&save);
    }
    app.world.res.set_item_quantity(27, 30);
    app.tables.as_mut().unwrap().groups[1].activity_1d = 1;
    app.tables.as_mut().unwrap().groups[1].target_group = 0;
    app.update_world_ai();
    let pop = &app.population_cache[&1];
    let a = pop.actors.iter().find(|a| a.entity.index == 10).unwrap();
    assert_eq!(a.entity.equipment[1], 23, "bow auto-equipped from the bag");
    assert_eq!(a.entity.special_equipment, 27, "ammo auto-filled");
    assert_eq!(a.entity.raw_0x12, 1, "bow becomes the active weapon slot");
    assert!(
        crate::anim::battle_from_flags(a.entity.flags_b),
        "the wake frame forces battle stance"
    );
    assert!(
        app.ai_schedule.actions() >= 1,
        "the equipment refresh charges FUN_004116CC"
    );
    assert!(
        pop.motion.get(&10).and_then(|m| m.combat_target).is_none(),
        "the wake frame idles instead of acquiring"
    );
}

/// A `0x4D` object walk cycle eventually seeds the `Entity+0x12` walk delay.
#[test]
fn mo_walk_completion_seeds_entity_0x12_delay() {    let Some(mut app) = synthetic_app(entity(10, 0, 0x4D, 1, 15, 15), 0x4D) else {
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

/// Two ants in synthetic group 1 plus an optional companion, so the persistent
/// engage order's re-acquisition pass (`FUN_0041E2D4`) can be exercised.
fn ant_pair_app(leader: (i32, i32), companion: Option<(i32, i32)>) -> Option<PlayableApp> {
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
    groups[1] = group(1, 10, 2, 0x41, rect);
    let mut entities = vec![entity(0, 0, 0, 0, 0, 0); 2000];
    for (i, e) in entities.iter_mut().enumerate() {
        e.index = i;
    }
    entities[10] = entity(10, 0, 0x41, 1, 16, 15);
    entities[11] = entity(11, 0, 0x41, 1, 18, 15);
    app.tables = Some(GameTables { groups, entities });
    app.population_cache.clear();
    app.current_map = 1;
    app.ensure_population(1);
    app.player_x = leader.0 as f64;
    app.player_y = leader.1 as f64;
    app.player_render_x = app.player_x;
    app.player_render_y = app.player_y;
    app.player.dead = false;
    app.party.clear();
    if let Some(cell) = companion {
        app.party.push(PartyMember {
            entity: entity(20, 0, 0x00, 0, cell.0 as u16, cell.1 as u16),
            motion: ActorMotion {
                cell,
                render_cell: cell,
                ..Default::default()
            },
        });
        app.party_selected[1] = true;
    }
    Some(app)
}

/// Step whole logic frames (`FUN_00421B50`) until `done` holds.
///
/// `FUN_004116CC` lets a unit take a turn only when the cursor is behind it, so
/// the frame after a unit acted it can be skipped; the original behaves the same
/// way. Tests therefore assert "within a few frames", not "after one call".
fn tick_until(
    app: &mut PlayableApp,
    mut done: impl FnMut(&PlayableApp) -> bool,
    frames: usize,
) -> bool {
    for _ in 0..frames {
        app.combat_tick_once();
        if done(app) {
            return true;
        }
    }
    false
}

/// `A` writes the persistent engage order and clears the concrete target; the
/// next leader tick acquires the nearest hostile group member (`FUN_0041E2D4`).
#[test]
fn engage_button_writes_persistent_order_and_acquires_next_tick() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.update_group_relations();
    assert!(
        tick_until(
            &mut app,
            |a| a.population_cache[&1]
                .motion
                .get(&10)
                .and_then(|m| m.combat_target)
                == Some(CombatTarget::Player),
            4
        ),
        "the ant never acquired the player"
    );
    app.player_attack_target = None;
    app.party_engage();
    assert!(
        crate::anim::battle_from_flags(app.player.flags_b),
        "engage must put the leader in battle stance"
    );
    assert!(
        app.player_engaged,
        "engage must write the persistent order bit"
    );
    assert_eq!(
        app.player_attack_target, None,
        "the original engage order assigns no concrete target"
    );
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(10)),
            4
        ),
        "the first ordered tick must acquire the nearest hostile"
    );
}

/// An engaged companion keeps killing the next ant after each kill instead of
/// stopping after one (`FUN_0041E2D4`/`FUN_0041FB64`/`FUN_00420190`).
#[test]
fn companion_reacquires_the_next_ant_after_a_kill() {
    let Some(mut app) = ant_pair_app((20, 15), Some((17, 15))) else {
        return;
    };
    app.party_engage();
    assert!(app.party[0].motion.engaged);
    assert!(
        tick_until(
            &mut app,
            |a| a.party[0].motion.combat_target == Some(CombatTarget::World(10)),
            4
        ),
        "the companion did not acquire the first ant"
    );
    // First ant dies: the engaged companion must pick the next one.
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .dead = true;
    assert!(
        tick_until(
            &mut app,
            |a| a.party[0].motion.combat_target == Some(CombatTarget::World(11)),
            4
        ),
        "engaged companion stopped after a single kill"
    );
    // Last ant dies: no hostile group left, the order ends.
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&11)
        .unwrap()
        .dead = true;
    assert!(
        tick_until(
            &mut app,
            |a| a.party[0].motion.combat_target.is_none() && !a.party[0].motion.engaged,
            4
        ),
        "the order must clear when no hostile remains"
    );
}

/// The leader uses the same persistent order and re-acquires after a kill.
#[test]
fn leader_reacquires_the_next_ant_after_a_kill() {
    let Some(mut app) = ant_pair_app((17, 15), None) else {
        return;
    };
    app.party_engage();
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(10)),
            4
        ),
        "the leader did not acquire the first ant"
    );
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .dead = true;
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(11)),
            4
        ),
        "engaged leader stopped after a single kill"
    );
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&11)
        .unwrap()
        .dead = true;
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target.is_none() && !a.player_engaged,
            4
        ),
        "the order must clear when no hostile remains"
    );
}

/// `FUN_0041FB64` only searches the leader's +/-30x/13y area: a hostile group
/// beyond it never enters the order.
#[test]
fn reacquisition_ignores_hostiles_outside_the_leader_area() {
    let Some(mut app) = ant_pair_app((20, 15), Some((17, 15))) else {
        return;
    };
    for (id, cell) in [(10, (200, 100)), (11, (210, 100))] {
        app.population_cache
            .get_mut(&1)
            .unwrap()
            .motion
            .get_mut(&id)
            .unwrap()
            .cell = cell;
    }
    app.party_engage();
    app.update_party_combat();
    assert_eq!(app.party[0].motion.combat_target, None);    assert!(
        !app.party[0].motion.engaged,
        "a hostile outside the leader area must not start a fight"
    );
}

/// `FUN_0041E2D4` clears the ranged bit (`+0x02 &= 0xFB`) on a successful
/// re-acquire: after a kill the unit falls back to the melee/approach cycle
/// until a new order re-arms the bow.
#[test]
fn reacquire_clears_the_ranged_order() {
    let Some(mut app) = ant_pair_app((17, 15), None) else {
        return;
    };
    app.party_engage();
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(10)),
            4
        ),
        "the leader did not acquire the first ant"
    );
    app.player_attack_ranged = true;
    app.population_cache
        .get_mut(&1)
        .unwrap()
        .motion
        .get_mut(&10)
        .unwrap()
        .dead = true;
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(11)),
            4
        ),
        "the leader did not re-acquire"
    );
    assert!(
        !app.player_attack_ranged,
        "a re-acquire must clear the ranged order"
    );
}

/// `FUN_0041EB48` needs the attacker's engage order (`Entity+0x02 & 1`): a
/// foreign actor that merely stands next to the leader never triggers
/// retaliation.
#[test]
fn retaliation_requires_the_attackers_engage_order() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    // No activity pass: the ant must not re-acquire on its own.
    app.tables.as_mut().unwrap().groups[1].activity_1d = 0;
    {
        let pop = app.population_cache.get_mut(&1).unwrap();
        let m = pop.motion.get_mut(&10).unwrap();
        m.combat_target = None;
        m.engaged = false;
    }
    for _ in 0..3 {
        app.combat_tick_once();
    }
    assert_eq!(
        app.player_attack_target, None,
        "an attacker without an engage order must not provoke retaliation"
    );
}

/// An idle leader acquires an adjacent attacker that already targets it (`FUN_0041EB48`).
#[test]
fn idle_party_retaliates_against_adjacent_attacker() {
    let Some(mut app) = synthetic_app(entity(10, 0, 0x41, 1, 15, 15), 0x41) else {
        return;
    };
    app.update_group_relations();
    assert!(
        tick_until(
            &mut app,
            |a| a.population_cache[&1]
                .motion
                .get(&10)
                .and_then(|m| m.combat_target)
                == Some(CombatTarget::Player),
            4
        ),
        "the ant never acquired the player"
    );
    app.player_attack_target = None;
    app.player_path.clear();
    app.player_active = false;
    assert!(
        tick_until(
            &mut app,
            |a| a.player_attack_target == Some(CombatTarget::World(10)),
            4
        ),
        "idle leader did not retaliate against an adjacent attacker"
    );
}
