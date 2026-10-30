//! The game's own spell visuals: glowing bolts, flashes, expanding rings and floor areas in each
//! school's colour, and floating combat text. Built from simple meshes, bloom does the glow.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rules::spells::School;

use crate::MatchEntity;
use crate::camera::MainCam;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, (fade, float_text));
}

const SCHOOLS: [School; 7] = [
    School::Physical,
    School::Fire,
    School::Frost,
    School::Arcane,
    School::Shadow,
    School::Nature,
    School::Holy,
];

fn idx(s: School) -> usize {
    SCHOOLS.iter().position(|x| *x == s).unwrap_or(0)
}

pub fn color(s: School) -> Color {
    let [r, g, b] = s.color();
    Color::srgb(r, g, b)
}

#[derive(Resource)]
pub struct FxAssets {
    pub sphere: Handle<Mesh>,
    /// A unit disc lying on the floor.
    pub disc: Handle<Mesh>,
    /// A thin unit ring lying on the floor.
    pub ring: Handle<Mesh>,
    /// Per school: a bright core, and a see-through veil for areas and shields.
    pub glow: Vec<Handle<StandardMaterial>>,
    pub veil: Vec<Handle<StandardMaterial>>,
    pub team_gold: Handle<StandardMaterial>,
    pub team_red: Handle<StandardMaterial>,
}

impl FxAssets {
    pub fn glow(&self, s: School) -> Handle<StandardMaterial> {
        self.glow[idx(s)].clone()
    }
    pub fn veil(&self, s: School) -> Handle<StandardMaterial> {
        self.veil[idx(s)].clone()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let flat = |m: Mesh| m.rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    let emissive = |c: [f32; 3], k: f32| LinearRgba::rgb(c[0] * k, c[1] * k, c[2] * k);
    let glow = SCHOOLS
        .iter()
        .map(|s| {
            mats.add(StandardMaterial {
                base_color: color(*s),
                emissive: emissive(s.color(), 6.0),
                unlit: true,
                ..default()
            })
        })
        .collect();
    let veil = SCHOOLS
        .iter()
        .map(|s| {
            mats.add(StandardMaterial {
                base_color: color(*s).with_alpha(0.28),
                emissive: emissive(s.color(), 1.5),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .collect();
    let mut team = |c: Color, e: [f32; 3]| {
        mats.add(StandardMaterial {
            base_color: c.with_alpha(0.85),
            emissive: emissive(e, 2.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })
    };
    let team_gold = team(Color::srgb(1.0, 0.8, 0.3), [1.0, 0.75, 0.25]);
    let team_red = team(Color::srgb(0.9, 0.15, 0.1), [0.9, 0.12, 0.08]);
    commands.insert_resource(FxAssets {
        sphere: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        disc: meshes.add(flat(Circle::new(1.0).mesh().resolution(48).build())),
        ring: meshes.add(flat(Annulus::new(0.86, 1.0).mesh().resolution(48).build())),
        glow,
        veil,
        team_gold,
        team_red,
    });
}

/// A visual that grows from `from` to `to` scale over `life` seconds, then goes.
#[derive(Component)]
pub struct Fade {
    t: f32,
    life: f32,
    from: f32,
    to: f32,
}

fn fade(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Transform, &mut Fade)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut f) in &mut q {
        f.t += dt;
        let u = (f.t / f.life).min(1.0);
        let s = f.from + (f.to - f.from) * (1.0 - (1.0 - u) * (1.0 - u));
        tf.scale = Vec3::splat(s.max(0.001));
        if u >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}

/// A rising number or word over a world point, drawn in screen space.
#[derive(Component)]
pub struct FloatText {
    world: Vec3,
    t: f32,
}

const FLOAT_SECS: f32 = 1.0;

fn float_text(
    mut commands: Commands,
    time: Res<Time<Real>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut q: Query<(Entity, &mut Node, &mut FloatText, &mut TextColor)>,
) {
    let Ok((cam, cam_tf)) = cams.single() else { return };
    let dt = time.delta_secs();
    for (e, mut node, mut ft, mut color) in &mut q {
        ft.t += dt;
        if ft.t >= FLOAT_SECS {
            commands.entity(e).despawn();
            continue;
        }
        let world = ft.world + Vec3::Y * (2.6 + ft.t * 1.6);
        match cam.world_to_viewport(cam_tf, world) {
            Ok(p) => {
                node.left = Val::Px(p.x - 20.0);
                node.top = Val::Px(p.y);
                node.display = Display::Flex;
            }
            Err(_) => node.display = Display::None,
        }
        color.0 = color.0.with_alpha(1.0 - (ft.t / FLOAT_SECS).powi(2));
    }
}

/// The calls that put visuals in the arena.
#[derive(SystemParam)]
pub struct Fx<'w, 's> {
    commands: Commands<'w, 's>,
    assets: Res<'w, FxAssets>,
}

impl Fx<'_, '_> {
    /// A burst of light at `pos`.
    pub fn flash(&mut self, pos: Vec3, school: School, size: f32) {
        self.commands.spawn((
            MatchEntity,
            Mesh3d(self.assets.sphere.clone()),
            MeshMaterial3d(self.assets.glow(school)),
            Transform::from_translation(pos + Vec3::Y * 1.0).with_scale(Vec3::splat(size * 0.3)),
            Fade { t: 0.0, life: 0.25, from: size * 0.3, to: size * 0.05 },
        ));
    }

    /// A ring rushing out from `pos` to `radius`.
    pub fn nova(&mut self, pos: Vec3, school: School, radius: f32) {
        self.commands.spawn((
            MatchEntity,
            Mesh3d(self.assets.ring.clone()),
            MeshMaterial3d(self.assets.glow(school)),
            Transform::from_translation(pos + Vec3::Y * 0.15),
            Fade { t: 0.0, life: 0.35, from: 0.5, to: radius },
        ));
        self.commands.spawn((
            MatchEntity,
            Mesh3d(self.assets.disc.clone()),
            MeshMaterial3d(self.assets.veil(school)),
            Transform::from_translation(pos + Vec3::Y * 0.1),
            Fade { t: 0.0, life: 0.35, from: 0.5, to: radius },
        ));
    }

    /// A floating number or word over `pos`.
    pub fn text(&mut self, pos: Vec3, s: impl Into<String>, color: Color, size: f32) {
        self.commands.spawn((
            MatchEntity,
            FloatText { world: pos, t: 0.0 },
            Text::new(s),
            TextFont { font_size: size, ..default() },
            TextColor(color),
            TextShadow { offset: Vec2::splat(1.5), color: Color::BLACK },
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(50),
        ));
    }

    /// A glowing sphere in `school`'s colour (a bolt in flight), with `extra` on it.
    pub fn glow_sphere(&mut self, extra: impl Bundle, school: School, tf: Transform) {
        let (mesh, mat) = (self.assets.sphere.clone(), self.assets.glow(school));
        self.commands.spawn((MatchEntity, extra, Mesh3d(mesh), MeshMaterial3d(mat), tf));
    }

}

pub const WHITE: Color = Color::WHITE;
pub const HEAL: Color = Color::srgb(0.35, 1.0, 0.4);
pub const CRIT: Color = Color::srgb(1.0, 0.85, 0.2);
