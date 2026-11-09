//! `compose` subsystem of [`crate::hud`] (split from `hud.rs`).

use super::*;

/// Which INTR portrait sprite to draw, ported from the original selector at
/// VA `0x42C730`.
///
/// The original reads:
/// - `character + 0x04` status byte: low 3 bits = appearance, bit `0x80` =
///   special condition;
/// - `character + 0x5C` health (u16);
/// - `character + 0xD6` portrait group.
pub fn portrait_id(is_main_hero: bool, portrait_group: u8, status: u8, health: u16) -> usize {
    let group = portrait_group as usize;
    let base = if is_main_hero {
        215 + group * 25
    } else {
        195 + group * 25 + (status & 0x07) as usize * 5
    };
    let variant = if status & 0x80 != 0 {
        4
    } else if health <= 400 {
        3
    } else if health <= 800 {
        2
    } else if health <= 1200 {
        1
    } else {
        0
    };
    base + variant
}

/// HP-indicator fill sprite, ported from `FUN_0042C730`'s bar branch:
/// poisoning (`Entity+0x50 != 0`) wins, else the health tier selects the fill
/// (`<401` deep-wounded, `<801` wounded, else healthy).
pub fn hp_bar_id(poisoned: bool, health: u16) -> usize {
    if poisoned {
        crate::intr::HP_BAR_POISONED
    } else if health <= 400 {
        crate::intr::HP_BAR_DEEP_WOUNDED
    } else if health <= 800 {
        crate::intr::HP_BAR_WOUNDED
    } else {
        crate::intr::HP_BAR_HEALTHY
    }
}

/// HP-indicator fill height in logical px: `max(1, health * bar_h / 1600)`
/// (`FUN_0042C730`: `health * H / 0x640`, clamped to at least 1).
pub fn hp_fill(health: u16, bar_h: i32) -> i32 {
    ((health as i32 * bar_h) / 1600).max(1)
}

/// One party portrait to overlay on the panel: the portrait sprite plus the
/// original selection marker (`FUN_00411708`) and HP indicator.
#[derive(Clone, Copy, Debug)]
pub struct PartyPortrait {
    pub cell: usize,
    pub intr_id: usize,
    /// Member is in the 10-slot selection set (`FUN_00411708`).
    pub selected: bool,
    /// `Entity+0x5C` health; `0` suppresses the HP bar (dead).
    pub health: u16,
    /// `Entity+0x50` periodic damage (poison) selects the pink fill.
    pub poisoned: bool,
}

/// One resolved visible inventory cell: `(visible slot index, INTR icon id)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemIcon {
    pub slot: usize,
    pub icon: usize,
    /// Class-12 stack quantity (shown as a small badge); `None` for non-stacks.
    pub count: Option<u32>,
}

/// Runtime HUD state: party portraits, the resolved visible inventory icons and
/// the lower-left controls. The inventory itself lives on the protagonist; this
/// is a per-frame view of it and never re-parses or re-initializes anything.
#[derive(Clone, Debug, Default)]
pub struct HudState {
    pub party: Vec<PartyPortrait>,
    pub items: Vec<ItemIcon>,
    /// AUTO button `(intr_id, x, y)` to overlay, if any.
    pub auto: Option<(usize, i32, i32)>,
    /// The original seven-icon lower-left strip (`0x81CF68[0..7]`), in draw
    /// order: weapon plaque then stance/map/halt/engage/quest/character.
    pub icons: Vec<(usize, i32, i32)>,
    /// Command code whose icon is drawn pressed (`|0x80`, 2 px higher).
    pub pressed: Option<u8>,
}

/// Composite the HUD onto `frame` using `lookup` to resolve INTR sprites.
///
/// `lookup` is called once per distinct sprite index; callers pass a cached
/// resolver such as [`Res::intr_sprite`] so no archive parsing or RGBA copy
/// happens per frame.
pub fn compose<F>(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    layout: &HudLayout,
    state: &HudState,
    mut lookup: F,
) where
    F: FnMut(usize) -> Option<Img>,
{
    // Whole-screen overlay (party panel + bottom bar) at (0,0).
    if let Some(bg) = lookup(layout.hud_id) {
        blit(frame, fw, fh, &bg.data, bg.w, bg.h, 0, 0);
    }

    // Lower-left button strip (`FUN_0042CA44`): weapon plaque then the six
    // interface buttons. The pressed icon is shifted up by 2 px (`|0x80`
    // branch in the original).
    let pressed_slot = state.pressed.and_then(slot_for_code);
    for (slot, &(id, x, y)) in state.icons.iter().enumerate() {
        let y = if pressed_slot == Some(slot) { y - 2 } else { y };
        if let Some(im) = lookup(id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
        }
    }

    // Scroll-arrow end caps (drawn by the original strip renderer even at
    // scroll 0), before the slot backgrounds.
    for &(id, x, y) in &layout.arrows {
        if let Some(im) = lookup(id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
        }
    }

    // Empty inventory slot backgrounds, repeated across the strip.
    if let Some(slot) = lookup(layout.slot_id) {
        let step = layout.slot_step;
        for i in 0..layout.visible_slots {
            let x = layout.inventory_origin.0 + i as i32 * step;
            blit(
                frame,
                fw,
                fh,
                &slot.data,
                slot.w,
                slot.h,
                x,
                layout.inventory_origin.1,
            );
        }
    }

    // Item icons, centered in their visible cells (empty cells keep the slot
    // background drawn above). `icon` is the original INTR index.
    for item in &state.items {
        let Some(cell) = inventory_slot_rect(layout, item.slot, 70) else {
            continue;
        };
        let Some(im) = lookup(item.icon) else {
            continue;
        };
        let ox = cell.x + (cell.w - im.w as i32) / 2;
        let oy = cell.y + (cell.h - im.h as i32) / 2;
        blit(frame, fw, fh, &im.data, im.w, im.h, ox, oy);
    }

    // AUTO combat button (traced gameplay-HUD element).
    if let Some((id, x, y)) = state.auto {
        if let Some(im) = lookup(id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
        }
    }

    // Party portraits, at the recovered per-slot origins (`0x45AF10`). Each is
    // followed by the selection marker and the HP indicator, exactly as
    // `FUN_0042C730` does for one member.
    for p in &state.party {
        let Some((ox, oy)) = layout.party_origin(p.cell) else {
            continue;
        };
        if let Some(im) = lookup(p.intr_id) {
            blit(frame, fw, fh, &im.data, im.w, im.h, ox, oy);
        }
        // Selection marker (`INTR[173]`) drawn at the same origin.
        if p.selected {
            if let Some(im) = lookup(crate::intr::SELECTION_MARKER) {
                blit(frame, fw, fh, &im.data, im.w, im.h, ox, oy);
            }
        }
        // HP indicator: bottom `fill` rows of the tier/poison fill sprite.
        if p.health != 0 {
            if let Some(im) = lookup(hp_bar_id(p.poisoned, p.health)) {
                let bx = ox + layout.hp_bar.dx;
                let by = oy + layout.hp_bar.dy;
                let fill = hp_fill(p.health, layout.hp_bar.height).min(im.h as i32);
                let skip = (im.h as i32 - fill).max(0) as usize;
                crate::res::blit_rows(frame, fw, fh, &im.data, im.w, im.h, skip, bx, by);
            }
        }
    }
}

/// Draw the HUD for the active logical framebuffer over the game world.
///
/// Resolves sprites through [`Res::intr_sprite`] (lazy `Rc` cache), so repeated
/// calls neither re-parse the archive nor re-allocate sprite buffers.
pub fn draw_game_hud(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    inventory: &[u16; 32],
    scroll: usize,
    held: Option<u16>,
    cursor: (i32, i32),
    party: &[PartyPortrait],
    auto_on: bool,
    weapon_slot: u8,
    battle: bool,
    pressed: Option<u8>,
) {
    if res.intr.is_none() {
        return;
    }
    let l = layout(fw as i32);

    // Resolve the visible inventory icons (fixed scroll for this task). The
    // protagonist's array is read directly; nothing is re-initialized here.
    let mut items = Vec::new();
    for slot in 0..l.visible_slots {
        let idx = scroll + slot;
        let Some(&id) = inventory.get(idx) else {
            continue;
        };
        if id == 0 || Some(id) == held {
            continue;
        }
        if let Some(info) = res.item_info(id) {
            let count = if info.class == 12 {
                res.item_quantity(id)
            } else {
                None
            };
            items.push(ItemIcon {
                slot,
                icon: info.icon as usize,
                count,
            });
        }
    }

    let party = party.to_vec();
    let auto_id = if auto_on {
        crate::intr::AUTO_ON
    } else {
        crate::intr::AUTO_OFF
    };
    let state = HudState {
        party,
        items: items.clone(),
        auto: Some((auto_id, l.auto_pos.0, l.auto_pos.1)),
        icons: icon_plan(l.logical_w, weapon_slot, battle).to_vec(),
        pressed,
    };
    compose(frame, fw, fh, &l, &state, |id| res.intr_sprite(id));

    // Class-12 stack count badges.
    if let Some(font) = res.fonts.small.as_ref() {
        for item in &items {
            let Some(count) = item.count else { continue };
            if count <= 1 {
                continue;
            }
            let Some(cell) = inventory_slot_rect(&l, item.slot, 70) else {
                continue;
            };
            let s = count.to_string();
            let w = font.measure(&s);
            let x = cell.x + cell.w - w - 3;
            let y = cell.y + cell.h - 13;
            let _ = font.draw(frame, fw, fh, &s, x, y, Some([235, 235, 210]));
        }
    }

    // Held item follows the cursor (the original strip hides it while lifted).
    if let Some(id) = held {
        if let Some(info) = res.item_info(id) {
            if let Some(im) = res.intr_sprite(info.icon as usize) {
                let x = cursor.0 - im.w as i32 / 2;
                let y = cursor.1 - im.h as i32 / 2;
                crate::res::blit(frame, fw, fh, &im.data, im.w, im.h, x, y);
            }
        }
    }
}

/// Draw the original lower-panel hover text.
///
/// `FUN_0042BD8C` paints the value returned by `FUN_00430958` / `FUN_0042F5F8`
/// / `FUN_0042E9CC` with the small font `0x58BC34`, left aligned at
/// `(_DAT_0081D738 + 10, _DAT_0081D760 + 5)`, wrapped to
/// `(right - left) - 15`, line advance 15 px. At 800x600 that is (150, 560),
/// 644 px wide; at 640x480 (122, 449), 512 px wide. The routine first re-blits
/// the HUD background clipped to that region, which the per-frame HUD draw here
/// already provides.
pub fn draw_hover_text(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    logical_w: i32,
    res: &crate::res::Res,
    text: &str,
) {
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let l = layout(logical_w);
    let max_w = (l.inventory_bar.w - 16).max(1);
    let x = l.inventory_bar.x + 10;
    let mut y = l.inventory_bar.y + 5;
    for line in wrap_by_measure(text, max_w, &|s| font.measure(s)) {
        if y >= l.logical_h {
            break;
        }
        font.draw(frame, fw, fh, &line, x, y, Some([235, 235, 210]));
        y += 15;
    }
}

/// Greedy word wrap with a hard split for words wider than `max_w`.
///
/// Extracted so the panel-wrap policy can be tested without a font table.
pub(super) fn wrap_by_measure<F>(text: &str, max_w: i32, measure: &F) -> Vec<String>
where
    F: Fn(&str) -> i32,
{
    let max_w = max_w.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }
        let trial = if line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", line, word)
        };
        if measure(&trial) <= max_w {
            line = trial;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if measure(word) <= max_w {
            line.push_str(word);
            continue;
        }
        let mut chunk = String::new();
        for ch in word.chars() {
            let mut candidate = chunk.clone();
            candidate.push(ch);
            if measure(&candidate) > max_w && !chunk.is_empty() {
                lines.push(std::mem::take(&mut chunk));
            }
            chunk.push(ch);
        }
        line = chunk;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
