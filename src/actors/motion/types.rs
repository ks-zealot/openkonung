//! Runtime actor motion state types.
//!
//! [`ActorMotion`] is the port of the original per-entity movement fields
//! (`Entity+0x08/+0x0A` cell, `+0x01` facing, `+0x10` frame, `+0x13` direction
//! queue, `+0xE8` draw cell). It is the only place runtime position lives;
//! [`RuntimeActor::entity`] keeps the authored `Game.N` record untouched.
//!
//! The one non-obvious invariant: the committed cell **leads** the drawn sprite
//! by one step. `cell` is advanced at the START of a walk segment
//! (`FUN_0041435C`) while `render_cell` keeps the cell the segment started from.

use super::super::*;

/// Live movement state for one runtime actor (original Entity grid).
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ActorMotion {
    /// Current **committed** runtime/Entity cell (`Entity+0x08/+0x0A`). Advanced
    /// at the START of each walk segment (`FUN_0041435C`), so while walking it is
    /// the cell being stepped INTO.
    pub cell: (i32, i32),
    /// Draw-only cell: the pre-step (source) cell of the animating segment
    /// (`Entity+0xE8`). The sprite is drawn here; the anchor sweeps it onto
    /// `cell` over the cycle. Equals `cell` when idle.
    pub render_cell: (i32, i32),
    /// True while a walk segment is animating (the final segment's direction is
    /// consumed at segment START, so `path` is already empty yet the sprite is
    /// still walking).
    pub walking: bool,
    /// Destination cell, when walking.
    pub target: Option<(i32, i32)>,
    /// Remaining movement-direction codes (0..=7) to the target
    /// (`FUN_0043DA1F` queue; each entry is consumed by one walk cycle).
    pub path: Vec<u8>,
    /// Current facing (0..7).
    pub facing: u8,
    /// Walk-cycle length in frames for this actor (`Res::walk_cycle_frames`,
    /// equals the original `Entity+0xD7`). `0` = unresolved -> [`FALLBACK_WALK_FRAMES`].
    pub walk_frames: u8,
    /// Idle-action cycle length in frames for the current stance
    /// (`Res::action_cycle_frames` at [`crate::anim::idle_action`]). Retained for
    /// the eventual `flags_a & 0x40 == 0` idle loop; the shipped active
    /// population holds the idle frame instead (see [`update_population`]).
    pub idle_frames: u8,
    /// Walk-animation frame counter; steps a cell every `walk_frames` ticks.
    pub frame: u8,
    /// Current original action row (`Entity+0x00`): idle/walk/attack/death for
    /// the current stance ([`crate::anim`]).
    pub action: u8,
    /// Idle cooldown in logic ticks before the next think.
    pub wait: u16,
    /// Enumerating group's spawn rectangle A (wander bounds).
    pub rect_a: Rect,
    /// Whether this actor belongs to a **citizen** group. The original
    /// per-frame dispatcher `FUN_00421B50` selects the AI by group class
    /// (`Group+0x1E`): `class & 0x80 != 0` (and `& 0x40 == 0`) uses the
    /// waypoint/wander AI `FUN_004168A4`, while `class & 0x40` (object/enemy
    /// groups) uses `FUN_00412E00`, which **idles in place** while the group
    /// activity byte `Group+0x1D == 0`. Non-citizens therefore never wander.
    pub citizen: bool,
    /// Current melee target. `None` for ordinary citizens/wanderers.
    pub combat_target: Option<crate::combat::CombatTarget>,
    /// Persistent engage order (`Entity+0x02 & 0x01`, the `'A'` order written by
    /// `FUN_00421458`/`FUN_0042260C`). While set, a unit that loses its target
    /// re-acquires the next hostile of the nearest enemy group around the leader
    /// (`FUN_0041E2D4`/`FUN_0041FB64`/`FUN_00420190`); the order clears when no
    /// hostile remains, on halt (`0x60`), on peace (`& 0xFA`) or on death.
    ///
    /// Runtime-only: the original persists it in `Entity+0x02`, but the remake
    /// save payload has no order byte, so `#[serde(skip)]` keeps the bincode
    /// layout unchanged and load derives it from a live `combat_target`.
    #[serde(skip)]
    pub engaged: bool,
    /// Cycle length for the active attack: melee row `5`
    /// (`FUN_0041EB48`/`FUN_004123BC` case 5) or ranged row `4`.
    pub attack_frames: u8,
    /// Ranged order (action row 4) for bow-armed actors.
    pub ranged_order: bool,
    /// The bow has already released its arrow for the current attack cycle, so
    /// the spawn frame cannot fire twice.
    pub ranged_fired: bool,
    /// Death-cycle length (`Res::action_cycle_frames` for the death row).
    pub death_frames: u8,
    /// Death state: the death row has been entered and the final frame is held.
    pub dead: bool,
}

/// How a [`RuntimeActor`] gets its draw position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActorPlacement {
    /// Persisted Entity x/y (+0x08/+0x0A) are the draw coordinates.
    Persisted,
    /// Runtime-assigned position; the original Entity x/y remain `(0,0)`.
    Runtime {
        /// Runtime grid column (`0..=319`).
        x: i32,
        /// Runtime grid row (`0..=159`).
        y: i32,
        /// Placement attempt that produced this cell (1-based, max 100).
        attempts: u8,
        /// The group spawn rectangle A the point was drawn from.
        spawn_rect: Rect,
    },
}

/// A renderable non-player entity.
///
/// Holds the **one canonical parsed [`Entity`]** (graphics + authored gameplay
/// state: stats, equipment, inventory, ids) plus the enumerating group id and
/// the runtime placement. Runtime placement only adds a position; it never
/// writes back into `entity`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeActor {
    /// Canonical parsed Entity — the source of truth for all authored state.
    pub entity: Entity,
    /// The group index that enumerated this entity.
    pub source_group_id: usize,
    /// Where the draw position comes from: authored x/y or a runtime spawn.
    pub placement: ActorPlacement,
}
