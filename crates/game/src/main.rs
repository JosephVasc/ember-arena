//! Ember Arena: hold the ring against waves of rival fighters. Pick a class and an arena, move
//! with WASD; your abilities cast themselves, you level up and pick upgrades, choose a spec at
//! level 10, and a champion who can stun, fear or hex you arrives every couple of minutes.

mod arena;
mod art;
mod auto;
mod combat;
mod fx;
mod hero;
mod rivals;
mod run;
mod ui;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ember Arena".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.07, 0.05, 0.06)))
        .init_state::<Phase>()
        .add_plugins((
            art::plugin,
            arena::plugin,
            run::plugin,
            hero::plugin,
            rivals::plugin,
            combat::plugin,
            fx::plugin,
            ui::plugin,
            auto::plugin,
        ))
        .run();
}

/// The screen the game is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    /// Waiting on the character models and their animations.
    #[default]
    Loading,
    Menu,
    Playing,
    LevelUp,
    Paused,
    GameOver,
}

/// Everything spawned for a run, despawned when it ends.
#[derive(Component)]
pub struct RunEntity;

/// The gameplay systems' order inside a frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Input,
    Rivals,
    Combat,
    Vitals,
}

/// A floor point `[x, z]` as a world position on the arena floor.
pub fn at(p: rules::Pt) -> Vec3 {
    Vec3::new(p[0], 0.0, p[1])
}

/// A world position's floor point.
pub fn pt(v: Vec3) -> rules::Pt {
    [v.x, v.z]
}

/// The ground-plane part of a vector.
pub fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// The yaw that faces `dir` (the models face +Z).
pub fn yaw(dir: Vec3) -> Quat {
    Quat::from_rotation_y(dir.x.atan2(dir.z))
}

/// xorshift64*: the game's dice.
#[derive(Resource)]
pub struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }
}
