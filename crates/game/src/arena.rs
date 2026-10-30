//! The ring itself, built from the KayKit dungeon pieces around an [`ArenaDef`]: a stone floor,
//! a curved wall hung with the two teams' banners and lit by torches, pillars to fight around,
//! and rubble beyond. Plus the light and the camera.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use rules::waves::ArenaDef;

use crate::art::Art;
use crate::run::Selection;
use crate::{Phase, at};

pub fn plugin(app: &mut App) {
    app.init_resource::<CamRig>()
        .init_resource::<Built>()
        .init_resource::<Focus>()
        .add_systems(Startup, camera_and_light)
        .add_systems(Update, (build.run_if(not(in_state(Phase::Loading))), place_camera));
}

/// The arena built, and which one, so a new pick rebuilds it.
#[derive(Component)]
struct ArenaPiece;

#[derive(Resource, Default)]
struct Built(Option<usize>);

/// The camera's follow distance, wheel-zoomed.
#[derive(Resource)]
pub struct CamRig {
    pub dist: f32,
    pub orbit: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self { dist: 30.0, orbit: 0.0 }
    }
}

#[derive(Component)]
pub struct MainCam;

/// What the camera looks at: the hero in a run, the arena's heart at the menu.
#[derive(Resource, Default)]
pub struct Focus(pub Vec3);

fn camera_and_light(mut commands: Commands) {
    commands.spawn((
        MainCam,
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 24.0, 22.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // A low, warm evening sun over the ring, and a cool fill.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.75, 0.7, 0.9),
        brightness: 350.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.82, 0.62),
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.6, -0.85, 0.0)),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 30.0,
            maximum_distance: 120.0,
            ..default()
        }
        .build(),
    ));
}

/// Build the picked arena when it changes.
fn build(
    mut commands: Commands,
    art: Res<Art>,
    sel: Res<Selection>,
    mut built: ResMut<Built>,
    old: Query<Entity, With<ArenaPiece>>,
) {
    if built.0 == Some(sel.arena) {
        return;
    }
    built.0 = Some(sel.arena);
    for e in &old {
        commands.entity(e).despawn();
    }
    spawn_arena(&mut commands, &art, sel.arena());
}

fn piece(commands: &mut Commands, scene: &Handle<Scene>, tf: Transform) {
    commands.spawn((ArenaPiece, SceneRoot(scene.clone()), tf));
}

fn spawn_arena(commands: &mut Commands, art: &Art, def: &ArenaDef) {
    let a = &art.arena;
    let r = def.radius;
    // The floor: stone tiles inside the wall, dirt beyond it to the rubble.
    let tile = 4.0;
    let reach = ((r + 14.0) / tile).ceil() as i32;
    for i in -reach..=reach {
        for j in -reach..=reach {
            let p = Vec3::new(i as f32 * tile, 0.0, j as f32 * tile);
            let d = p.length();
            if d <= r + 2.0 {
                piece(commands, &a.floor, Transform::from_translation(p));
            } else if d <= r + 14.0 {
                piece(commands, &a.dirt, Transform::from_translation(p - Vec3::Y * 0.02));
            }
        }
    }
    // The wall: four-yard segments around the ring, stretched to close the gaps, an arch every
    // so often and one broken stretch.
    let wall_r = r + 0.6;
    let n = (TAU * wall_r / tile).ceil() as usize;
    let stretch = TAU * wall_r / n as f32 / tile * 1.03;
    for k in 0..n {
        let ang = k as f32 / n as f32 * TAU;
        let out = Vec3::new(ang.cos(), 0.0, ang.sin());
        // The segment runs along its local X: turn X onto the ring's tangent.
        let rot = Quat::from_rotation_y(-ang - FRAC_PI_2);
        let scene = match k % 9 {
            0 => &a.wall_arched,
            5 if k == n / 2 + 5 => &a.wall_broken,
            _ => &a.wall,
        };
        piece(
            commands,
            scene,
            Transform::from_translation(out * wall_r)
                .with_rotation(rot)
                .with_scale(Vec3::new(stretch, 1.0, 1.0)),
        );
        // The teams' colours hang on the inner face; torches between them. Both pieces face
        // their +Z, here turned to the ring's heart; a banner hangs from the wall's centre line,
        // a torch from its face.
        let face = Quat::from_rotation_y(3.0 * FRAC_PI_2 - ang);
        let inner = out * (wall_r - 0.5);
        match k % 6 {
            1 => {
                let banner = if k < n / 2 { &a.banner_red } else { &a.banner_blue };
                piece(
                    commands,
                    banner,
                    Transform::from_translation(out * wall_r).with_rotation(face),
                );
            }
            4 => {
                piece(
                    commands,
                    &a.torch,
                    Transform::from_translation(inner + Vec3::Y * 2.4).with_rotation(face),
                );
                if k % 12 == 4 {
                    commands.spawn((
                        ArenaPiece,
                        PointLight {
                            color: Color::srgb(1.0, 0.55, 0.2),
                            intensity: 120_000.0,
                            range: 14.0,
                            ..default()
                        },
                        Transform::from_translation(inner - out * 0.8 + Vec3::Y * 3.0),
                    ));
                }
            }
            _ => {}
        }
    }
    // Pillars to fight around: a grand one for a big footprint.
    for &(p, pr) in def.pillars {
        let (scene, base) = if pr > 2.0 { (&a.pillar_grand, 1.1) } else { (&a.pillar, 0.75) };
        let s = pr / base;
        piece(
            commands,
            scene,
            Transform::from_translation(at(p)).with_scale(Vec3::new(s, 1.0 + s * 0.15, s)),
        );
    }
    // Beyond the wall: barricades, barrels and rubble.
    for k in 0..10 {
        let ang = k as f32 / 10.0 * TAU + 0.3;
        let out = Vec3::new(ang.cos(), 0.0, ang.sin());
        let (scene, d) = match k % 3 {
            0 => (&a.rubble, r + 6.0),
            1 => (&a.barrel, r + 4.0),
            _ => (&a.barrier, r + 3.5),
        };
        piece(
            commands,
            scene,
            Transform::from_translation(out * d).with_rotation(Quat::from_rotation_y(-ang)),
        );
    }
}

/// Follow the hero from above at a fixed tilt, wheel to zoom; slowly orbit the ring at the menu.
fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    focus: Res<Focus>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    sel: Res<Selection>,
    mut cams: Query<&mut Transform, With<MainCam>>,
) {
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.1 * notches)).clamp(12.0, 55.0);
    }
    let Ok(mut cam) = cams.single_mut() else { return };
    let (eye, look) = if matches!(phase.get(), Phase::Menu | Phase::Loading) {
        rig.orbit += time.delta_secs() * 0.08;
        let d = sel.arena().radius * 1.15;
        (
            Vec3::new(rig.orbit.sin() * d, d * 0.55, rig.orbit.cos() * d),
            Vec3::Y * 1.0,
        )
    } else {
        let look = focus.0 + Vec3::Y * 1.2;
        (look + Vec3::new(0.0, 0.82, 0.57).normalize() * rig.dist, look)
    };
    *cam = Transform::from_translation(eye).looking_at(look, Vec3::Y);
}
