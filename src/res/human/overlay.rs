//! Stance-specific equipment overlay ordering (`FUN_00427e9c`).

use super::super::*;

/// `FUN_00427e9c` action bucket: walk (`1`) → 3, engage (`5`) → 2, use (`4`)
/// → 1, everything else (`0`/`2`/`3`) → 0.
pub fn battle_overlay_bucket(action: usize) -> usize {
    match action {
        1 => 3,
        5 => 2,
        4 => 1,
        _ => 0,
    }
}

/// Overlay order and the slots whose `equipped_visual` selector is advanced by
/// one (the "in hand"/alternate GRAPH block) for the current stance.
///
/// Mirrors `FUN_00427e9c`:
/// - peace: fixed direction order, no variants;
/// - battle, `Entity+0x12 == 1`, bucket != 2: order B, slot 1 variant;
/// - else: order A, active slot (unless it is 1) and slot 5 variants.
pub fn overlay_plan(
    battle: bool,
    weapon_slot: u8,
    action: usize,
    direction: usize,
) -> (&'static [usize; 6], [bool; 6]) {
    let dir = direction & 7;
    if !battle {
        return (&HUMAN_DRAW_ORDER_PEACE[dir], [false; 6]);
    }
    let bucket = battle_overlay_bucket(action);
    let mut variant = [false; 6];
    if weapon_slot == 1 && bucket != 2 {
        variant[1] = true;
        (&HUMAN_DRAW_ORDER_BATTLE_B[bucket][dir], variant)
    } else {
        if weapon_slot != 1 {
            if let Some(v) = variant.get_mut(weapon_slot as usize) {
                *v = true;
            }
        }
        variant[5] = true;
        (&HUMAN_DRAW_ORDER_BATTLE_A[bucket][dir], variant)
    }
}