//! World-object pickup/respawn tests.

use super::super::*;

use super::common::*;

#[test]
fn herb_world_object_pickup_and_respawn() {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::loc::make_location;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let (Ok(game_res), Ok(exe), Ok(konung), Ok(save), Ok(konung3)) = (
        std::fs::read(dir.join("Game.res")),
        std::fs::read(dir.join("CLIENT.EXE")),
        std::fs::read(dir.join("konung.res")),
        std::fs::read(dir.join("Game.1")),
        std::fs::read(dir.join("Konung.3")),
    ) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let mut res = Res::new(game_res, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    res.set_save_game(&save);
    if let Ok(intr) = std::fs::read(dir.join("INTR800R.RES")) {
        let _ = res.load_intr(&intr);
    }
    let loc = make_location(3, konung3).expect("Konung.3");
    let world = World::new(res, vec![loc]);
    let h = *hero(0).unwrap();
    let player =
        PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
    let mut app = PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        3,
        0.0,
        0.0,
        5,
        None,
        Vec::new(),
    );
    app.world_frame = 100;

    let (source_index, item, cell) = app
        .world_object_sources
        .iter()
        .enumerate()
        .find_map(|(i, s)| {
            if s.loc != 3 {
                return None;
            }
            let info = app.world.res.item_info(s.items[0])?;
            (info.class == 9 && info.subtype == 1).then_some((
                i,
                s.items[0],
                (s.x as i32, s.y as i32),
            ))
        })
        .expect("location 3 has Earth-nut nodes");
    app.rebuild_world_objects();
    let active = app
        .active_world_objects
        .iter()
        .find(|o| o.cell() == cell)
        .expect("Earth-nut node active on location 3");
    assert_eq!(active.sprite, 160, "FUN_00438CC8 world sprite");
    if app.world.res.intr.is_some() {
        assert!(
            app.world.res.intr_sprite(160).is_some(),
            "herb world sprite is an occupied INTR index"
        );
    }
    assert_eq!(app.player.inventory, [0; 32]);
    // The marker pass must actually draw the herb sprite at the projected cell.
    let (lx, ly) = crate::coords::entity_to_local_px(cell.0, cell.1);
    app.detail_pan = [
        -(lx as f64 - VIRTUAL_W as f64 / 2.0),
        -(ly as f64 - VIRTUAL_H as f64 / 2.0),
    ];
    app.active_world_objects.clear();
    app.draw_detail();
    let without = app.frame.clone();
    app.rebuild_world_objects();
    app.draw_detail();
    let cx = VIRTUAL_W as usize / 2;
    let cy = VIRTUAL_H as usize / 2;
    let mut changed = 0usize;
    for y in cy.saturating_sub(32)..(cy + 32).min(VIRTUAL_H as usize) {
        for x in cx.saturating_sub(32)..(cx + 32).min(VIRTUAL_W as usize) {
            let o = (y * VIRTUAL_W as usize + x) * 4;
            if without[o..o + 4] != app.frame[o..o + 4] {
                changed += 1;
            }
        }
    }
    assert!(changed > 0, "herb sprite must be blitted into the frame");

    app.player_x = cell.0 as f64;
    app.player_y = cell.1 as f64;
    app.try_pickup_here();
    assert_eq!(
        app.player.inventory[0], item,
        "collected at inventory slot 0"
    );
    assert!(
        !app.active_world_objects.iter().any(|o| o.cell() == cell),
        "runtime marker removed"
    );
    let src = &app.world_object_sources[source_index];
    assert_eq!((src.t0, src.t4), (-100, -100), "deactivation timestamp");

    // Before 0x1518 frames it stays gone on map re-entry.
    app.world_frame = 100 + 0x1518 - 1;
    app.rebuild_world_objects();
    assert!(!app.active_world_objects.iter().any(|o| o.cell() == cell));
    // At the cooldown boundary it reappears with a newly allocated instance.
    app.world_frame = 100 + 0x1518;
    app.rebuild_world_objects();
    let respawned = app
        .active_world_objects
        .iter()
        .find(|o| o.cell() == cell)
        .expect("herb respawned after 5400 frames");
    assert_eq!(respawned.sprite, 160);
    assert_ne!(app.world_object_sources[source_index].items[0], item);
    let info = app
        .world
        .res
        .item_info(app.world_object_sources[source_index].items[0])
        .unwrap();
    assert_eq!((info.class, info.subtype), (9, 1));
}

#[test]
fn settlement_linkage_and_prop_flames_tick() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let dir = install_dir();
    let Ok(flames) = std::fs::read(dir.join("FLAMES.RES")) else {
        eprintln!("skipping: FLAMES.RES not present");
        return;
    };
    let (Ok(game), Ok(graph), Ok(objects), Ok(exe), Ok(konung), Ok(save)) = (
        std::fs::read(dir.join("Game.res")),
        std::fs::read(dir.join("GRAPH.RES")),
        std::fs::read(dir.join("Objects.res")),
        std::fs::read(dir.join("CLIENT.EXE")),
        std::fs::read(dir.join("konung.res")),
        std::fs::read(dir.join("Game.1")),
    ) else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let mut res = crate::res::Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    res.set_save_game(&save);
    if res.load_flames(&flames).is_err() {
        eprintln!("skipping: FLAMES.RES did not decode");
        return;
    }
    app.world.res = res;

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
    let loc = crate::loc::Location {
        loc_id: 4,
        data: Vec::new(),
        props: vec![prop],
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    app.world.loc_index.insert(4, app.world.locs.len());
    app.world.locs.push(loc);
    app.current_map = 4;

    // key 0 base = 7; state 3 -> object record index 10 -> object id 40.
    let mut settlement = crate::population::Settlement::default();
    settlement.loc_id = 4;
    settlement.group_id = 1;
    settlement.targets = vec![crate::population::SettlementTarget {
        key: 0,
        state: 3,
        prop_index: 1,
        timer: 0,
    }];
    app.settlements = vec![settlement];
    app.apply_settlement_props();
    assert_eq!(app.world.locs[0].props[0].object_id, 40);

    // Object 41 has 7 flame emitters; ticking must advance their phases.
    let emitters = app.world.res.object_flames(41);
    assert_eq!(emitters.len(), 7);
    {
        let p = &mut app.world.locs[0].props[0];
        p.object_id = 41;
        p.flame_emitters = emitters;
        p.flame_frames = vec![0; 7];
    }
    let before = app.world.locs[0].props[0].flame_frames.clone();
    app.world.tick_prop_flames(4);
    let after = app.world.locs[0].props[0].flame_frames.clone();
    assert_ne!(before, after, "flame frames did not advance");

    let sprites = app.world.collect_props(4);
    assert!(
        sprites.len() >= 8,
        "expected the prop plus 7 flame sprites, got {}",
        sprites.len()
    );
}

fn push_dig_spot(
    app: &mut PlayableApp,
    loc: u32,
    cell: (i32, i32),
    money: i16,
    item: u16,
) -> usize {
    let index = app.world_object_sources.len();
    let mut items = [0u16; crate::world_objects::WORLD_OBJECT_ITEM_SLOTS];
    items[0] = item;
    app.world_object_sources
        .push(crate::world_objects::WorldObjectSource {
            index,
            t0: 0,
            t4: 0,
            x: cell.0 as u16,
            y: cell.1 as u16,
            money,
            loc: loc as u16,
            items,
        });
    index
}

fn free_inventory_slot(app: &PlayableApp) -> usize {
    app.player
        .inventory
        .iter()
        .position(|&v| v == 0)
        .expect("free inventory slot")
}

#[test]
fn e1_buried_spot_needs_the_shovel_to_be_collected() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    let item = app
        .world
        .res
        .alloc_item_instance(4, 69, 0, -1, 1.0, 0, 0)
        .expect("free item instance");
    let cell = (40, 40);
    let source = push_dig_spot(&mut app, 4, cell, -100, item);
    app.rebuild_world_objects();
    assert!(
        app.active_world_objects.iter().any(|o| o.cell() == cell),
        "a buried record is part of the active marker pool"
    );

    app.player_x = cell.0 as f64;
    app.player_y = cell.1 as f64;
    let money = app.player.money;
    app.try_pickup_here();
    assert!(
        !app.player.inventory.contains(&item),
        "FUN_00421B50: a buried spot is skipped without FUN_00433A8C(0x19)"
    );
    assert_eq!(app.player.money, money);
    assert!(app
        .active_world_objects
        .iter()
        .any(|o| o.cell() == cell));

    let shovel = app
        .world
        .res
        .alloc_item_instance(11, 24, 0, -1, 1.0, 0, 0)
        .expect("free shovel instance");
    let slot = free_inventory_slot(&app);
    app.player.inventory[slot] = shovel;

    app.try_pickup_here();
    assert!(app.player.inventory.contains(&item), "shovel digs it up");
    assert_eq!(app.player.money, money + 100, "FUN_0043EAA2 is abs(money)");
    assert!(!app
        .active_world_objects
        .iter()
        .any(|o| o.cell() == cell));
    assert_eq!(
        app.world_object_sources[source].loc, 0,
        "an emptied non-herb record is permanently removed (FUN_0041093C)"
    );
}

#[test]
fn e1_copper_mirror_reveals_a_buried_spot() {
    let Some(mut app) = build_party_app_with_free_location() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    app.world_frame = 100;
    let item = app
        .world
        .res
        .alloc_item_instance(4, 69, 0, -1, 1.0, 0, 0)
        .expect("free item instance");
    let cell = (60, 30);
    let source = push_dig_spot(&mut app, 4, cell, -100, item);
    app.rebuild_world_objects();

    let (lx, ly) = crate::coords::entity_to_local_px(cell.0, cell.1);
    app.detail_pan = [
        -(lx as f64 - VIRTUAL_W as f64 / 2.0),
        -(ly as f64 - VIRTUAL_H as f64 / 2.0),
    ];
    app.cursor_virtual = (VIRTUAL_W as f64 / 2.0, VIRTUAL_H as f64 / 2.0);
    assert_ne!(
        app.cursor_index(),
        7,
        "FUN_00428740 hides the ground cursor for negative money"
    );

    let mirror = app
        .world
        .res
        .alloc_item_instance(11, 48, 0, -1, 1.0, 0, 0)
        .expect("free mirror instance");
    let slot = free_inventory_slot(&app);
    app.player.inventory[slot] = mirror;
    assert!(app.use_class11_item(
        crate::inventory::InventoryOwner::Protagonist,
        mirror,
        None
    ));
    assert_eq!(app.world_object_sources[source].money, 100);
    assert!(app
        .active_world_objects
        .iter()
        .any(|o| o.cell() == cell && o.money > 0));
    assert_eq!(
        app.cursor_index(),
        7,
        "the revealed spot shows the ground cursor"
    );

    app.player_x = cell.0 as f64;
    app.player_y = cell.1 as f64;
    app.try_pickup_here();
    assert!(
        app.player.inventory.contains(&item),
        "a revealed spot needs no shovel"
    );
    assert_eq!(app.world.res.item_class(mirror), Some(0xFF));
}
