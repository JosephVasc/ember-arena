//! The rivals: they pour over the ring's wall in waves, chase the hero (casters stop at range and
//! cast, interruptibly), swing when in reach, and a champion every couple of minutes controls the
//! hero with its signature move.

use std::collections::HashMap;

use bevy::prelude::*;
use rules::control::{Held, Landed};
use rules::waves::{self, Cast, Control};

use crate::art::{self, Anim, Clip, character};
use crate::fx::{self, Fx};
use crate::hero::{HERO_RADIUS, Hero, hurt};
use crate::run::{Incoming, Roster, Run};
use crate::{Phase, Rng, RunEntity, Step, at, flat, pt, yaw};

pub fn plugin(app: &mut App) {
    app.init_resource::<Spawner>().add_systems(
        Update,
        (spawn, chase)
            .chain()
            .in_set(Step::Rivals)
            .run_if(in_state(Phase::Playing)),
    )
    .add_systems(Update, reap)
    .add_systems(OnEnter(Phase::Menu), |mut s: ResMut<Spawner>| *s = Spawner::default());
}

/// One damage-over-time on a rival.
pub struct Dot {
    pub dps: f32,
    pub left: f32,
    pub tick: f32,
    pub school: rules::kit::School,
}

#[derive(Component)]
pub struct Foe {
    pub name: String,
    pub hp: f32,
    pub max_hp: f32,
    pub damage: f32,
    pub speed: f32,
    pub radius: f32,
    pub xp: u32,
    pub champion: bool,
    pub attack_cd: f32,
    pub cc: Held,
    pub dots: Vec<Dot>,
    /// Seconds left on a cast at the hero.
    pub casting: Option<f32>,
    pub cast: Option<Cast>,
    pub control: Option<Control>,
    pub control_cd: f32,
    pub dead: bool,
    pub gear: waves::Gear,
}

impl Foe {
    pub fn interrupt(&mut self) -> bool {
        self.casting.take().is_some()
    }
}

/// A corpse playing out its death.
#[derive(Component)]
pub struct Dying(pub f32);

#[derive(Resource, Default)]
pub struct Spawner {
    timer: f32,
}

fn spawn_foe(
    commands: &mut Commands,
    art: &art::Art,
    fxa: &fx::FxAssets,
    def: &waves::Rival,
    elapsed: f32,
    pos: Vec3,
    hero: Vec3,
) {
    let hp = def.hp * waves::hp_scale(elapsed);
    let champion = def.control.is_some();
    commands
        .spawn((
            RunEntity,
            character(art, def.kind.model, def.kind.gear),
            Transform::from_translation(pos)
                .with_rotation(yaw(flat(hero - pos)))
                .with_scale(Vec3::splat(def.scale)),
            Foe {
                name: def.name.clone(),
                hp,
                max_hp: hp,
                damage: def.damage * waves::damage_scale(elapsed),
                speed: def.kind.speed,
                radius: 0.6 * def.scale,
                xp: def.xp,
                champion,
                attack_cd: 0.6,
                cc: Held::default(),
                dots: Vec::new(),
                casting: None,
                cast: def.kind.cast,
                control: def.control,
                control_cd: waves::CONTROL_EVERY * 0.5,
                dead: false,
                gear: def.kind.gear,
            },
        ))
        .observe(art::on_ready)
        .with_children(|c| {
            c.spawn((
                Mesh3d(fxa.ring.clone()),
                MeshMaterial3d(fxa.team_red.clone()),
                Transform::from_xyz(0.0, 0.06, 0.0)
                    .with_scale(Vec3::splat(if champion { 1.3 } else { 0.9 })),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<art::Art>,
    fxa: Res<fx::FxAssets>,
    roster: Res<Roster>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    hero: Query<&Transform, With<Hero>>,
    foes: Query<&Foe>,
) {
    let Ok(h) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let hpos = h.translation;
    let elapsed = run.elapsed;
    let arena = run.arena;
    // Over the wall at a bearing from the centre, never on top of the hero.
    let rim = |a: f32| {
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let p = dir * (arena.radius - 1.5);
        if flat(p - hpos).length() < 10.0 { -p } else { p }
    };

    let due = ((elapsed / waves::CHAMPION_EVERY) as u32).min(roster.champions.len() as u32);
    while run.champions < due {
        let def = &roster.champions[run.champions as usize];
        let pos = rim(rng.range(0.0, std::f32::consts::TAU));
        spawn_foe(&mut commands, &art, &fxa, def, elapsed, pos, hpos);
        run.champions += 1;
        run.banner = Some((format!("{} enters the arena!", def.name), 4.0));
    }

    // Half way between champions, a ring of the moment's weakest closes in from every side.
    let pool = waves::wave_pool(elapsed, roster.rivals.len());
    let rings = ((elapsed + waves::CHAMPION_EVERY / 2.0) / waves::CHAMPION_EVERY) as u32;
    if run.rings < rings {
        run.rings = rings;
        let def = &roster.rivals[pool[0]];
        let n = 20 + (waves::progress(elapsed) * 40.0) as usize;
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let p = Vec3::new(a.cos(), 0.0, a.sin()) * (arena.radius - 1.5);
            spawn_foe(&mut commands, &art, &fxa, def, elapsed, p, hpos);
        }
        run.banner = Some(("The gates open on every side!".into(), 3.0));
    }

    spawner.timer -= time.delta_secs();
    if spawner.timer > 0.0 {
        return;
    }
    spawner.timer = 0.15;
    let alive = foes.iter().filter(|f| !f.dead).count();
    let target = waves::target_alive(elapsed);
    if alive >= target {
        return;
    }
    let n = ((target - alive) as f32 / 6.0).ceil().min(3.0) as usize;
    for _ in 0..n {
        let def = &roster.rivals[pool[rng.index(pool.len())]];
        let pos = rim(rng.range(0.0, std::f32::consts::TAU));
        spawn_foe(&mut commands, &art, &fxa, def, elapsed, pos, hpos);
    }
}

/// Bolts fly this fast at the hero.
const BOLT_SPEED: f32 = 24.0;

#[allow(clippy::type_complexity)]
fn chase(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut hero: Query<(&Transform, &mut Anim), (With<Hero>, Without<Foe>)>,
    mut foes: Query<(Entity, &mut Transform, &mut Foe, &mut Anim), Without<Hero>>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((htf, mut hanim)) = hero.single_mut() else { return };
    let hpos = htf.translation;
    let down = run.dead_t.is_some();
    let lost = run.vanish_t > 0.0;

    // Separation over a coarse grid: only neighbours in the same or next cells push.
    const CELL: f32 = 3.0;
    let bodies: Vec<(Entity, Vec3, f32)> = foes
        .iter()
        .filter(|(_, _, f, _)| !f.dead)
        .map(|(e, t, f, _)| (e, t.translation, f.radius))
        .collect();
    let key = |p: Vec3| ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, b) in bodies.iter().enumerate() {
        grid.entry(key(b.1)).or_default().push(i);
    }
    let mut push: HashMap<Entity, Vec3> = HashMap::with_capacity(bodies.len());
    for (i, &(e, p, r)) in bodies.iter().enumerate() {
        let (cx, cz) = key(p);
        let mut acc = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                for &j in grid.get(&(cx + dx, cz + dz)).into_iter().flatten() {
                    if j == i {
                        continue;
                    }
                    let (_, o, or) = bodies[j];
                    let d = flat(p - o);
                    let len = d.length();
                    if len < r + or && len > 1e-4 {
                        acc += d / len * (r + or - len);
                    } else if len <= 1e-4 {
                        acc += Vec3::new((i as f32).sin(), 0.0, (i as f32).cos()) * 0.1;
                    }
                }
            }
        }
        push.insert(e, acc);
    }

    for (e, mut tf, mut f, mut anim) in &mut foes {
        let f = &mut *f;
        if f.dead {
            continue;
        }
        f.attack_cd -= dt;
        f.control_cd -= dt;
        f.cc.tick(dt);
        let pos = tf.translation;
        let to_hero = flat(hpos - pos);
        let dist = to_hero.length();

        // A cast something stopped is lost.
        if f.casting.is_some() && (f.cc.silenced() || down || lost) {
            f.casting = None;
        }
        let caster = f.cast.is_some() && !f.champion && !f.cc.silenced();
        let standing = caster && dist <= waves::CAST_RANGE * 0.9;
        let held = f.cc.held() || lost || f.casting.is_some();
        let feared = f.cc.fear_t > 0.0;
        let contact = dist <= HERO_RADIUS + f.radius + 0.4;
        let mut moving = false;
        let mut next = pos;
        if !held && !down {
            let dir = if feared { -to_hero } else { to_hero }.normalize_or_zero();
            if feared || !(contact || standing) {
                next += dir * f.speed * f.cc.speed() * dt;
                moving = true;
            }
            if dir != Vec3::ZERO {
                tf.rotation = tf.rotation.slerp(yaw(dir), (dt * 10.0).min(1.0));
            }
        } else if f.casting.is_some() && to_hero != Vec3::ZERO {
            tf.rotation = tf.rotation.slerp(yaw(to_hero), (dt * 10.0).min(1.0));
        }
        next += push.get(&e).copied().unwrap_or_default() * (dt * 8.0).min(0.5);
        tf.translation = at(run.arena.confine(pt(next), f.radius));
        anim.base = if f.casting.is_some() {
            Clip::Casting
        } else if moving {
            Clip::Run
        } else {
            Clip::Idle
        };

        // The cast: begun in range, landing as a bolt.
        if caster && !down && !lost && !f.cc.out() {
            if let Some(cast) = f.cast {
                match f.casting.as_mut() {
                    Some(left) => {
                        *left -= dt;
                        if *left <= 0.0 {
                            f.casting = None;
                            f.attack_cd = waves::CAST_EVERY;
                            anim.play(Clip::Cast, 0.5);
                            let at_t = run.elapsed + dist / BOLT_SPEED;
                            run.incoming.push(Incoming {
                                at: at_t,
                                damage: f.damage * waves::CAST_DAMAGE,
                                cast,
                                from: pt(pos),
                            });
                            fx.flash(pos + Vec3::Y * 0.6, cast.school, 1.2);
                        }
                    }
                    None if dist <= waves::CAST_RANGE && f.attack_cd <= 0.0 => {
                        f.casting = Some(waves::CAST_SECS);
                    }
                    None => {}
                }
            }
        }

        // A champion's control on the hero.
        if let Some(c) = f.control {
            if f.control_cd <= 0.0 && dist <= c.reach && !down && !lost && !f.cc.out() {
                f.control_cd = waves::CONTROL_EVERY;
                anim.play(Clip::Cast, 0.6);
                fx.nova(hpos, c.school, 2.5);
                let landed = if run.immune_t > 0.0 {
                    Landed::Immune
                } else {
                    run.cc.land(c.effect, 1.0, pt(pos))
                };
                let word = match landed {
                    Landed::Immune => "Immune".to_string(),
                    _ => c.name.to_string(),
                };
                fx.text(hpos, word, fx::color(c.school), 20.0);
            }
        }

        // The swing.
        if contact && !lost && !f.cc.out() && f.casting.is_none() && f.attack_cd <= 0.0 && !down {
            f.attack_cd = if f.champion { 1.0 } else { 1.3 };
            anim.play(Clip::attack(f.gear), 0.6);
            let taken = hurt(&mut run, f.damage);
            if taken > 0.0 {
                fx.text(hpos, format!("{}", taken.round() as i64), fx::HURT, 18.0);
                hanim.play(Clip::Hit, 0.3);
            }
        }
    }
}

/// Corpses play their death, then go.
fn reap(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Dying)>) {
    for (e, mut d) in &mut q {
        d.0 += time.delta_secs();
        if d.0 > 2.5 {
            commands.entity(e).despawn();
        }
    }
}
