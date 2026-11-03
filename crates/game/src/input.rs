//! Your hands: WoW-style movement and camera, targeting, and the action bar. Every key comes from
//! [`Settings`] (rebindable); the defaults:
//!
//! - W / S: forward and back; A / D: strafe; Space: jump; both mouse buttons: run forward.
//! - Right drag: turn the camera and your character with it. Left drag: look around freely.
//! - Wheel: zoom. Tab: next enemy. Click a fighter: target it. F1-F3: yourself and your party.
//! - 1-0: your abilities. Q: trinket. Esc: clear target, then pause.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rules::spells::{Aim, Use};
use rules::unit::Fail;

use crate::camera::{Cam, MainCam, PITCH_MAX, PITCH_MIN};
use crate::fight::{BODY, Fighter, Match, READY_SECS, Release, SPEED, Stride, situation, start};
use crate::art::{Anim, Clip};
use crate::fx::{self, Fx};
use crate::settings::{Action, Overlay, Settings};
use crate::{Phase, Selection, Step, at, flat, pt, yaw};

pub fn plugin(app: &mut App) {
    app.init_resource::<Errors>()
        .init_resource::<Drag>()
        .add_message::<Press>()
        .add_systems(Update, (camera, move_player, targeting, cast_keys).chain().in_set(Step::Input).run_if(playing))
        .add_systems(Update, pause_keys.before(Step::Input));
}

fn playing(phase: Res<State<Phase>>, auto: Option<Res<crate::auto::Autopilot>>) -> bool {
    matches!(phase.get(), Phase::Gates | Phase::Fight) && auto.is_none()
}

/// The red line at the top of the screen when an ability can't go off, and how long it shows.
#[derive(Resource, Default)]
pub struct Errors(pub Option<(String, f32)>);

/// How far the mouse travelled while a button was held, to tell a click from a drag.
#[derive(Resource, Default)]
struct Drag(f32);

/// The auto attack's slot on the bar; the trinket's is `usize::MAX`.
pub const AUTO_SLOT: usize = usize::MAX - 1;

/// An action bar slot clicked with the mouse (`usize::MAX` for the trinket).
#[derive(Message, Clone, Copy)]
pub struct Press(pub usize);

const TURN: f32 = 0.006;
/// A jump's upward speed, yards a second.
pub const JUMP: f32 = 8.0;

fn camera(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut cam: ResMut<Cam>,
    mut drag: ResMut<Drag>,
    mut fighters: Query<(&Fighter, &mut Transform)>,
) {
    let held = mouse.any_pressed([MouseButton::Left, MouseButton::Right]);
    if mouse.any_just_pressed([MouseButton::Left, MouseButton::Right]) {
        drag.0 = 0.0;
    }
    if held {
        drag.0 += motion.delta.length();
        cam.yaw -= motion.delta.x * TURN;
        cam.pitch = (cam.pitch + motion.delta.y * TURN).clamp(PITCH_MIN, PITCH_MAX);
    }
    // Right button: your character turns with the camera.
    if mouse.pressed(MouseButton::Right) {
        if let Some((_, mut tf)) = fighters.iter_mut().find(|(f, _)| f.player && !f.unit.dead) {
            tf.rotation = yaw(cam.forward());
        }
    }
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    sel: Res<Selection>,
    settings: Res<Settings>,
    state: Res<Match>,
    mut fighters: Query<(&mut Fighter, &mut Transform, &mut Anim)>,
) {
    let dt = time.delta_secs();
    let Some((mut f, mut tf, mut anim)) = fighters.iter_mut().find(|(f, _, _)| f.player) else { return };
    f.stride = Stride::Still;
    if f.unit.dead || f.dash.is_some() {
        return;
    }
    let fwd = flat(tf.rotation * Vec3::Z).normalize_or(Vec3::Z);
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    let both = mouse.pressed(MouseButton::Left) && mouse.pressed(MouseButton::Right);
    let mut dir = Vec3::ZERO;
    if settings.pressed(&keys, Action::Forward) || both {
        dir += fwd;
    }
    if settings.pressed(&keys, Action::Back) {
        dir -= fwd;
    }
    // Strafing runs along the screen's sideways line.
    if settings.pressed(&keys, Action::StrafeLeft) {
        dir -= right;
    }
    if settings.pressed(&keys, Action::StrafeRight) {
        dir += right;
    }
    let cc = f.unit.cc;
    // A jump from the ground, unless something holds you.
    if settings.just(&keys, Action::Jump) && f.air <= 0.0 && !cc.held() && !cc.out() {
        f.vy = JUMP;
        f.air = 0.01;
        anim.play(Clip::Jump, 0.9);
    }
    if cc.fear_t > 0.0 {
        dir = flat(tf.translation - at(cc.from)).normalize_or(fwd);
        tf.rotation = yaw(dir);
    } else if cc.held() || cc.out() {
        return;
    }
    if dir == Vec3::ZERO {
        return;
    }
    let back = dir.dot(fwd) < -0.1;
    let pace = if back { 0.6 } else if cc.fear_t > 0.0 { 0.7 } else { 1.0 };
    let step = dir.normalize() * SPEED * cc.speed() * pace * dt;
    let mut next = sel.arena().confine(pt(tf.translation + step), BODY);
    // Behind the gates you stay in your pen.
    if !state.opened {
        next = sel.arena().confine_pen(next, f.team, BODY);
    }
    tf.translation = at(next);
    let side = dir.dot(right);
    f.stride = if back {
        Stride::Back
    } else if dir.dot(fwd) > 0.3 {
        Stride::Forward
    } else if side < 0.0 {
        Stride::Left
    } else {
        Stride::Right
    };
}

#[allow(clippy::too_many_arguments)]
fn targeting(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<Settings>,
    mouse: Res<ButtonInput<MouseButton>>,
    drag: Res<Drag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    ui: Query<&Interaction>,
    mut fighters: Query<(Entity, &mut Fighter, &Transform)>,
) {
    let Some((me, my_team, my_pos, my_fwd)) = fighters
        .iter()
        .find(|(_, f, _)| f.player)
        .map(|(e, f, t)| (e, f.team, t.translation, flat(t.rotation * Vec3::Z)))
    else {
        return;
    };
    let mut pick: Option<Option<Entity>> = None;
    let mut attack = false;
    if settings.just(&keys, Action::NextTarget) {
        // The nearest living enemy in front, cycling past the current target.
        let current = fighters.get(me).ok().and_then(|(_, f, _)| f.target);
        let mut foes: Vec<(f32, Entity)> = fighters
            .iter()
            .filter(|(_, f, _)| f.team != my_team && !f.unit.dead)
            .map(|(e, _, t)| {
                let to = flat(t.translation - my_pos);
                let behind = if to.normalize_or_zero().dot(my_fwd) < 0.0 { 1000.0 } else { 0.0 };
                (to.length() + behind, e)
            })
            .collect();
        foes.sort_by(|a, b| a.0.total_cmp(&b.0));
        let next = match current.and_then(|c| foes.iter().position(|x| x.1 == c)) {
            Some(i) => foes.get(i + 1).or(foes.first()),
            None => foes.first(),
        };
        pick = next.map(|x| Some(x.1));
    }
    for (i, a) in [Action::TargetSelf, Action::TargetParty1, Action::TargetParty2].into_iter().enumerate() {
        if settings.just(&keys, a) {
            let mut party: Vec<(usize, Entity)> = fighters
                .iter()
                .filter(|(_, f, _)| f.team == my_team)
                .map(|(e, f, _)| (if f.player { 0 } else { f.slot + 1 }, e))
                .collect();
            party.sort();
            pick = party.get(i).map(|x| Some(x.1));
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        pick = Some(None);
    }
    // A click (not a drag, not on the HUD) on a fighter targets it; a right click on an enemy
    // attacks it too.
    let on_ui = ui.iter().any(|i| *i != Interaction::None);
    let right = mouse.just_released(MouseButton::Right);
    if (mouse.just_released(MouseButton::Left) || right) && drag.0 < 6.0 && !on_ui {
        if let (Some(cursor), Ok((cam, cam_tf))) =
            (windows.single().ok().and_then(|w| w.cursor_position()), cams.single())
        {
            let hit = fighters
                .iter()
                .filter_map(|(e, _, t)| {
                    let p = cam.world_to_viewport(cam_tf, t.translation + Vec3::Y * 1.0).ok()?;
                    let d = p.distance(cursor);
                    (d < 45.0).then_some((d, e))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, e)) = hit {
                pick = Some(Some(e));
                attack = right && fighters.get(e).is_ok_and(|(_, f, _)| f.team != my_team);
            }
        }
    }
    if let Some(t) = pick {
        if let Ok((_, mut f, _)) = fighters.get_mut(me) {
            f.target = t;
            if t.is_none() {
                f.unit.attacking = false;
            }
            if attack {
                f.unit.attacking = true;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_keys(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<Settings>,
    phase: Res<State<Phase>>,
    sel: Res<Selection>,
    mut presses: MessageReader<Press>,
    mut errors: ResMut<Errors>,
    mut fighters: Query<(Entity, &mut Fighter, &Transform)>,
    mut out: MessageWriter<Release>,
    mut fx: Fx,
) {
    if let Some((_, t)) = errors.0.as_mut() {
        *t -= time.delta_secs();
        if *t <= 0.0 {
            errors.0 = None;
        }
    }
    let mut want: Vec<usize> = presses.read().map(|p| p.0).collect();
    want.extend((0..10).filter(|&i| settings.just(&keys, Action::Ability(i))));
    want.extend((0..2).filter(|&i| settings.just(&keys, Action::Buff(i))).map(|i| 10 + i));
    if settings.just(&keys, Action::Trinket) {
        want.push(usize::MAX);
    }
    if settings.just(&keys, Action::Attack) {
        want.push(AUTO_SLOT);
    }
    if want.is_empty() {
        return;
    }
    let Some(me) = fighters.iter().find(|(_, f, _)| f.player).map(|(e, _, _)| e) else { return };
    let fail = |errors: &mut Errors, text: &str| errors.0 = Some((text.to_string(), 2.0));
    for slot in want {
        let Ok((_, f, tf)) = fighters.get(me) else { return };
        if slot == AUTO_SLOT {
            let starting = !f.unit.attacking;
            let target_ok = f.target.and_then(|t| fighters.get(t).ok()).is_some_and(|(_, o, _)| o.team != f.team && !o.unit.dead);
            if starting && !target_ok {
                fail(&mut errors, Fail::NoTarget.text());
            } else if let Ok((_, mut f, _)) = fighters.get_mut(me) {
                f.unit.attacking = starting;
            }
            continue;
        }
        if slot == usize::MAX {
            let pos = tf.translation;
            if let Ok((_, mut f, _)) = fighters.get_mut(me) {
                if f.unit.trinket() {
                    fx.nova(pos, rules::spells::School::Holy, 3.0);
                    fx.text(pos, "Free!", fx::HEAL, 22.0);
                } else if f.unit.trinket_cd > 0.0 {
                    fail(&mut errors, Fail::NotReady.text());
                }
            }
            continue;
        }
        if *phase.get() == Phase::Gates && f.unit.spell(slot).ai != Use::Buff {
            fail(&mut errors, "The gates have not opened yet");
            continue;
        }
        let target = f.target;
        let moving = f.stride != Stride::Still || f.air > 0.0;
        let target_info = target.and_then(|t| fighters.get(t).ok()).map(|(_, tf_f, ttf)| (tf_f, ttf));
        let s = situation(sel.arena(), (f, tf), target_info, moving, false);
        match f.unit.check(slot, s) {
            Ok(()) => {
                if let Ok((_, mut f, _)) = fighters.get_mut(me) {
                    // A melee ability starts the swings and a shot the auto shot; a spell starts
                    // nothing, so a caster won't break their own controls by standing close.
                    let spell = f.unit.spell(slot);
                    if spell.aim == Aim::Enemy && (spell.range <= 6.0 || f.unit.class.auto.range > 6.0) {
                        f.unit.attacking = true;
                    }
                    start(&mut f, me, slot, target, &mut out);
                }
            }
            Err(e) => fail(&mut errors, e.text()),
        }
    }
}

fn pause_keys(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    fighters: Query<&Fighter>,
    mut state: ResMut<Match>,
    mut overlay: ResMut<Overlay>,
    mut next: ResMut<NextState<Phase>>,
) {
    // The Keybinds screen takes Esc first: to cancel a capture, else to close.
    if overlay.open {
        if keys.just_pressed(KeyCode::Escape) && !overlay.capturing() {
            overlay.open = false;
        }
        return;
    }
    match phase.get() {
        // Esc clears a target first (see `targeting`); with none, it pauses.
        Phase::Fight | Phase::Gates if keys.just_pressed(KeyCode::Escape) => {
            if fighters.iter().any(|f| f.player && f.target.is_none()) {
                next.set(Phase::Paused);
            }
        }
        Phase::Paused if keys.just_pressed(KeyCode::Escape) => {
            next.set(if state.opened { Phase::Fight } else { Phase::Gates });
        }
        // Ready: cut the wait at the gates short.
        Phase::Gates if keys.just_pressed(KeyCode::Enter) => state.gates_t = state.gates_t.min(READY_SECS),
        Phase::Paused if keys.just_pressed(KeyCode::KeyM) => next.set(Phase::Menu),
        Phase::Results if keys.just_pressed(KeyCode::Enter) => next.set(Phase::Menu),
        _ => {}
    }
}
