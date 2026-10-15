//! Unit tests for the [`crate::hud`] module.

use super::*;
use crate::res::Image;
use std::rc::Rc;

fn solid(w: usize, h: usize, c: [u8; 4]) -> Img {
    Rc::new(Image::new(w, h, c))
}

fn px(frame: &[u8], fw: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * fw + x) * 4;
    [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]
}

#[test]
fn slot_step_is_width_minus_one() {
    assert_eq!(slot_step(70), 69);
    assert_eq!(slot_step(52), 51);
    assert_eq!(slot_step(1), 0);
}

#[test]
fn layout_800_matches_executable_constants() {
    let l = layout_800();
    assert_eq!((l.logical_w, l.logical_h), (800, 600));
    assert_eq!((l.inventory_region.x, l.inventory_region.right()), (140, 800));
    assert_eq!(l.inventory_origin, (159, 486));
    assert_eq!(l.visible_slots, 9);
    assert_eq!(l.slot_step, 69);
    assert_eq!(l.arrows, [(18, 139, 486), (20, 780, 486)]);
}

#[test]
fn layout_640_matches_executable_constants() {
    let l = layout_640();
    assert_eq!((l.logical_w, l.logical_h), (640, 480));
    assert_eq!((l.inventory_region.x, l.inventory_region.right()), (112, 640));
    assert_eq!(l.inventory_origin, (134, 375));
    assert_eq!(l.visible_slots, 7);
    assert_eq!(l.arrows, [(18, 111, 375), (20, 620, 375)]);
}

#[test]
fn layout_selects_variant_by_logical_width() {
    assert_eq!(layout(800).logical_w, 800);
    assert_eq!(layout(640).logical_w, 640);
    assert_eq!(layout(400).logical_w, 640);
}

#[test]
fn inventory_slots_are_within_region_and_step_spaced() {
    for l in [layout_800(), layout_640()] {
        let rects = l.inventory_slot_rects(70);
        assert_eq!(rects.len(), l.visible_slots);
        let region = l.inventory_region;
        for (i, r) in rects.iter().enumerate() {
            assert_eq!(r.w, 70);
            assert_eq!(r.h, 70);
            assert_eq!(r.y, l.inventory_origin.1);
            if i > 0 {
                assert_eq!(r.x - rects[i - 1].x, slot_step(70));
            }
            assert!(
                r.x >= region.x && r.right() <= region.right(),
                "{:?} not within {:?}",
                r,
                region
            );
        }
    }
}

#[test]
fn party_grid_is_two_by_five() {
    for l in [layout_800(), layout_640()] {
        assert_eq!(l.party_cells.len(), 10);
        // Column 0 shared x, column 1 shared x; five distinct rows.
        let col0 = l.party_cells[0].x;
        let col1 = l.party_cells[1].x;
        assert!(col1 > col0);
        for row in 0..PARTY_ROWS {
            assert_eq!(l.party_cells[row * 2].x, col0);
            assert_eq!(l.party_cells[row * 2 + 1].x, col1);
            assert_eq!(l.party_cells[row * 2].y, l.party_cells[row * 2 + 1].y);
            if row > 0 {
                assert!(l.party_cells[row * 2].y > l.party_cells[(row - 1) * 2].y);
            }
        }
        // All cells inside the left panel.
        for c in l.party_cells {
            assert!(c.x >= 0 && c.right() <= l.inventory_region.x);
        }
    }
}

#[test]
fn portrait_id_reference_cases() {
    // appearance/status = 0x05, portrait_group = 1, health = 1600.
    assert_eq!(portrait_id(true, 1, 0x05, 1600), 240);
    assert_eq!(portrait_id(false, 1, 0x05, 1600), 245);
}

#[test]
fn portrait_id_health_thresholds() {
    // Main hero, group 0 => base 215.
    assert_eq!(portrait_id(true, 0, 0x00, 400), 215 + 3);
    assert_eq!(portrait_id(true, 0, 0x00, 800), 215 + 2);
    assert_eq!(portrait_id(true, 0, 0x00, 1200), 215 + 1);
    assert_eq!(portrait_id(true, 0, 0x00, 1201), 215 + 0);
    // Special condition flag 0x80 forces variant 4 (regardless of health).
    assert_eq!(portrait_id(true, 0, 0x80, 1600), 215 + 4);
    // Ordinary party uses appearance bits.
    assert_eq!(portrait_id(false, 0, 0x07, 1600), 195 + 35);
}

#[test]
fn portrait_ids_stay_in_reference_range() {
    // Three portrait groups (0..=2), five appearances (0..=4, the low 3 bits
    // of the status byte), and the five health/special variants span the
    // 195..=269 range; the selector never leaves the INTR 200..=274 block by
    // more than the reserved low entries.
    let healths = [0u16, 400, 800, 1200, 1600];
    for group in 0..=2u8 {
        for appearance in 0..=4u8 {
            for &health in &healths {
                let main = portrait_id(true, group, appearance, health);
                let party = portrait_id(false, group, appearance, health);
                assert!((195..=269).contains(&main), "main {main}");
                assert!((195..=269).contains(&party), "party {party}");
            }
            // Special condition forces variant 4.
            let sp = portrait_id(true, group, appearance | 0x80, 1600);
            assert!((195..=269).contains(&sp), "special {sp}");
        }
    }
}

#[test]
fn compose_preserves_world_through_transparent_pixels() {
    let fw = 8;
    let fh = 8;
    let mut frame = vec![0u8; fw * fh * 4];
    for p in frame.chunks_mut(4) {
        p.copy_from_slice(&[0x11, 0x22, 0x33, 0xFF]);
    }
    let l = HudLayout {
        logical_w: 8,
        logical_h: 8,
        hud_id: 3,
        slot_id: 17,
        inventory_region: Rect { x: 0, y: 0, w: 8, h: 8 },
        inventory_origin: (0, 0),
        visible_slots: 0,
        slot_step: 69,
        inventory_bar: Rect { x: 0, y: 0, w: 8, h: 8 },
        arrows: [(18, -1000, -1000), (20, -1000, -1000)],
        auto_pos: (-1000, -1000),
        party_cells: [Rect { x: 0, y: 0, w: 8, h: 8 }; 10],
    };
    // 2x2 HUD: top-left transparent, others opaque red.
    let mut hud = Image::new(2, 2, [255, 0, 0, 255]);
    hud.data[3] = 0; // transparent top-left
    let hud = Rc::new(hud);
    let state = HudState::default();
    compose(&mut frame, fw, fh, &l, &state, |_| Some(hud.clone()));
    assert_eq!(px(&frame, fw, 0, 0), [0x11, 0x22, 0x33, 0xFF]); // preserved
    assert_eq!(px(&frame, fw, 1, 0), [255, 0, 0, 255]); // overwritten
    assert_eq!(px(&frame, fw, 0, 1), [255, 0, 0, 255]);
}

#[test]
fn compose_clips_out_of_bounds_without_panic() {
    let fw = 4;
    let fh = 4;
    let mut frame = vec![0u8; fw * fh * 4];
    let l = HudLayout {
        logical_w: 800,
        logical_h: 600,
        hud_id: 3,
        slot_id: 17,
        inventory_region: Rect { x: 0, y: 0, w: 800, h: 70 },
        inventory_origin: (0, 0),
        visible_slots: 7,
        slot_step: 69,
        inventory_bar: Rect { x: 0, y: 0, w: 800, h: 45 },
        arrows: [(18, -1000, -1000), (20, -1000, -1000)],
        auto_pos: (-1000, -1000),
        party_cells: [Rect { x: 0, y: 0, w: 68, h: 68 }; 10],
    };
    let big = solid(70, 70, [1, 2, 3, 255]);
    let state = HudState::default();
    // Larger-than-frame sprites must be clipped, not panic.
    compose(&mut frame, fw, fh, &l, &state, |_| Some(big.clone()));
    assert_eq!(px(&frame, fw, 3, 3), [1, 2, 3, 255]);
}

#[test]
fn layout_has_inventory_bar_text_area() {
    let l800 = layout_800();
    assert_eq!(l800.inventory_bar, Rect { x: 140, y: 555, w: 660, h: 45 });
    let l640 = layout_640();
    assert_eq!(l640.inventory_bar, Rect { x: 112, y: 444, w: 528, h: 36 });
}

#[test]
fn auto_button_position_matches_traced_table() {
    assert_eq!(layout_800().auto_pos, (644, 379));
    assert_eq!(layout_640().auto_pos, (515, 303));
}

#[test]
fn inventory_slot_rect_bounds() {
    let l = layout_800();
    assert_eq!(l.inventory_slot_rect(0, 70).unwrap().x, 159);
    assert_eq!(l.inventory_slot_rect(1, 70).unwrap().x, 159 + 69);
    assert_eq!(l.inventory_slot_rect(8, 70).unwrap().x, 159 + 8 * 69);
    assert!(l.inventory_slot_rect(9, 70).is_none());
}

#[test]
fn arrow_caps_match_the_original_strip_renderer() {
    // Left cap at left-1; right cap at right - arrow_w(20) + 1; both at the
    // slot-row top. INTR[18]=left, INTR[20]=right at scroll 0.
    let l = layout_800();
    assert_eq!(l.arrows[0], (18, 139, 486));
    assert_eq!(l.arrows[1], (20, 780, 486));
    let l = layout_640();
    assert_eq!(l.arrows[0], (18, 111, 375));
    assert_eq!(l.arrows[1], (20, 620, 375));
}

#[test]
fn item_icons_are_centered_in_their_slots() {
    let fw = 800;
    let fh = 600;
    let mut frame = vec![0u8; fw * fh * 4];
    let l = layout_800();
    let state = HudState {
        party: Vec::new(),
        items: vec![
            ItemIcon { slot: 0, icon: 50 },
            ItemIcon { slot: 5, icon: 23 },
        ],
        hovered: None,
        auto: None,
    };
    let art = solid(40, 40, [10, 200, 10, 255]);
    compose(&mut frame, fw, fh, &l, &state, |id| {
        if id == 3 || id == 17 {
            None
        } else {
            Some(art.clone())
        }
    });
    for slot in [0usize, 5] {
        let cell = l.inventory_slot_rect(slot, 70).unwrap();
        let cx = (cell.x + (cell.w - 40) / 2) as usize;
        let cy = (cell.y + (cell.h - 40) / 2) as usize;
        assert_eq!(px(&frame, fw, cx, cy), [10, 200, 10, 255], "slot {slot}");
    }
    // An unfilled slot (e.g. slot 6) has no icon.
    let cell6 = l.inventory_slot_rect(6, 70).unwrap();
    let cx6 = (cell6.x + 35) as usize;
    let cy6 = (cell6.y + 35) as usize;
    assert_eq!(px(&frame, fw, cx6, cy6), [0, 0, 0, 0]);
}

#[test]
fn compose_is_idempotent() {
    let fw = 32;
    let fh = 32;
    let mut a = vec![0u8; fw * fh * 4];
    let mut b = vec![0u8; fw * fh * 4];
    for p in a.chunks_mut(4) {
        p.copy_from_slice(&[9, 9, 9, 255]);
    }
    b.copy_from_slice(&a);
    let l = layout_800();
    let state = HudState::default();
    let bg = solid(800, 600, [20, 40, 60, 255]);
    let slot = solid(70, 70, [80, 90, 100, 255]);
    let run = |frame: &mut Vec<u8>| {
        compose(frame, fw, fh, &l, &state, |id| {
            Some(if id == 3 { bg.clone() } else { slot.clone() })
        })
    };
    run(&mut a);
    run(&mut b);
    assert_eq!(a, b);
}

#[test]
fn portraits_center_in_cells() {
    let fw = 200;
    let fh = 40;
    let mut frame = vec![0u8; fw * fh * 4];
    let l = layout_640();
    let cell = l.party_cells[0];
    let state = HudState {
        party: vec![PartyPortrait { cell: 0, intr_id: 240 }],
        ..HudState::default()
    };
    // 20x20 portrait: centered offset within the cell.
    let art = solid(20, 20, [222, 0, 0, 255]);
    compose(&mut frame, fw, fh, &l, &state, |id| {
        if id == 240 {
            Some(art.clone())
        } else {
            None
        }
    });
    let ox = (cell.x + (cell.w - 20) / 2) as usize;
    let oy = (cell.y + (cell.h - 20) / 2) as usize;
    assert_eq!(px(&frame, fw, ox, oy), [222, 0, 0, 255]);
}
