//! `human` subsystem of [`crate::res`] (split from `res.rs`).
//!
//! Static draw-order tables and GRAPH layout constants live in
//! [`crate::constants::human`]; this module re-exports them so existing
//! `crate::res::*` call sites stay unchanged.

pub use crate::constants::human::*;

mod compose;
mod fallback;
mod frames;
mod overlay;
mod paper_doll;

pub use self::frames::{human_frame_count, human_mapped_frame, render_graph_4d28_frame};
pub use self::overlay::{battle_overlay_bucket, overlay_plan};