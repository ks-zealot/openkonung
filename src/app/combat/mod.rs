//! `combat` subsystem of [`crate::app`]: melee orchestration, HP/death and the
//! fixed-rate poison tick.
//!
//! Rules live in [`crate::combat`]; this module only maps the runtime player,
//! companions and world actors onto them. Ordering follows `FUN_0042260C`
//! (right-click attack order, battle stance required), `FUN_004123BC` case 5
//! (melee resolves on the last attack frame) and `FUN_0041D048` (periodic
//! damage every 16 logic frames).

mod accessors;
mod acquire;
mod ai;
mod approach;
mod death;
mod debug;
mod dot;
mod melee;
mod movement;
mod orders;
mod ranged;
mod schedule;
mod tick;
mod unit_ticks;
mod xp;

pub use schedule::AiSchedule;

/// `FUN_00410E84` / `FUN_0041ECC8` armed-archer gate.
///
/// The original tests `Entity+0x12 == 1`, the **bow** slot `equipment[1]`
/// (`+0x68`) and the ammo id (`+0xCC`); it never looks at the melee slot 0.
/// Returns the bow item id so callers can read its subtype for the sample count
/// (`u16@0x459B84 + subtype*0x40`).
pub(in crate::app) fn armed_bow(slot: u8, equipment: &[u16; 6], ammo: u16) -> Option<u16> {
    (slot == 1 && equipment[1] != 0 && ammo != 0).then_some(equipment[1])
}
