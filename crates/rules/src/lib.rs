//! Ember Arena's rules, free of any engine: what each class and ability does, how crowd control
//! diminishes, and how a run's waves grow. The game crate draws and drives these; a future
//! multiplayer server can run the same code authoritatively.

pub mod control;
pub mod kit;
pub mod text;
pub mod waves;

/// A point on the arena floor, in yards: `[x, z]`.
pub type Pt = [f32; 2];
