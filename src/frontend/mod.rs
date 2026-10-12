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

pub mod layout;
pub mod main_menu;
pub mod new_hero;

use crate::app::TextItem;
use crate::font::{Fonts, OriginalFont};
use crate::menu_res::{MenuImage, MenuRes};
use crate::res::{blit, fill_rect};

use layout::{
    FrontendRes, MainMenuAction, Rect, MAIN_MENU, MENU_IMAGE_A, MENU_IMAGE_SETTINGS,
    NEWH_BACKGROUND, NEWH_CLASS_MARKER, SAVE_IMAGE_LOAD, SAVE_IMAGE_SAVE,
};
use new_hero::NewHeroState;

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

const KEY_K: u32 = 75;
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

    /// Original Settings screen: `MENU{width}R.RES` image #1. Individual
    /// controls (640x480/800x600 toggles, option rows, Ok/Отмена) have no
    /// recovered hitboxes yet, so only Escape/Back is implemented.
    fn draw_settings(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        draw_image(
            &mut self.frame,
            fw,
            fh,
            &self.menu.images[MENU_IMAGE_SETTINGS],
            0,
            0,
        );
    }

    /// Original Save/Load screen (`SAVE{width}R.RES`): `#0` = "Загрузка игры"
    /// (Load) and `#1` = "Сохранение игры" (Save). Save-slot parsing/
    /// serialization is deferred to the next milestone; the screen is a real
    /// routed state and Escape/Back returns to the menu. When the SAVE asset is
    /// absent, a black placeholder is drawn.
    fn draw_save_load(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        let index = match self.screen {
            FrontendScreen::LoadGame => SAVE_IMAGE_LOAD,
            _ => SAVE_IMAGE_SAVE,
        };
        if let Some(save) = &self.save {
            if let Some(img) = save.images.get(index) {
                draw_image(&mut self.frame, fw, fh, img, 0, 0);
            }
        }
    }

    fn draw_main_menu(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        draw_image(&mut self.frame, fw, fh, &self.menu.images[MENU_IMAGE_A], 0, 0);
        for i in main_menu::visible_indices(self.game_active) {
            let def = &MAIN_MENU[i];
            let img = if self.hovered == Some(i) {
                def.hover_image
            } else {
                def.normal_image
            };
            let rect = self.res.main_rect(def);
            draw_image(
                &mut self.frame,
                fw,
                fh,
                &self.menu.images[img],
                rect.x as i32,
                rect.y as i32,
            );
        }
    }

    fn draw_new_hero(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        draw_image(
            &mut self.frame,
            fw,
            fh,
            &self.newh.images[NEWH_BACKGROUND],
            0,
            0,
        );

        for hero in 0..layout::HERO_REGIONS.len() {
            let region = layout::HERO_REGIONS[hero];
            let img = self.new_hero.hero_image(hero, region);
            let rect = NewHeroState::draw_rect(self.res, region);
            draw_image(
                &mut self.frame,
                fw,
                fh,
                &self.newh.images[img],
                rect.x as i32,
                rect.y as i32,
            );
        }
        // Original round class-selection marker (NEWH image #2), centered inside
        // the selected class draw rectangle (VA 0x42EA6C; region = class+1).
        if let Some(class) = self.new_hero.selected_class {
            let region = class.region();
            let r = self.res.newh_draw()[region];
            let marker = &self.newh.images[NEWH_CLASS_MARKER];
            let mx = r.x as i32 + (r.w as i32 - marker.width as i32) / 2;
            let my = r.y as i32 + (r.h as i32 - marker.height as i32) / 2;
            draw_image(&mut self.frame, fw, fh, marker, mx, my);
        }

        // Dynamic character values in the original small bitmap font.
        if let Some(build) = self.new_hero.build {
            self.draw_new_hero_values(build);
        }

        for bottom in 0..layout::BOTTOM_REGIONS.len() {
            let region = layout::BOTTOM_REGIONS[bottom];
            let img = self.new_hero.bottom_image(bottom, region);
            let rect = NewHeroState::draw_rect(self.res, region);
            draw_image(
                &mut self.frame,
                fw,
                fh,
                &self.newh.images[img],
                rect.x as i32,
                rect.y as i32,
            );
        }

        // Always-on green borders around the true secondary-skill selector
        // hitboxes: #15 Fencing, #16 Marksmanship, #17 Identification, #18
        // Trade, #19 Healing. Uses the exact `NEWH_HIT` rects. #13/#14
        // (Smithing/Building) are original no-op handlers, so they are not
        // highlighted here.
        for region in 15..=19 {
            rect_outline(
                &mut self.frame,
                fw,
                fh,
                self.res.newh_hit()[region],
                [0, 255, 0, 255],
            );
        }
    }

    /// Draw the dynamic stat/skill values with the original small font at the
    /// recovered `NEWH_DRAW` value-box rects (task R4.2). Labels are baked into
    /// `NEWH#0` and are never redrawn.
    fn draw_new_hero_values(&mut self, build: crate::character::CharacterBuild) {
        use crate::character::{max_load, max_party, PrimaryStat};
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        // (region, text): points #24, max load #11, max party #12, stats #7..10,
        // skills #13..19.
        let mut entries: Vec<(usize, String)> = Vec::with_capacity(14);
        entries.push((layout::NEWH_POINTS_REGION, build.available_xp.to_string()));
        entries.push((11, format!("{:4.1}", max_load(build.endurance))));
        entries.push((12, max_party(build.class, build.charisma).to_string()));
        for (i, stat) in PrimaryStat::ORDER.iter().enumerate() {
            entries.push((7 + i, build.stat_value(*stat).to_string()));
        }
        entries.push((13, build.smithing.to_string()));
        entries.push((14, build.building.to_string()));
        entries.push((15, build.melee_mastery.to_string()));
        entries.push((16, build.archery_mastery.to_string()));
        entries.push((17, build.identification.to_string()));
        entries.push((18, build.trade.to_string()));
        entries.push((19, build.healing.to_string()));

        let draws = self.res.newh_draw();
        for (region, text) in entries {
            let r = draws[region];
            let tw = self.fonts.measure(OriginalFont::Small, &text);
            let x = r.x as i32 + (r.w as i32 - tw) / 2;
            let y = r.y as i32 + (r.h as i32 - 12) / 2; // small font cap height
            self.fonts.draw(
                &mut self.frame,
                fw,
                fh,
                OriginalFont::Small,
                &text,
                x,
                y,
                None,
            );
        }
    }

    /// Debug overlay (key `K`): outlines every original hit rectangle and labels
    /// it with its region/button index. Off by default (task §30).
    fn draw_region_debug(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        match self.screen {
            FrontendScreen::MainMenu => {
                for i in main_menu::visible_indices(self.game_active) {
                    let rect = self.res.main_rect(&MAIN_MENU[i]);
                    rect_outline(&mut self.frame, fw, fh, rect, [255, 80, 80, 255]);
                    self.texts.push(TextItem {
                        x: rect.x as i32 + 2,
                        y: rect.y as i32 + 1,
                        rgb: 0xFF_FF80,
                        scale: 0.8,
                        s: format!("{} {}", i, action_name(MAIN_MENU[i].action)),
                    });
                }
            }
            FrontendScreen::NewHero => {
                for (r, rect) in self.res.newh_hit().iter().enumerate() {
                    let (color, tag) = match new_hero::region_status(r) {
                        new_hero::RegionStatus::Hero => ([120, 255, 120, 255], "hero"),
                        new_hero::RegionStatus::Class => ([255, 160, 60, 255], "class"),
                        new_hero::RegionStatus::Stepper => ([200, 120, 255, 255], "stepper"),
                        new_hero::RegionStatus::Points => ([120, 255, 255, 255], "points"),
                        new_hero::RegionStatus::Bottom => ([255, 200, 60, 255], "ok/back"),
                        // Real original hitbox, behavior not yet recovered.
                        new_hero::RegionStatus::Unknown => ([120, 180, 255, 255], "?"),
                    };
                    rect_outline(&mut self.frame, fw, fh, *rect, color);
                    self.texts.push(TextItem {
                        x: rect.x as i32 + 2,
                        y: rect.y as i32 + 1,
                        rgb: 0xFF_FFFF,
                        scale: 0.8,
                        s: format!("{r} {tag}"),
                    });
                }
            }
            _ => {}
        }
        self.texts.push(TextItem {
            x: 6,
            y: self.res.height() - 16,
            rgb: 0xAF_F0C3,
            scale: 0.9,
            s: format!(
                "frontend debug | {:?} logical {}x{} hero={:?} class={:?}",
                self.screen,
                self.res.width(),
                self.res.height(),
                self.new_hero.selected_hero,
                self.new_hero.selected_class,
            ),
        });
    }
}

fn action_name(a: MainMenuAction) -> &'static str {
    match a {
        MainMenuAction::Resume => "Resume",
        MainMenuAction::Save => "Save",
        MainMenuAction::Load => "Load",
        MainMenuAction::NewGame => "NewGame",
        MainMenuAction::Settings => "Settings",
        MainMenuAction::Exit => "Exit",
    }
}

fn draw_image(frame: &mut [u8], fw: usize, fh: usize, img: &MenuImage, x: i32, y: i32) {
    blit(
        frame,
        fw,
        fh,
        &img.rgba,
        img.width as usize,
        img.height as usize,
        x,
        y,
    );
}

fn rect_outline(frame: &mut [u8], fw: usize, fh: usize, r: Rect, c: [u8; 4]) {
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, r.w as i32, 1, c);
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32 + r.h as i32 - 1, r.w as i32, 1, c);
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, 1, r.h as i32, c);
    fill_rect(frame, fw, fh, r.x as i32 + r.w as i32 - 1, r.y as i32, 1, r.h as i32, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout::{NEWH_HIT_640, MENU_IMAGE_A};

    fn synthetic_frontend() -> Frontend {
        // Minimal 15-image MENU and 13-image NEWH resources, 1x1 each.
        let mut menu_bytes = Vec::new();
        for _ in 0..15 {
            menu_bytes.extend_from_slice(&rec(1, 1, &[[0x00]]));
        }
        let mut newh_bytes = Vec::new();
        for _ in 0..13 {
            newh_bytes.extend_from_slice(&rec(1, 1, &[[0x00]]));
        }
        Frontend::new(
            FrontendRes::R640,
            "MENU640R.RES",
            &menu_bytes,
            "NEWH640R.RES",
            &newh_bytes,
            None,
            &[],
        )
        .unwrap()
    }

    /// Click the main-menu action with the given index at the 640 layout.
    fn click_action(f: &mut Frontend, action: MainMenuAction) -> Option<FrontendCommand> {
        f.on_resize(640, 480);
        let r = MAIN_MENU[action.index()].rect_640;
        f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0)
    }

    fn rec(w: u16, h: u16, rows: &[[u8; 1]]) -> Vec<u8> {
        let mut out = vec![0x00, 0x00, 0x00, 0x05];
        let sum = rows.len();
        let payload = 4 + rows.len() * 2 + sum;
        out.extend_from_slice(&(payload as u32).to_le_bytes());
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        for _ in rows {
            out.extend_from_slice(&1u16.to_le_bytes());
        }
        for r in rows {
            out.extend_from_slice(r);
        }
        out
    }

    #[test]
    fn startup_menu_omits_resume_and_save() {
        let visible = main_menu::visible_indices(false);
        assert_eq!(visible, vec![2, 3, 4, 5]);
        assert!(!visible.contains(&MainMenuAction::Resume.index()));
        assert!(!visible.contains(&MainMenuAction::Save.index()));
    }

    #[test]
    fn new_game_switches_to_new_hero() {
        let mut f = synthetic_frontend();
        assert_eq!(f.screen, FrontendScreen::MainMenu);
        // Click the NewGame rect.
        let r = MAIN_MENU[MainMenuAction::NewGame.index()].rect_640;
        f.on_resize(640, 480);
        let cmd = f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0);
        assert_eq!(cmd, None);
        assert_eq!(f.screen, FrontendScreen::NewHero);
    }

    #[test]
    fn hero_and_class_selection_then_confirm_starts() {
        use crate::character::CharacterClass;
        let mut f = synthetic_frontend();
        f.screen = FrontendScreen::NewHero;
        f.on_resize(640, 480);
        let ok = layout::NEWH_HIT_640[layout::BOTTOM_OK_REGION];
        let click = |f: &mut Frontend, r: crate::frontend::layout::Rect| {
            f.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0)
        };

        // Confirm with neither hero nor class is a no-op.
        assert_eq!(click(&mut f, ok), None);

        // Select hero 1; still no class -> no confirm.
        assert_eq!(click(&mut f, NEWH_HIT_640[1]), None);
        assert_eq!(f.new_hero.selected_hero, Some(1));
        assert_eq!(click(&mut f, ok), None);

        // Select Merchant (region 3) -> preset applied.
        assert_eq!(click(&mut f, NEWH_HIT_640[CharacterClass::Merchant.region()]), None);
        assert_eq!(f.new_hero.selected_class, Some(CharacterClass::Merchant));

        // Modify the draft (stat/skill editing is future work; this exercises the
        // copy path that Start Game must honour, §10).
        {
            let b = f.new_hero.build.as_mut().unwrap();
            b.charisma = 24;
            b.trade = 11;
        }
        match click(&mut f, ok) {
            Some(FrontendCommand::StartGame { hero_index, build }) => {
                assert_eq!(hero_index, 1);
                assert_eq!(build.class, CharacterClass::Merchant);
                assert_eq!(build.charisma, 24);
                assert_eq!(build.trade, 11);
            }
            other => panic!("expected StartGame, got {other:?}"),
        }
    }

    #[test]
    fn cancel_returns_to_main_menu() {
        let mut f = synthetic_frontend();
        f.screen = FrontendScreen::NewHero;
        f.on_resize(640, 480);
        let cancel = layout::NEWH_HIT_640[layout::BOTTOM_CANCEL_REGION];
        let cmd = f.pointer_down(1, cancel.x as f64 + 1.0, cancel.y as f64 + 1.0);
        assert_eq!(cmd, None);
        assert_eq!(f.screen, FrontendScreen::MainMenu);
    }

    #[test]
    fn hero_regions_pick_distinct_images() {
        let mut s = NewHeroState::default();
        assert_eq!(s.hero_image(0, 0), 3);
        s.selected_hero = Some(0);
        assert_eq!(s.hero_image(0, 0), 4);
        assert_eq!(s.hero_image(1, 1), 5);
        s.hovered_region = Some(1);
        assert_eq!(s.hero_image(1, 1), 6);
    }

    #[test]
    fn menu_draws_without_panicking() {
        let mut f = synthetic_frontend();
        f.step();
        assert_eq!(f.frame.len(), 640 * 480 * 4);
        f.debug_regions = true;
        f.step();
        assert!(!f.texts.is_empty());
    }

    #[test]
    fn background_image_is_index_zero() {
        assert_eq!(MENU_IMAGE_A, 0);
    }

    #[test]
    fn host_size_reflects_on_resize() {
        let mut f = synthetic_frontend();
        // Defaults to the logical resolution until JS reports the real host size.
        assert_eq!(f.host_size(), (640, 480));
        f.on_resize(1920, 1080);
        assert_eq!(f.host_size(), (1920, 1080));
        // Degenerate sizes are clamped to at least 1x1.
        f.on_resize(0, -5);
        assert_eq!(f.host_size(), (1, 1));
    }

    #[test]
    fn in_game_menu_exposes_all_six() {
        assert_eq!(main_menu::visible_indices(true), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn settings_entry_and_back_preserves_game_active() {
        let mut f = synthetic_frontend();
        f.game_active = true;
        assert_eq!(click_action(&mut f, MainMenuAction::Settings), None);
        assert_eq!(f.screen, FrontendScreen::Settings);
        assert_eq!(f.key_down(KEY_ESCAPE), None);
        assert_eq!(f.screen, FrontendScreen::MainMenu);
        assert!(f.game_active);
    }

    #[test]
    fn save_and_load_entry_and_back() {
        let mut f = synthetic_frontend();
        f.game_active = true;
        assert_eq!(click_action(&mut f, MainMenuAction::Save), None);
        assert_eq!(f.screen, FrontendScreen::SaveGame);
        assert_eq!(f.key_down(KEY_ESCAPE), None);
        assert_eq!(f.screen, FrontendScreen::MainMenu);

        assert_eq!(click_action(&mut f, MainMenuAction::Load), None);
        assert_eq!(f.screen, FrontendScreen::LoadGame);
        assert_eq!(f.key_down(KEY_ESCAPE), None);
        assert_eq!(f.screen, FrontendScreen::MainMenu);
    }

    #[test]
    fn resume_request_only_when_game_active() {
        let mut f = synthetic_frontend();
        f.game_active = true;
        assert_eq!(click_action(&mut f, MainMenuAction::Resume), Some(FrontendCommand::Resume));
        // Fresh menu: Resume is not visible, so no action is hit.
        let mut g = synthetic_frontend();
        g.on_resize(640, 480);
        let r = MAIN_MENU[MainMenuAction::Resume.index()].rect_640;
        assert_eq!(g.pointer_down(1, r.x as f64 + 1.0, r.y as f64 + 1.0), None);
    }

    #[test]
    fn escape_resumes_with_game_and_quits_without() {
        let mut f = synthetic_frontend();
        f.game_active = true;
        assert_eq!(f.key_down(KEY_ESCAPE), Some(FrontendCommand::Resume));
        let mut g = synthetic_frontend();
        assert_eq!(g.key_down(KEY_ESCAPE), Some(FrontendCommand::Quit));
    }

    #[test]
    fn new_hero_cancel_preserves_game_active() {
        let mut f = synthetic_frontend();
        f.game_active = true;
        assert_eq!(click_action(&mut f, MainMenuAction::NewGame), None);
        assert_eq!(f.screen, FrontendScreen::NewHero);
        assert!(f.game_active, "entering New Hero must not clear the live game");
        f.on_resize(640, 480);
        let cancel = NEWH_HIT_640[layout::BOTTOM_CANCEL_REGION];
        assert_eq!(f.pointer_down(1, cancel.x as f64 + 1.0, cancel.y as f64 + 1.0), None);
        assert_eq!(f.screen, FrontendScreen::MainMenu);
        assert!(f.game_active);
    }

    #[test]
    fn settings_and_save_load_render_without_panicking() {
        let mut f = synthetic_frontend();
        f.screen = FrontendScreen::Settings;
        f.step();
        assert_eq!(f.frame.len(), 640 * 480 * 4);
        f.screen = FrontendScreen::SaveGame;
        f.step();
        f.screen = FrontendScreen::LoadGame;
        f.step();
        assert_eq!(f.res.width(), 640);
    }

    #[test]
    fn screen_codes_are_stable() {
        let mut f = synthetic_frontend();
        assert_eq!(f.screen_code(), 0);
        f.screen = FrontendScreen::NewHero;
        assert_eq!(f.screen_code(), 1);
        f.screen = FrontendScreen::Settings;
        assert_eq!(f.screen_code(), 2);
        f.screen = FrontendScreen::LoadGame;
        assert_eq!(f.screen_code(), 3);
        f.screen = FrontendScreen::SaveGame;
        assert_eq!(f.screen_code(), 4);
    }

    #[test]
    fn skill_selector_regions_route_to_specializations() {
        use crate::character::{CivilSpecialization, CombatSpecialization};
        let mut f = synthetic_frontend();
        f.screen = FrontendScreen::NewHero;
        f.on_resize(640, 480);
        let click = |f: &mut Frontend, region: usize| {
            let r = NEWH_HIT_640[region];
            f.pointer_down(1, r.x as f64 + r.w as f64 / 2.0, r.y as f64 + r.h as f64 / 2.0)
        };
        click(&mut f, 6); // Warrior
        let b = f.new_hero.build.unwrap();
        assert_eq!(b.combat_specialization, CombatSpecialization::Fencing);
        assert_eq!(b.civil_specialization, CivilSpecialization::Identification);

        click(&mut f, 16); // Marksmanship
        let b = f.new_hero.build.unwrap();
        assert_eq!(b.combat_specialization, CombatSpecialization::Marksmanship);
        assert_eq!(
            (b.melee_mastery, b.archery_mastery, b.building, b.smithing),
            (0, 100, 2, 5)
        );

        click(&mut f, 18); // Trade
        let b = f.new_hero.build.unwrap();
        assert_eq!(b.civil_specialization, CivilSpecialization::Trade);
        assert_eq!((b.trade, b.healing, b.identification), (20, 50, 10));
        // Combat fields untouched by the civil selection.
        assert_eq!(
            (b.melee_mastery, b.archery_mastery, b.building, b.smithing),
            (0, 100, 2, 5)
        );
    }

    #[test]
    fn smithing_and_building_regions_are_noops() {
        let mut f = synthetic_frontend();
        f.screen = FrontendScreen::NewHero;
        f.on_resize(640, 480);
        let click = |f: &mut Frontend, region: usize| {
            let r = NEWH_HIT_640[region];
            f.pointer_down(1, r.x as f64 + r.w as f64 / 2.0, r.y as f64 + r.h as f64 / 2.0)
        };
        click(&mut f, 6); // class
        let before = f.new_hero.build.unwrap().skills();
        click(&mut f, 13); // Smithing (no-op)
        click(&mut f, 14); // Building (no-op)
        assert_eq!(f.new_hero.build.unwrap().skills(), before);
    }
}

