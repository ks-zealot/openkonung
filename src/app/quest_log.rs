//! Quest-log window (interface mode 2).
//!
//! `FUN_004242BC` toggles the mode, `FUN_0042CBD0(1)` draws it: `INTR[5]` stone
//! fill, `INTR[1]` border at the recovered per-resolution position, and one
//! line per active `B7` slot whose `+0x02` journal word is used as a `B6` text
//! line. The original text engine advances 15 px per line
//! (`FUN_0042BD8C`).

use super::*;

impl PlayableApp {
    pub fn toggle_quest_log(&mut self) {
        if self.dialog.is_some() || self.trade.is_some() || self.character_screen {
            return;
        }
        if self.quest_log_open {
            self.quest_log_open = false;
            self.queue_sound(4);
        } else {
            self.map_open = false;
            self.map_overlay = None;
            self.quest_log_open = true;
            self.hovered_slot = None;
            self.queue_sound(4);
        }
    }

    pub fn close_quest_log(&mut self) {
        self.quest_log_open = false;
        self.hovered_slot = None;
    }

    /// `FUN_0042CBD0` case 2 layout: border at `0x4632C4`, prompt column at
    /// `0x4632E4`, line start `0x4632E8`, width `0x463304`.
    pub(super) fn draw_quest_log_screen(&mut self) {
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        let l = crate::hud::layout(VIRTUAL_W);
        let vp = l.viewport;
        let wide = VIRTUAL_W > 640;
        let border = if wide { (144, 0) } else { (114, 0) };
        let text_x = if wide { 166 } else { 133 };
        let mut text_y = if wide { 25 } else { 20 };

        let PlayableApp {
            frame,
            world,
            quest_flags,
            ..
        } = self;

        crate::world_map::draw_stone(frame, fw, fh, &mut world.res, vp);
        if let Some(b) = world.res.intr_sprite(1) {
            blit(
                frame,
                fw,
                fh,
                &b.data,
                b.w,
                b.h,
                border.0,
                border.1,
            );
        }

        let Some(quest) = world.res.quest.as_ref() else {
            return;
        };
        let Some(font) = world.res.fonts.small.as_ref() else {
            return;
        };
        let bottom = vp.bottom();
        for i in 0..quest_flags.len().min(300) {
            let word = quest_flags[i];
            if word & 0x80 == 0 {
                continue;
            }
            // Runtime `B7 +0x02` (`0x61506A + i*4`), not the initial file word.
            let line = (word >> 16) as u16;
            if line == 0xFFFF {
                continue;
            }
            let text = quest.line_text(line);
            if text.is_empty() {
                continue;
            }
            let max_w = if wide { 606 } else { 485 };
            for part in wrap_text(font, &text, max_w) {
                if text_y + 15 > bottom {
                    break;
                }
                let _ = font.draw(frame, fw, fh, &part, text_x, text_y, Some([235, 235, 210]));
                text_y += 15;
            }
            if text_y + 15 > bottom {
                break;
            }
        }
    }
}

/// Word-wrap using the small-font metrics (`FUN_0042BD8C`, max width from
/// `0x463304`). A single overlong word is hard-split.
fn wrap_text(font: &crate::font::FontTable, text: &str, max_w: i32) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if font.measure(&candidate) <= max_w {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            out.push(std::mem::take(&mut line));
        }
        if font.measure(word) <= max_w {
            line = word.to_string();
            continue;
        }
        let mut chunk = String::new();
        for ch in word.chars() {
            let next = format!("{chunk}{ch}");
            if !chunk.is_empty() && font.measure(&next) > max_w {
                out.push(std::mem::take(&mut chunk));
            }
            chunk.push(ch);
        }
        line = chunk;
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

