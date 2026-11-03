//! The match: two teams spawned at opposite gates, a countdown, then the fight. Every fighter is a
//! [`rules::unit::Unit`]; the computer ones think with [`rules::ai`], yours with your hands. An
//! ability that goes off is a [`Release`]: it moves its caster (charges, blinks), flies as a bolt
//! or lands at once, and what it does to its target comes from the rules. The last team standing
//! wins.

use bevy::prelude::*;
use rules::ai::{self, Brain, Seen};
use rules::arena::ArenaDef;
use rules::control::Ctl;
use rules::spells::{self, Aim, Role, School, Special, Spell, Use};
use rules::unit::{self, Feedback, Situation, Source, TargetInfo, Unit};

use crate::art::{self, Anim, Clip, character};
use crate::camera::Focus;
use crate::fx::{self, Fx};
use crate::{MatchEntity, Phase, Rng, Selection, Step, at, flat, pt, yaw};

pub fn plugin(app: &mut App) {
    app.add_message::<Release>()
        .add_message::<Arrive>()
        .init_resource::<Match>()
        .add_systems(OnEnter(Phase::Gates), spawn_match)
        .add_systems(OnEnter(Phase::Menu), clear)
        .add_systems(Update, gates.run_if(in_state(Phase::Gates)))
        .add_systems(Update, think.in_set(Step::Think).run_if(in_state(Phase::Fight)))
        .add_systems(Update, prep_buffs.in_set(Step::Think).run_if(in_state(Phase::Gates)))
        .add_systems(
            Update,
            (steer, dashes, gravity, advance, swings, resolve, fly, arrive, deaths, verdict)
                .chain()
                .in_set(Step::Resolve)
                .run_if(in_state(Phase::Fight).or(in_state(Phase::Gates))),
        )
        .add_systems(Update, animate.in_set(Step::Show));
}

/// Run speed, yards a second.
pub const SPEED: f32 = 7.0;
/// The models at this scale: a touch smaller than life against the arena.
pub const SCALE: f32 = 0.8;
pub const BODY: f32 = 0.6;
/// The time to buff up behind the gates; Enter cuts it short to [`READY_SECS`].
const GATES_SECS: f32 = 30.0;
pub const READY_SECS: f32 = 3.0;
const BOLT_SPEED: f32 = 40.0;
/// Yards a second: a charge, a leap, and a roll.
const CHARGE_SPEED: f32 = 30.0;
const LEAP_SPEED: f32 = 26.0;
const ROLL_SPEED: f32 = 20.0;

/// How a fighter is moving this frame, for its animation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stride {
    #[default]
    Still,
    Forward,
    Back,
    Left,
    Right,
}

#[derive(Component)]
pub struct Fighter {
    pub unit: Unit,
    pub team: usize,
    pub name: String,
    /// Fixed for the match: the fighter's place in the AI's view.
    pub slot: usize,
    pub target: Option<Entity>,
    /// The target a cast under way will land on.
    pub cast_target: Option<Entity>,
    /// Played by you (unless the autopilot drives).
    pub player: bool,
    pub brain: Brain,
    pub goal: Option<rules::Pt>,
    pub stride: Stride,
    /// Height above the floor while jumping, and the upward speed.
    pub air: f32,
    pub vy: f32,
    /// Under way on a charge, leap or roll: the body isn't its own until it's over.
    pub dash: Option<Dash>,
}

/// A body hurtling somewhere.
#[derive(Clone, Copy)]
pub struct Dash {
    /// Who it's after, or `None` for a spot on the floor.
    target: Option<Entity>,
    point: Vec3,
    speed: f32,
    /// What lands on arrival.
    arrive: Option<Arrive>,
    /// Seconds before it gives up (a pillar in the way).
    left: f32,
    /// Going backward, still facing forward.
    backward: bool,
}

impl Fighter {
    pub fn hostile_to(&self, other: &Fighter) -> bool {
        self.team != other.team
    }
}

#[derive(Resource, Default)]
pub struct Match {
    /// The fighters are in the arena.
    pub spawned: bool,
    /// The gates have opened.
    pub opened: bool,
    pub gates_t: f32,
    pub elapsed: f32,
    /// `Some(true)` when your team won.
    pub won: Option<bool>,
}

/// An ability going off: from a cast completing or an instant.
#[derive(Message, Clone, Copy)]
pub struct Release {
    pub caster: Entity,
    pub slot: usize,
    pub target: Option<Entity>,
}

/// A bolt reaching its target.
#[derive(Message, Clone, Copy)]
pub struct Arrive {
    caster: Entity,
    target: Entity,
    spell: &'static Spell,
    src: Source,
}

#[derive(Component)]
struct Bolt {
    arrive: Arrive,
}

/// A corpse, for the results to read and the camera to skip.
#[derive(Component)]
pub struct Fallen;

const NAMES: &[&str] = &[
    "Kaelith", "Brannoc", "Sefra", "Torvik", "Ilyse", "Morrow", "Dagna", "Vesk", "Orrin", "Thessaly",
    "Ruun", "Calder", "Ysolde", "Grimm", "Pell", "Anwen", "Jorath", "Mirela",
];

/// `n` more fighters for a side of `size`: a healer in every 3v3 side and half of the 2v2s.
fn comp(rng: &mut Rng, size: usize, n: usize, have_healer: bool) -> Vec<&'static spells::Class> {
    let healer = spells::CLASSES.iter().find(|c| c.role == Role::Healer).expect("a healer class");
    let others: Vec<&spells::Class> = spells::CLASSES.iter().filter(|c| c.role != Role::Healer).collect();
    let mut out = Vec::new();
    let want_healer = !have_healer && (size >= 3 || (size == 2 && rng.f32() < 0.5));
    if want_healer && n > 0 {
        out.push(healer);
    }
    while out.len() < n {
        out.push(others[rng.index(others.len())]);
    }
    out
}

fn spawn_match(
    mut commands: Commands,
    art: Res<art::Art>,
    fxa: Res<fx::FxAssets>,
    sel: Res<Selection>,
    mut rng: ResMut<Rng>,
    mut state: ResMut<Match>,
    mut cam: ResMut<crate::camera::Cam>,
) {
    // Back from a pause they're already here.
    if state.spawned {
        return;
    }
    *state = Match { spawned: true, gates_t: GATES_SECS, ..default() };
    let n = sel.bracket.clamp(1, 3);
    let me = sel.class();
    let mut ours = vec![me];
    ours.extend(comp(&mut rng, n, n - 1, me.role == Role::Healer));
    let theirs = comp(&mut rng, n, n, false);
    let mut names: Vec<&str> = NAMES.to_vec();
    let mut slot = 0;
    for (team, side) in [ours, theirs].into_iter().enumerate() {
        for (i, class) in side.into_iter().enumerate() {
            let player = team == 0 && i == 0;
            let name = if player { "You".to_string() } else { names.swap_remove(rng.index(names.len())).to_string() };
            let p = at(sel.arena().start(team, i, n));
            let facing = yaw(-flat(p).normalize_or(Vec3::Z));
            let ring = if team == 0 { fxa.team_gold.clone() } else { fxa.team_red.clone() };
            commands
                .spawn((
                    MatchEntity,
                    character(&art, class.model, class.gear),
                    Transform::from_translation(p).with_rotation(facing).with_scale(Vec3::splat(SCALE)),
                    Fighter {
                        unit: Unit::new(class),
                        team,
                        name,
                        slot,
                        target: None,
                        cast_target: None,
                        player,
                        brain: Brain::default(),
                        goal: None,
                        stride: Stride::Still,
                        air: 0.0,
                        vy: 0.0,
                        dash: None,
                    },
                ))
                .observe(art::on_ready)
                .with_children(|c| {
                    c.spawn((
                        Mesh3d(fxa.ring.clone()),
                        MeshMaterial3d(ring),
                        Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::splat(1.0)),
                    ));
                });
            if player {
                // Behind you, looking where you face: toward the other gate.
                cam.yaw = if team == 0 { 0.0 } else { std::f32::consts::PI };
            }
            slot += 1;
        }
    }
}

fn clear(mut commands: Commands, mut state: ResMut<Match>, spawned: Query<Entity, With<MatchEntity>>) {
    *state = Match::default();
    for e in &spawned {
        commands.entity(e).despawn();
    }
}

/// The countdown; when it ends everyone is made whole and the gates open.
fn gates(time: Res<Time>, mut state: ResMut<Match>, mut fighters: Query<&mut Fighter>, mut next: ResMut<NextState<Phase>>) {
    state.gates_t -= time.delta_secs();
    if state.gates_t <= 0.0 {
        state.opened = true;
        state.elapsed = 0.0;
        for mut f in &mut fighters {
            f.unit.prepare();
        }
        next.set(Phase::Fight);
    }
}

/// Behind the gates the computer's fighters buff up: their own buffs, and the team's on anyone
/// missing them.
fn prep_buffs(auto: Option<Res<crate::auto::Autopilot>>, mut fighters: Query<(Entity, &mut Fighter)>, mut out: MessageWriter<Release>) {
    let worn: Vec<(usize, Vec<&'static str>)> =
        fighters.iter().map(|(_, f)| (f.team, f.unit.auras.iter().map(|a| a.name).collect())).collect();
    for (me, mut f) in &mut fighters {
        if (f.player && auto.is_none()) || f.unit.gcd > 0.0 {
            continue;
        }
        let first = f.unit.class.spells.len();
        for (k, b) in f.unit.class.buffs.iter().enumerate() {
            let needed = if b.aim == Aim::Party {
                worn.iter().any(|(team, names)| *team == f.team && !names.contains(&b.name))
            } else {
                !f.unit.auras.iter().any(|a| a.name == b.name)
            };
            if needed {
                start(&mut f, me, first + k, Some(me), &mut out);
                break;
            }
        }
    }
}

/// What a fighter's cast would face: its target's side, distance, sight and bearing.
pub fn situation(
    arena: &ArenaDef,
    me: (&Fighter, &Transform),
    target: Option<(&Fighter, &Transform)>,
    moving: bool,
    turn_free: bool,
) -> Situation {
    let (f, tf) = me;
    Situation {
        target: target.map(|(t, ttf)| {
            let to = flat(ttf.translation - tf.translation);
            let dist = (to.length() - BODY).max(0.0);
            let ahead = turn_free || dist < 1.5 || (tf.rotation * Vec3::Z).dot(to.normalize_or_zero()) > 0.0;
            TargetInfo {
                hostile: f.hostile_to(t),
                dist,
                sight: arena.sight(pt(tf.translation), pt(ttf.translation)),
                ahead,
                dead: t.unit.dead,
            }
        }),
        moving,
    }
}

/// Start `slot` on `target` if the rules allow: a cast bar, or an instant [`Release`].
pub fn start(f: &mut Fighter, me: Entity, slot: usize, target: Option<Entity>, out: &mut MessageWriter<Release>) {
    if f.unit.begin(slot) {
        out.write(Release { caster: me, slot, target });
    } else {
        f.cast_target = target;
    }
}

#[allow(clippy::type_complexity)]
fn think(
    time: Res<Time>,
    sel: Res<Selection>,
    auto: Option<Res<crate::auto::Autopilot>>,
    mut rng: ResMut<Rng>,
    mut fighters: Query<(Entity, &mut Fighter, &mut Transform)>,
    mut out: MessageWriter<Release>,
) {
    let dt = time.delta_secs();
    let arena = sel.arena();
    let mut list: Vec<(usize, Entity, Seen)> = fighters
        .iter()
        .map(|(e, f, tf)| {
            let u = &f.unit;
            (
                f.slot,
                e,
                Seen {
                    team: f.team,
                    pos: pt(tf.translation),
                    hp: u.hp_frac(),
                    dead: u.dead,
                    role: u.class.role,
                    casting: u.casting.map(|c| 1.0 - c.left / c.total),
                    out: u.cc.out(),
                    cleansable: u.cc.fear_t > 0.0 || u.cc.incap_t > 0.0 || u.cc.root_t > 0.0 || u.cc.silence_t > 0.0,
                    slowed: u.cc.slow_t > 0.0,
                },
            )
        })
        .collect();
    list.sort_by_key(|x| x.0);
    let seen: Vec<Seen> = list.iter().map(|x| x.2).collect();
    let ents: Vec<Entity> = list.iter().map(|x| x.1).collect();
    for i in 0..ents.len() {
        let me = ents[i];
        let Ok((_, mut f, mut tf)) = fighters.get_mut(me) else { continue };
        if f.unit.dead || (f.player && auto.is_none()) {
            continue;
        }
        let f = &mut *f;
        let plan = ai::think(i, &f.unit, &seen, arena, &mut f.brain, dt, rng.f32());
        if plan.trinket {
            f.unit.trinket();
        }
        f.target = f.brain.target.map(|t| ents[t]);
        // The computer's fighters keep swinging at whoever they're after.
        f.unit.attacking = f.target.is_some();
        f.goal = plan.goal;
        if let Some((slot, on)) = plan.cast {
            let target = ents[on];
            // The AI turns to its target as it casts.
            if target != me {
                let to = flat(at(seen[on].pos) - tf.translation);
                if to.length() > 0.1 {
                    tf.rotation = yaw(to);
                }
            }
            f.goal = if f.unit.spell(slot).cast > 0.0 { None } else { f.goal };
            start(f, me, slot, Some(target), &mut out);
        }
    }
}

/// The computer fighters walk to their goals (or flee in fear), facing their target when still.
fn steer(
    time: Res<Time>,
    sel: Res<Selection>,
    auto: Option<Res<crate::auto::Autopilot>>,
    mut fighters: Query<(&mut Fighter, &mut Transform)>,
    targets: Query<&GlobalTransform, With<Fighter>>,
    mut focus: ResMut<Focus>,
) {
    let dt = time.delta_secs();
    let arena = sel.arena();
    for (mut f, mut tf) in &mut fighters {
        if f.player {
            focus.0 = tf.translation;
        }
        if f.unit.dead || f.dash.is_some() || (f.player && auto.is_none()) {
            continue;
        }
        let pos = tf.translation;
        let cc = f.unit.cc;
        let goal = if cc.fear_t > 0.0 {
            Some(pt(pos + flat(pos - at(cc.from)).normalize_or(Vec3::Z) * 5.0))
        } else if cc.held() || cc.out() || f.unit.casting.is_some() {
            None
        } else {
            f.goal
        };
        f.stride = Stride::Still;
        if let Some(g) = goal {
            let to = flat(at(g) - pos);
            if to.length() > 0.5 {
                let dir = to.normalize();
                let step = SPEED * cc.speed() * if cc.fear_t > 0.0 { 0.7 } else { 1.0 } * dt;
                tf.translation = at(arena.confine(pt(pos + dir * step.min(to.length())), BODY));
                tf.rotation = tf.rotation.slerp(yaw(dir), (dt * 12.0).min(1.0));
                f.stride = Stride::Forward;
            }
        }
        if f.stride == Stride::Still {
            if let Some(t) = f.target.and_then(|t| targets.get(t).ok()) {
                let to = flat(t.translation() - pos);
                if to.length() > 0.1 {
                    tf.rotation = tf.rotation.slerp(yaw(to), (dt * 10.0).min(1.0));
                }
            }
        }
    }
}

/// Charges, leaps and rolls carry their bodies along; a stun or a root stops them dead, and what
/// a charge carries lands when it arrives.
fn dashes(
    time: Res<Time>,
    sel: Res<Selection>,
    mut fighters: Query<(Entity, &mut Fighter, &mut Transform)>,
    mut arrivals: MessageWriter<Arrive>,
) {
    let dt = time.delta_secs();
    let arena = sel.arena();
    let ids: Vec<Entity> = fighters.iter().filter(|(_, f, _)| f.dash.is_some()).map(|x| x.0).collect();
    for me in ids {
        let Ok((_, f, _)) = fighters.get(me) else { continue };
        let Some(mut d) = f.dash else { continue };
        let stopped = f.unit.dead || f.unit.cc.out() || f.unit.cc.root_t > 0.0;
        let chased = d.target.and_then(|t| fighters.get(t).ok()).map(|(_, o, t)| (o.unit.dead, t.translation));
        if let Some((_, p)) = chased {
            d.point = p;
        }
        let lost = chased.is_some_and(|(dead, _)| dead) || (d.target.is_some() && chased.is_none());
        let Ok((_, mut f, mut tf)) = fighters.get_mut(me) else { continue };
        d.left -= dt;
        if stopped || lost || d.left <= 0.0 {
            f.dash = None;
            continue;
        }
        let to = flat(d.point - tf.translation);
        let step = d.speed * dt;
        // Up to arm's length of a target, or onto a spot.
        let stop = if d.target.is_some() { 1.5 } else { 0.1 };
        if to.length() <= stop + step {
            if let Some(a) = d.arrive {
                arrivals.write(a);
            }
            f.dash = None;
            continue;
        }
        let dir = to.normalize();
        tf.translation = at(arena.confine(pt(tf.translation + dir * step), BODY));
        f.stride = if d.backward { Stride::Back } else { Stride::Forward };
        if !d.backward {
            tf.rotation = yaw(dir);
        }
        f.dash = Some(d);
    }
}

/// Pulls you down from a jump.
pub const GRAVITY: f32 = 20.0;

/// Jumps rise and fall; whatever moved a fighter along the floor, it stands at its jump height.
fn gravity(time: Res<Time>, mut fighters: Query<(&mut Fighter, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut f, mut tf) in &mut fighters {
        if f.air > 0.0 || f.vy > 0.0 {
            f.vy -= GRAVITY * dt;
            f.air += f.vy * dt;
            if f.air <= 0.0 {
                f.air = 0.0;
                f.vy = 0.0;
            }
        }
        tf.translation.y = f.air;
    }
}

/// Casts run down (moving or jumping breaks them), auras tick, and a finished cast goes off.
fn advance(
    time: Res<Time>,
    mut state: ResMut<Match>,
    mut fighters: Query<(Entity, &mut Fighter, &Transform)>,
    mut out: MessageWriter<Release>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    state.elapsed += dt;
    for (e, mut f, tf) in &mut fighters {
        let moving = f.stride != Stride::Still || f.air > 0.0;
        if let Some(slot) = f.unit.advance(dt, moving) {
            let target = f.cast_target.take();
            out.write(Release { caster: e, slot, target });
        }
        for fb in f.unit.tick(dt) {
            show(&mut fx, tf.translation, fb);
        }
    }
}

/// White hits: anyone attacking swings when the timer's up and the target's in reach, a blade at
/// once, an auto shot as a bolt. They stop when the target's dead or gone.
#[allow(clippy::too_many_arguments)]
fn swings(
    phase: Res<State<Phase>>,
    sel: Res<Selection>,
    mut rng: ResMut<Rng>,
    mut fighters: Query<(Entity, &mut Fighter, &Transform, &mut Anim)>,
    mut arrivals: MessageWriter<Arrive>,
    mut fx: Fx,
) {
    if *phase.get() != Phase::Fight {
        return;
    }
    let ids: Vec<Entity> = fighters.iter().map(|x| x.0).collect();
    for me in ids {
        let Ok((_, f, tf, _)) = fighters.get(me) else { continue };
        let spell = f.unit.class.auto;
        if !f.unit.attacking {
            continue;
        }
        let target = f.target;
        let info = target.and_then(|t| fighters.get(t).ok()).map(|(_, o, otf, _)| (o, otf));
        let gone = info.is_none_or(|(o, _)| o.unit.dead || o.team == f.team);
        let moving = f.stride != Stride::Still || f.air > 0.0;
        // The computer's fighters turn to their target as they swing.
        let s = situation(sel.arena(), (f, tf), info, moving, !f.player);
        let ready = f.unit.can_swing(s);
        let from = tf.translation;
        let team_src = Source {
            dealt: f.unit.dealt(),
            crit: rng.f32() < f.unit.crit_chance(),
            from: pt(from),
            hostile: true,
        };
        let gear = f.unit.class.gear;
        let Ok((_, mut f, _, mut anim)) = fighters.get_mut(me) else { continue };
        if gone {
            f.unit.attacking = false;
            continue;
        }
        let Some(target) = target.filter(|_| ready) else { continue };
        f.unit.swung();
        anim.play(Clip::attack(gear), 0.5);
        let arrive = Arrive { caster: me, target, spell, src: team_src };
        if spell.range > 6.0 {
            fx.glow_sphere(
                Bolt { arrive },
                spell.school,
                Transform::from_translation(from + Vec3::Y * 1.2).with_scale(Vec3::splat(0.12)),
            );
        } else {
            arrivals.write(arrive);
        }
    }
}

/// Floating text for what happened to a fighter at `pos`.
pub fn show(fx: &mut Fx, pos: Vec3, fb: Feedback) {
    match fb {
        Feedback::Damage { amount, crit, school } => {
            let color = if crit { fx::CRIT } else { fx::color(school) };
            let text = if crit { format!("{}!", amount.round()) } else { format!("{}", amount.round()) };
            fx.text(pos, text, color, if crit { 26.0 } else { 18.0 });
        }
        Feedback::Absorbed(a) => fx.text(pos, format!("Absorb {}", a.round()), fx::WHITE, 15.0),
        Feedback::Heal(h) if h >= 1.0 => fx.text(pos, format!("+{}", h.round()), fx::HEAL, 18.0),
        Feedback::Heal(_) => {}
        Feedback::Immune => fx.text(pos, "Immune", fx::WHITE, 18.0),
        // A control lands with a burst in its colour, and its name large.
        Feedback::Ctl(c @ Ctl::Slow(..)) => {
            fx.nova(pos, School::Frost, 1.8);
            fx.text(pos, c.name(), fx::color(School::Frost), 17.0);
        }
        Feedback::Ctl(c) => {
            let school = match c {
                Ctl::Stun(_) => School::Holy,
                Ctl::Root(_) => School::Frost,
                Ctl::Fear(_) => School::Shadow,
                _ => School::Arcane,
            };
            fx.nova(pos, school, 3.5);
            fx.flash(pos, school, 2.2);
            fx.text(pos, c.name(), fx::color(school), 26.0);
        }
        Feedback::Interrupted => {
            fx.nova(pos, School::Arcane, 3.0);
            fx.text(pos, "Interrupted", fx::CRIT, 26.0);
        }
        Feedback::Cleansed => fx.text(pos, "Cleansed", fx::HEAL, 18.0),
    }
}

/// An ability goes off: it moves its caster if it must, then lands on its targets now or as a bolt.
#[allow(clippy::too_many_arguments)]
fn resolve(
    sel: Res<Selection>,
    mut rng: ResMut<Rng>,
    mut releases: MessageReader<Release>,
    mut fighters: Query<(&mut Fighter, &mut Transform, &mut Anim)>,
    mut arrivals: MessageWriter<Arrive>,
    mut fx: Fx,
) {
    let arena = sel.arena();
    let list: Vec<Release> = releases.read().copied().collect();
    for r in list {
        let Ok((mut f, mut tf, mut anim)) = fighters.get_mut(r.caster) else { continue };
        if f.unit.dead {
            continue;
        }
        let spell = f.unit.spell(r.slot);
        let src = Source {
            dealt: f.unit.dealt(),
            crit: rng.f32() < f.unit.crit_chance(),
            from: pt(tf.translation),
            hostile: true,
        };
        let team = f.team;
        let me = tf.translation;
        let melee = spell.range <= 6.0;
        anim.play(
            if melee && spell.aim == Aim::Enemy { Clip::attack(f.unit.class.gear) } else { Clip::Cast },
            0.6,
        );
        if spell.aim == Aim::Myself {
            f.unit.on_self(spell);
            let fwd = flat(tf.rotation * Vec3::Z).normalize_or(Vec3::Z);
            match spell.special {
                Special::Blink(d) => {
                    fx.flash(me, spell.school, 2.0);
                    tf.translation = at(arena.confine(pt(me + fwd * d), BODY));
                    fx.flash(tf.translation, spell.school, 2.0);
                }
                Special::Leap(d) => {
                    // A roll back: the body goes there, still facing the fight.
                    let spot = at(arena.confine(pt(me - fwd * d), BODY));
                    f.dash = Some(Dash { target: None, point: spot, speed: ROLL_SPEED, arrive: None, left: 2.0, backward: true });
                    anim.play(Clip::Roll, d / ROLL_SPEED);
                }
                _ => fx.nova(me, spell.school, 2.0),
            }
            let (fb, _) = unit::land(spell, Source { hostile: false, ..src }, &mut f.unit);
            for b in fb {
                show(&mut fx, me, b);
            }
            continue;
        }
        drop(f);
        // Who it lands on.
        let mut targets: Vec<Entity> = Vec::new();
        match spell.aim {
            Aim::Around => fx.nova(me, spell.school, spell.radius),
            Aim::Party => fx.nova(me, spell.school, 4.0),
            Aim::Friend => {
                let friendly = r.target.and_then(|t| fighters.get(t).ok()).is_some_and(|(o, _, _)| o.team == team && !o.unit.dead);
                targets.push(if friendly { r.target.unwrap_or(r.caster) } else { r.caster });
            }
            Aim::Enemy => targets.extend(r.target),
            Aim::Myself => {}
        }
        for t in targets {
            land_or_fly(&mut fighters, r.caster, t, spell, src, me, &mut arrivals, &mut fx, arena);
        }
        if spell.aim == Aim::Around {
            around(&mut fighters, spell, src, team, me, arena, &mut fx);
        }
        if spell.aim == Aim::Party {
            for (mut o, otf, _) in fighters.iter_mut() {
                if o.team == team && !o.unit.dead {
                    let (fb, _) = unit::land(spell, Source { hostile: false, ..src }, &mut o.unit);
                    fx.flash(otf.translation, spell.school, 1.5);
                    for b in fb {
                        show(&mut fx, otf.translation, b);
                    }
                }
            }
        }
    }
}

/// The caster's moves for an aimed ability (charges, steps), then the landing: a bolt for ranged
/// harm, at once otherwise.
#[allow(clippy::too_many_arguments)]
fn land_or_fly(
    fighters: &mut Query<(&mut Fighter, &mut Transform, &mut Anim)>,
    caster: Entity,
    target: Entity,
    spell: &'static Spell,
    src: Source,
    from: Vec3,
    arrivals: &mut MessageWriter<Arrive>,
    fx: &mut Fx,
    arena: &ArenaDef,
) {
    let Ok((tf_t, team_t, dead)) = fighters.get(target).map(|(f, t, _)| (*t, f.team, f.unit.dead)) else { return };
    if dead {
        return;
    }
    let hostile = fighters.get(caster).is_ok_and(|(f, _, _)| f.team != team_t);
    let src = Source { hostile, ..src };
    let arrive = Arrive { caster, target, spell, src };
    match spell.special {
        // A charge or a leap goes there under its own power, and lands on arrival.
        Special::Charge | Special::LeapTo => {
            if let Ok((mut f, tf, mut anim)) = fighters.get_mut(caster) {
                let leap = spell.special == Special::LeapTo;
                let speed = if leap { LEAP_SPEED } else { CHARGE_SPEED };
                if leap {
                    let t = flat(tf_t.translation - tf.translation).length() / speed;
                    f.vy = GRAVITY * t * 0.5;
                    f.air = 0.01;
                    anim.play(Clip::Jump, t);
                }
                f.dash = Some(Dash { target: Some(target), point: tf_t.translation, speed, arrive: Some(arrive), left: 2.5, backward: false });
            }
            return;
        }
        // A step through the shadows really is a teleport.
        Special::Behind => {
            if let Ok((_, mut tf, _)) = fighters.get_mut(caster) {
                let spot = tf_t.translation - flat(tf_t.rotation * Vec3::Z).normalize_or(Vec3::Z) * 1.5;
                fx.flash(tf.translation, spell.school, 1.5);
                tf.translation = at(arena.confine(pt(spot), BODY));
                tf.rotation = yaw(flat(tf_t.translation - tf.translation));
            }
        }
        _ => {}
    }
    let ranged = hostile && spell.range > 6.0 && spell.special != Special::Behind;
    if ranged {
        fx.glow_sphere(
            Bolt { arrive },
            spell.school,
            Transform::from_translation(from + Vec3::Y * 1.2).with_scale(Vec3::splat(0.22)),
        );
    } else {
        arrivals.write(arrive);
    }
}

#[allow(clippy::too_many_arguments)]
fn around(
    fighters: &mut Query<(&mut Fighter, &mut Transform, &mut Anim)>,
    spell: &'static Spell,
    src: Source,
    team: usize,
    me: Vec3,
    arena: &ArenaDef,
    fx: &mut Fx,
) {
    for (mut o, otf, _) in fighters.iter_mut() {
        if o.team == team || o.unit.dead {
            continue;
        }
        let d = flat(otf.translation - me).length();
        if d > spell.radius + BODY || !arena.sight(pt(me), pt(otf.translation)) {
            continue;
        }
        let (fb, _) = unit::land(spell, src, &mut o.unit);
        for b in fb {
            show(fx, otf.translation, b);
        }
    }
}

/// Bolts home on their target and arrive.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Transform, &Bolt), Without<Fighter>>,
    targets: Query<&Transform, With<Fighter>>,
    mut arrivals: MessageWriter<Arrive>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, b) in &mut bolts {
        let Ok(t) = targets.get(b.arrive.target) else {
            commands.entity(e).despawn();
            continue;
        };
        let goal = t.translation + Vec3::Y * 1.0;
        let d = goal - tf.translation;
        let step = BOLT_SPEED * dt;
        if d.length() <= step.max(0.4) {
            arrivals.write(b.arrive);
            commands.entity(e).despawn();
        } else {
            tf.translation += d.normalize() * step;
        }
    }
}

/// What an ability does to its target, through the rules; a drain heals its caster.
fn arrive(
    mut arrivals: MessageReader<Arrive>,
    mut fighters: Query<(&mut Fighter, &Transform)>,
    mut fx: Fx,
) {
    for a in arrivals.read() {
        let (fb, pos) = if a.caster == a.target {
            let Ok((mut f, tf)) = fighters.get_mut(a.target) else { continue };
            let (fb, d) = unit::land(a.spell, a.src, &mut f.unit);
            f.unit.mend(d);
            (fb, tf.translation)
        } else {
            let Ok([(mut c, _), (mut t, ttf)]) = fighters.get_many_mut([a.caster, a.target]) else { continue };
            let (fb, d) = unit::land(a.spell, a.src, &mut t.unit);
            for b in &fb {
                match *b {
                    Feedback::Damage { amount, .. } => c.unit.damage_done += amount,
                    Feedback::Heal(h) => c.unit.healing_done += h,
                    _ => {}
                }
            }
            if d > 0.0 {
                c.unit.mend(d);
            }
            (fb, ttf.translation)
        };
        fx.flash(pos, a.spell.school, if a.spell.ai == Use::Auto { 0.6 } else { 1.3 });
        for b in fb {
            show(&mut fx, pos, b);
        }
    }
}

fn deaths(mut commands: Commands, mut fighters: Query<(Entity, &Fighter, &mut Anim), Without<Fallen>>) {
    for (e, f, mut anim) in &mut fighters {
        if f.unit.dead {
            anim.die();
            commands.entity(e).insert(Fallen);
        }
    }
}

fn verdict(
    fighters: Query<&Fighter>,
    mut state: ResMut<Match>,
    mut next: ResMut<NextState<Phase>>,
    mut anims: Query<(&Fighter, &mut Anim)>,
) {
    if state.won.is_some() {
        return;
    }
    let alive = |team: usize| fighters.iter().any(|f| f.team == team && !f.unit.dead);
    let won = match (alive(0), alive(1)) {
        (true, false) => true,
        (false, _) => false,
        _ => return,
    };
    state.won = Some(won);
    let winners = if won { 0 } else { 1 };
    for (f, mut a) in &mut anims {
        if f.team == winners && !f.unit.dead {
            a.play(Clip::Cheer, 4.0);
        }
    }
    next.set(Phase::Results);
}

/// Each fighter's looping animation: running, strafing, backing up, casting, or standing.
fn animate(mut fighters: Query<(&Fighter, &mut Anim)>) {
    for (f, mut anim) in &mut fighters {
        anim.stride_speed = if f.dash.is_some() { 1.8 } else { f.unit.cc.speed() };
        anim.base = if f.unit.cc.stun_t > 0.0 {
            Clip::Stunned
        } else if f.unit.cc.incap_t > 0.0 {
            Clip::Incapped
        } else if f.unit.casting.is_some() {
            Clip::Casting
        } else {
            match f.stride {
                Stride::Still => Clip::Idle,
                Stride::Forward => Clip::Run,
                Stride::Back => Clip::Back,
                Stride::Left => Clip::StrafeLeft,
                Stride::Right => Clip::StrafeRight,
            }
        };
    }
}
