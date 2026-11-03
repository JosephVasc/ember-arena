//! The gates: each team waits behind an iron fence around its pen while the countdown runs, and
//! when it ends the fences sink into the floor.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::fight::Match;
use crate::{MatchEntity, Phase, Selection, at};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Phase::Gates), spawn).add_systems(Update, sink);
}

/// How tall the bars stand, and how fast they sink, yards a second.
const HEIGHT: f32 = 5.0;
const SINK: f32 = 2.5;
/// Yards between bars.
const GAP: f32 = 1.0;

#[derive(Component)]
struct Fence;

fn spawn(
    mut commands: Commands,
    sel: Res<Selection>,
    state: Res<Match>,
    fences: Query<(), With<Fence>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Back from a pause they're still standing.
    if state.opened || !fences.is_empty() {
        return;
    }
    let iron = mats.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.15, 0.16),
        metallic: 0.8,
        perceptual_roughness: 0.45,
        ..default()
    });
    let bar = meshes.add(Cuboid::new(0.16, HEIGHT, 0.16));
    let rail = meshes.add(Cuboid::new(GAP * 1.1, 0.16, 0.12));
    let arena = sel.arena();
    for team in 0..2 {
        let (center, r) = arena.pen(team);
        let n = (TAU * r / GAP).ceil() as usize;
        commands
            .spawn((MatchEntity, Fence, Transform::default(), Visibility::default()))
            .with_children(|fence| {
                for k in 0..n {
                    let a = k as f32 / n as f32 * TAU;
                    let p = at(center) + Vec3::new(a.cos(), 0.0, a.sin()) * r;
                    // Only the part of the circle inside the ring: the wall does the rest.
                    if p.length() > arena.radius {
                        continue;
                    }
                    fence.spawn((
                        Mesh3d(bar.clone()),
                        MeshMaterial3d(iron.clone()),
                        Transform::from_translation(p + Vec3::Y * HEIGHT / 2.0),
                    ));
                    for y in [1.4, HEIGHT - 0.4] {
                        fence.spawn((
                            Mesh3d(rail.clone()),
                            MeshMaterial3d(iron.clone()),
                            Transform::from_translation(p + Vec3::Y * y)
                                .with_rotation(Quat::from_rotation_y(-a - std::f32::consts::FRAC_PI_2)),
                        ));
                    }
                }
            });
    }
}

fn sink(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<Match>,
    mut fences: Query<(Entity, &mut Transform), With<Fence>>,
) {
    if !state.opened {
        return;
    }
    for (e, mut tf) in &mut fences {
        tf.translation.y -= SINK * time.delta_secs();
        if tf.translation.y < -HEIGHT {
            commands.entity(e).despawn();
        }
    }
}
