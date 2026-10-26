//! Movie trigger tests (game-over and victory paths).

use super::common::*;
use crate::video::{self, VideoAfter, VideoClip};

#[test]
fn dead_leader_requests_gameover_once() {
    let Some(mut app) = build_test_app() else {
        return;
    };
    video::reset();
    app.player.dead = true;
    app.check_game_over_video();
    let p = video::pending().expect("gameover pending");
    assert_eq!(p.clip, VideoClip::GameOver);
    assert_eq!(p.after, VideoAfter::Menu { game_active: false });
    assert!(app.game_over);
    // Second frame must not queue another request.
    app.check_game_over_video();
    assert_eq!(video::pending().unwrap().clip, VideoClip::GameOver);
    video::reset();
}

#[test]
fn source_zero_world_object_requests_win() {
    use crate::world_objects::ActiveWorldObject;
    let Some(mut app) = build_test_app() else {
        return;
    };
    video::reset();
    app.player_x = 0.0;
    app.player_y = 0.0;
    app.active_world_objects = vec![ActiveWorldObject {
        source: 0,
        x: 0,
        y: 0,
        money: 0,
        items: vec![1007],
        sprite: 0xA3,
    }];
    app.try_pickup_here();
    let p = video::pending().expect("win pending");
    assert_eq!(p.clip, VideoClip::Win);
    assert_eq!(p.after, VideoAfter::Menu { game_active: false });
    assert!(app.active_world_objects[0].items == vec![1007], "pickup is deferred");
    video::reset();
}

#[test]
fn engaged_settlement_defeat_requests_location_movie() {
    use crate::actors::{ActorMotion, ActorPlacement, PopulationBuild, RuntimeActor};
    let Some(mut app) = build_party_app() else {
        return;
    };
    video::reset();
    app.current_map = 4;
    let settlement = app
        .settlements
        .iter()
        .find(|s| s.loc_id == 4 && s.flags_4b2 & 0x80 == 0)
        .expect("settlement on map 4")
        .clone();
    let group_id = settlement.group_id as usize;
    let group = app
        .tables
        .as_mut()
        .unwrap()
        .groups
        .iter_mut()
        .find(|g| g.index == group_id)
        .expect("settlement group");
    group.activity_1d = 1;
    let mut entity = app
        .tables
        .as_ref()
        .unwrap()
        .groups
        .iter()
        .find(|g| g.index == group_id)
        .and_then(|g| app.tables.as_ref().unwrap().entities.get(g.first_entity as usize))
        .expect("group entity")
        .clone();
    entity.stats.level = 1;
    entity.stats.hp_internal = 1;
    let mut pop = PopulationBuild::default();
    pop.actors.push(RuntimeActor {
        entity: entity.clone(),
        source_group_id: group_id,
        placement: ActorPlacement::Persisted,
    });
    pop.motion.insert(entity.index, ActorMotion::default());
    app.population_cache.insert(4, pop);
    app.world_frame = 9;
    app.check_defeat_video();
    let p = video::pending().expect("defeat pending");
    assert_eq!(p.clip, VideoClip::Defeat(4));
    assert_eq!(p.after, VideoAfter::Gameplay);
    assert_ne!(
        app.settlements
            .iter()
            .find(|s| s.index == settlement.index)
            .unwrap()
            .flags_4b2
            & 0x80,
        0
    );
    video::reset();
}

