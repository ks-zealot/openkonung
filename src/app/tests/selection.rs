//! Selection-ellipse tests (`FUN_0042532C` party rings).

use crate::app::world_draw::{selection_ring_position, selection_ring_sprite};

/// The original picks the ellipse by `Entity+0x5C` against `0x191` (401) and
/// `0x321` (801): red below 25 HP, yellow below 50, green otherwise (the
/// palette entries are `(238,41,24)`, `(246,230,41)` and `(0,255,0)`).
#[test]
fn selection_ring_sprite_matches_the_health_tiers() {
    assert_eq!(selection_ring_sprite(0), 172);
    assert_eq!(selection_ring_sprite(400), 172);
    assert_eq!(selection_ring_sprite(401), 126);
    assert_eq!(selection_ring_sprite(800), 126);
    assert_eq!(selection_ring_sprite(801), 178);
    assert_eq!(selection_ring_sprite(1600), 178);
}

/// Idle units place the ellipse at `(projection - 0x1D, -0x10)`, i.e. 56x32
/// centred one pixel left of the projected feet.
#[test]
fn selection_ring_position_is_centred_on_the_feet_when_idle() {
    let (x, y) = selection_ring_position(
        0,
        0,
        100.0,
        100.0,
        crate::anim::ACT_PEACE_IDLE as u8,
        7,
        3,
    );
    assert_eq!((x, y), (100 - 0x1D, 100 - 0x10));
}

/// Walk rows (`1` battle / `7` peace) apply the original per-facing frame bob
/// (`f / 2 + f * 2` lateral, `f / 2 + f` vertical). Frame 5 is the worked case.
#[test]
fn selection_ring_position_applies_the_walk_frame_bob() {
    // x = 100 - 29 = 71, y = 100 - 16 = 84; f = 5 -> side 12, diag 7.
    let walk = crate::anim::ACT_PEACE_WALK as u8;
    let cases = [
        (0u8, (71 - 25, 84)),
        (1, (71 - 12, 84 - 7)),
        (2, (71, 84 - 12)),
        (3, (71 + 12, 84 - 7)),
        (4, (71 + 25, 84)),
        (5, (71 + 12, 84 + 7)),
        (6, (71, 84 + 12)),
        (7, (71 - 12, 84 + 7)),
    ];
    for (facing, expected) in cases {
        assert_eq!(
            selection_ring_position(0, 0, 100.0, 100.0, walk, 5, facing),
            expected,
            "facing {facing}"
        );
    }
    // The battle walk row uses the same switch.
    assert_eq!(
        selection_ring_position(0, 0, 100.0, 100.0, crate::anim::ACT_BATTLE_WALK as u8, 5, 4),
        (71 + 25, 84)
    );
}

/// The camera pan shifts the ellipse with the world.
#[test]
fn selection_ring_position_includes_the_world_pan() {
    let (x, y) = selection_ring_position(
        140,
        0,
        100.0,
        100.0,
        crate::anim::ACT_PEACE_IDLE as u8,
        0,
        0,
    );
    assert_eq!((x, y), (140 + 71, 84));
}
