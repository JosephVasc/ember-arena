//! A hands-off match for testing, off unless asked for: `EMBER_AUTO=<class key>` starts a match as
//! that class with the AI playing you too (`EMBER_BRACKET` 1-3, `EMBER_ARENA` index), saves a
//! screenshot every few seconds into `EMBER_SHOTS` (default `shots/`) and quits after `EMBER_SECS`
//! (default 60).

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::camera::Cam;
use crate::fight::{Fighter, Match};
use crate::{Phase, Selection};

/// Present while the autopilot plays.
#[derive(Resource)]
pub struct Autopilot {
    class: usize,
    bracket: usize,
    arena: usize,
    dir: std::path::PathBuf,
    quit_after: f32,
    t: f32,
    shot_t: f32,
    shots: u32,
    /// The moment you first lost control has been captured.
    caught_cc: bool,
}

fn env<T: std::str::FromStr>(key: &str) -> Option<T> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

pub fn plugin(app: &mut App) {
    let Ok(key) = std::env::var("EMBER_AUTO") else { return };
    let class = rules::spells::CLASSES.iter().position(|c| c.key == key.trim()).unwrap_or(0);
    let dir = std::path::PathBuf::from(std::env::var("EMBER_SHOTS").unwrap_or_else(|_| "shots".into()));
    let _ = std::fs::create_dir_all(&dir);
    app.insert_resource(Autopilot {
        class,
        bracket: env("EMBER_BRACKET").unwrap_or(2).clamp(1, 3),
        arena: env("EMBER_ARENA").filter(|&a: &usize| a < rules::arena::ARENAS.len()).unwrap_or(0),
        dir,
        quit_after: env("EMBER_SECS").unwrap_or(60.0),
        t: 0.0,
        shot_t: 4.0,
        shots: 0,
        caught_cc: false,
    })
    .add_systems(Update, drive);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut auto: ResMut<Autopilot>,
    phase: Res<State<Phase>>,
    mut sel: ResMut<Selection>,
    mut cam: ResMut<Cam>,
    mut state: ResMut<Match>,
    fighters: Query<(&Fighter, &Transform)>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    auto.t += dt;
    if phase.is_changed() {
        info!("auto: phase {:?} at {:.1}s", phase.get(), auto.t);
    }
    if *phase.get() == Phase::Menu {
        sel.class = auto.class;
        sel.bracket = auto.bracket;
        sel.arena = auto.arena;
        if auto.t > 2.0 {
            next.set(Phase::Gates);
        }
    }
    // Long enough at the gates to see everyone buff, no longer.
    if *phase.get() == Phase::Gates {
        state.gates_t = state.gates_t.min(8.0);
    }
    // The camera rides behind you, as a player's would.
    if let Some((_, tf)) = fighters.iter().find(|(f, _)| f.player) {
        let fwd = tf.rotation * Vec3::Z;
        cam.yaw = fwd.x.atan2(fwd.z);
    }
    let controlled = fighters.iter().any(|(f, _)| f.player && f.unit.loss_of_control().is_some_and(|d| d.left < d.total - 0.2));
    if controlled && !auto.caught_cc {
        auto.caught_cc = true;
        let path = auto.dir.join("controlled.png");
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    auto.shot_t -= dt;
    if auto.shot_t <= 0.0 {
        auto.shot_t = 5.0;
        auto.shots += 1;
        let path = auto.dir.join(format!("shot-{:03}-{:?}.png", auto.shots, phase.get()));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        let line: Vec<String> = fighters
            .iter()
            .map(|(f, _)| format!("{}[{}] {}:{:.0}", f.name, f.team, f.unit.class.name, f.unit.hp))
            .collect();
        info!("auto: t={:.0}s {:?} {}", auto.t, phase.get(), line.join(" | "));
    }
    if auto.t > auto.quit_after {
        exit.write(AppExit::Success);
    }
}
