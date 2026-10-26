//! Unit tests for the packed resource representation (no live loaders).

use super::*;

fn named(kind: u32, name: &str, byte: u8) -> NamedBlob {
    NamedBlob {
        name: name.to_string(),
        kind,
        bytes: vec![byte; 8],
    }
}

fn minimal_pack() -> ResourcePack {
    ResourcePack {
        format: PACK_FORMAT,
        build: String::new(),
        source: BTreeMap::new(),
        resources: vec![
            named(0, "Game.res", 0),
            named(1, "GRAPH.RES", 1),
            named(2, "Objects.res", 2),
            named(3, "CLIENT.EXE", 3),
            named(4, "konung.res", 4),
            named(5, "MENU640R.RES", 5),
            named(6, "MENU800R.RES", 6),
            named(7, "NEWH640R.RES", 7),
            named(8, "NEWH800R.RES", 8),
            named(11, "INTR640R.RES", 11),
            named(12, "INTR800R.RES", 12),
            named(13, "Quest_r.res", 13),
            named(14, "SOUNDS.RES", 14),
            named(16, "LIGHTS.RES", 16),
        ],
        profiles: vec![
            IdBlob {
                id: 0,
                name: "Game.0".into(),
                bytes: vec![0; 8],
            },
            IdBlob {
                id: 1,
                name: "Game.1".into(),
                bytes: vec![1; 8],
            },
            IdBlob {
                id: 2,
                name: "Game.2".into(),
                bytes: vec![2; 8],
            },
        ],
        konungs: vec![IdBlob {
            id: 1,
            name: "Konung.1".into(),
            bytes: vec![1; 8],
        }],
        light_maps: vec![IdBlob {
            id: 1,
            name: "LIGHTS.1".into(),
            bytes: vec![1; 8],
        }],
        music: vec![MusicBlob {
            name: "music/track02.ogg".into(),
            bytes: vec![9; 8],
        }],
        videos: vec![VideoBlob {
            name: "poster".into(),
            bytes: vec![7; 8],
        }],
    }
}

#[test]
fn roundtrips_through_bincode() {
    let mut pack = minimal_pack();
    pack.build = pack.compute_build();
    let bytes = pack.to_bytes().unwrap();
    let back = ResourcePack::from_bytes(&bytes).unwrap();
    assert_eq!(back, pack);
    assert_eq!(back.compute_build(), pack.build);
}

#[test]
fn validates_a_complete_pack() {
    minimal_pack().validate().unwrap();
}

#[test]
fn rejects_wrong_format() {
    let mut pack = minimal_pack();
    pack.format = PACK_FORMAT + 1;
    assert!(pack.validate().is_err());
}

#[test]
fn rejects_missing_core_kind() {
    let mut pack = minimal_pack();
    pack.resources.retain(|r| r.kind != 1);
    let e = pack.validate().unwrap_err();
    assert!(e.message.contains("kind 1"), "{}", e.message);
}

#[test]
fn rejects_duplicate_kind() {
    let mut pack = minimal_pack();
    pack.resources.push(named(1, "GRAPH2.RES", 99));
    let e = pack.validate().unwrap_err();
    assert!(e.message.contains("duplicate"), "{}", e.message);
}

#[test]
fn rejects_missing_profile() {
    let mut pack = minimal_pack();
    pack.profiles.retain(|p| p.id != 2);
    let e = pack.validate().unwrap_err();
    assert!(e.message.contains("Game.0/1/2"), "{}", e.message);
}

#[test]
fn build_id_is_content_sensitive() {
    let mut a = minimal_pack();
    a.build = a.compute_build();
    let mut b = minimal_pack();
    b.resources[0].bytes[0] ^= 0xFF;
    b.build = b.compute_build();
    assert_ne!(a.build, b.build);
    assert_eq!(a.build.len(), 12);
}
