//! The hero: spawned for a run, WASD over the ring under whatever holds him, the class escape on
//! F, the trinket on Q, and his health: regeneration, heals, enemy bolts landing, death and
//! victory.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rules::kit::{self, Effect, Move};
use rules::waves::Gear;

use crate::arena::{Focus, MainCam};
use crate::art::{self, Anim, Clip, character};
use crate::fx::{self, Fx};
use crate::rivals::Foe;
use crate::run::Run;
use crate::{Phase, RunEntity, Step, at, flat, pt, yaw};

pub fn plugin(app: &mut App) {
    app.init_resource::<Aim>()
        .add_systems(OnEnter(Phase::Playing), spawn_hero)
        .add_systems(
            Update,
            (aim, move_hero, escape, trinket)
                .chain()
                .in_set(Step::Input)
                .run_if(in_state(Phase::Playing)),
        )
        .add_systems(Update, vitals.in_set(Step::Vitals).run_if(in_state(Phase::Playing)))
        .add_systems(Update, auras)
        .configure_sets(
            Update,
            (Step::Input, Step::Rivals, Step::Combat, Step::Vitals).chain(),
        );
}

#[derive(Component)]
pub struct Hero;

/// The see-through shell over the hero while a shield, guard or immunity holds.
#[derive(Component)]
struct Shell;

/// The body radius rivals stop at.
pub const HERO_RADIUS: f32 = 0.7;
pub const MOBILITY_KEY: KeyCode = KeyCode::KeyF;
pub const TRINKET_KEY: KeyCode = KeyCode::KeyQ;

/// The floor under the cursor.
#[derive(Resource, Default)]
pub struct Aim(pub Option<Vec3>);

/// The loadout each class fights with.
pub fn gear(class: &kit::Class) -> Gear {
    match class.model {
        kit::Model::Knight => Gear::SwordShield,
        kit::Model::Barbarian => Gear::GreatAxe,
        kit::Model::Mage => Gear::Staff,
        kit::Model::Rogue => Gear::Knives,
        kit::Model::HoodedRogue => Gear::Crossbow,
    }
}

fn spawn_hero(
    mut commands: Commands,
    art: Res<art::Art>,
    fxa: Res<fx::FxAssets>,
    run: Res<Run>,
    heroes: Query<(), With<Hero>>,
) {
    // Back from a level-up or a pause the hero is still here.
    if !heroes.is_empty() {
        return;
    }
    let pos = at(run.arena.start);
    commands
        .spawn((
            Hero,
            RunEntity,
            character(&art, run.class.model, gear(run.class)),
            Transform::from_translation(pos),
        ))
        .observe(art::on_ready)
        .with_children(|c| {
            c.spawn((
                Mesh3d(fxa.ring.clone()),
                MeshMaterial3d(fxa.team_gold.clone()),
                Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::splat(1.1)),
            ));
            c.spawn((
                Shell,
                Mesh3d(fxa.sphere.clone()),
                MeshMaterial3d(fxa.veil(kit::School::Holy)),
                Transform::from_xyz(0.0, 1.1, 0.0).with_scale(Vec3::splat(1.5)),
                Visibility::Hidden,
            ));
        });
}

fn aim(
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    mut aim: ResMut<Aim>,
) {
    aim.0 = (|| {
        let cursor = windows.single().ok()?.cursor_position()?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam.viewport_to_world(tf, cursor).ok()?;
        let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
        Some(ray.get_point(t))
    })();
}

fn move_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    run: Res<Run>,
    aim: Res<Aim>,
    mut focus: ResMut<Focus>,
    mut hero: Query<(&mut Transform, &mut Anim), With<Hero>>,
) {
    let Ok((mut tf, mut anim)) = hero.single_mut() else { return };
    let dt = time.delta_secs();
    let pos = tf.translation;
    focus.0 = pos;
    if run.dead_t.is_some() {
        return;
    }
    let mut dir = Vec3::ZERO;
    for (k, d) in [
        (KeyCode::KeyW, Vec3::NEG_Z),
        (KeyCode::KeyS, Vec3::Z),
        (KeyCode::KeyA, Vec3::NEG_X),
        (KeyCode::KeyD, Vec3::X),
    ] {
        if keys.pressed(k) {
            dir += d;
        }
    }
    // Feared, the hero runs from what feared him; held, he goes nowhere.
    let feared = run.cc.fear_t > 0.0;
    if feared {
        dir = flat(pos - at(run.cc.from)).normalize_or(Vec3::Z);
    } else if run.cc.held() || run.cc.out() {
        dir = Vec3::ZERO;
    }
    let moving = dir != Vec3::ZERO && dt > 0.0;
    if moving {
        let dir = dir.normalize();
        let pace = if feared { 0.7 } else { 1.0 };
        let haste = if run.vanish_t > 0.0 { run.haste } else { 1.0 };
        let speed = run.stats().speed * run.cc.speed() * pace * haste;
        let next = run.arena.confine(pt(pos + dir * speed * dt), HERO_RADIUS);
        tf.translation = at(next);
    }
    // He faces the cursor when free, where he runs when feared.
    let face = if feared {
        Some(dir)
    } else {
        aim.0.map(|a| flat(a - pos)).filter(|d| d.length() > 0.5)
    };
    if let Some(f) = face.filter(|f| *f != Vec3::ZERO) {
        tf.rotation = tf.rotation.slerp(yaw(f), (dt * 18.0).min(1.0));
    }
    anim.base = if moving { Clip::Run } else { Clip::Idle };
}

/// F: the class's escape, learned at level 5. Held, only a blink gets out, and only of a stun or
/// a root.
fn escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    aim: Res<Aim>,
    mut hero: Query<(&mut Transform, &mut Anim), With<Hero>>,
    mut foes: Query<(&Transform, &mut Foe), Without<Hero>>,
    mut fx: Fx,
) {
    if !keys.just_pressed(MOBILITY_KEY)
        || !run.has_mobility()
        || run.mobility_cd > 0.0
        || run.dead_t.is_some()
    {
        return;
    }
    let Ok((mut tf, mut anim)) = hero.single_mut() else { return };
    let m = &run.class.mobility;
    let blink = matches!(m.kind, Move::Blink(_));
    let cc = run.cc;
    if cc.fear_t > 0.0 || cc.incap_t > 0.0 || (!blink && (cc.out() || cc.held())) {
        return;
    }
    let pos = tf.translation;
    let toward = aim
        .0
        .map(|a| flat(a - pos))
        .filter(|d| d.length() > 0.5)
        .map_or(tf.rotation * Vec3::Z, |d| d.normalize());
    let school = kit::ability(run.class.start).school;
    match m.kind {
        Move::Blink(yd) => {
            run.cc.stun_t = 0.0;
            run.cc.root_t = 0.0;
            let far = aim.0.map_or(yd, |a| flat(a - pos).length().min(yd));
            fx.flash(pos, school, 3.0);
            tf.translation = at(run.arena.confine(pt(pos + toward * far), HERO_RADIUS));
            fx.flash(tf.translation, school, 3.0);
        }
        Move::Charge(yd) => {
            let mark = aim.0.unwrap_or(pos);
            let target = foes
                .iter_mut()
                .filter(|(t, f)| !f.dead && flat(t.translation - pos).length() <= yd)
                .min_by(|a, b| {
                    let da = flat(a.0.translation - mark).length();
                    let db = flat(b.0.translation - mark).length();
                    da.total_cmp(&db)
                });
            let to = match target {
                Some((t, mut f)) => {
                    let scale = if f.champion { 0.3 } else { 1.0 };
                    f.cc.land(Effect::Stun(m.secs), scale, pt(pos));
                    f.casting = None;
                    let d = flat(t.translation - pos).normalize_or(toward);
                    tf.translation = t.translation - d * 1.5;
                    d
                }
                None => {
                    tf.translation += toward * yd * 0.5;
                    toward
                }
            };
            tf.translation = at(run.arena.confine(pt(tf.translation), HERO_RADIUS));
            tf.rotation = yaw(to);
            fx.flash(tf.translation, kit::School::Physical, 2.5);
        }
        Move::Vanish(x) => {
            run.vanish_t = m.secs;
            run.haste = x;
            fx.nova(pos, kit::School::Shadow, 4.0);
        }
    }
    anim.play(Clip::Cast, 0.4);
    run.mobility_cd = m.cooldown;
}

/// Q: break every control and snare, once every two minutes.
fn trinket(keys: Res<ButtonInput<KeyCode>>, mut run: ResMut<Run>, hero: Query<&Transform, With<Hero>>, mut fx: Fx) {
    if !keys.just_pressed(TRINKET_KEY) || run.trinket_cd > 0.0 || !run.cc.any() || run.dead_t.is_some() {
        return;
    }
    let Ok(tf) = hero.single() else { return };
    run.cc.clear();
    run.trinket_cd = kit::TRINKET_COOLDOWN;
    fx.nova(tf.translation, kit::School::Holy, 3.0);
    fx.text(tf.translation, "Free!", fx::HEAL, 22.0);
}

/// Damage the hero takes from one blow, after immunity, armour, guard and shield.
pub fn hurt(run: &mut Run, raw: f32) -> f32 {
    if run.immune_t > 0.0 || run.dead_t.is_some() {
        return 0.0;
    }
    let mut dmg = (raw - run.stats().armor).max(1.0);
    if run.guard_t > 0.0 {
        dmg *= 1.0 - run.guard;
    }
    if run.shield_t > 0.0 && run.shield > 0.0 {
        let absorbed = dmg.min(run.shield);
        run.shield -= absorbed;
        dmg -= absorbed;
    }
    run.hp -= dmg;
    if dmg > 0.0 {
        run.cc.hurt();
    }
    dmg
}

fn vitals(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    mut hero: Query<(&Transform, &mut Anim), With<Hero>>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    let Ok((tf, mut anim)) = hero.single_mut() else { return };
    if let Some(t) = run.dead_t.as_mut() {
        *t += dt;
        if *t > 3.0 {
            next.set(Phase::GameOver);
        }
        return;
    }
    let stats = run.stats();
    run.elapsed += dt;
    let r = &mut *run;
    for t in [
        &mut r.shield_t,
        &mut r.immune_t,
        &mut r.guard_t,
        &mut r.empower_t,
        &mut r.trinket_cd,
        &mut r.mobility_cd,
        &mut r.vanish_t,
    ] {
        *t = (*t - dt).max(0.0);
    }
    r.cc.tick(dt);
    let mut heal = stats.regen * dt;
    for (rate, t) in &mut r.heals {
        heal += *rate * dt.min(*t);
        *t -= dt;
    }
    r.heals.retain(|&(_, t)| t > 0.0);
    r.hp = (r.hp + heal).min(stats.max_hp);

    // Enemy bolts land.
    let now = run.elapsed;
    let (due, flying): (Vec<_>, Vec<_>) =
        std::mem::take(&mut run.incoming).into_iter().partition(|b| b.at <= now);
    run.incoming = flying;
    for bolt in due {
        fx.flash(tf.translation, bolt.cast.school, 1.6);
        let taken = hurt(&mut run, bolt.damage);
        if taken > 0.0 {
            fx.text(tf.translation, format!("{}", taken.round() as i64), fx::HURT, 18.0);
            anim.play(Clip::Hit, 0.3);
        }
        if run.immune_t <= 0.0 {
            run.cc.land(bolt.cast.effect, 1.0, bolt.from);
        }
    }

    if run.hp <= 0.0 {
        run.hp = 0.0;
        run.dead_t = Some(0.0);
        anim.die();
    } else if run.elapsed >= rules::waves::VICTORY_SECS {
        run.won = true;
        anim.play(Clip::Cheer, 3.0);
        next.set(Phase::GameOver);
    }
}

/// The shell over the hero while a shield, guard or immunity holds.
fn auras(run: Res<Run>, mut shells: Query<&mut Visibility, With<Shell>>) {
    let on = run.shield_t > 0.0 || run.immune_t > 0.0 || run.guard_t > 0.0;
    for mut v in &mut shells {
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}
