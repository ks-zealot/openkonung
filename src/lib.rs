//! OpenKonung browser port — raw wasm ABI.
//!
//! No `wasm-bindgen`: JS copies resource bytes into linear memory via `alloc`,
//! hands them over with `load_*`, then drives the app with `step` and reads the
//! RGBA framebuffer from `frame_ptr`.
//!
//! The runtime owns **both** the original frontend and the live game at once
//! ([`Runtime`]), so opening the menu from gameplay (Escape) never recreates or
//! discards [`PlayableApp`]. `active` selects which one is stepped/rendered and
//! receives input. `frontend_boot` starts at the original startup menu; `boot`
//! is retained as a direct game start.

#![allow(clippy::missing_safety_doc)]

pub mod actors;
pub mod app;
pub mod character;
pub mod coords;
pub mod edges;
pub mod font;
pub mod frontend;
pub mod hero;
pub mod loc;
pub mod menu_res;
pub mod names;
pub mod population;
pub mod render;
pub mod res;

use std::collections::HashMap;
use std::ptr::{addr_of, addr_of_mut, slice_from_raw_parts_mut};

use app::{PlayableApp, TextItem};
use character::{CharacterBuild, CharacterClass, PlayerCharacter};
use frontend::layout::FrontendRes;
use frontend::{Frontend, FrontendCommand, FrontendScreen};
use loc::Location;
use render::World;

pub const KIND_GAME: u32 = 0;
pub const KIND_GRAPH: u32 = 1;
pub const KIND_OBJECTS: u32 = 2;
pub const KIND_EXE: u32 = 3;
pub const KIND_NAMES: u32 = 4;
/// Menu-image RES families (see `menu_res.rs` for the format).
pub const KIND_MENU640: u32 = 5;
pub const KIND_MENU800: u32 = 6;
pub const KIND_NEWH640: u32 = 7;
pub const KIND_NEWH800: u32 = 8;
pub const KIND_SAVE640: u32 = 9;
pub const KIND_SAVE800: u32 = 10;

#[derive(Default)]
struct Loader {
    game: Vec<u8>,
    graph: Vec<u8>,
    objects: Vec<u8>,
    exe: Vec<u8>,
    names: Vec<u8>,
    menu640: Vec<u8>,
    menu800: Vec<u8>,
    newh640: Vec<u8>,
    newh800: Vec<u8>,
    save640: Vec<u8>,
    save800: Vec<u8>,
    profiles: HashMap<u32, Vec<u8>>,
    konungs: Vec<(u32, Vec<u8>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveMode {
    Frontend,
    Game,
}

/// Single owner of the frontend and the live game. The game is preserved while
/// the frontend is active; `host_w`/`host_h` are the real canvas backing-store
/// size and are forwarded to both so mode switches never reset input mapping.
struct Runtime {
    frontend: Option<Frontend>,
    game: Option<Box<PlayableApp>>,
    active: ActiveMode,
    host_w: i32,
    host_h: i32,
    quit: bool,
}

impl Runtime {
    fn new() -> Runtime {
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

static mut LOADER: Option<Loader> = None;
static mut RUNTIME: Option<Runtime> = None;

fn loader_mut() -> &'static mut Loader {
    unsafe {
        let slot = addr_of_mut!(LOADER);
        (*slot).get_or_insert_with(Loader::default)
    }
}

fn runtime_mut() -> &'static mut Runtime {
    unsafe {
        let slot = addr_of_mut!(RUNTIME);
        (*slot).get_or_insert_with(Runtime::new)
    }
}

fn frontend_mut() -> Option<&'static mut Frontend> {
    runtime_mut().frontend.as_mut()
}

/// All active text-overlay items (frontend or game).
fn texts() -> Option<&'static Vec<TextItem>> {
    let rt = unsafe { &*addr_of!(RUNTIME) }.as_ref()?;
    match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| &f.texts),
        ActiveMode::Game => rt.game.as_deref().map(|g| &g.texts),
    }
}

/// Outcome of an input event, resolved after the runtime borrow has ended so the
/// active mode may be switched (e.g. Escape opens the menu).
enum InputAction {
    None,
    Command(FrontendCommand),
    OpenMenu,
}

#[inline]
unsafe fn take_bytes(ptr: *mut u8, len: u32) -> Vec<u8> {
    std::slice::from_raw_parts(ptr, len as usize).to_vec()
}

// --------------------------------------------------------------------------
// Memory
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let mut b = vec![0u8; len as usize].into_boxed_slice();
    let p = b.as_mut_ptr();
    std::mem::forget(b);
    p
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(slice_from_raw_parts_mut(ptr, len as usize)));
    }
}

// --------------------------------------------------------------------------
// Resource loading
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn load_resource(kind: u32, ptr: *mut u8, len: u32) {
    let data = unsafe { take_bytes(ptr, len) };
    let l = loader_mut();
    match kind {
        KIND_GAME => l.game = data,
        KIND_GRAPH => l.graph = data,
        KIND_OBJECTS => l.objects = data,
        KIND_EXE => l.exe = data,
        KIND_NAMES => l.names = data,
        KIND_MENU640 => l.menu640 = data,
        KIND_MENU800 => l.menu800 = data,
        KIND_NEWH640 => l.newh640 = data,
        KIND_NEWH800 => l.newh800 = data,
        KIND_SAVE640 => l.save640 = data,
        KIND_SAVE800 => l.save800 = data,
        _ => {}
    }
    dealloc(ptr, len);
}

#[no_mangle]
pub extern "C" fn load_profile(id: u32, ptr: *mut u8, len: u32) {
    let data = unsafe { take_bytes(ptr, len) };
    loader_mut().profiles.insert(id, data);
    dealloc(ptr, len);
}

#[no_mangle]
pub extern "C" fn load_konung(id: u32, ptr: *mut u8, len: u32) {
    let data = unsafe { take_bytes(ptr, len) };
    loader_mut().konungs.push((id, data));
    dealloc(ptr, len);
}

// --------------------------------------------------------------------------
// Boot
// --------------------------------------------------------------------------

struct FrontendAssets {
    menu_name: String,
    menu: &'static [u8],
    newh_name: String,
    newh: &'static [u8],
    save_name: String,
    save: &'static [u8],
    /// `konung.res` (embedded original bitmap fonts).
    fonts: &'static [u8],
}

fn frontend_assets(res: FrontendRes) -> FrontendAssets {
    let l = loader_mut();
    let fonts = l.names.as_slice();
    match res {
        FrontendRes::R640 => FrontendAssets {
            menu_name: "MENU640R.RES".to_string(),
            menu: l.menu640.as_slice(),
            newh_name: "NEWH640R.RES".to_string(),
            newh: l.newh640.as_slice(),
            save_name: "SAVE640R.RES".to_string(),
            save: l.save640.as_slice(),
            fonts,
        },
        FrontendRes::R800 => FrontendAssets {
            menu_name: "MENU800R.RES".to_string(),
            menu: l.menu800.as_slice(),
            newh_name: "NEWH800R.RES".to_string(),
            newh: l.newh800.as_slice(),
            save_name: "SAVE800R.RES".to_string(),
            save: l.save800.as_slice(),
            fonts,
        },
    }
}

/// Build a [`Frontend`] for one resolution from the loaded menu assets.
fn build_frontend(res: FrontendRes) -> Result<Frontend, String> {
    let a = frontend_assets(res);
    if a.menu.is_empty() || a.newh.is_empty() {
        return Err(format!(
            "menu assets not loaded for {res:?}: {}={}B {}={}B",
            a.menu_name,
            a.menu.len(),
            a.newh_name,
            a.newh.len()
        ));
    }
    let save = if a.save.is_empty() { None } else { Some((a.save_name.as_str(), a.save)) };
    Frontend::new(res, &a.menu_name, a.menu, &a.newh_name, a.newh, save, a.fonts)
}

/// Apply the recorded host size, if the host has reported one.
fn apply_host(f: &mut Frontend, host_w: i32, host_h: i32) {
    if host_w > 0 && host_h > 0 {
        f.on_resize(host_w, host_h);
    }
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

/// Build the game from the loaded resources, install it, and make it active.
///
/// `hero_index` is the hero-identity index (0 Dragomir, 1 Mikhail, 2 Wolf) and
/// is mapped to its `Game.N` file via [`hero::HEROES`]; it is intentionally
/// **not** the `Game.N` suffix. `build` is the final character-creation result,
/// copied once into the runtime [`PlayerCharacter`] (the class preset is not
/// regenerated). `_variant` is kept for ABI stability and unused.
/// Returns 1 on success, 0 on failure.
fn start_game(hero_index: u32, build: CharacterBuild, _variant: i32) -> u32 {
    let l = loader_mut();
    if l.game.is_empty() || l.graph.is_empty() || l.objects.is_empty() || l.exe.is_empty() {
        return 0;
    }
    if l.konungs.is_empty() {
        return 0;
    }

    let res = res::Res::new(
        std::mem::take(&mut l.game),
        std::mem::take(&mut l.graph),
        std::mem::take(&mut l.objects),
        std::mem::take(&mut l.exe),
    );

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
    // The one authoritative runtime player: hero identity + final build.
    let player = PlayerCharacter::from_build(h, build);

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
    let e0 = match game_tables.entities.get(0) {
        Some(e) => e,
        None => return 0,
    };
    let start_x = e0.x as f64;
    let start_y = e0.y as f64;

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
    );
    let rt = runtime_mut();
    apply_host_to_app(&mut app, rt.host_w, rt.host_h);
    rt.game = Some(Box::new(app));
    rt.active = ActiveMode::Game;
    1
}

fn apply_host_to_app(a: &mut PlayableApp, host_w: i32, host_h: i32) {
    if host_w > 0 && host_h > 0 {
        a.on_resize(host_w, host_h);
    }
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

/// Open the frontend main menu while preserving the live game. Called when the
/// game's Escape key handler sets `request_menu`.
fn open_frontend_from_game() {
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
fn handle_frontend_command(cmd: FrontendCommand) {
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

#[no_mangle]
pub extern "C" fn step(dt: f32) {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.step();
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.step(dt as f64);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn frame_ptr() -> *const u8 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| f.frame_ptr()).unwrap_or(std::ptr::null()),
        ActiveMode::Game => rt.game.as_deref().map(|g| g.frame.as_ptr()).unwrap_or(std::ptr::null()),
    }
}

#[no_mangle]
pub extern "C" fn frame_w() -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| f.logical_w() as u32).unwrap_or(app::VIRTUAL_W as u32),
        ActiveMode::Game => app::VIRTUAL_W as u32,
    }
}

#[no_mangle]
pub extern "C" fn frame_h() -> u32 {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => rt.frontend.as_ref().map(|f| f.logical_h() as u32).unwrap_or(app::VIRTUAL_H as u32),
        ActiveMode::Game => app::VIRTUAL_H as u32,
    }
}

#[no_mangle]
pub extern "C" fn should_quit() -> u32 {
    runtime_mut().quit as u32
}

// --------------------------------------------------------------------------
// Input
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn on_resize(w: u32, h: u32) {
    let rt = runtime_mut();
    rt.host_w = w as i32;
    rt.host_h = h as i32;
    if let Some(f) = rt.frontend.as_mut() {
        f.on_resize(w as i32, h as i32);
    }
    if let Some(g) = rt.game.as_deref_mut() {
        g.on_resize(w as i32, h as i32);
    }
}

#[no_mangle]
pub extern "C" fn pointer_down(button: u32, x: f32, y: f32) {
    let action = {
        let rt = runtime_mut();
        match rt.active {
            ActiveMode::Frontend => match rt.frontend.as_mut() {
                Some(f) => f
                    .pointer_down(button, x as f64, y as f64)
                    .map(InputAction::Command)
                    .unwrap_or(InputAction::None),
                None => InputAction::None,
            },
            ActiveMode::Game => {
                if let Some(g) = rt.game.as_deref_mut() {
                    g.pointer_down(button, x as f64, y as f64);
                }
                InputAction::None
            }
        }
    };
    resolve_input(action);
}

#[no_mangle]
pub extern "C" fn pointer_up(button: u32, x: f32, y: f32) {
    let _ = (x, y);
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.pointer_up(button);
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.pointer_up(button);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn pointer_move(x: f32, y: f32) {
    let rt = runtime_mut();
    match rt.active {
        ActiveMode::Frontend => {
            if let Some(f) = rt.frontend.as_mut() {
                f.pointer_move(x as f64, y as f64);
            }
        }
        ActiveMode::Game => {
            if let Some(g) = rt.game.as_deref_mut() {
                g.pointer_move(x as f64, y as f64);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn wheel(delta: f32) {
    let rt = runtime_mut();
    if rt.active == ActiveMode::Game {
        if let Some(g) = rt.game.as_deref_mut() {
            g.wheel(delta as f64);
        }
    }
}

#[no_mangle]
pub extern "C" fn key_down(code: u32) {
    let action = {
        let rt = runtime_mut();
        match rt.active {
            ActiveMode::Frontend => match rt.frontend.as_mut() {
                Some(f) => f
                    .key_down(code)
                    .map(InputAction::Command)
                    .unwrap_or(InputAction::None),
                None => InputAction::None,
            },
            ActiveMode::Game => {
                let mut open = false;
                if let Some(g) = rt.game.as_deref_mut() {
                    g.key_down(code);
                    if g.request_menu {
                        g.request_menu = false;
                        open = true;
                    }
                }
                if open {
                    InputAction::OpenMenu
                } else {
                    InputAction::None
                }
            }
        }
    };
    resolve_input(action);
}

fn resolve_input(action: InputAction) {
    match action {
        InputAction::None => {}
        InputAction::Command(c) => handle_frontend_command(c),
        InputAction::OpenMenu => open_frontend_from_game(),
    }
}

// --------------------------------------------------------------------------
// Diagnostics (tests / tooling)
// --------------------------------------------------------------------------

/// 1 while the original frontend is active, 0 while the game is active.
#[no_mangle]
pub extern "C" fn frontend_is_active() -> u32 {
    (runtime_mut().active == ActiveMode::Frontend) as u32
}

/// 1 while a live game is retained (active or suspended).
#[no_mangle]
pub extern "C" fn debug_game_exists() -> u32 {
    runtime_mut().game.is_some() as u32
}

/// Live player runtime x/y (0 when no game). Used to prove state preservation.
#[no_mangle]
pub extern "C" fn debug_player_x() -> f32 {
    runtime_mut().game.as_deref().map(|g| g.player_x as f32).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn debug_player_y() -> f32 {
    runtime_mut().game.as_deref().map(|g| g.player_y as f32).unwrap_or(0.0)
}

/// Host (canvas backing-store) size as recorded by the last `on_resize`,
/// independent of the active mode.
#[no_mangle]
pub extern "C" fn host_w() -> u32 {
    runtime_mut().host_w.max(0) as u32
}

#[no_mangle]
pub extern "C" fn host_h() -> u32 {
    runtime_mut().host_h.max(0) as u32
}

/// Frontend screen code (0 MainMenu, 1 NewHero, 2 Settings, 3 LoadGame,
/// 4 SaveGame), or 255 when no frontend exists.
#[no_mangle]
pub extern "C" fn frontend_state() -> u32 {
    match frontend_mut() {
        Some(f) => f.screen_code(),
        None => 255,
    }
}

/// Selected hero archetype (0/1/2) or -1.
#[no_mangle]
pub extern "C" fn frontend_selected_hero() -> i32 {
    frontend_mut().map(|f| f.selected_hero()).unwrap_or(-1)
}

/// Hovered index for the current frontend screen, or -1.
#[no_mangle]
pub extern "C" fn frontend_hover() -> i32 {
    frontend_mut().map(|f| f.hover_index()).unwrap_or(-1)
}

/// Selected class `Entity+0x04` value (5/3/4/2), or -1.
#[no_mangle]
pub extern "C" fn frontend_selected_class() -> i32 {
    frontend_mut().map(|f| f.selected_class_entity()).unwrap_or(-1)
}

static mut DEBUG_PLAYER_STATE: Option<Vec<u8>> = None;

/// Format the active game's [`PlayerCharacter::debug_block`] into a static
/// buffer. Returns 1 when a game exists, 0 otherwise. Diagnostic only.
#[no_mangle]
pub extern "C" fn debug_player_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            let bytes = g.player.debug_block().into_bytes();
            unsafe {
                *addr_of_mut!(DEBUG_PLAYER_STATE) = Some(bytes);
            }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_player_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_player_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_PLAYER_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

static mut DEBUG_ENTITY_STATE: Option<Vec<u8>> = None;

/// Format the active game's source `Entity` with the given id (task §7) into a
/// static buffer. Returns 1 when a game is active and the id is in range.
#[no_mangle]
pub extern "C" fn debug_entity_state(id: u32) -> u32 {
    let block = match runtime_mut().game.as_deref() {
        Some(g) => g.entity_debug_block(id as usize),
        None => None,
    };
    match block {
        Some(s) => {
            unsafe {
                *addr_of_mut!(DEBUG_ENTITY_STATE) = Some(s.into_bytes());
            }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn debug_entity_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_entity_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_ENTITY_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}

// --------------------------------------------------------------------------
// Debug: decoded menu images (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------

static mut DEBUG_IMAGE: Option<Vec<u8>> = None;
static mut DEBUG_IMAGE_W: u32 = 0;
static mut DEBUG_IMAGE_H: u32 = 0;

/// Decode `kind` (menu RES family) image `index` into a static RGBA buffer and
/// return 1 on success. Used by `tools/menu_png.mjs`; not used at runtime.
#[no_mangle]
pub extern "C" fn debug_decode_menu_image(kind: u32, index: u32) -> u32 {
    let bytes: Vec<u8> = {
        let l = loader_mut();
        match kind {
            KIND_MENU640 => l.menu640.clone(),
            KIND_MENU800 => l.menu800.clone(),
            KIND_NEWH640 => l.newh640.clone(),
            KIND_NEWH800 => l.newh800.clone(),
            KIND_SAVE640 => l.save640.clone(),
            KIND_SAVE800 => l.save800.clone(),
            _ => return 0,
        }
    };
    let name = format!("kind{kind}");
    let res = match menu_res::MenuRes::parse_named(&name, &bytes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 0;
        }
    };
    let Some(img) = res.images.get(index as usize) else {
        return 0;
    };
    unsafe {
        let slot = addr_of_mut!(DEBUG_IMAGE);
        *slot = Some(img.rgba.clone());
        DEBUG_IMAGE_W = img.width as u32;
        DEBUG_IMAGE_H = img.height as u32;
    }
    1
}

#[no_mangle]
pub extern "C" fn debug_image_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_IMAGE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn debug_image_w() -> u32 {
    unsafe { DEBUG_IMAGE_W }
}

#[no_mangle]
pub extern "C" fn debug_image_h() -> u32 {
    unsafe { DEBUG_IMAGE_H }
}

// --------------------------------------------------------------------------
// Text overlay queue (rendered by the browser with real fonts)
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn text_count() -> u32 {
    texts().map(|t| t.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_x(i: u32) -> i32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.x).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_y(i: u32) -> i32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.y).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn text_rgb(i: u32) -> u32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.rgb).unwrap_or(0xFFFFFF)
}

#[no_mangle]
pub extern "C" fn text_scale(i: u32) -> f32 {
    texts().and_then(|t| t.get(i as usize)).map(|t| t.scale).unwrap_or(1.0)
}

#[no_mangle]
pub extern "C" fn text_ptr(i: u32) -> *const u8 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn text_len(i: u32) -> u32 {
    texts()
        .and_then(|t| t.get(i as usize))
        .map(|t| t.s.len() as u32)
        .unwrap_or(0)
}
