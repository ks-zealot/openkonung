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
