//! Item hover tooltip used by the character screen and exchange window.
//!
//! The original `FUN_0042F5F8` hover path draws only the item **name**; the
//! shipped data has no flavour-text table. This module keeps the recovered name
//! and appends a compact, data-derived property block (weight, price,
//! durability, combat value, range and requirements) so every item surface has
//! the same readable detail. The extra block is a documented remake
//! enhancement, never invented content.

use crate::res::{fill_rect, Res};

impl super::PlayableApp {
    /// Item instance under the character-screen cursor, if any.
    pub(super) fn character_hover_item(&self, cursor: (i32, i32)) -> Option<u16> {
        let owner = self.inventory_owner;
        let equipment = self.owner_equipment(owner);
        let raw = self.owner_raw_0x12(owner);
        let slots = self.slots(owner)?;
        let hit = crate::character_screen::hit_test(
            cursor.0,
            cursor.1,
            super::VIRTUAL_W,
            &equipment,
            raw,
        );
        use crate::character_screen::ScreenHit;
        let id = match hit {
            ScreenHit::Main(s) => equipment.get(s).copied().unwrap_or(0),
            ScreenHit::Additional(s) => slots.additional.get(s).copied().unwrap_or(0),
            ScreenHit::Special => slots.special,
            ScreenHit::Inventory(s) => {
                let idx = self.inventory_scroll + s;
                slots.inventory.get(idx).copied().unwrap_or(0)
            }
            _ => 0,
        };
        (id != 0).then_some(id)
    }

    /// Diagnostic: item under the character-screen cursor (0 when none).
    pub fn debug_hover_item(&self) -> u16 {
        let cursor = (
            self.cursor_virtual.0.round() as i32,
            self.cursor_virtual.1.round() as i32,
        );
        self.character_hover_item(cursor).unwrap_or(0)
    }
}

/// Property lines for item instance `id` (excluding the name line).
pub fn item_detail_lines(res: &Res, id: u16) -> Vec<String> {
    let Some(info) = res.item_info(id) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let class = res.item_class(id).unwrap_or(info.class);

    let weight = crate::inventory::item_weight(res, id);
    let price = res.item_trade_value(id).unwrap_or(0);
    if class == 12 {
        if let Some(q) = res.item_quantity(id) {
            lines.push(format!("Количество: {}", q));
        }
    }
    lines.push(format!(
        "Вес: {:.2}   Цена: {}",
        weight as f32 / 1000.0,
        price
    ));

    if class != 12 {
        if let Some((cur, max)) = res.item_durability(id) {
            if max > 0.0 && cur.is_finite() && max.is_finite() {
                lines.push(format!("Прочность: {:.1}/{:.1}", cur, max));
            }
        }
    }

    if matches!(class, 0..=5 | 12) && info.rating > 0 {
        lines.push(format!("Боевое значение: {}", info.rating));
    }
    if info.range > 1 {
        lines.push(format!("Дистанция: {}", info.range));
    }
    if let Some(label) = requirement_stat_label(info.requirement_stat) {
        if info.requirement_value > 0 {
            lines.push(format!("Требуется: {} {}", label, info.requirement_value));
        }
    }
    lines
}

/// CP866 name of one item requirement selector (1..=4: charisma, strength,
/// dexterity, endurance).
///
/// `0` and unknown selectors have no label, so the requirement line is skipped.
fn requirement_stat_label(stat: u16) -> Option<&'static str> {
    match stat {
        1 => Some("Харизма"),
        2 => Some("Сила"),
        3 => Some("Ловкость"),
        4 => Some("Выносливость"),
        _ => None,
    }
}

/// Draw the tooltip for `id` near `cursor`, clamped to `clip` (x, y, w, h).
pub fn draw_item_tooltip(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &Res,
    id: u16,
    cursor: (i32, i32),
    clip: (i32, i32, i32, i32),
) {
    if id == 0 {
        return;
    }
    let Some(info) = res.item_info(id) else {
        return;
    };
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let mut title = info.name.clone();
    if let (Some(db), Some(inst)) = (res.item_db.as_ref(), res.item_instance(id)) {
        if let Some(suffix) = db.modifier_name(inst.modifier_index) {
            title.push_str(&suffix);
        }
    }
    let details = item_detail_lines(res, id);
    let line_h = 13;
    let pad = 6;
    let title_w = font.measure(&title);
    let body_w = details.iter().map(|l| font.measure(l)).max().unwrap_or(0);
    let w = title_w.max(body_w) + pad * 2;
    let h = line_h * (1 + details.len() as i32) + pad * 2;
    let (cx, cy, cw, ch) = clip;
    let mut x = cursor.0 + 18;
    let mut y = cursor.1 + 18;
    if x + w > cx + cw {
        x = cursor.0 - w - 8;
    }
    if y + h > cy + ch {
        y = cursor.1 - h - 8;
    }
    x = x.clamp(cx, (cx + cw - w).max(cx));
    y = y.clamp(cy, (cy + ch - h).max(cy));
    let _ = fh;
    fill_rect(frame, fw, fh, x, y, w, h, [12, 16, 14, 255]);
    fill_rect(frame, fw, fh, x, y, w, 1, [96, 112, 96, 255]);
    fill_rect(frame, fw, fh, x, y + h - 1, w, 1, [96, 112, 96, 255]);
    fill_rect(frame, fw, fh, x, y, 1, h, [96, 112, 96, 255]);
    fill_rect(frame, fw, fh, x + w - 1, y, 1, h, [96, 112, 96, 255]);
    font.draw(
        frame,
        fw,
        fh,
        &title,
        x + pad,
        y + pad,
        Some([255, 235, 120]),
    );
    for (i, line) in details.iter().enumerate() {
        font.draw(
            frame,
            fw,
            fh,
            line,
            x + pad,
            y + pad + line_h * (i as i32 + 1),
            Some([220, 224, 208]),
        );
    }
}
