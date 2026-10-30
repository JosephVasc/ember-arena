//! The fight: every ability the hero owns casts itself on its cooldown at the targets its kind
//! picks (an arena player's choices: kicks wait for a cast, controls go to the casters), its bolt
//! flies, its hit lands through the rival's diminishing returns, and the fallen drop experience.

use bevy::prelude::*;
use rules::control::Landed;
use rules::kit::{Ability, Effect, Kind, School};
use rules::waves;

use crate::art::{Anim, Clip};
use crate::fx::{self, Fx};
use crate::hero::Hero;
use crate::rivals::{Dot, Dying, Foe};
use crate::run::Run;
use crate::{Phase, Rng, Step, flat};

pub fn plugin(app: &mut App) {
    app.add_message::<Hit>().add_systems(
        Update,
        (cast, fly, areas, land, collect)
            .chain()
            .in_set(Step::Combat)
            .run_if(in_state(Phase::Playing)),
    );
}

/// One blow landing on one rival.
#[derive(Message, Clone)]
pub struct Hit {
    pub target: Entity,
    pub damage: f32,
    pub dot: f32,
    pub dot_dur: f32,
    pub effect: Effect,
    /// Also hits everything this close to the target, for 70%.
    pub splash: f32,
    pub crit: bool,
    pub school: School,
}

/// A missile in flight at a rival; it lands its hit on arrival.
#[derive(Component)]
struct Bolt {
    target: Entity,
    hit: Hit,
    speed: f32,
}

/// A floor area ticking on whatever stands in it.
#[derive(Component)]
struct Area {
    radius: f32,
    left: f32,
    every: f32,
    next: f32,
    hit: Hit,
}

/// Experience (or a mend) on the floor.
#[derive(Component)]
pub struct Orb {
    xp: u32,
    heal: f32,
    pulled: bool,
    speed: f32,
}

/// A rival as targeting sees it this frame.
#[derive(Clone, Copy)]
struct Target {
    e: Entity,
    pos: Vec3,
    hp: f32,
    dist: f32,
    radius: f32,
    champion: bool,
    casting: bool,
    frozen: bool,
    controlled: bool,
}

const BOLT_SPEED: f32 = 30.0;

fn nearest(t: &[Target], range: f32, n: usize) -> Vec<Target> {
    t.iter().filter(|f| f.dist <= range + f.radius).take(n).copied().collect()
}

fn toughest(t: &[Target], range: f32, n: usize) -> Vec<Target> {
    let mut v: Vec<Target> = t.iter().filter(|f| f.dist <= range + f.radius).copied().collect();
    v.sort_by(|a, b| b.hp.total_cmp(&a.hp));
    v.truncate(n);
    v
}

/// Casters first, then the nearest still free; champions shrug controls off, so last.
fn control_targets(t: &[Target], range: f32, n: usize) -> Vec<Target> {
    let mut v: Vec<Target> = t
        .iter()
        .filter(|f| f.dist <= range + f.radius && !f.controlled)
        .copied()
        .collect();
    v.sort_by_key(|f| (f.champion, !f.casting));
    v.truncate(n);
    v
}

fn around(t: &[Target], c: Vec3, r: f32) -> Vec<Target> {
    t.iter().filter(|f| flat(f.pos - c).length() <= r + f.radius * 0.5).copied().collect()
}

/// The point among the nearest in range with the most others within `r`.
fn densest(t: &[Target], range: f32, r: f32) -> Option<Vec3> {
    let near: Vec<&Target> = t.iter().filter(|f| f.dist <= range).take(40).collect();
    near.iter()
        .map(|f| (f.pos, near.iter().filter(|o| flat(o.pos - f.pos).length() <= r).count()))
        .max_by_key(|&(_, n)| n)
        .map(|(p, _)| p)
}

fn hit_for(def: &Ability, target: Entity, damage: f32, dot: f32, dur: f32, radius: f32, crit: bool) -> Hit {
    Hit {
        target,
        damage,
        dot,
        dot_dur: dur,
        effect: def.effect,
        splash: if matches!(def.kind, Kind::Bolt | Kind::Strike) { radius } else { 0.0 },
        crit,
        school: def.school,
    }
}

#[allow(clippy::too_many_arguments)]
fn cast(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut hero: Query<(&Transform, &mut Anim), With<Hero>>,
    foes: Query<(Entity, &Transform, &Foe)>,
    mut hits: MessageWriter<Hit>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((htf, mut anim)) = hero.single_mut() else { return };
    let hpos = htf.translation;
    let stats = run.stats();
    let mut targets: Vec<Target> = foes
        .iter()
        .filter(|(_, _, f)| !f.dead)
        .map(|(e, t, f)| Target {
            e,
            pos: t.translation,
            hp: f.hp,
            dist: flat(t.translation - hpos).length(),
            radius: f.radius,
            champion: f.champion,
            casting: f.casting.is_some(),
            frozen: f.cc.frozen(),
            controlled: f.cc.out() || f.cc.held(),
        })
        .collect();
    targets.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    // A held hero can only reach for what breaks or weathers it; a silenced one still swings.
    let cc = run.cc;
    let usable = |k: Kind| match k {
        Kind::Immune | Kind::Guard => true,
        _ if cc.out() => false,
        Kind::Melee | Kind::Empower => true,
        _ => cc.silence_t <= 0.0,
    };
    let hurt = run.hp < stats.max_hp * 0.6;
    let close = targets.first().is_some_and(|f| f.dist <= 8.0);
    let mut swung = None;

    for i in 0..run.abilities.len() {
        run.abilities[i].cd -= dt;
        let o = run.abilities[i];
        if o.cd > 0.0 || !usable(o.def.kind) {
            continue;
        }
        let def = o.def;
        let c = o.cast(&stats);
        let roll = |rng: &mut Rng, d: f32| {
            if rng.f32() < stats.crit { (d * 2.0, true) } else { (d, false) }
        };
        let fired = match def.kind {
            Kind::Bolt | Kind::Strike | Kind::Melee => {
                let reach = if def.kind == Kind::Melee { def.range + 0.5 } else { def.range };
                let picked = if def.effect.seeks_casters() {
                    control_targets(&targets, reach, c.count as usize)
                } else if def.toughest {
                    toughest(&targets, reach, c.count as usize)
                } else {
                    nearest(&targets, reach, c.count as usize)
                };
                for t in &picked {
                    let base = if def.shatter > 0.0 && t.frozen { c.dmg * def.shatter } else { c.dmg };
                    let (d, crit) = roll(&mut rng, base);
                    let hit = hit_for(def, t.e, d, c.dot, c.duration, c.radius, crit);
                    if def.kind == Kind::Bolt {
                        fx.glow_sphere(
                            Bolt { target: t.e, hit, speed: BOLT_SPEED },
                            def.school,
                            Transform::from_translation(hpos + Vec3::Y * 1.3)
                                .with_scale(Vec3::splat(0.22)),
                        );
                    } else {
                        fx.flash(t.pos, def.school, 1.4);
                        hits.write(hit);
                    }
                }
                if !picked.is_empty() {
                    swung = Some(def.kind == Kind::Melee);
                }
                !picked.is_empty()
            }
            Kind::Nova | Kind::Cone => {
                let mut picked = around(&targets, hpos, c.radius);
                if def.kind == Kind::Cone {
                    if let Some(first) = picked.first().copied() {
                        let aim = flat(first.pos - hpos).normalize_or_zero();
                        picked.retain(|f| {
                            let d = flat(f.pos - hpos);
                            d.length() < 2.0 || d.normalize_or_zero().dot(aim) >= 0.75
                        });
                    }
                }
                for t in &picked {
                    let (d, crit) = roll(&mut rng, c.dmg);
                    hits.write(hit_for(def, t.e, d, c.dot, c.duration, 0.0, crit));
                }
                if !picked.is_empty() {
                    fx.nova(hpos, def.school, c.radius);
                    swung = Some(false);
                }
                !picked.is_empty()
            }
            Kind::Ground => {
                let center = if def.range <= 0.0 {
                    targets.first().filter(|f| f.dist <= c.radius + 3.0).map(|_| hpos)
                } else {
                    densest(&targets, def.range, c.radius)
                };
                match center {
                    None => false,
                    Some(center) => {
                        let burst = def.duration <= 0.0;
                        fx.veil_disc(
                            Area {
                                radius: c.radius,
                                left: if burst { 0.6 } else { c.duration },
                                every: if burst { f32::INFINITY } else { 1.0 },
                                next: if burst { 0.0 } else { 0.3 },
                                hit: Hit {
                                    dot_dur: if burst { 5.0 } else { 0.0 },
                                    ..hit_for(def, Entity::PLACEHOLDER, c.dmg, c.dot, 0.0, 0.0, false)
                                },
                            },
                            def.school,
                            Transform::from_translation(center + Vec3::Y * 0.08)
                                .with_scale(Vec3::splat(c.radius)),
                        );
                        swung = Some(false);
                        true
                    }
                }
            }
            Kind::Chain => match nearest(&targets, def.range, 1).first().copied() {
                None => false,
                Some(first) => {
                    let mut chain = vec![first];
                    while chain.len() < c.count as usize + 1 {
                        let last = chain[chain.len() - 1].pos;
                        let next = targets
                            .iter()
                            .filter(|f| !chain.iter().any(|x| x.e == f.e))
                            .filter(|f| flat(f.pos - last).length() <= c.radius)
                            .min_by(|a, b| flat(a.pos - last).length().total_cmp(&flat(b.pos - last).length()));
                        match next {
                            Some(f) => chain.push(*f),
                            None => break,
                        }
                    }
                    for (k, t) in chain.iter().enumerate() {
                        let (d, crit) = roll(&mut rng, c.dmg * 0.85f32.powi(k as i32));
                        fx.flash(t.pos, def.school, 1.2);
                        hits.write(hit_for(def, t.e, d, c.dot, c.duration, 0.0, crit));
                    }
                    swung = Some(true);
                    true
                }
            },
            Kind::Dot => {
                let mut pool: Vec<Target> =
                    targets.iter().filter(|f| f.dist <= def.range).take(c.count as usize * 3).copied().collect();
                let mut n = 0;
                while n < c.count && !pool.is_empty() {
                    let t = pool.swap_remove(rng.index(pool.len()));
                    fx.flash(t.pos, def.school, 1.0);
                    hits.write(hit_for(def, t.e, 0.0, c.dot, c.duration, 0.0, false));
                    n += 1;
                }
                if n > 0 {
                    swung = Some(false);
                }
                n > 0
            }
            Kind::Interrupt => match targets.iter().find(|f| f.casting && f.dist <= def.range + f.radius) {
                None => false,
                Some(t) => {
                    let (d, crit) = roll(&mut rng, c.dmg);
                    fx.flash(t.pos, def.school, 1.8);
                    hits.write(hit_for(def, t.e, d, 0.0, 0.0, 0.0, crit));
                    swung = Some(def.range <= 8.0);
                    true
                }
            },
            Kind::Guard => {
                if hurt && close && run.guard_t <= 0.0 && run.immune_t <= 0.0 {
                    run.guard = def.damage;
                    run.guard_t = c.duration;
                    fx.nova(hpos, def.school, 3.0);
                    fx.text(hpos, def.name, fx::color(def.school), 18.0);
                    true
                } else {
                    false
                }
            }
            Kind::Empower => {
                let near = targets.iter().take_while(|f| f.dist <= def.range).count();
                let champ = targets.iter().any(|f| f.champion && f.dist <= def.range);
                if (near >= 6 || champ) && run.empower_t <= 0.0 {
                    run.empower = def.damage;
                    run.empower_t = c.duration;
                    fx.nova(hpos, def.school, 4.0);
                    fx.text(hpos, def.name, fx::color(def.school), 18.0);
                    true
                } else {
                    false
                }
            }
            Kind::Immune => {
                if (run.hp < stats.max_hp * 0.4 || cc.out()) && close {
                    run.immune_t = c.duration;
                    run.cc.clear();
                    fx.nova(hpos, def.school, 3.0);
                    fx.text(hpos, def.name, fx::color(def.school), 20.0);
                    true
                } else {
                    false
                }
            }
            Kind::Shield => {
                run.shield = c.dmg;
                run.shield_t = c.duration;
                fx.nova(hpos, def.school, 2.5);
                true
            }
            Kind::Heal => {
                if run.hp < stats.max_hp * 0.85 {
                    let secs = c.duration.max(1.0);
                    run.heals.push((c.dmg / secs, secs));
                    fx.text(hpos, format!("+{}", c.dmg.round() as i64), fx::HEAL, 18.0);
                    true
                } else {
                    false
                }
            }
        };
        run.abilities[i].cd = if fired { c.cooldown } else { 0.2 };
    }
    // One swing or one cast animation a frame, whichever fired.
    match swung {
        Some(true) => anim.play(Clip::attack(crate::hero::gear(run.class)), 0.5),
        Some(false) => anim.play(Clip::Cast, 0.45),
        None => {}
    }
}

/// Bolts home on their target; a dead or gone target lets the bolt fizzle.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Transform, &Bolt), Without<Foe>>,
    foes: Query<(&Transform, &Foe)>,
    mut hits: MessageWriter<Hit>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    for (e, mut tf, b) in &mut bolts {
        let Ok((t, f)) = foes.get(b.target) else {
            commands.entity(e).despawn();
            continue;
        };
        if f.dead {
            commands.entity(e).despawn();
            continue;
        }
        let goal = t.translation + Vec3::Y * 1.1;
        let d = goal - tf.translation;
        let step = b.speed * dt;
        if d.length() <= step.max(0.4) {
            fx.flash(t.translation, b.hit.school, 1.3);
            hits.write(b.hit.clone());
            commands.entity(e).despawn();
        } else {
            tf.translation += d.normalize() * step;
        }
    }
}

/// Areas tick on whoever stands in them, then fade.
fn areas(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &Transform, &mut Area)>,
    foes: Query<(Entity, &Transform, &Foe)>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    for (e, tf, mut a) in &mut q {
        a.left -= dt;
        a.next -= dt;
        if a.next <= 0.0 {
            a.next += a.every;
            for (fe, ft, f) in &foes {
                if !f.dead && flat(ft.translation - tf.translation).length() <= a.radius + f.radius * 0.5 {
                    hits.write(Hit { target: fe, ..a.hit.clone() });
                }
            }
        }
        if a.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Land every hit: damage and its number, the over-time part, the control through the rival's
/// diminishing returns (a cast it breaks shows), and death with its orb.
#[allow(clippy::too_many_arguments)]
fn land(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut incoming: MessageReader<Hit>,
    mut foes: Query<(Entity, &Transform, &mut Foe, &mut Anim)>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    let mut all: Vec<Hit> = Vec::new();
    for h in incoming.read() {
        if h.splash > 0.0 {
            if let Ok((_, c, ..)) = foes.get(h.target) {
                let c = c.translation;
                for (e, t, f, _) in foes.iter() {
                    if e != h.target && !f.dead && flat(t.translation - c).length() <= h.splash {
                        all.push(Hit { target: e, damage: h.damage * 0.7, splash: 0.0, ..h.clone() });
                    }
                }
            }
        }
        all.push(Hit { splash: 0.0, ..h.clone() });
    }

    let mut deaths: Vec<(Entity, Vec3, u32, bool)> = Vec::new();
    for h in all {
        let Ok((e, tf, mut f, mut anim)) = foes.get_mut(h.target) else { continue };
        if f.dead {
            continue;
        }
        let mut dmg = h.damage;
        if let Effect::Execute(th) = h.effect {
            if !f.champion && f.hp / f.max_hp <= th {
                dmg = dmg.max(f.hp);
            }
        }
        if dmg > 0.0 {
            f.hp -= dmg;
            run.damage_done += f64::from(dmg);
            let color = if h.crit { fx::CRIT } else { fx::WHITE };
            fx.text(tf.translation, format!("{}", dmg.round() as i64), color, if h.crit { 24.0 } else { 16.0 });
            if let Effect::Drain(frac) = h.effect {
                let max = run.stats().max_hp;
                run.hp = (run.hp + dmg * frac).min(max);
            }
            if !matches!(h.effect, Effect::Incap(_)) {
                f.cc.hurt();
            }
        }
        if h.dot > 0.0 && h.dot_dur > 0.0 {
            if f.dots.len() >= 6 {
                f.dots.remove(0);
            }
            f.dots.push(Dot { dps: h.dot, left: h.dot_dur, tick: 1.0, school: h.school });
        }
        // Champions are never feared or incapacitated and shrug off most stuns and roots.
        let landed = match h.effect {
            Effect::Fear(_) | Effect::Incap(_) if f.champion => Landed::Immune,
            Effect::Slow(m, t) if f.champion => f.cc.land(Effect::Slow(m.max(0.75), t), 1.0, [0.0; 2]),
            e2 => {
                let scale = match e2 {
                    Effect::Stun(_) | Effect::Root(_) if f.champion => 0.3,
                    _ => 1.0,
                };
                f.cc.land(e2, scale, crate::pt(tf.translation))
            }
        };
        let single = matches!(h.effect, Effect::Incap(_) | Effect::Silence(_) | Effect::Interrupt(_));
        match landed {
            Landed::Immune if f.champion || single => {
                fx.text(tf.translation, "Immune", fx::WHITE, 16.0);
            }
            Landed::For(_) if f.cc.silenced() && f.interrupt() => {
                fx.text(tf.translation, "Interrupted", fx::CRIT, 20.0);
            }
            _ => {}
        }
        if f.hp <= 0.0 {
            f.dead = true;
            anim.die();
            deaths.push((e, tf.translation, f.xp, f.champion));
        }
    }

    // Damage over time, a tick a second.
    for (e, tf, mut f, mut anim) in &mut foes {
        if f.dead || f.dots.is_empty() {
            continue;
        }
        let mut dealt = 0.0;
        let mut school = School::Physical;
        for d in &mut f.dots {
            d.left -= dt;
            d.tick -= dt;
            if d.tick <= 0.0 {
                d.tick += 1.0;
                dealt += d.dps;
                school = d.school;
            }
        }
        f.dots.retain(|d| d.left > 0.0);
        if dealt > 0.0 {
            f.cc.hurt();
            f.hp -= dealt;
            run.damage_done += f64::from(dealt);
            fx.text(tf.translation, format!("{}", dealt.round() as i64), fx::color(school), 14.0);
            if f.hp <= 0.0 {
                f.dead = true;
                anim.die();
                deaths.push((e, tf.translation, f.xp, f.champion));
            }
        }
    }

    for (e, pos, xp, champion) in deaths {
        run.kills += 1;
        commands.entity(e).insert(Dying(0.0));
        if champion {
            run.banner = Some(("The champion falls!".into(), 3.0));
        }
        let orb = |xp: u32, heal: f32| Orb { xp, heal, pulled: false, speed: 6.0 };
        let mut spawn = |o: Orb, at: Vec3| {
            let heal = o.heal > 0.0;
            let size = if heal { 0.35 } else { 0.18 + (o.xp as f32).sqrt() * 0.04 };
            let tf = Transform::from_translation(at + Vec3::Y * 0.5).with_scale(Vec3::splat(size));
            fx.orb(o, heal, tf);
        };
        spawn(orb(xp, 0.0), pos);
        if champion || rng.f32() < 0.015 {
            spawn(orb(0, if champion { 1000.0 } else { 30.0 }), pos + Vec3::X * 0.8);
        }
    }
}

/// Orbs in the pull's reach fly to the hero; levels bank as they land.
fn collect(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    hero: Query<&Transform, (With<Hero>, Without<Orb>)>,
    mut orbs: Query<(Entity, &mut Transform, &mut Orb)>,
) {
    let Ok(h) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let stats = run.stats();
    let goal = h.translation + Vec3::Y;
    for (e, mut tf, mut o) in &mut orbs {
        let d = goal - tf.translation;
        let dist = d.length();
        if !o.pulled && flat(d).length() <= stats.magnet {
            o.pulled = true;
        }
        if !o.pulled {
            continue;
        }
        if dist < 0.9 {
            run.xp += o.xp as f32 * waves::XP_RATE * stats.growth;
            run.hp = (run.hp + o.heal).min(stats.max_hp);
            commands.entity(e).despawn();
            continue;
        }
        o.speed += 30.0 * dt;
        tf.translation += d / dist * (o.speed * dt).min(dist);
    }
    while run.xp >= waves::xp_to_next(run.level) {
        run.xp -= waves::xp_to_next(run.level);
        run.level += 1;
        run.pending_levels += 1;
        if run.level == rules::kit::MOBILITY_LEVEL {
            let name = run.class.mobility.name;
            run.banner = Some((format!("You learned {name}! Press F."), 4.0));
        }
    }
    if run.pending_levels > 0 {
        next.set(Phase::LevelUp);
    }
}
