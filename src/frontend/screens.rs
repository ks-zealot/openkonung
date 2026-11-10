//! Screen painters for [`crate::frontend`] (split from `mod.rs`).
//!
//! One painter per [`FrontendScreen`]: [`Frontend::draw_main_menu`] (`MENU#0` +
//! the [`MAIN_MENU`] button images), [`Frontend::draw_new_hero`]
//! (`FUN_0042EA6C`: tile `NEWH#1`, overdraw `NEWH#0`, then the portraits,
//! values, markers, bottom controls, paper doll and description),
//! [`Frontend::draw_settings`] (`MENU#1` + the `MENU#2` markers) and
//! [`Frontend::draw_save_load`] (`SAVE#0`/`SAVE#1` + the ten slot names).
//! [`Frontend::draw_region_debug`] is the `K` overlay, not a screen.

use super::*;

/// Stable ASCII name of a main-menu action, used by the debug overlay so a
/// region label is readable without the localized button artwork.
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

/// Blit one decoded `MENU`/`NEWH`/`SAVE` image at a logical position.
///
/// The blit is clipped to the framebuffer by `res::blit`; the image keeps its
/// decoded size (`img.width`/`img.height`) and is never scaled.
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

/// Tile `img` across the whole logical framebuffer, clipped at the edges
/// (original `FUN_0042EA6C` 100-px background tiling).
pub fn tile_image(frame: &mut [u8], fw: usize, fh: usize, img: &MenuImage) {
    let step_w = img.width as i32;
    let step_h = img.height as i32;
    if step_w <= 0 || step_h <= 0 {
        return;
    }
    let cols = (fw as i32 + step_w - 1) / step_w;
    let rows = (fh as i32 + step_h - 1) / step_h;
    for row in 0..rows {
        for col in 0..cols {
            draw_image(frame, fw, fh, img, col * step_w, row * step_h);
        }
    }
}

/// Draw a 1-pixel outline around `r` with four `fill_rect` spans.
///
/// `r` is inclusive on all edges, so the bottom/right spans sit on
/// `y + h - 1` / `x + w - 1`; the corners are written twice, which is harmless
/// because the colour is opaque.
pub fn rect_outline(frame: &mut [u8], fw: usize, fh: usize, r: Rect, c: [u8; 4]) {
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, r.w as i32, 1, c);
    fill_rect(
        frame,
        fw,
        fh,
        r.x as i32,
        r.y as i32 + r.h as i32 - 1,
        r.w as i32,
        1,
        c,
    );
    fill_rect(frame, fw, fh, r.x as i32, r.y as i32, 1, r.h as i32, c);
    fill_rect(
        frame,
        fw,
        fh,
        r.x as i32 + r.w as i32 - 1,
        r.y as i32,
        1,
        r.h as i32,
        c,
    );
}

impl Frontend {
    /// Original Settings screen: `MENU{width}R.RES` image #1 plus the 13x13
    /// `MENU#2` marker centered in every enabled control (`FUN_0043060C`).
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
        let rects = self.res.settings_hit_rects();
        for control in 0..rects.len() {
            if !self.settings.marker(control) {
                continue;
            }
            let r = rects[control];
            let marker = &self.menu.images[MENU_IMAGE_SMALL];
            let mx = r.x as i32 + (r.w as i32 - marker.width as i32) / 2;
            let my = r.y as i32 + (r.h as i32 - marker.height as i32) / 2;
            draw_image(&mut self.frame, fw, fh, marker, mx, my);
        }
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
        // Original `FUN_004304F4`: 10 slot names at the recovered row rects.
        let rects = self.res.save_slot_rects();
        for (slot, r) in rects.iter().enumerate() {
            let text = crate::save_slot_name(slot as u32);
            if text.is_empty() {
                continue;
            }
            let x = r.x as i32 + 4;
            let y = r.y as i32 + (r.h as i32 - 12) / 2;
            let font = if self.save_slot_hover == Some(slot) {
                OriginalFont::Highlighted
            } else {
                OriginalFont::Small
            };
            self.fonts
                .draw(&mut self.frame, fw, fh, font, &text, x, y, None);
        }
    }

    /// Draw the main menu: `MENU{width}R.RES` `#0` full screen, then the image
    /// of every visible [`MAIN_MENU`] button at its recovered rect.
    ///
    /// The hover image replaces the normal one while `hovered` names that
    /// button; `Save`/`Resume` are only drawn when a live game exists
    /// ([`main_menu::visible_indices`]).
    pub(super) fn draw_main_menu(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        draw_image(
            &mut self.frame,
            fw,
            fh,
            &self.menu.images[MENU_IMAGE_A],
            0,
            0,
        );
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

    /// Draw the New-Hero screen in the original paint order (`FUN_0042EA6C`).
    ///
    /// 1. tile the 100x100 stone `NEWH#1` over the whole viewport, then draw the
    ///    partly transparent `NEWH#0` artwork on top;
    /// 2. the `NEWH#2` class marker centered in the selected class box;
    /// 3. the three hero portrait slots (`NEWH#3..#8`);
    /// 4. the numeric values and the combat/civil skill markers;
    /// 5. the two bottom controls (`NEWH#11/#12` Ok, `#9/#10` Cancel);
    /// 6. the pre-rendered 160x160 paper doll at region 22;
    /// 7. the big-panel description at region 23.
    pub(super) fn draw_new_hero(&mut self) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        // `FUN_0042EA6C`: tile the 100x100 stone panel (`NEWH#1`) over the whole
        // viewport, then draw the transparent `NEWH#0` artwork on top. The old
        // black fill showed through the ~38% transparent pixels.
        tile_image(&mut self.frame, fw, fh, &self.newh.images[NEWH_AUX_100]);
        draw_image(
            &mut self.frame,
            fw,
            fh,
            &self.newh.images[NEWH_BACKGROUND],
            0,
            0,
        );

        // Original round class-selection marker (NEWH image #2), centered inside
        // the selected class draw rectangle (region = class + 1).
        if let Some(class) = self.new_hero.selected_class {
            self.draw_new_hero_marker(class.region());
        }

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

        // Dynamic character values in the original small bitmap font.
        if let Some(build) = self.new_hero.build {
            self.draw_new_hero_values(build);
            self.draw_new_hero_skill_markers();
        }

        for bottom in 0..layout::BOTTOM_REGIONS.len() {
            let region = layout::BOTTOM_REGIONS[bottom];
            let img = self.new_hero.bottom_image(bottom);
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

        self.draw_new_hero_preview();
        self.draw_new_hero_description();
    }

    /// Draw the NEWH image `#2` marker centered in a 20x20 control box
    /// (`FUN_0042EA6C` uses the same marker for the class and skill boxes).
    fn draw_new_hero_marker(&mut self, region: usize) {
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        let r = self.res.newh_draw()[region];
        let marker = &self.newh.images[NEWH_CLASS_MARKER];
        let mx = r.x as i32 + (r.w as i32 - marker.width as i32) / 2;
        let my = r.y as i32 + (r.h as i32 - marker.height as i32) / 2;
        draw_image(&mut self.frame, fw, fh, marker, mx, my);
    }

    /// Skill selector markers. `FUN_0042EA6C` draws image `#2` in the combat box
    /// selected by `archery < melee` and in each civil box whose skill exceeds
    /// its threshold (`identification > 10`, `trade > 10`, `healing > 50`).
    fn draw_new_hero_skill_markers(&mut self) {
        if let Some(regions) = self.new_hero.skill_marker_regions() {
            for region in regions {
                self.draw_new_hero_marker(region);
            }
        }
    }

    /// Blit the pre-rendered 160x160 paper doll at the original region-22 origin.
    fn draw_new_hero_preview(&mut self) {
        let Some(hero) = self.new_hero.selected_hero else {
            return;
        };
        let slot = (self.new_hero.preview_weapon_slot as usize)
            .min(crate::frontend::previews::PREVIEW_SLOTS - 1);
        let index = hero * crate::frontend::previews::PREVIEW_SLOTS + slot;
        let Some(img) = self.hero_previews.get(index) else {
            return;
        };
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        let origin = self.res.newh_preview();
        blit(
            &mut self.frame,
            fw,
            fh,
            &img.data,
            img.w,
            img.h,
            origin.x as i32,
            origin.y as i32,
        );
    }

    /// Draw the big-panel description (`FUN_0042F578` + `FUN_0042BD8C`): greedy
    /// word wrap at the region-23 width, 15 px line advance, small font.
    fn draw_new_hero_description(&mut self) {
        let Some(region) = self.new_hero.description_region else {
            return;
        };
        let Some(text) = crate::frontend::description::description_for_region(region) else {
            return;
        };
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        let r = self.res.newh_description();
        let lines = self.wrap_description(text, r.w as i32);
        let mut y = r.y as i32;
        for line in lines {
            self.fonts.draw(
                &mut self.frame,
                fw,
                fh,
                OriginalFont::Small,
                &line,
                r.x as i32,
                y,
                None,
            );
            y += 15;
        }
    }

    /// `FUN_0042BD8C` word wrapping: tokens keep the delimiter (a trailing
    /// space); a token that would reach the width starts a new line.
    fn wrap_description(&self, text: &str, max_width: i32) -> Vec<String> {
        if max_width <= 0 {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let mut line = String::new();
        let mut line_width = 0;
        for token in text.split_inclusive(' ') {
            let w = self.fonts.measure(OriginalFont::Small, token);
            if !line.is_empty() && line_width + w >= max_width {
                lines.push(line.trim_end().to_string());
                line.clear();
                line_width = 0;
            }
            line.push_str(token);
            line_width += w;
        }
        if !line.trim().is_empty() {
            lines.push(line.trim_end().to_string());
        }
        lines
    }

    /// Draw the dynamic numeric values with the original small font at the
    /// recovered `NEWH_DRAW` value-box rects. Labels are baked into `NEWH#0`.
    ///
    /// Only regions 7..14 and 24 are numeric. Regions 15..19 are marker boxes
    /// (`draw_new_hero_skill_markers`), not skill value fields.
    pub(super) fn draw_new_hero_values(&mut self, build: crate::character::CharacterBuild) {
        use crate::character::{max_load, max_party, PrimaryStat};
        let fw = self.res.width() as usize;
        let fh = self.res.height() as usize;
        // (region, text): points #24, max load #11, max party #12, stats #7..10,
        // smithing #13, building #14.
        let mut entries: Vec<(usize, String)> = Vec::with_capacity(9);
        entries.push((layout::NEWH_POINTS_REGION, build.available_xp.to_string()));
        entries.push((11, format!("{:4.1}", max_load(build.endurance))));
        entries.push((12, max_party(build.class, build.charisma).to_string()));
        for (i, stat) in PrimaryStat::ORDER.iter().enumerate() {
            entries.push((7 + i, build.stat_value(*stat).to_string()));
        }
        entries.push((13, build.smithing.to_string()));
        entries.push((14, build.building.to_string()));

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
                        new_hero::RegionStatus::Skill => ([255, 120, 200, 255], "skill"),
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
