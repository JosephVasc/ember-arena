//! Ember Arena: team arena battles. Pick a class and a bracket (1v1, 2v2, 3v3); your team and the
//! other start at opposite gates; target with Tab or a click, cast your ten abilities from the bar,
//! play the pillars for line of sight, and be the last team standing.

mod alerts;
mod arena;
mod art;
mod auto;
mod camera;
mod fight;
mod fx;
mod icons;
mod input;
mod minimap;
mod settings;
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
        .init_resource::<Selection>()
        .init_resource::<Rng>()
        .configure_sets(Update, (Step::Input, Step::Think, Step::Resolve, Step::Show).chain())
        .add_plugins((
            art::plugin,
            arena::plugin,
            camera::plugin,
            fight::plugin,
            input::plugin,
            fx::plugin,
            icons::plugin,
            alerts::plugin,
            ui::plugin,
            auto::plugin,
            settings::plugin,
            minimap::plugin,
        ))
        .add_systems(OnEnter(Phase::Paused), |mut t: ResMut<Time<Virtual>>| t.pause())
        .add_systems(OnExit(Phase::Paused), |mut t: ResMut<Time<Virtual>>| t.unpause())
        .run();
}

/// The screen the game is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    /// Waiting on the character models and their animations.
    #[default]
    Loading,
    Menu,
    /// At the gates: free to move, not to cast, while the countdown runs.
    Gates,
    Fight,
    Paused,
    Results,
}

impl Phase {
    /// A match is on: the fighters are in the arena.
    pub fn in_match(self) -> bool {
        matches!(self, Phase::Gates | Phase::Fight | Phase::Paused | Phase::Results)
    }
}

/// The frame's order: input, the AI's thinking, resolving casts and hits, then showing it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Input,
    Think,
    Resolve,
    Show,
}

/// What the menu picked.
#[derive(Resource)]
pub struct Selection {
    pub class: usize,
    /// Fighters a side: 1, 2 or 3.
    pub bracket: usize,
    pub arena: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Self { class: 2, bracket: 2, arena: 0 }
    }
}

impl Selection {
    pub fn class(&self) -> &'static rules::spells::Class {
        &rules::spells::CLASSES[self.class]
    }
    pub fn arena(&self) -> &'static rules::arena::ArenaDef {
        &rules::arena::ARENAS[self.arena]
    }
}

/// Everything spawned for a match, despawned when it ends.
#[derive(Component)]
pub struct MatchEntity;

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
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }
}
