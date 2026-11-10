//! Cursor selection parity (`FUN_00428740`).

use super::super::cursor::{
    cursor_for_class, select_cursor, CursorActor, CursorHover, CursorState,
};

use super::common::*;

fn default_state() -> CursorState {
    CursorState::default()
}

#[test]
fn select_cursor_matches_original_precedence() {
    // Base id != 4 always wins (item in hand), even over actors/props.
    let monster = CursorHover {
        actor: Some(CursorActor {
            group_id: 3,
            dialogue_id: 0xFF,
        }),
        ..Default::default()
    };
    assert_eq!(select_cursor(10, &monster, &default_state()), 10);

    // Plain free ground.
    let free = CursorHover::default();
    assert_eq!(select_cursor(4, &free, &default_state()), 4);

    // Static/entity occupancy -> "no".
    let blocked = CursorHover {
        occupied: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &blocked, &default_state()), 3);

    // Ground WorldObject cell (0x2000) and free -> hand.
    let marker = CursorHover {
        flag_2000: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &marker, &default_state()), 7);

    // Marker cell also occupied by an entity -> "no".
    let marker_occupied = CursorHover {
        flag_2000: true,
        occupied: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &marker_occupied, &default_state()), 3);

    // Interactive prop without a ready archer blocks the ground branch (base).
    let prop = CursorHover {
        prop: true,
        flag_2000: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &prop, &default_state()), 4);

    // Prop + settlement + selected archer ready -> arrow frame.
    let ready = CursorState {
        settlement: true,
        ranged_ready: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &prop, &ready), 6);
    let no_settlement = CursorState {
        settlement: false,
        ranged_ready: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &prop, &no_settlement), 4);

    // Own party / leader: neutral, Ctrl -> talk.
    let party = CursorHover {
        actor: Some(CursorActor {
            group_id: 0,
            dialogue_id: 4,
        }),
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &party, &default_state()), 4);
    let ctrl = CursorState {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &party, &ctrl), 5);

    // Other actor in peace: talkable -> talk, monster -> "no".
    let npc = CursorHover {
        actor: Some(CursorActor {
            group_id: 5,
            dialogue_id: 7,
        }),
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &npc, &default_state()), 5);
    let monster = CursorHover {
        actor: Some(CursorActor {
            group_id: 5,
            dialogue_id: 0xFF,
        }),
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &monster, &default_state()), 3);

    // A selected unit in battle stance attacks any other actor.
    let battle = CursorState {
        party_battle: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &npc, &battle), 1);
    assert_eq!(select_cursor(4, &monster, &battle), 1);

    // Shift forces the attack/"no" frame; leader battle stance suppresses talk.
    let shift = CursorState {
        shift: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &npc, &shift), 3);
    let leader_battle = CursorState {
        leader_battle: true,
        ..Default::default()
    };
    assert_eq!(select_cursor(4, &npc, &leader_battle), 3);

    // Prop precedence: ready archer -> 6 even when an actor is also under it;
    // not ready -> the actor branch decides.
    let both = CursorHover {
        prop: true,
        actor: Some(CursorActor {
            group_id: 5,
            dialogue_id: 7,
        }),
        flag_2000: true,
        occupied: false,
    };
    assert_eq!(select_cursor(4, &both, &ready), 6);
    assert_eq!(select_cursor(4, &both, &default_state()), 5);
}

#[test]
fn hand_cursor_over_ground_world_object_cell() {
    let Some(mut app) = build_movement_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let cell = (110, 60);
    let (lx, ly) = crate::coords::entity_to_local_px_f(cell.0 as f64, cell.1 as f64);
    app.detail_pan = [140.0 - lx.round(), 300.0 - ly.round()];
    app.cursor_virtual = cell_screen(&app, cell.0, cell.1);
    app.active_world_objects = vec![crate::world_objects::ActiveWorldObject {
        source: 0,
        x: cell.0,
        y: cell.1,
        money: 0,
        items: vec![1007],
        sprite: 168,
    }];
    assert_eq!(app.debug_cursor_frame(), 7, "ground marker hand");

    // The same cell without the marker is ordinary free ground.
    app.active_world_objects.clear();
    assert_eq!(app.debug_cursor_frame(), 4, "free ground");

    // Authored static occupancy -> "no".
    let idx = (cell.0 * 160 + cell.1) as usize;
    app.world.locs[0].collision[idx] = 0x0FFF;
    assert_eq!(app.debug_cursor_frame(), 3, "blocked cell");
    app.world.locs[0].collision[idx] = 0;
}

#[test]
fn held_item_selects_class_cursor_and_disables_hover() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Some((id, class)) =
        (1..8192u16).find_map(|id| app.world.res.item_class(id).map(|c| (id, c)))
    else {
        eprintln!("skipping: no item instances");
        return;
    };
    let (lx, ly) = crate::coords::entity_to_local_px_f(150.0, 100.0);
    app.detail_pan = [200.0 - lx.round(), 300.0 - ly.round()];
    app.cursor_virtual = (200.0, 300.0);
    assert_eq!(app.debug_cursor_frame(), 4);
    app.held_item = Some(crate::inventory::HeldItem {
        id,
        owner: crate::inventory::InventoryOwner::Protagonist,
        source: None,
    });
    assert_eq!(app.debug_cursor_frame(), cursor_for_class(class));
    app.held_item = None;
    assert_eq!(app.debug_cursor_frame(), 4);
}

#[test]
fn hand_over_authored_herb_marker() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    app.rebuild_world_objects();
    let Some(cell) = app
        .active_world_objects
        .iter()
        .find(|o| o.money >= 0)
        .map(|o| o.cell())
    else {
        eprintln!("skipping: Game.1 has no visible WorldObjects on loc 4");
        return;
    };
    let (lx, ly) = crate::coords::entity_to_local_px_f(cell.0 as f64, cell.1 as f64);
    app.detail_pan = [300.0 - lx.round(), 300.0 - ly.round()];
    app.cursor_virtual = (300.0, 300.0);
    assert_eq!(
        app.debug_cursor_frame(),
        7,
        "authored WorldObject cell {cell:?} must show the hand"
    );
}

#[test]
fn interactive_flag_is_parsed_from_placement_record() {
    let dir = install_dir();
    let Ok(bytes) = std::fs::read(dir.join("Konung.4")) else {
        eprintln!("skipping: Konung.4 not present");
        return;
    };
    let loc = crate::loc::make_location(4, bytes).expect("Konung.4");
    // Placement 32 in loc 4 carries record +0x22 = 0xFF (interactive).
    let prop = loc.props.iter().find(|p| p.ri == 32).expect("placement 32");
    assert!(prop.interactive);
    assert!(loc.props.iter().any(|p| !p.interactive));
}

#[test]
fn selected_stance_and_ranged_ready_predicates() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.party_selected[0] = true;
    app.set_ctrl(true);
    assert!(app.ctrl_down);
    app.set_ctrl(false);
    assert!(!app.ctrl_down);
    assert!(!app.selected_party_in_battle());

    app.player.flags_b |= crate::anim::FLAG_BATTLE;
    assert!(app.selected_party_in_battle());

    // The cursor's archer gate is deliberately looser than the firing gate:
    // bow slot + bow id + ammo id + plain ammo instance.
    app.player.raw_0x12 = 1;
    app.player.equipment[1] = 1;
    let Some(ammo) = (1..8192u16).find(|&id| {
        app.world
            .res
            .item_instance(id)
            .map(|i| i.modifier_index == 0)
            .unwrap_or(false)
    }) else {
        eprintln!("skipping: no plain ammo instance");
        return;
    };
    app.player.special_equipment = ammo;
    assert!(app.cursor_ranged_ready());
    app.player.special_equipment = 0;
    assert!(!app.cursor_ranged_ready());

    // Deselecting the leader removes the stance/ready contribution.
    app.player.special_equipment = ammo;
    app.party_selected[0] = false;
    assert!(!app.selected_party_in_battle());
    assert!(!app.cursor_ranged_ready());
}
