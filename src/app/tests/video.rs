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
    // The defeat check uses `FUN_0041B658` power, not HP: make the single
    // defender a zero-power human so the party ratio is infinite.
    let first = {
        let tables = app.tables.as_mut().unwrap();
        let group = tables.groups.get_mut(group_id).expect("settlement group");
        group.activity_1d = 1;
        group.entity_count = 1;
        group.first_entity as usize
    };
    {
        let entity = app.tables.as_mut().unwrap().entities.get_mut(first).unwrap();
        entity.kind = 6;
        entity.archetype &= 0x7F;
        entity.visual_index = 0;
        entity.flags_a = 0x40;
        entity.stats = Default::default();
        entity.stats.hp_internal = 1600;
        entity.equipment = [0; 6];
        entity.inventory = [0; 32];
        entity.additional_equipment = [0; 5];
        entity.special_equipment = 0;
    }
    app.world_frame = 9;
    app.check_defeat_video();
    let p = video::pending().expect("defeat pending");
    assert_eq!(p.clip, VideoClip::Defeat(4));
    assert_eq!(p.after, VideoAfter::Gameplay);
    let settlement = app
        .settlements
        .iter()
        .find(|s| s.index == settlement.index)
        .unwrap();
    assert_ne!(settlement.flags_4b2 & 0x80, 0);
    assert_eq!(settlement.money_pool_10, 0);
    video::reset();
}

