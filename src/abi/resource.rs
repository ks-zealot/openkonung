//! `resource` subsystem of [`crate::lib`] (split from `lib.rs`).

use super::*;

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

/// Original in-game interface image archives (see `intr.rs`).
pub const KIND_INTR640: u32 = 11;

pub const KIND_INTR800: u32 = 12;

/// `QUEST_R.RES` dialogue/quest data (see `quest.rs`).
pub const KIND_QUEST: u32 = 13;

/// `SOUNDS.RES` sound archive (see `sound.rs`).
pub const KIND_SOUNDS: u32 = 14;

#[derive(Default)]
pub struct Loader {
    pub(super) game: Vec<u8>,
    pub(super) graph: Vec<u8>,
    pub(super) objects: Vec<u8>,
    pub(super) exe: Vec<u8>,
    pub(super) names: Vec<u8>,
    pub(super) menu640: Vec<u8>,
    pub(super) menu800: Vec<u8>,
    pub(super) newh640: Vec<u8>,
    pub(super) newh800: Vec<u8>,
    pub(super) save640: Vec<u8>,
    pub(super) save800: Vec<u8>,
    pub(super) intr640: Vec<u8>,
    pub(super) intr800: Vec<u8>,
    pub(super) quest: Vec<u8>,
    pub(super) sounds: Vec<u8>,
    pub(super) profiles: HashMap<u32, Vec<u8>>,
    pub(super) konungs: Vec<(u32, Vec<u8>)>,
    /// One-time pre-rendered New-Hero paper-doll frames (hero x weapon slot).
    pub(super) hero_previews: Option<Vec<crate::res::Img>>,
}

pub fn loader_mut() -> &'static mut Loader {
    unsafe {
        let slot = addr_of_mut!(LOADER);
        (*slot).get_or_insert_with(Loader::default)
    }
}

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
        KIND_INTR640 => l.intr640 = data,
        KIND_INTR800 => l.intr800 = data,
        KIND_QUEST => l.quest = data,
        KIND_SOUNDS => l.sounds = data,
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

pub struct FrontendAssets {
    pub(super) menu_name: String,
    pub(super) menu: &'static [u8],
    pub(super) newh_name: String,
    pub(super) newh: &'static [u8],
    pub(super) save_name: String,
    pub(super) save: &'static [u8],
    /// `konung.res` (embedded original bitmap fonts).
    pub(super) fonts: &'static [u8],
}

pub fn frontend_assets(res: FrontendRes) -> FrontendAssets {
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
/// Compose the New-Hero paper-doll previews once, borrowing the loaded
/// GRAPH/OBJECTS/Game.res/exe buffers and returning them afterwards.
///
/// The original preview compositor (`FUN_00427E9C`) reads the live item-instance
/// table. Before any `Game.N` is loaded (`FUN_0043A870` has not run) that table
/// is zeroed, so the preview is composed against a zeroed `0x20000` table.
fn build_hero_previews() -> Option<Vec<crate::res::Img>> {
    let l = loader_mut();
    if let Some(cached) = &l.hero_previews {
        return Some(cached.clone());
    }
    if l.game.is_empty() || l.graph.is_empty() || l.objects.is_empty() || l.exe.is_empty() {
        return None;
    }
    let game = std::mem::take(&mut l.game);
    let graph = std::mem::take(&mut l.graph);
    let objects = std::mem::take(&mut l.objects);
    let exe = std::mem::take(&mut l.exe);
    let mut res = crate::res::Res::new(game, graph, objects, exe);
    res.load_items_and_fonts(&l.names);
    let instances = vec![0u8; 0x20000];
    res.set_save_game(&instances);
    let previews = crate::frontend::previews::compose_previews(&mut res);
    let (game, graph, objects, exe) = res.into_parts();
    l.game = game;
    l.graph = graph;
    l.objects = objects;
    l.exe = exe;
    l.hero_previews = Some(previews.clone());
    Some(previews)
}

pub fn build_frontend(res: FrontendRes) -> Result<Frontend, String> {
    let previews = build_hero_previews();
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
    let save = if a.save.is_empty() {
        None
    } else {
        Some((a.save_name.as_str(), a.save))
    };
    let mut frontend = Frontend::new(
        res,
        &a.menu_name,
        a.menu,
        &a.newh_name,
        a.newh,
        save,
        a.fonts,
    )?;
    if let Some(previews) = previews {
        frontend.set_hero_previews(previews);
    }
    Ok(frontend)
}
