//! `lifecycle` subsystem of [`crate::lib`] (split from `lib.rs`).

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveMode {
    Frontend,
    Game,
}

/// Single owner of the frontend and the live game. The game is preserved while
/// the frontend is active; `host_w`/`host_h` are the real canvas backing-store
/// size and are forwarded to both so mode switches never reset input mapping.
pub struct Runtime {
    pub(super) frontend: Option<Frontend>,
    pub(super) game: Option<Box<PlayableApp>>,
    pub(super) active: ActiveMode,
    pub(super) host_w: i32,
    pub(super) host_h: i32,
    pub(super) quit: bool,
}

pub static mut LOADER: Option<Loader> = None;

pub static mut RUNTIME: Option<Runtime> = None;

pub fn runtime_mut() -> &'static mut Runtime {
    unsafe {
        let slot = addr_of_mut!(RUNTIME);
        (*slot).get_or_insert_with(Runtime::new)
    }
}

pub fn frontend_mut() -> Option<&'static mut Frontend> {
    runtime_mut().frontend.as_mut()
}

/// All active text-overlay items (frontend or game).
pub fn texts() -> Option<&'static Vec<TextItem>> {
    let rt = unsafe { &*addr_of!(RUNTIME) }.as_ref()?;
    match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| &f.texts),
        ActiveMode::Game => rt.game.as_deref().map(|g| &g.texts),
    }
}

/// Outcome of an input event, resolved after the runtime borrow has ended so the
/// active mode may be switched (e.g. Escape opens the menu).
pub enum InputAction {
    None,
    Command(FrontendCommand),
    OpenMenu,
}

/// Apply the recorded host size, if the host has reported one.
pub fn apply_host(f: &mut Frontend, host_w: i32, host_h: i32) {
    if host_w > 0 && host_h > 0 {
        f.on_resize(host_w, host_h);
    }
}

pub fn apply_host_to_app(a: &mut PlayableApp, host_w: i32, host_h: i32) {
    if host_w > 0 && host_h > 0 {
        a.on_resize(host_w, host_h);
    }
}

/// Build the game from the loaded resources, install it, and make it active.
///
/// `hero_index` is the hero-identity index (0 Dragomir, 1 Mikhail, 2 Wolf) and
/// is mapped to its `Game.N` file via [`hero::HEROES`]; it is intentionally
/// **not** the `Game.N` suffix. `build` is the final character-creation result,
/// copied once into the runtime [`PlayerCharacter`] (the class preset is not
/// regenerated). `_variant` is kept for ABI stability and unused.
/// Returns 1 on success, 0 on failure.
pub fn start_game(hero_index: u32, build: CharacterBuild, _variant: i32) -> u32 {
    let l = loader_mut();
    if l.game.is_empty() || l.graph.is_empty() || l.objects.is_empty() || l.exe.is_empty() {
        return 0;
    }
    if l.konungs.is_empty() {
        return 0;
    }

    let mut res = res::Res::new(
        std::mem::take(&mut l.game),
        std::mem::take(&mut l.graph),
        std::mem::take(&mut l.objects),
        std::mem::take(&mut l.exe),
    );

    // Original in-game interface images (INTR). Prefer the 800x600 variant (the
    // game framebuffer is fixed 800x600); fall back to 640. Absence or a decode
    // error is non-fatal: the game runs unchanged without INTR, which is not yet
    // consumed by the renderer.
    let intr_bytes = if !l.intr800.is_empty() {
        std::mem::take(&mut l.intr800)
    } else {
        std::mem::take(&mut l.intr640)
    };
    if !intr_bytes.is_empty() {
        match res.load_intr(&intr_bytes) {
            Ok(n) => eprintln!("INTR: loaded {n} interface sprites"),
            Err(e) => eprintln!("INTR: {e}"),
        }
    }

    // Item templates + original fonts from konung.res (used by the HUD labels).
    res.load_items_and_fonts(&l.names);

    // Dialogue/quest database (QUEST_R.RES). Absence or a bad image is
    // non-fatal: NPCs simply do not open a dialogue.
    if !l.quest.is_empty() {
        match res.load_quest(&l.quest) {
            Ok(()) => eprintln!("QUEST: dialogue database loaded"),
            Err(e) => eprintln!("QUEST: {e}"),
        }
    }

    let mut konungs = std::mem::take(&mut l.konungs);
    konungs.sort_by_key(|(id, _)| *id);
    let mut locs: Vec<Location> = Vec::with_capacity(konungs.len());
    for (id, data) in konungs {
        // Fail the whole boot (boot() -> 0) on a truncated/malformed Konung.N
        // rather than running with a silently all-free collision table.
        match loc::make_location(id, data) {
            Ok(loc) => locs.push(loc),
            Err(e) => {
                eprintln!("{}", e);
                return 0;
            }
        }
    }

    let profiles: HashMap<u32, Vec<edges::Transition>> = l
        .profiles
        .iter()
        .map(|(k, v)| (*k, edges::read_transitions(v)))
        .collect();
    if profiles.is_empty() {
        return 0;
    }

    // Hero identity -> Game.N mapping (`hero_index != Game.N` suffix).
    let h = match hero::hero(hero_index) {
        Some(h) => *h,
        None => return 0,
    };
    let game_suffix = h.game_suffix;
    // Item instances are read from the loaded Game.N save, not Game.res.
    if let Some(data) = l.profiles.get(&game_suffix) {
        res.set_save_game(data);
    }
    // The one authoritative runtime player: hero identity + final build.
    let mut player = PlayerCharacter::from_build(h, build);

    // The selected Game.N supplies population tables, transitions and the
    // starting Entity[0] coordinates. Require it (fail loudly if malformed).
    let game_tables = match l
        .profiles
        .get(&game_suffix)
        .and_then(|d| population::parse_tables(d).ok())
    {
        Some(t) => t,
        None => {
            eprintln!("Game.{} population parse failed", game_suffix);
            return 0;
        }
    };
    // Citizen home/role waypoints (Game.N `Settlement[20]`).
    let settlements = match l.profiles.get(&game_suffix) {
        Some(d) => match population::parse_settlements(d) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Game.{} settlement parse failed: {e}", game_suffix);
                return 0;
            }
        },
        None => Vec::new(),
    };

    let e0 = match game_tables.entities.get(0) {
        Some(e) => e,
        None => return 0,
    };
    let start_x = e0.x as f64;
    let start_y = e0.y as f64;
    // Authored starting equipment + inventory belong to this protagonist only.
    player.attach_starting_items(e0);

    let edges_vec = edges::build_exact_edges(Some(game_suffix), &profiles, &locs);
    let (pairs, one_way, self_edges) = edges::reciprocal_pairs(&edges_vec);
    let layout = edges::spring_layout(&locs, &edges_vec, &pairs);

    let location_names = names::parse_location_names(&l.names);
    let world = World::new(res, locs);

    let mut app = PlayableApp::new(
        world,
        edges_vec,
        pairs,
        one_way,
        self_edges,
        layout,
        game_suffix,
        location_names,
        player,
        h.start_location,
        start_x,
        start_y,
        hero::NEW_GAME_FACING,
        Some(game_tables),
        settlements,
    );
    let rt = runtime_mut();
    apply_host_to_app(&mut app, rt.host_w, rt.host_h);
    rt.game = Some(Box::new(app));
    rt.active = ActiveMode::Game;
    1
}

/// Direct game start (no frontend). Retained for existing tools/tests.
///
/// There is no character-creation screen here, so a default Warrior build is
/// used (same `Entity+0x04 = 5` the heroes previously carried).
#[no_mangle]
pub extern "C" fn boot(hero_index: u32, variant: i32) -> u32 {
    start_game(
        hero_index,
        CharacterBuild::from_class(CharacterClass::Warrior),
        variant,
    )
}

/// Start at the original frontend main menu. `width` selects the original
/// layout/resource resolution (must be `640` or `800`; anything else is `800`).
/// A live game, if any, is preserved. Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn frontend_boot(width: u32) -> u32 {
    let res = match width {
        640 => FrontendRes::R640,
        _ => FrontendRes::R800,
    };
    match build_frontend(res) {
        Ok(mut f) => {
            let rt = runtime_mut();
            f.game_active = rt.game.is_some();
            apply_host(&mut f, rt.host_w, rt.host_h);
            rt.frontend = Some(f);
            rt.active = ActiveMode::Frontend;
            1
        }
        Err(e) => {
            eprintln!("frontend_boot failed: {e}");
            0
        }
    }
}

/// Rebuild the currently active frontend at a different original resolution
/// (debug aid; task §23). Preserves the live game, `game_active`, and the real
/// host size. Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn frontend_set_resolution(width: u32) -> u32 {
    let res = match width {
        640 => FrontendRes::R640,
        _ => FrontendRes::R800,
    };
    // Snapshot runtime state without holding a borrow across build_frontend.
    let (has_frontend, game_active, host_w, host_h) = {
        let rt = runtime_mut();
        (
            rt.frontend.is_some(),
            rt.game.is_some(),
            rt.host_w,
            rt.host_h,
        )
    };
    if !has_frontend {
        return 0;
    }
    match build_frontend(res) {
        Ok(mut f) => {
            f.game_active = game_active;
            apply_host(&mut f, host_w, host_h);
            let rt = runtime_mut();
            rt.frontend = Some(f);
            rt.active = ActiveMode::Frontend;
            1
        }
        Err(e) => {
            eprintln!("frontend_set_resolution failed: {e}");
            0
        }
    }
}

/// Open the frontend main menu while preserving the live game. Called when the
/// game's Escape key handler sets `request_menu`.
pub fn open_frontend_from_game() {
    let rt = runtime_mut();
    if rt.frontend.is_none() {
        match build_frontend(FrontendRes::R800) {
            Ok(mut f) => {
                f.game_active = rt.game.is_some();
                apply_host(&mut f, rt.host_w, rt.host_h);
                rt.frontend = Some(f);
            }
            Err(e) => {
                eprintln!("cannot open menu: {e}");
                return;
            }
        }
    }
    let game_active = rt.game.is_some();
    if let Some(f) = rt.frontend.as_mut() {
        f.game_active = game_active;
        f.screen = FrontendScreen::MainMenu;
        f.hovered = None;
    }
    rt.active = ActiveMode::Frontend;
}

/// Handle a command returned by frontend input.
pub fn handle_frontend_command(cmd: FrontendCommand) {
    match cmd {
        FrontendCommand::StartGame { hero_index, build } => {
            // Confirmed New Game: only now is any previous game replaced. The
            // final build is copied once into the runtime player.
            if start_game(hero_index, build, -1) == 0 {
                eprintln!("failed to start game for hero index {}", hero_index);
            }
        }
        FrontendCommand::Resume => {
            let rt = runtime_mut();
            if rt.game.is_some() {
                rt.active = ActiveMode::Game;
            }
        }
        FrontendCommand::Quit => {
            runtime_mut().quit = true;
        }
    }
}

// --------------------------------------------------------------------------
// Runtime
// --------------------------------------------------------------------------

pub fn resolve_input(action: InputAction) {
    match action {
        InputAction::None => {}
        InputAction::Command(c) => handle_frontend_command(c),
        InputAction::OpenMenu => open_frontend_from_game(),
    }
}

// --------------------------------------------------------------------------
// Diagnostics (tests / tooling)
// --------------------------------------------------------------------------

impl Runtime {
    pub(super) fn new() -> Runtime {
        Runtime {
            frontend: None,
            game: None,
            active: ActiveMode::Frontend,
            host_w: 0,
            host_h: 0,
            quit: false,
        }
    }
}
