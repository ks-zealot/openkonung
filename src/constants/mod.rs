//! Central constant tables and magic values.
//!
//! Each submodule collects one subsystem's recovered numbers: combat staging
//! and settlement-prop values ([`combat`]), runtime/projection geometry
//! ([`coords`]), diagnostic preview sizes ([`debug`]), DGROUP table anchors
//! ([`exe_tables`]), `Game.N` and `Konung.N` layouts ([`game_layout`],
//! [`konung_layout`]), human GRAPH draw orders ([`human`]), virtual-key codes
//! ([`keys`]), movement/AI timings ([`motion`], [`timing`]) and the virtual
//! framebuffer size ([`render`]).

pub mod combat;
pub mod coords;
pub mod debug;
pub mod exe_tables;
pub mod game_layout;
pub mod human;
pub mod keys;
pub mod konung_layout;
pub mod motion;
pub mod render;
pub mod timing;
