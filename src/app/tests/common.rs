//! Shared fixtures for [crate::app] tests.

use super::super::*;

/// Return the install directory that holds the original game data files.
///
/// `CARGO_MANIFEST_DIR/..` - the crate lives in `web/`, while `Game.res`,
/// `Game.N`, `Konung.N`, `konung.res` and `QUEST_R.RES` are one level up. Every
/// fixture reads from here and returns `None` when a required file is missing.
pub(super) fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A minimal playable app (real `Game.res` + `CLIENT.EXE`; synthetic
/// GRAPH/Objects) for input-routing tests. Gated on files being present.
///
/// `GRAPH.RES`/`Objects.res` are zeroed, so nothing can be drawn or decoded from
/// them; the app therefore has no locations, no edges/pairs, no population
/// tables and no settlements. It starts on map 1 at cell `(0, 0)` facing 5 with
/// the player at full health (`hp_internal = 1600` = 100 displayed HP) and
/// equipment slot 2 marked occupied by instance 1, which gives the weapon-cycle
/// tests a single occupied slot to jump to.
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
    let mut player =
        PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
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
///
/// The single location is id 1 with an all-zero `320x160` collision grid, no
/// data/props/gates and bounds `(319, 159)`, so every cell is passable; the app
/// keeps no population tables or settlements, starts on map 1 facing 0 and has
/// no equipment.
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
    let mut player =
        PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
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
///
/// Applies the verified `FUN_00438A78` projection (`entity_to_local_px_f`) and
/// adds `detail_pan`, so a test can turn a grid cell into a pointer position.
pub(super) fn cell_screen(app: &PlayableApp, cx: i32, cy: i32) -> (f64, f64) {
    let (lx, ly) = crate::coords::entity_to_local_px_f(cx as f64, cy as f64);
    (app.detail_pan[0] + lx, app.detail_pan[1] + ly)
}

/// A playable app with the real `Game.1` tables and `QUEST_R.RES` (Dragomir's
/// start map 4), so dialogue condition/action execution and recruitment can be
/// exercised against the shipped data.
///
/// Loads `konung.res` (item templates + fonts), attaches `Game.1` as the active
/// save game and parses its entity/group tables and settlements; the quest file
/// is read as `Quest_r.res` first, then `QUEST_R.RES`, so the shipped spelling
/// and the executable's literal name both resolve. `GRAPH.RES`/`Objects.res` stay
/// zeroed, there is no decoded location (so routes are rejected until a fixture
/// adds one), the start map is 4 at `(0, 0)` facing 5 and the player starts at
/// full health with no gear.
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
    res.set_save_game(&game1);
    let _ = res.load_quest(&quest);
    let world = World::new(res, Vec::new());
    let h = *hero(0).unwrap();
    let mut player =
        PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
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

/// A playable app with the real `Game.1` tables + `QUEST_R.RES` and one
/// synthetic, fully-free location 4 (the start map), so population/AI tests
/// can run the settlement's citizens.
///
/// The location is appended after the (empty) location list and registered in
/// `world.loc_index`; its collision grid is all zero and it carries no props or
/// gates, so the shipped citizens of location 4 spawn on a fully passable map.
pub(super) fn build_party_app_with_free_location() -> Option<PlayableApp> {
    let mut app = build_party_app()?;
    use crate::loc::Location;
    let loc = Location {
        loc_id: 4,
        data: Vec::new(),
        props: Vec::new(),
        gates: Vec::new(),
        graph_placements: Vec::new(),
        collision: vec![0u16; 320 * 160],
        max_x: 319,
        max_y: 159,
    };
    let index = app.world.locs.len();
    app.world.locs.push(loc);
    app.world.loc_index.insert(4, index);
    Some(app)
}

/// A party app with the real `Game.1` tables + `QUEST_R.RES` and one synthetic,
/// fully-free location (id `1`), so a world click issues real routes to the
/// leader and its selected companions. Player starts at `(100, 80)`.
///
/// Same resource/tables setup as `build_party_app`, but the location list holds
/// the free location 1 as its only entry, the start map is 1 facing 0, and the
/// zeroed collision grid makes every ordered route reachable.
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
    res.set_save_game(&game1);
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
    let mut player =
        PlayerCharacter::from_build(h, CharacterBuild::from_class(CharacterClass::Warrior));
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
