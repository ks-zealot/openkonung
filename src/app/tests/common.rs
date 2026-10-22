//! Shared fixtures for [crate::app] tests.

use super::super::*;

pub(super) fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A minimal playable app (real `Game.res` + `CLIENT.EXE`; synthetic
/// GRAPH/Objects) for input-routing tests. Gated on files being present.
pub(super) fn build_test_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let graph = vec![0u8; 0x30000]; // palettes/terrain unused in this test
    let objects = vec![0u8; 8 + 512 * 8];
    let res = Res::new(game, graph, objects, exe);
    let world = World::new(res, Vec::new());
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
    player.equipment = [0, 0, 1, 0, 0, 0];
    Some(PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        1,
        0.0,
        0.0,
        5,
        None,
        Vec::new(),
    ))
}


/// A playable app with one synthetic, fully-free location so world clicks issue
/// real routes (the plain `build_test_app` has no location, so every route is
/// rejected). Player starts at `(100, 80)`.
pub(super) fn build_movement_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::loc::Location;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let res = Res::new(game, graph, objects, exe);
    let loc = Location {
        loc_id: 1,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let world = World::new(res, vec![loc]);
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
    Some(PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        1,
        100.0,
        80.0,
        0,
        None,
        Vec::new(),
    ))
}

/// Screen coordinate of runtime cell `(cx, cy)` under the app's current pan.
pub(super) fn cell_screen(app: &PlayableApp, cx: i32, cy: i32) -> (f64, f64) {
    let (lx, ly) = crate::coords::entity_to_local_px_f(cx as f64, cy as f64);
    (app.detail_pan[0] + lx, app.detail_pan[1] + ly)
}


/// A playable app with the real `Game.1` tables and `QUEST_R.RES` (Dragomir's
/// start map 4), so dialogue condition/action execution and recruitment can be
/// exercised against the shipped data.
pub(super) fn build_party_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let konung = std::fs::read(dir.join("konung.res")).ok()?;
    let game1 = std::fs::read(dir.join("Game.1")).ok()?;
    let quest = ["Quest_r.res", "QUEST_R.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok())?;
    let tables = crate::population::parse_tables(&game1).ok()?;
    let settlements = crate::population::parse_settlements(&game1).unwrap_or_default();
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    let _ = res.load_quest(&quest);
    let world = World::new(res, Vec::new());
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
    Some(PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        4,
        0.0,
        0.0,
        5,
        Some(tables),
        settlements,
    ))
}


/// A party app with the real `Game.1` tables + `QUEST_R.RES` and one synthetic,
/// fully-free location (id `1`), so a world click issues real routes to the
/// leader and its selected companions. Player starts at `(100, 80)`.
pub(super) fn build_party_move_app() -> Option<PlayableApp> {
    use crate::character::{CharacterBuild, CharacterClass, PlayerCharacter};
    use crate::hero::hero;
    use crate::loc::Location;
    use crate::render::World;
    use crate::res::Res;
    let dir = install_dir();
    let game = std::fs::read(dir.join("Game.res")).ok()?;
    let exe = std::fs::read(dir.join("CLIENT.EXE")).ok()?;
    let konung = std::fs::read(dir.join("konung.res")).ok()?;
    let game1 = std::fs::read(dir.join("Game.1")).ok()?;
    let quest = ["Quest_r.res", "QUEST_R.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok())?;
    let tables = crate::population::parse_tables(&game1).ok()?;
    let settlements = crate::population::parse_settlements(&game1).unwrap_or_default();
    let graph = vec![0u8; 0x30000];
    let objects = vec![0u8; 8 + 512 * 8];
    let mut res = Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&konung);
    let _ = res.load_quest(&quest);
    let loc = Location {
        loc_id: 1,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let world = World::new(res, vec![loc]);
    let h = *hero(0).unwrap();
    let mut player = PlayerCharacter::from_build(
        h,
        CharacterBuild::from_class(CharacterClass::Warrior),
    );
    player.hp_internal = 1600;
    Some(PlayableApp::new(
        world,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        1,
        Vec::new(),
        player,
        1,
        100.0,
        80.0,
        0,
        Some(tables),
        settlements,
    ))
}

