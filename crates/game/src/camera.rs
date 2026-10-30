//! The third-person camera: it hangs behind and above your character at the yaw and pitch the mouse
//! sets (see `input`), the wheel zooms, and it slowly circles the arena at the menu.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::{Phase, Selection, Step};

pub fn plugin(app: &mut App) {
    app.init_resource::<Cam>()
        .init_resource::<Focus>()
        .add_systems(Startup, spawn)
        .add_systems(Update, place.in_set(Step::Show));
}

#[derive(Component)]
pub struct MainCam;

/// Where the camera looks from: the direction it faces (yaw, radians, 0 along +Z), how far it tilts
/// down (pitch), and how far back it hangs.
#[derive(Resource)]
pub struct Cam {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    orbit: f32,
}

impl Default for Cam {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.42, dist: 13.0, orbit: 0.0 }
    }
}

pub const PITCH_MIN: f32 = -0.25;
pub const PITCH_MAX: f32 = 1.35;

impl Cam {
    /// The flat direction the camera faces.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
}

/// The point the camera follows: your character.
#[derive(Resource, Default)]
pub struct Focus(pub Vec3);

fn spawn(mut commands: Commands) {
    commands.spawn((
        MainCam,
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 24.0, 22.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn place(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    focus: Res<Focus>,
    sel: Res<Selection>,
    mut cam: ResMut<Cam>,
    mut wheel: MessageReader<MouseWheel>,
    mut cams: Query<&mut Transform, With<MainCam>>,
) {
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        cam.dist = (cam.dist * (1.0 - 0.1 * notches)).clamp(3.0, 40.0);
    }
    let Ok(mut tf) = cams.single_mut() else { return };
    if !phase.get().in_match() {
        cam.orbit += time.delta_secs() * 0.06;
        let d = sel.arena().radius * 1.1;
        let eye = Vec3::new(cam.orbit.sin() * d, d * 0.5, cam.orbit.cos() * d);
        *tf = Transform::from_translation(eye).looking_at(Vec3::Y, Vec3::Y);
        return;
    }
    let look = focus.0 + Vec3::Y * 1.6;
    let back = -cam.forward() * cam.pitch.cos() + Vec3::Y * cam.pitch.sin();
    let mut eye = look + back * cam.dist;
    // Never under the floor.
    eye.y = eye.y.max(0.4);
    *tf = Transform::from_translation(eye).looking_at(look, Vec3::Y);
}
