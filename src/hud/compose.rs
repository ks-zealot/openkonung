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
/// the hovered slot. The inventory itself lives on the protagonist; this is a
/// per-frame view of it and never re-parses or re-initializes anything.
#[derive(Clone, Debug, Default)]
pub struct HudState {
    pub party: Vec<PartyPortrait>,
    pub items: Vec<ItemIcon>,
    pub hovered: Option<usize>,
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
    hovered: Option<usize>,
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
        hovered,
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

    // Hovered item label, centered in the bottom bar (documented approximation:
    // the original hover-text routine/coordinates could not be recovered).
    let Some(slot) = hovered else { return };
    if slot >= l.visible_slots {
        return;
    }
    let idx = scroll + slot;
    let Some(&id) = inventory.get(idx) else {
        return;
    };
    if id == 0 {
        return;
    }
    let Some(info) = res.item_info(id) else {
        return;
    };
    let Some(font) = res.fonts.small.as_ref() else {
        return;
    };
    let label = if info.class == 12 {
        match res.item_quantity(id) {
            Some(q) => format!("{} x{}", info.name, q),
            None => info.name.clone(),
        }
    } else {
        info.name.clone()
    };
    let tw = font.measure(&label);
    let bar = l.inventory_bar;
    let name_y = bar.y + (bar.h - 24).max(0) / 2;
    let x = bar.x + (bar.w - tw).max(0) / 2;
    font.draw(frame, fw, fh, &label, x, name_y, Some([235, 235, 210]));
    // Second line: verified numeric summary (weight/price). The original only
    // drew the name; this is the same data-derived detail block used by the
    // character screen / trade tooltip.
    if let Some(detail) = crate::app::tooltip::item_detail_lines(res, id)
        .into_iter()
        .find(|line| line.contains("Вес:"))
    {
        let dw = font.measure(&detail);
        let dx = bar.x + (bar.w - dw).max(0) / 2;
        font.draw(
            frame,
            fw,
            fh,
            &detail,
            dx,
            name_y + 12,
            Some([200, 208, 188]),
        );
    }
}
