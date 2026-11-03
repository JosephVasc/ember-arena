//! What holds a fighter, shown on their body so it reads from across the arena, one look per kind:
//! stars circling a stun, ice spikes around a root, frost hoops sliding down a slow, a dark shroud
//! and spirits around a fear, a spinning cage around an incapacitate, a ring around a silenced
//! head, and a golden shell for immunity or a ward.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use bevy::prelude::*;
use rules::spells::School;
use rules::unit::Aura;

use crate::Step;
use crate::fight::Fighter;
use crate::fx::FxAssets;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, meshes).add_systems(Update, (dress, animate).in_set(Step::Show));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Stun,
    Root,
    Slow,
    Fear,
    Incap,
    Silence,
    Immune,
    Ward,
}

/// One piece of a status look: its kind, and which piece of that kind it is.
#[derive(Component)]
struct Piece(Kind, usize);

#[derive(Resource)]
struct Shapes {
    spike: Handle<Mesh>,
    hoop: Handle<Mesh>,
}

fn meshes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(Shapes {
        spike: meshes.add(Cone::new(0.16, 0.95)),
        hoop: meshes.add(Torus::new(0.86, 1.0)),
    });
}

/// How each look is made: what kind, how many pieces, in which mesh and colour.
fn pieces(fxa: &FxAssets, shapes: &Shapes) -> Vec<(Kind, usize, Handle<Mesh>, Handle<StandardMaterial>)> {
    let sphere = || fxa.sphere.clone();
    vec![
        (Kind::Stun, 6, sphere(), fxa.glow(School::Holy)),
        (Kind::Root, 9, shapes.spike.clone(), fxa.glow(School::Frost)),
        (Kind::Slow, 3, shapes.hoop.clone(), fxa.glow(School::Frost)),
        (Kind::Fear, 5, sphere(), fxa.glow(School::Shadow)),
        (Kind::Incap, 3, shapes.hoop.clone(), fxa.glow(School::Arcane)),
        (Kind::Silence, 2, shapes.hoop.clone(), fxa.glow(School::Arcane)),
        (Kind::Immune, 1, sphere(), fxa.veil(School::Holy)),
        (Kind::Ward, 1, sphere(), fxa.veil(School::Holy)),
    ]
}

/// Every fighter carries every look, hidden until needed. A fear also wears a shroud and an
/// incapacitate a shell: their extra piece is the last one, see-through.
fn dress(mut commands: Commands, fxa: Res<FxAssets>, shapes: Res<Shapes>, added: Query<Entity, Added<Fighter>>) {
    for e in &added {
        commands.entity(e).with_children(|c| {
            for (kind, n, mesh, mat) in pieces(&fxa, &shapes) {
                for i in 0..n {
                    c.spawn((Piece(kind, i), Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::default(), Visibility::Hidden));
                }
            }
            c.spawn((Piece(Kind::Fear, 99), Mesh3d(fxa.sphere.clone()), MeshMaterial3d(fxa.veil(School::Shadow)), Transform::default(), Visibility::Hidden));
            c.spawn((Piece(Kind::Incap, 99), Mesh3d(fxa.sphere.clone()), MeshMaterial3d(fxa.veil(School::Arcane)), Transform::default(), Visibility::Hidden));
        });
    }
}

fn on(kind: Kind, f: &Fighter) -> bool {
    let u = &f.unit;
    if u.dead {
        return false;
    }
    match kind {
        Kind::Stun => u.cc.stun_t > 0.0,
        Kind::Root => u.cc.root_t > 0.0,
        Kind::Slow => u.cc.slow_t > 0.0,
        Kind::Fear => u.cc.fear_t > 0.0,
        Kind::Incap => u.cc.incap_t > 0.0,
        Kind::Silence => u.cc.silence_t > 0.0,
        Kind::Immune => u.immune_t > 0.0,
        Kind::Ward => u.auras.iter().any(is_ward),
    }
}

fn is_ward(a: &Aura) -> bool {
    matches!(a.def.kind, rules::spells::AuraKind::Absorb(_)) && a.pool > 0.0
}

/// Where a piece sits at time `t`.
fn place(kind: Kind, i: usize, t: f32) -> Transform {
    let around = |n: usize, r: f32, spin: f32| {
        let a = i as f32 / n as f32 * TAU + t * spin;
        Vec3::new(a.cos() * r, 0.0, a.sin() * r)
    };
    match (kind, i) {
        // Over a seated head, sparks circling.
        (Kind::Stun, _) => Transform::from_translation(around(6, 0.62, 3.5) + Vec3::Y * (2.15 + 0.1 * (t * 6.0 + i as f32).sin()))
            .with_scale(Vec3::splat(0.14)),
        // Spikes jutting out of the floor around the feet, leaning out and shivering.
        (Kind::Root, _) => {
            let p = around(9, 0.82, 0.0);
            let lean = Quat::from_rotation_arc(Vec3::Y, (Vec3::Y + p.normalize_or_zero() * 0.45).normalize());
            let s = 1.0 + if i % 2 == 0 { 0.35 } else { 0.0 } + 0.05 * (t * 9.0 + i as f32).sin();
            Transform::from_translation(p + Vec3::Y * 0.42).with_rotation(lean).with_scale(Vec3::splat(s))
        }
        // Hoops sliding down the body, one after another, tightening as they go.
        (Kind::Slow, _) => {
            let u = (t * 0.55 + i as f32 / 3.0).fract();
            let y = 2.3 * (1.0 - u) + 0.05;
            Transform::from_translation(Vec3::Y * y).with_scale(Vec3::new(0.95 - 0.25 * u, 0.5, 0.95 - 0.25 * u))
        }
        // A dark shroud, and spirits racing round it.
        (Kind::Fear, 99) => Transform::from_translation(Vec3::Y * 1.2).with_scale(Vec3::splat(1.45 + 0.08 * (t * 5.0).sin())),
        (Kind::Fear, _) => Transform::from_translation(around(5, 1.1, -5.0) + Vec3::Y * (2.2 + 0.5 * (t * 4.0 + i as f32 * 1.7).sin()))
            .with_scale(Vec3::splat(0.2)),
        // A shell over the fallen body, caged by rings spinning on every axis.
        (Kind::Incap, 99) => Transform::from_translation(Vec3::Y * 0.7).with_scale(Vec3::splat(1.55)),
        (Kind::Incap, _) => {
            let tilt = i as f32 * PI / 3.0;
            let rot = Quat::from_rotation_y(t * 2.2 + tilt) * Quat::from_rotation_x(FRAC_PI_4 + tilt);
            Transform::from_translation(Vec3::Y * 0.7).with_rotation(rot).with_scale(Vec3::new(1.5, 0.7, 1.5))
        }
        // Two rings round the head, swinging.
        (Kind::Silence, _) => {
            let swing = (t * 3.0 + i as f32 * PI).sin() * 0.4;
            Transform::from_translation(Vec3::Y * (2.35 + i as f32 * 0.25))
                .with_rotation(Quat::from_rotation_x(swing))
                .with_scale(Vec3::new(0.55 - i as f32 * 0.12, 0.4, 0.55 - i as f32 * 0.12))
        }
        (Kind::Immune, _) => Transform::from_translation(Vec3::Y * 1.2).with_scale(Vec3::splat(1.75 + 0.05 * (t * 4.0).sin())),
        (Kind::Ward, _) => Transform::from_translation(Vec3::Y * 1.15).with_scale(Vec3::splat(1.4)),
    }
}

fn animate(time: Res<Time>, fighters: Query<&Fighter>, mut parts: Query<(&Piece, &ChildOf, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    for (p, parent, mut tf, mut vis) in &mut parts {
        let active = fighters.get(parent.parent()).is_ok_and(|f| on(p.0, f));
        let want = if active { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if active {
            *tf = place(p.0, p.1, t);
        }
    }
}
