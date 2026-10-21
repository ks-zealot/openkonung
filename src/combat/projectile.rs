//! Original projectile/effect pool entry (`0x80B0D0`, 100 x 0x20) and the
//! ranged spawn geometry recovered from `FUN_0041BF44` / `FUN_00420F30`.

use super::{BowShot, CombatTarget};

pub const POOL_CAP: usize = 100;
/// `FUN_00420F30` moves a projectile eight sub-steps per logic frame.
pub const SUBSTEPS: usize = 8;
/// `0x45013c` = 0.25, `0x450144` = 0.001.
pub const SPEED_SCALE: f32 = 0.25;
pub const SPEED_EPSILON: f32 = 0.001;
/// `FUN_0041BF44` aims 30 px above the target's projected cell.
pub const AIM_Y_OFFSET: f32 = 30.0;
/// INTR sprite base for `+0x1C = facing*2 + 0xB8`.
pub const SPRITE_BASE: usize = 0xB8;

/// `0x45ACD0 + facing*2` (signed byte): projectile start x offset.
pub const MUZZLE_X: [i8; 8] = [-70, -48, 12, 68, 80, 52, -8, -60];
/// `0x45ACD1 + facing*2` (signed byte): projectile start y offset.
pub const MUZZLE_Y: [i8; 8] = [-65, -99, -85, -90, -58, -26, -10, -31];
/// `0x4580A0 + facing*2` (signed byte): draw x offset (`entry+0x10`).
pub const DRAW_DX: [i8; 8] = [0, 0, 0, 0, -3, -29, -29, -23];
/// `0x4580A1 + facing*2` (signed byte): draw y offset (`entry+0x1A`).
pub const DRAW_DY: [i16; 8] = [-3, 0, 0, -3, 0, 0, -17, 0];

/// One live ranged projectile.
#[derive(Clone, Debug)]
pub struct Projectile {
    /// Local-world screen pixels (camera added at draw time).
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Frames remaining (`entry+0x1D`).
    pub life: u8,
    pub owner: CombatTarget,
    /// Negative prop target index (`Entity+0x06 == -(index+1)`).
    pub prop_target: Option<usize>,
    pub bow: BowShot,
    /// INTR sprite index base (`184 + facing*2`).
    pub sprite_base: usize,
    pub draw_dx: i8,
    pub draw_dy: i16,
}

impl Projectile {
    pub fn sprite_index(&self, world_frame: u64) -> usize {
        self.sprite_base + (world_frame as usize & 1)
    }
}

pub fn muzzle_offset(facing: u8) -> (f32, f32) {
    let f = (facing & 7) as usize;
    (MUZZLE_X[f] as f32, MUZZLE_Y[f] as f32)
}

/// `FUN_0041BF44` velocity normalization.
pub fn aim_velocity(from: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let fx = dx.abs() * SPEED_SCALE + SPEED_EPSILON;
    let fy = dy.abs() * SPEED_SCALE + SPEED_EPSILON;
    if fx < fy {
        (dx / fy, dy / fy)
    } else {
        (dx / fx, dy / fx)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    owner: CombatTarget,
    from: (f32, f32),
    to: (f32, f32),
    facing: u8,
    bow: BowShot,
    life: u8,
    prop_target: Option<usize>,
) -> Projectile {
    let (vx, vy) = aim_velocity(from, to);
    let f = (facing & 7) as usize;
    Projectile {
        x: from.0,
        y: from.1,
        vx,
        vy,
        life: life.max(1),
        owner,
        prop_target,
        bow,
        sprite_base: SPRITE_BASE + f * 2,
        draw_dx: DRAW_DX[f],
        draw_dy: DRAW_DY[f],
    }
}
