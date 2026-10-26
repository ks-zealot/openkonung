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

#[test]
fn v2_payload_decodes_with_zeroed_growth_extras() {
    use crate::save::{GroupExtraV2, SaveEnvelopeV2, SaveExtraV2};
    let Some(app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let envelope = app.capture_save();
    let legacy = SaveEnvelopeV2 {
        game: envelope.game,
        extra: SaveExtraV2 {
            player_dialogue_id: 0x2A,
            player_interface_mode_d2: 3,
            player_state_d3: 4,
            settlements: envelope.extra.settlements,
            groups: envelope
                .extra
                .groups
                .iter()
                .map(|g| GroupExtraV2 {
                    activity_1d: g.activity_1d,
                    visited: g.visited,
                })
                .collect(),
        },
    };
    let raw = bincode::serialize(&legacy).expect("serialize v2 envelope");
    let mut enc =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).expect("compress");
    let compressed = enc.finish().expect("finish");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"OPKS");
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&compressed);
    let decoded = SaveEnvelope::decode(&bytes).expect("v2 decode");
    assert_eq!(decoded.extra.player_dialogue_id, 0x2A);
    assert!(!decoded.extra.groups.is_empty());
    assert!(decoded
        .extra
        .groups
        .iter()
        .all(|g| g.growth_timer == 0 && g.growth_countdown == 0));
}

#[test]
fn growth_accumulator_roundtrips() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    if let Some(t) = app.tables.as_mut() {
        t.groups[2].growth_timer = -4242;
        t.groups[2].growth_countdown = 1234;
        t.groups[2].count_18 = 9;
        t.groups[2].entity_count = 3;
    } else {
        eprintln!("skipping: no population tables");
        return;
    }
    app.leader_entity_index = 7;
    let bytes = app.capture_save().encode().expect("encode");
    let decoded = SaveEnvelope::decode(&bytes).expect("decode");
    app.apply_save(decoded).expect("apply");
    assert_eq!(app.leader_entity_index, 7);
    let g = &app.tables.as_ref().unwrap().groups[2];
    assert_eq!(g.growth_timer, -4242);
    assert_eq!(g.growth_countdown, 1234);
    assert_eq!(g.count_18, 9);
    assert_eq!(g.entity_count, 3);
}

#[test]
fn legacy_snapshot_round_trips_live_state() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    app.quest_flags[12] = 0x0001_8001;
    app.player.money = 4321;
    app.player.strength_current = 17;
    app.world_frame = 98_765;
    app.discovered_locations[5] = 0x40;

    let bytes = app.export_legacy_snapshot();
    assert_eq!(bytes.len(), crate::legacy_save::SNAPSHOT_LEN);
    let snap = crate::legacy_save::LegacySnapshot::parse(&bytes).expect("parse export");
    assert_eq!(snap.quest_flags[12], 0x0001_8001);
    assert_eq!(snap.world_tick, 98_765);
    assert_eq!(snap.discovered[5], 0x40);
    let leader = snap.leader().expect("leader");
    assert_eq!(leader.stats.money, 4321);
    assert_eq!(leader.stats.strength_current, 17);

    let Some(mut restored) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    restored.import_legacy_snapshot(&bytes).expect("import");
    assert_eq!(restored.quest_flags[12], 0x0001_8001);
    assert_eq!(restored.player.money, 4321);
    assert_eq!(restored.player.strength_current, 17);
    assert_eq!(restored.world_frame, 98_765);
    assert_eq!(restored.discovered_locations[5], 0x40);
}

#[test]
fn legacy_truncated_snapshot_is_rejected() {
    let Some(mut app) = build_party_move_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    assert!(app.import_legacy_snapshot(&[0u8; 100]).is_err());
}

#[test]
fn legacy_slot_names_round_trip() {
    let mut names: [String; crate::legacy_save::SLOT_COUNT] = Default::default();
    names[0] = "???????? ??????".to_string();
    let bytes = crate::legacy_save::write_slot_names(&names);
    let back = crate::legacy_save::parse_slot_names(&bytes).expect("names");
    assert_eq!(back[0], "???????? ??????");
}

