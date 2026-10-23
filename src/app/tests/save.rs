//! Save/load payload round-trip tests.

use super::common::build_party_move_app;
use crate::actors::{ActorMotion, PartyMember};
use crate::save::SaveEnvelope;
use std::io::Write;

#[test]
fn save_roundtrip_preserves_quest_progress_and_state() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };

    // Full B7 slot: active bit + journal line in +0x02.
    app.quest_flags[207] = 0x0001_0080;
    app.player.money = 1234;
    app.player.hp_internal = 900;
    app.player.dialogue_id = 7;
    app.player.interface_mode_d2 = 0x13;
    app.player.state_d3 = 0x46;
    if let Some(s) = app.settlements.first_mut() {
        s.money_pool_10 = 321;
        s.target_base0 = 2;
        s.target_base1 = 3;
    }
    if let Some(t) = app.tables.as_mut() {
        if let Some(g) = t.groups.first_mut() {
            g.visited = true;
            g.activity_1d = 1;
        }
    }
    app.world_frame = 4242;
    app.discovered_locations[3] = 1;
    app.player_x = 44.0;
    app.player_y = 55.0;
    app.player_dir = 3;

    if let Some(tables) = app.tables.as_ref() {
        let entity = tables.entities[1].clone();
        app.party.push(PartyMember {
            entity,
            motion: ActorMotion::default(),
        });
    }

    let bytes = app.capture_save().encode().expect("encode");
    let decoded = SaveEnvelope::decode(&bytes).expect("decode");
    app.apply_save(decoded).expect("apply");

    assert_eq!(app.quest_flags[207], 0x0001_0080);
    assert_eq!(app.quest_flags[207] & 0x80, 0x80);
    assert_eq!(app.quest_flags[207] >> 16, 1);
    assert_eq!(app.player.money, 1234);
    assert_eq!(app.player.hp_internal, 900);
    assert_eq!(app.player.dialogue_id, 7);
    assert_eq!(app.player.interface_mode_d2, 0x13);
    assert_eq!(app.player.state_d3, 0x46);
    if let Some(s) = app.settlements.first() {
        assert_eq!(s.money_pool_10, 321);
        assert_eq!((s.target_base0, s.target_base1), (2, 3));
    }
    if let Some(t) = app.tables.as_ref() {
        if let Some(g) = t.groups.first() {
            assert!(g.visited);
            assert_eq!(g.activity_1d, 1);
        }
    }
    assert_eq!(app.world_frame, 4242);
    assert_eq!(app.discovered_locations[3], 1);
    assert_eq!((app.player_x, app.player_y), (44.0, 55.0));
    assert_eq!(app.player_dir, 3);
    assert_eq!(app.party.len(), 1);
    assert_eq!(app.current_map, 1);
}

#[test]
fn encoded_payload_is_compressed_and_versioned() {
    let Some(app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let bytes = app.capture_save().encode().expect("encode");
    assert_eq!(&bytes[..4], b"OPKS");
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), crate::save::VERSION);
    assert!(bytes.len() < crate::save::ITEM_TABLE_LEN);
}

#[test]
fn v1_payload_migrates_with_default_quest_extras() {
    let Some(app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let envelope = app.capture_save();
    let raw = bincode::serialize(&envelope.game).expect("serialize v1 game");
    let mut enc =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).expect("compress");
    let compressed = enc.finish().expect("finish");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"OPKS");
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&compressed);
    let decoded = SaveEnvelope::decode(&bytes).expect("v1 decode");
    assert_eq!(decoded.extra.player_dialogue_id, 0xFF);
    assert_eq!(decoded.extra.player_interface_mode_d2, 0);
    assert_eq!(decoded.extra.player_state_d3, 0);
    assert!(decoded.extra.settlements.is_empty());
}
