//! Ember Arena's rules, free of any engine: the classes and their abilities, casting and what
//! lands, crowd control with diminishing returns, the arenas, and how a computer fighter plays.
//! The game crate draws and drives these; a multiplayer server can run the same code.

pub mod ai;
pub mod arena;
pub mod control;
pub mod spells;
pub mod unit;

/// A point on the arena floor, in yards: `[x, z]`.
pub type Pt = [f32; 2];
