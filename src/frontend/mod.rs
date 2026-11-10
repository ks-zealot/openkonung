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
//!
//! # Screen state machine
//!
//! ```text
//! MainMenu --Load--------> LoadGame --Escape--> MainMenu
//! MainMenu --Save--------> SaveGame --Escape--> MainMenu   (only with a live game)
//! MainMenu --NewGame-----> NewHero -----Escape--> MainMenu
//! NewHero  --Ok (region 20) -> StartGame command (hero_index + CharacterBuild)
//! NewHero  --Cancel (21)  -> MainMenu
//! MainMenu --Settings----> Settings --Ok-----> MainMenu (+ pending_resolution)
//! Settings --Cancel/Esc--> MainMenu (entry snapshot restored)
//! MainMenu --Resume------> Resume command      (live game re-activated)
//! MainMenu --Exit--------> Quit command + quit flag
//! ```
//!
//! Entering a sub-screen never discards the preserved game: `game_active` stays
//! set and the host runtime keeps owning the live [`crate::app::PlayableApp`].
//! Every screen keeps its own hover slot (`hovered`, `save_slot_hover`,
//! `new_hero.hovered_region`, `settings.hovered`) so a click and the following
//! redraw agree on what the pointer is over.
//!
//! # Original draw routine
//!
//! One frame per [`Frontend::step`]: clear the logical framebuffer to opaque
//! black, blit the current screen's decoded image stack, then optionally outline
//! every original hit rectangle when `K` toggled the debug overlay. The original
//! frontend draws into the same 640x480/800x600 surface and is driven by the
//! mode switch in `FUN_00436214` (case 0 settings, case 1 main menu, case 2
//! New-Hero) with the per-screen painters `FUN_0043060C` (settings),
//! `FUN_0042EA6C` (New Hero) and `FUN_004304F4` (save/load slots).

mod screens;

pub use self::screens::*;

pub mod description;
pub mod layout;
pub mod main_menu;
pub mod new_hero;
pub mod previews;
pub mod settings;

use crate::app::TextItem;
use crate::font::{Fonts, OriginalFont};
use crate::menu_res::{MenuImage, MenuRes};
use crate::res::{blit, fill_rect};

use self::layout::*;
use self::new_hero::NewHeroState;
use self::settings::SettingsState;

/// Explicit frontend screens (task section 25). `Settings` renders the confirmed
/// `MENU{width}R.RES` #1 settings screen; `SaveGame`/`LoadGame` render the
/// original `SAVE{width}R.RES` screen. The live game (if any) is owned by the
/// host runtime, so there is no `InGame` screen here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendScreen {
    /// Startup / in-game menu painted from `MENU{width}R.RES` `#0`.
    MainMenu,
    /// Character creation painted from `NEWH{width}R.RES` (`FUN_0042EA6C`).
    NewHero,
    /// Options screen painted from `MENU{width}R.RES` `#1` (`FUN_0043060C`).
    Settings,
    /// Read-only `SAVE{width}R.RES` `#0` load screen: the ten slot rows, with a
    /// click requesting that slot from the host.
    LoadGame,
    /// `SAVE{width}R.RES` `#1` save screen, same slot rows; a click requests a
    /// save into that slot (needs a live game).
    SaveGame,
}

/// A command the frontend asks the wasm layer to perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendCommand {
    /// Start gameplay with the selected hero identity (0/1/2) and the final
    /// character-creation build. The build is copied into the runtime player
    /// exactly as configured — the class preset is not regenerated.
    StartGame {
        /// Hero identity index `0..=2` (Dragomir / Mikhail / Wolf), i.e. the
        /// `NEWH_HIT_*` hero region ordinal.
        hero_index: u32,
        /// Character-creation draft as edited on the New-Hero screen: class,
        /// primary stats, free XP and the seven secondary skills.
        build: crate::character::CharacterBuild,
    },
    /// Re-activate the preserved live game.
    Resume,
    /// Exit the application.
    Quit,
}

use crate::constants::keys::KEY_K;
/// Virtual key code of Escape (Win32 `VK_ESCAPE`) as dispatched by the shell.
const KEY_ESCAPE: u32 = 27;

/// The original frontend runtime: decoded menu resources, screen state and the
/// RGBA framebuffer handed to the browser shell.
///
/// One instance owns exactly one logical resolution; a Settings Ok that changes
/// the video mode is reported through [`Frontend::pending_resolution`] and the
/// host rebuilds this struct at the new resolution.
pub struct Frontend {
    /// Active logical resolution; selects the `MENU`/`NEWH`/`SAVE` layout
    /// tables and the framebuffer size.
    pub res: FrontendRes,
    menu: MenuRes,
    newh: MenuRes,
    /// Original `SAVE{width}R.RES` (Save/Load screen), when the assets are
    /// present. `None` renders a controlled placeholder.
    save: Option<MenuRes>,
    /// Original bitmap fonts from `konung.res` (used by the New-Hero screen).
    fonts: Fonts,
    /// Current screen; see the module state machine.
    pub screen: FrontendScreen,
    /// Hovered `MAIN_MENU` index while on the main menu.
    pub hovered: Option<usize>,
    /// Hovered save/load slot (0..9) on the SAVE screens.
    pub save_slot_hover: Option<usize>,
    /// Last logical mouse position.
    pub mouse: (i32, i32),
    /// New-Hero selection and character-creation draft.
    pub new_hero: NewHeroState,
    /// Settings screen live values plus the entry snapshot.
    pub settings: SettingsState,
    /// Pending frontend-resolution rebuild requested by Settings Ok.
    pub pending_resolution: Option<FrontendRes>,
    /// Pre-rendered 160x160 preview frames, flat-indexed
    /// `hero_index * previews::PREVIEW_SLOTS + weapon_slot`.
    hero_previews: Vec<crate::res::Img>,
    /// RGBA framebuffer, `res.width() * res.height() * 4` bytes.
    pub frame: Vec<u8>,
    /// Debug-overlay strings (regions, screen banner) the shell draws on top.
    pub texts: Vec<TextItem>,
    /// Sound ids requested this frame (original main-menu click `4`).
    pub sounds: Vec<u16>,
    /// Debug overlay: draw every original hit rectangle with its index.
    pub debug_regions: bool,
    /// Whether a live game exists (drives the startup menu visibility).
    pub game_active: bool,
    host_w: i32,
    host_h: i32,
    /// Set by Exit (or Escape with no live game); the shell then closes.
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
            Some((name, bytes)) if !bytes.is_empty() => {
                Some(MenuRes::parse_named(name, bytes).map_err(|e| e.to_string())?)
            }
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
            save_slot_hover: None,
            mouse: (0, 0),
            new_hero: NewHeroState::default(),
            settings: SettingsState::default(),
            pending_resolution: None,
            hero_previews: Vec::new(),
            frame: vec![0u8; w * h * 4],
            texts: Vec::new(),
            sounds: Vec::new(),
            debug_regions: false,
            game_active: false,
            host_w: res.width(),
            host_h: res.height(),
            quit: false,
        })
    }

    /// Attach pre-rendered character preview frames (built once at frontend
    /// boot). The vector is flat-indexed by hero and weapon slot; an empty
    /// vector simply skips the preview.
    pub fn set_hero_previews(&mut self, previews: Vec<crate::res::Img>) {
        self.hero_previews = previews;
    }

    /// Logical framebuffer width in pixels (640 or 800).
    pub fn logical_w(&self) -> i32 {
        self.res.width()
    }

    /// Logical framebuffer height in pixels (480 or 600).
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
        self.new_hero.selected_hero.map(|h| h as i32).unwrap_or(-1)
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
            FrontendScreen::NewHero => self.new_hero.hovered_region.map(|h| h as i32).unwrap_or(-1),
            FrontendScreen::Settings => self.settings.hovered.map(|h| h as i32).unwrap_or(-1),
            _ => -1,
        }
    }

    /// Raw pointer to the RGBA framebuffer.
    ///
    /// # Memory contract
    ///
    /// The pointer is owned by `self` and stays valid until the next [`Frontend::step`]
    /// or drop; the wasm ABI `frame_ptr` export hands it to JS together with
    /// [`Frontend::logical_w`] * [`Frontend::logical_h`] * 4 bytes.
    pub fn frame_ptr(&self) -> *const u8 {
        self.frame.as_ptr()
    }

    // --- window mapping -----------------------------------------------------

    /// Record the host (canvas backing-store) size used to map pointer input.
    ///
    /// Degenerate sizes are clamped to at least `1x1`; the logical framebuffer
    /// is never resized here.
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

    /// Update the per-screen hover from a host pointer position.
    ///
    /// Coordinates outside the letterboxed logical viewport are ignored (the
    /// last logical position stays). On New Hero this also re-targets the
    /// big-panel description exactly like `FUN_00436214` case 2: a stat stepper
    /// (`25..=32`) is normalised to its stat row `7..=10`, and a region with no
    /// description record leaves the previous text untouched.
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
                let region = NewHeroState::hit_test(self.res, mx, my);
                self.new_hero.hovered_region = region;
                // `FUN_00436214` case 2: hover over 0..19 (or a stat stepper,
                // normalised to 7..10) updates the big-panel description. The
                // other regions leave the last shown text untouched.
                if let Some(region) = region {
                    if description::description_for_region(region).is_some() {
                        self.new_hero.description_region = Some(if (25..=32).contains(&region) {
                            7 + (region - 25) / 2
                        } else {
                            region
                        });
                    }
                }
            }
            FrontendScreen::SaveGame | FrontendScreen::LoadGame => {
                self.save_slot_hover = self.res.save_slot_hit_test(mx, my);
            }
            FrontendScreen::Settings => {
                self.settings.hovered = SettingsState::hit_test(self.res, mx, my);
            }
        }
    }

    /// Handle a button press in logical coordinates and return a host command.
    ///
    /// Only buttons `1` (left) and `3` (right) are accepted; the original
    /// frontend acts on press, so a right click on a hit region behaves like a
    /// left click here (only a left click queues the UI click sound `4`).
    /// New Hero routes the region: hero `0..2`, class `3..6`, skill selectors
    /// `15..19`, stat steppers `25..32`, Ok `20` (`FrontendCommand::StartGame`)
    /// and the right-hand Cancel control `21`. SAVE rows request the slot from
    /// the host; Settings applies the control, and Ok additionally asks for a
    /// resolution rebuild when the video mode changed.
    pub fn pointer_down(&mut self, button: u32, x: f64, y: f64) -> Option<FrontendCommand> {
        if button != 1 && button != 3 {
            return None;
        }
        let (mx, my) = self.window_to_logical(x, y)?;
        self.mouse = (mx, my);
        match self.screen {
            FrontendScreen::MainMenu => {
                let i = main_menu::hit_test(self.res, self.game_active, mx, my)?;
                // `FUN_00436214` case 1: valid menu click -> sound 4.
                if button == 1 {
                    self.sounds.push(crate::sound::events::UI_CLICK);
                }
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
                        self.settings = SettingsState::enter();
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
                // `FUN_00436214` case 2 also calls `FUN_0042F578` after a click
                // on every control that has a description (0..19); a stepper
                // click shows its stat-row description (7..10).
                if description::description_for_region(region).is_some() {
                    self.new_hero.description_region = Some(if (25..=32).contains(&region) {
                        7 + (region - 25) / 2
                    } else {
                        region
                    });
                }
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
                    use crate::character::CivilSpecialization;
                    match region {
                        15 | 16 => {
                            let spec = if region == 15 {
                                crate::character::CombatSpecialization::Fencing
                            } else {
                                crate::character::CombatSpecialization::Marksmanship
                            };
                            self.new_hero.select_combat(spec);
                        }
                        17 | 18 | 19 => {
                            let spec = match region {
                                17 => CivilSpecialization::Identification,
                                18 => CivilSpecialization::Trade,
                                _ => CivilSpecialization::Healing,
                            };
                            if let Some(build) = self.new_hero.build.as_mut() {
                                build.select_civil(spec);
                            }
                        }
                        _ => {}
                    }
                } else if (layout::NEWH_STEPPER_FIRST_REGION..layout::NEWH_STEPPER_FIRST_REGION + 8)
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
                } else if region == layout::BOTTOM_OK_REGION {
                    if self.new_hero.can_confirm() {
                        let hero = self.new_hero.selected_hero.unwrap() as u32;
                        let build = self.new_hero.build.unwrap();
                        return Some(FrontendCommand::StartGame {
                            hero_index: hero,
                            build,
                        });
                    }
                } else if region == layout::BOTTOM_CANCEL_REGION {
                    // Left control ("Отмена", #9/#10) -> back.
                    self.screen = FrontendScreen::MainMenu;
                    self.hovered = None;
                }
            }
            FrontendScreen::SaveGame | FrontendScreen::LoadGame => {
                if let Some(slot) = self.res.save_slot_hit_test(mx, my) {
                    if self.screen == FrontendScreen::SaveGame {
                        if self.game_active {
                            crate::request_save_slot(slot as u32);
                        }
                    } else {
                        crate::request_load_slot(slot as u32);
                    }
                }
            }
            FrontendScreen::Settings => {
                let Some(control) = SettingsState::hit_test(self.res, mx, my) else {
                    return None;
                };
                if button == 1 {
                    self.sounds.push(crate::sound::events::UI_CLICK);
                }
                if control <= settings::CONTROL_SOUND {
                    self.settings.activate(control);
                } else if control == settings::CONTROL_OK {
                    self.settings.apply();
                    let width = self.settings.live.width();
                    if width != self.res.width() {
                        self.pending_resolution = Some(if width == 640 {
                            FrontendRes::R640
                        } else {
                            FrontendRes::R800
                        });
                    }
                    self.screen = FrontendScreen::MainMenu;
                    self.hovered = None;
                } else if control == settings::CONTROL_CANCEL {
                    self.settings.cancel();
                    self.screen = FrontendScreen::MainMenu;
                    self.hovered = None;
                }
            }
        }
        None
    }

    /// Release handler. The original frontend has no press/release distinction,
    /// so every action already fired in [`Frontend::pointer_down`] and this is a
    /// no-op kept for the shell's symmetric event calls.
    pub fn pointer_up(&mut self, _button: u32) {}

    /// Handle a key press by Win32 virtual key code.
    ///
    /// `K` toggles the hit-rectangle debug overlay. Escape leaves any sub-screen
    /// (restoring the Settings entry snapshot first) back to the main menu; on
    /// the main menu itself it resumes a preserved game, or sets [`Frontend::quit`]
    /// and returns `Quit` when the menu is the fresh startup menu.
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
                // main menu without touching the live game. Leaving Settings
                // restores the entry snapshot (original Cancel).
                _ => {
                    if self.screen == FrontendScreen::Settings {
                        self.settings.cancel();
                    }
                    self.screen = FrontendScreen::MainMenu;
                    self.hovered = None;
                    None
                }
            },
            _ => None,
        }
    }

    // --- drawing ------------------------------------------------------------

    /// Advance the frontend by one frame: drop the previous frame's sound queue
    /// and redraw the current screen.
    ///
    /// # Side effects
    ///
    /// `sounds` is cleared at the start of the call, so the shell must drain it
    /// before stepping; `frame` and `texts` are rebuilt from scratch.
    pub fn step(&mut self) {
        // One-frame queue: the shell drains it before this call.
        self.sounds.clear();
        self.draw();
    }

    /// Paint one complete frontend frame into `frame`.
    ///
    /// Clears to opaque black first (the `NEWH#0` artwork is partly
    /// transparent, so an uninitialised buffer would show through), dispatches
    /// to the per-screen painter and finally overlays the debug rectangles when
    /// `debug_regions` is set.
    fn draw(&mut self) {
        self.texts.clear();
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        fill_rect(
            &mut self.frame,
            fw,
            fh,
            0,
            0,
            fw as i32,
            fh as i32,
            [0, 0, 0, 255],
        );
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
