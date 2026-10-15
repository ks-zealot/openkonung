//! Original frontend: startup main menu and New-Hero screen.
//!
//! The frontend composes an original logical framebuffer (`640x480` or
//! `800x600`) from the decoded `MENU{width}R.RES` / `NEWH{width}R.RES` images
//! and the recovered original layout tables ([`layout`]). It never scales
//! individual coordinates and never hit-tests in host pixels: mouse input is
//! converted into the logical resolution first.
//!
//! On `New Game` (action index `3`) the screen switches to [`FrontendScreen::NewHero`];
//! confirming a selected hero returns [`FrontendCommand::StartGame`] to the wasm
//! layer, which hands the hero selection index to the existing game runtime.

mod screens;

pub use self::screens::*;

pub mod layout;
pub mod main_menu;
pub mod new_hero;

use crate::app::TextItem;
use crate::font::{Fonts, OriginalFont};
use crate::menu_res::{MenuImage, MenuRes};
use crate::res::{blit, fill_rect};

use self::layout::*;
use self::new_hero::NewHeroState;

/// Explicit frontend screens (task §25). `Settings` renders the confirmed
/// `MENU{width}R.RES` #1 settings screen; `SaveGame`/`LoadGame` render the
/// original `SAVE{width}R.RES` screen. The live game (if any) is owned by the
/// host runtime, so there is no `InGame` screen here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendScreen {
    MainMenu,
    NewHero,
    Settings,
    LoadGame,
    SaveGame,
}

/// A command the frontend asks the wasm layer to perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendCommand {
    /// Start gameplay with the selected hero identity (0/1/2) and the final
    /// character-creation build. The build is copied into the runtime player
    /// exactly as configured — the class preset is not regenerated.
    StartGame {
        hero_index: u32,
        build: crate::character::CharacterBuild,
    },
    /// Re-activate the preserved live game.
    Resume,
    /// Exit the application.
    Quit,
}

use crate::constants::keys::KEY_K;
const KEY_ESCAPE: u32 = 27;

pub struct Frontend {
    pub res: FrontendRes,
    menu: MenuRes,
    newh: MenuRes,
    /// Original `SAVE{width}R.RES` (Save/Load screen), when the assets are
    /// present. `None` renders a controlled placeholder.
    save: Option<MenuRes>,
    /// Original bitmap fonts from `konung.res` (used by the New-Hero screen).
    fonts: Fonts,
    pub screen: FrontendScreen,
    /// Hovered `MAIN_MENU` index while on the main menu.
    pub hovered: Option<usize>,
    /// Last logical mouse position.
    pub mouse: (i32, i32),
    pub new_hero: NewHeroState,
    pub frame: Vec<u8>,
    pub texts: Vec<TextItem>,
    /// Debug overlay: draw every original hit rectangle with its index.
    pub debug_regions: bool,
    /// Whether a live game exists (drives the startup menu visibility).
    pub game_active: bool,
    host_w: i32,
    host_h: i32,
    pub quit: bool,
}

impl Frontend {
    /// Parse the two original menu resources for one resolution. `res` selects
    /// the layout and framebuffer size; the bytes are the raw `.RES` assets.
    pub fn new(
        res: FrontendRes,
        menu_name: &str,
        menu_bytes: &[u8],
        newh_name: &str,
        newh_bytes: &[u8],
        save: Option<(&str, &[u8])>,
        font_bytes: &[u8],
    ) -> Result<Frontend, String> {
        let menu = MenuRes::parse_named(menu_name, menu_bytes).map_err(|e| e.to_string())?;
        let newh = MenuRes::parse_named(newh_name, newh_bytes).map_err(|e| e.to_string())?;
        if menu.images.len() <= 14 {
            return Err(format!(
                "{menu_name}: expected at least 15 images, found {}",
                menu.images.len()
            ));
        }
        if newh.images.len() <= 12 {
            return Err(format!(
                "{newh_name}: expected at least 13 images, found {}",
                newh.images.len()
            ));
        }
        let save = match save {
            Some((name, bytes)) if !bytes.is_empty() => Some(
                MenuRes::parse_named(name, bytes)
                    .map_err(|e| e.to_string())?,
            ),
            _ => None,
        };
        if let Some(s) = &save {
            if s.images.is_empty() {
                return Err("SAVE resource has no images".to_string());
            }
        }
        let fonts = Fonts::parse(font_bytes);
        let w = res.width() as usize;
        let h = res.height() as usize;
        Ok(Frontend {
            res,
            menu,
            newh,
            save,
            fonts,
            screen: FrontendScreen::MainMenu,
            hovered: None,
            mouse: (0, 0),
            new_hero: NewHeroState::default(),
            frame: vec![0u8; w * h * 4],
            texts: Vec::new(),
            debug_regions: false,
            game_active: false,
            host_w: res.width(),
            host_h: res.height(),
            quit: false,
        })
    }

    pub fn logical_w(&self) -> i32 {
        self.res.width()
    }

    pub fn logical_h(&self) -> i32 {
        self.res.height()
    }

    /// Screen code for diagnostics: 0 MainMenu, 1 NewHero, 2 Settings,
    /// 3 LoadGame, 4 SaveGame, 5 InGame.
    pub fn screen_code(&self) -> u32 {
        match self.screen {
            FrontendScreen::MainMenu => 0,
            FrontendScreen::NewHero => 1,
            FrontendScreen::Settings => 2,
            FrontendScreen::LoadGame => 3,
            FrontendScreen::SaveGame => 4,
        }
    }

    /// Selected hero identity (0/1/2), or -1 when none is selected.
    pub fn selected_hero(&self) -> i32 {
        self.new_hero
            .selected_hero
            .map(|h| h as i32)
            .unwrap_or(-1)
    }

    /// Selected class entity value (`Entity+0x04`), or -1 when none is selected.
    pub fn selected_class_entity(&self) -> i32 {
        self.new_hero
            .selected_class
            .map(|c| c.entity_class() as i32)
            .unwrap_or(-1)
    }

    /// Hovered index for the current screen (`MAIN_MENU` index or NEWH region),
    /// or -1 when nothing is hovered.
    pub fn hover_index(&self) -> i32 {
        match self.screen {
            FrontendScreen::MainMenu => self.hovered.map(|h| h as i32).unwrap_or(-1),
            FrontendScreen::NewHero => self
                .new_hero
                .hovered_region
                .map(|h| h as i32)
                .unwrap_or(-1),
            _ => -1,
        }
    }

    pub fn frame_ptr(&self) -> *const u8 {
        self.frame.as_ptr()
    }

    // --- window mapping -----------------------------------------------------

    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.host_w = w.max(1);
        self.host_h = h.max(1);
    }

    /// Current host (canvas backing-store) size, as supplied by `on_resize`.
    /// Callers replacing the frontend runtime must reapply this to the new
    /// instance so input mapping keeps using the real host coordinate system.
    pub fn host_size(&self) -> (i32, i32) {
        (self.host_w, self.host_h)
    }

    /// Map host (canvas device-pixel) coordinates into logical frontend pixels,
    /// letterboxed exactly like the game framebuffer. Returns `None` outside the
    /// logical viewport.
    pub fn window_to_logical(&self, x: f64, y: f64) -> Option<(i32, i32)> {
        let sw = self.host_w.max(1);
        let sh = self.host_h.max(1);
        let vw = self.res.width() as f64;
        let vh = self.res.height() as f64;
        let scale = (sw as f64 / vw).min(sh as f64 / vh);
        let w = (vw * scale).round().max(1.0) as i32;
        let h = (vh * scale).round().max(1.0) as i32;
        let rx = (sw - w) / 2;
        let ry = (sh - h) / 2;
        if x < rx as f64 || x >= (rx + w) as f64 || y < ry as f64 || y >= (ry + h) as f64 {
            return None;
        }
        Some((
            ((x - rx as f64) / scale) as i32,
            ((y - ry as f64) / scale) as i32,
        ))
    }

    // --- input --------------------------------------------------------------

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        let Some((mx, my)) = self.window_to_logical(x, y) else {
            return;
        };
        self.mouse = (mx, my);
        match self.screen {
            FrontendScreen::MainMenu => {
                self.hovered = main_menu::hit_test(self.res, self.game_active, mx, my);
            }
            FrontendScreen::NewHero => {
                self.new_hero.hovered_region = NewHeroState::hit_test(self.res, mx, my);
            }
            _ => {}
        }
    }

    pub fn pointer_down(&mut self, button: u32, x: f64, y: f64) -> Option<FrontendCommand> {
        if button != 1 && button != 3 {
            return None;
        }
        let (mx, my) = self.window_to_logical(x, y)?;
        self.mouse = (mx, my);
        match self.screen {
            FrontendScreen::MainMenu => {
                let i = main_menu::hit_test(self.res, self.game_active, mx, my)?;
                match MAIN_MENU[i].action {
                    MainMenuAction::Resume => {
                        if self.game_active {
                            return Some(FrontendCommand::Resume);
                        }
                    }
                    MainMenuAction::Save => {
                        // Save is only reachable with a live game (startup menu
                        // hides it; this guards against stale hit tests).
                        if self.game_active {
                            self.screen = FrontendScreen::SaveGame;
                            self.hovered = None;
                        }
                    }
                    MainMenuAction::Load => {
                        self.screen = FrontendScreen::LoadGame;
                        self.hovered = None;
                    }
                    MainMenuAction::NewGame => {
                        // Do not discard a suspended game yet: it is only
                        // replaced when a hero is confirmed.
                        self.screen = FrontendScreen::NewHero;
                        self.new_hero = NewHeroState::default();
                        self.hovered = None;
                    }
                    MainMenuAction::Settings => {
                        self.screen = FrontendScreen::Settings;
                        self.hovered = None;
                    }
                    MainMenuAction::Exit => {
                        self.quit = true;
                        return Some(FrontendCommand::Quit);
                    }
                }
            }
            FrontendScreen::NewHero => {
                let region = NewHeroState::hit_test(self.res, mx, my)?;
                if let Some(hero) = NewHeroState::hero_index_for_region(region) {
                    self.new_hero.select_hero(hero);
                } else if let Some(class) = crate::character::CharacterClass::from_region(region) {
                    // Class boxes (regions 3..6) apply the class preset to the
                    // character-creation draft.
                    self.new_hero.select_class(class);
                } else if (15..=19).contains(&region) {
                    // Secondary-skill selectors: #15/#16 combat, #17/#18/#19
                    // civil. Regions #13/#14 (Smithing/Building) are original
                    // no-op handlers and are intentionally not handled here.
                    use crate::character::{CivilSpecialization, CombatSpecialization};
                    if let Some(build) = self.new_hero.build.as_mut() {
                        match region {
                            15 => build.select_combat(CombatSpecialization::Fencing),
                            16 => build.select_combat(CombatSpecialization::Marksmanship),
                            17 => build.select_civil(CivilSpecialization::Identification),
                            18 => build.select_civil(CivilSpecialization::Trade),
                            19 => build.select_civil(CivilSpecialization::Healing),
                            _ => {}
                        }
                    }
                } else if (layout::NEWH_STEPPER_FIRST_REGION
                    ..layout::NEWH_STEPPER_FIRST_REGION + 8)
                    .contains(&region)
                {
                    // Stat steppers (regions 25..32): even offset = `-`, odd = `+`.
                    let off = region - layout::NEWH_STEPPER_FIRST_REGION;
                    let stat = crate::character::PrimaryStat::ORDER[off / 2];
                    if let Some(build) = self.new_hero.build.as_mut() {
                        if off % 2 == 0 {
                            build.decrease(stat);
                        } else {
                            build.try_increase(stat);
                        }
                    }
                } else if let Some(bottom) = NewHeroState::bottom_index_for_region(region) {
                    if bottom == 1 {
                        // Right control ("Ok", #11/#12) -> confirm. Requires both
                        // a hero identity and a class/build (no default class).
                        if self.new_hero.can_confirm() {
                            let hero = self.new_hero.selected_hero.unwrap() as u32;
                            let build = self.new_hero.build.unwrap();
                            return Some(FrontendCommand::StartGame {
                                hero_index: hero,
                                build,
                            });
                        }
                    } else {
                        // Left control ("Отмена", #9/#10) -> back.
                        self.screen = FrontendScreen::MainMenu;
                        self.hovered = None;
                    }
                }
            }
            _ => {}
        }
        None
    }

    pub fn pointer_up(&mut self, _button: u32) {}

    pub fn key_down(&mut self, code: u32) -> Option<FrontendCommand> {
        match code {
            KEY_K => {
                self.debug_regions = !self.debug_regions;
                None
            }
            KEY_ESCAPE => match self.screen {
                // From an in-game menu Escape resumes the preserved game; from a
                // fresh startup menu it behaves like Exit (browser: harmless).
                FrontendScreen::MainMenu => {
                    if self.game_active {
                        Some(FrontendCommand::Resume)
                    } else {
                        self.quit = true;
                        Some(FrontendCommand::Quit)
                    }
                }
                // Sub-screens (New Hero / Settings / Save / Load) return to the
                // main menu without touching the live game.
                _ => {
                    self.screen = FrontendScreen::MainMenu;
                    self.hovered = None;
                    None
                }
            },
            _ => None,
        }
    }

    // --- drawing ------------------------------------------------------------

    pub fn step(&mut self) {
        self.draw();
    }

    fn draw(&mut self) {
        self.texts.clear();
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        fill_rect(&mut self.frame, fw, fh, 0, 0, fw as i32, fh as i32, [0, 0, 0, 255]);
        match self.screen {
            FrontendScreen::MainMenu => self.draw_main_menu(),
            FrontendScreen::NewHero => self.draw_new_hero(),
            FrontendScreen::Settings => self.draw_settings(),
            FrontendScreen::SaveGame | FrontendScreen::LoadGame => self.draw_save_load(),
        }
        if self.debug_regions {
            self.draw_region_debug();
        }
    }

}
#[cfg(test)]
mod tests;
