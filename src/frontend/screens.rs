//! `screens` subsystem of [`crate::frontend`] (split from `mod.rs`).

use super::*;

pub fn action_name(a: MainMenuAction) -> &'static str {
    match a {
        MainMenuAction::Resume => "Resume",
        MainMenuAction::Save => "Save",
        MainMenuAction::Load => "Load",
        MainMenuAction::NewGame => "NewGame",
        MainMenuAction::Settings => "Settings",
        MainMenuAction::Exit => "Exit",
    }
}


pub fn draw_image(frame: &mut [u8], fw: usize, fh: usize, img: &MenuImage, x: i32, y: i32) {
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


pub fn rect_outline(frame: &mut [u8], fw: usize, fh: usize, r: Rect, c: [u8; 4]) {
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, r.w as i32, 1, c);
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32 + r.h as i32 - 1, r.w as i32, 1, c);
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, 1, r.h as i32, c);
    fill_rect(frame, fw, fh, r.x as i32 + r.w as i32 - 1, r.y as i32, 1, r.h as i32, c);
}


impl Frontend {
    /// Original Settings screen: `MENU{width}R.RES` image #1. Individual
    /// controls (640x480/800x600 toggles, option rows, Ok/Отмена) have no
    /// recovered hitboxes yet, so only Escape/Back is implemented.
    pub(super) fn draw_settings(&mut self) {
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
    pub(super) fn draw_save_load(&mut self) {
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


    pub(super) fn draw_main_menu(&mut self) {
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


    pub(super) fn draw_new_hero(&mut self) {
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
    pub(super) fn draw_new_hero_values(&mut self, build: crate::character::CharacterBuild) {
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
    pub(super) fn draw_region_debug(&mut self) {
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
