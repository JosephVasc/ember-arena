//! Your hands: WoW-style movement and camera, targeting, and the action bar.
//!
//! - W / S: forward and back; A / D: strafe; both mouse buttons: run forward.
//! - Right drag: turn the camera and your character with it. Left drag: look around freely.
//! - Wheel: zoom. Tab: next enemy. Click a fighter: target it. F1-F3: yourself and your party.
//! - 1-0: your abilities. Q: trinket. Esc: clear target, then pause.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rules::unit::Fail;

use crate::camera::{Cam, MainCam, PITCH_MAX, PITCH_MIN};
use crate::fight::{BODY, Fighter, Release, SPEED, Stride, situation, start};
use crate::fx::{self, Fx};
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

/// An action bar slot clicked with the mouse (`usize::MAX` for the trinket).
#[derive(Message, Clone, Copy)]
pub struct Press(pub usize);

pub const KEYS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];
pub const TRINKET: KeyCode = KeyCode::KeyQ;

const TURN: f32 = 0.006;

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
    mut fighters: Query<(&mut Fighter, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let Some((mut f, mut tf)) = fighters.iter_mut().find(|(f, _)| f.player) else { return };
    f.stride = Stride::Still;
    if f.unit.dead {
        return;
    }
    let fwd = flat(tf.rotation * Vec3::Z).normalize_or(Vec3::Z);
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    let both = mouse.pressed(MouseButton::Left) && mouse.pressed(MouseButton::Right);
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) || both {
        dir += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= fwd;
    }
    // A and D strafe; the models' +X is their left.
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    let cc = f.unit.cc;
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
    tf.translation = at(sel.arena().confine(pt(tf.translation + step), BODY));
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
    if keys.just_pressed(KeyCode::Tab) {
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
    for (i, k) in [KeyCode::F1, KeyCode::F2, KeyCode::F3].into_iter().enumerate() {
        if keys.just_pressed(k) {
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
    // A click (not a drag, not on the HUD) on a fighter targets it.
    let on_ui = ui.iter().any(|i| *i != Interaction::None);
    if mouse.just_released(MouseButton::Left) && drag.0 < 6.0 && !on_ui {
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
            }
        }
    }
    if let Some(t) = pick {
        if let Ok((_, mut f, _)) = fighters.get_mut(me) {
            f.target = t;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_keys(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
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
    want.extend(KEYS.iter().enumerate().filter(|(_, k)| keys.just_pressed(**k)).map(|(i, _)| i));
    if keys.just_pressed(TRINKET) {
        want.push(usize::MAX);
    }
    if want.is_empty() {
        return;
    }
    let Some(me) = fighters.iter().find(|(_, f, _)| f.player).map(|(e, _, _)| e) else { return };
    let fail = |errors: &mut Errors, text: &str| errors.0 = Some((text.to_string(), 2.0));
    for slot in want {
        let Ok((_, f, tf)) = fighters.get(me) else { return };
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
        if *phase.get() == Phase::Gates {
            fail(&mut errors, "The gates have not opened yet");
            continue;
        }
        let target = f.target;
        let moving = f.stride != Stride::Still;
        let target_info = target.and_then(|t| fighters.get(t).ok()).map(|(_, tf_f, ttf)| (tf_f, ttf));
        let s = situation(sel.arena(), (f, tf), target_info, moving, false);
        match f.unit.check(slot, s) {
            Ok(()) => {
                if let Ok((_, mut f, _)) = fighters.get_mut(me) {
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
    mut next: ResMut<NextState<Phase>>,
) {
    match phase.get() {
        // Esc clears a target first (see `targeting`); with none, it pauses.
        Phase::Fight | Phase::Gates if keys.just_pressed(KeyCode::Escape) => {
            if fighters.iter().any(|f| f.player && f.target.is_none()) {
                next.set(Phase::Paused);
            }
        }
        Phase::Paused if keys.just_pressed(KeyCode::Escape) => next.set(Phase::Fight),
        Phase::Paused if keys.just_pressed(KeyCode::KeyM) => next.set(Phase::Menu),
        Phase::Results if keys.just_pressed(KeyCode::Enter) => next.set(Phase::Menu),
        _ => {}
    }
}
