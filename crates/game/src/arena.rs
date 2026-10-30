//! The ring, built from the KayKit dungeon pieces around an [`ArenaDef`]: a stone floor, a curved
//! wall hung with the two teams' banners and lit by torches, great pillars to break line of sight,
//! and rubble beyond. Plus the light.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use rules::arena::ArenaDef;

use crate::art::Art;
use crate::{Phase, Selection, at};

pub fn plugin(app: &mut App) {
    app.init_resource::<Built>()
        .add_systems(Startup, light)
        .add_systems(Update, build.run_if(not(in_state(Phase::Loading))));
}

#[derive(Component)]
struct ArenaPiece;

/// Which arena stands, so a new pick rebuilds it.
#[derive(Resource, Default)]
struct Built(Option<usize>);

fn light(mut commands: Commands) {
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
            maximum_distance: 150.0,
            ..default()
        }
        .build(),
    ));
}

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
    // The wall: four-yard segments around the ring, stretched to close the gaps; an arch every so
    // often, the two gates where the teams wait.
    let wall_r = r + 0.6;
    let n = (TAU * wall_r / tile).ceil() as usize;
    let stretch = TAU * wall_r / n as f32 / tile * 1.03;
    for k in 0..n {
        let ang = k as f32 / n as f32 * TAU;
        let out = Vec3::new(ang.cos(), 0.0, ang.sin());
        // The segment runs along its local X: turn X onto the ring's tangent.
        let rot = Quat::from_rotation_y(-ang - FRAC_PI_2);
        let gate = k == n / 4 || k == 3 * n / 4;
        let scene = if gate || k % 9 == 0 { &a.wall_arched } else { &a.wall };
        piece(
            commands,
            scene,
            Transform::from_translation(out * wall_r)
                .with_rotation(rot)
                .with_scale(Vec3::new(stretch, 1.25, 1.0)),
        );
        // The teams' colours hang on the inner face (gold south, red north); torches between.
        let face = Quat::from_rotation_y(3.0 * FRAC_PI_2 - ang);
        let inner = out * (wall_r - 0.5);
        match k % 6 {
            1 => {
                let banner = if out.z < 0.0 { &a.banner_blue } else { &a.banner_red };
                piece(commands, banner, Transform::from_translation(out * wall_r).with_rotation(face));
            }
            4 => {
                piece(commands, &a.torch, Transform::from_translation(inner + Vec3::Y * 2.6).with_rotation(face));
                if k % 12 == 4 {
                    commands.spawn((
                        ArenaPiece,
                        PointLight {
                            color: Color::srgb(1.0, 0.55, 0.2),
                            intensity: 160_000.0,
                            range: 16.0,
                            ..default()
                        },
                        Transform::from_translation(inner - out * 0.8 + Vec3::Y * 3.2),
                    ));
                }
            }
            _ => {}
        }
    }
    // The pillars that break line of sight.
    for &(p, pr) in def.pillars {
        let (scene, base) = if pr >= 4.0 { (&a.pillar_grand, 1.1) } else { (&a.pillar, 0.75) };
        let s = pr / base;
        piece(commands, scene, Transform::from_translation(at(p)).with_scale(Vec3::new(s, 1.6, s)));
    }
    // Beyond the wall: barricades, barrels and rubble.
    for k in 0..14 {
        let ang = k as f32 / 14.0 * TAU + 0.3;
        let out = Vec3::new(ang.cos(), 0.0, ang.sin());
        let (scene, d) = match k % 3 {
            0 => (&a.rubble, r + 7.0),
            1 => (&a.barrel, r + 4.0),
            _ => (&a.barrier, r + 3.5),
        };
        piece(commands, scene, Transform::from_translation(out * d).with_rotation(Quat::from_rotation_y(-ang)));
    }
}
