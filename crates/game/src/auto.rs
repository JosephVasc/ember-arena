//! A hands-off run for testing, off unless asked for: `EMBER_AUTO=<class key>` starts a run as that
//! class (`EMBER_ARENA=<index>` picks the arena), takes the first card at every level and saves a
//! screenshot every few seconds into `EMBER_SHOTS` (default `shots/`), quitting after
//! `EMBER_SECS` seconds (default 60).

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::run::{self, Offer, Run, Selection};
use crate::{Phase, Rng};

#[derive(Resource)]
struct Auto {
    class: usize,
    arena: usize,
    dir: std::path::PathBuf,
    quit_after: f32,
    t: f32,
    shot_t: f32,
    shots: u32,
    wait: f32,
}

pub fn plugin(app: &mut App) {
    let Ok(key) = std::env::var("EMBER_AUTO") else { return };
    let class = rules::kit::CLASSES.iter().position(|c| c.key == key.trim()).unwrap_or(0);
    let arena = std::env::var("EMBER_ARENA")
        .ok()
        .and_then(|a| a.parse().ok())
        .filter(|&a: &usize| a < rules::waves::ARENAS.len())
        .unwrap_or(0);
    let dir = std::path::PathBuf::from(std::env::var("EMBER_SHOTS").unwrap_or_else(|_| "shots".into()));
    let _ = std::fs::create_dir_all(&dir);
    let quit_after = std::env::var("EMBER_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0);
    app.insert_resource(Auto { class, arena, dir, quit_after, t: 0.0, shot_t: 3.0, shots: 0, wait: 0.0 })
        .add_systems(Update, drive);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut auto: ResMut<Auto>,
    phase: Res<State<Phase>>,
    mut sel: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    if phase.is_changed() {
        info!("auto: phase {:?} at {:.1}s", phase.get(), auto.t);
    }
    auto.t += dt;
    auto.wait += dt;
    match phase.get() {
        Phase::Menu => {
            sel.class = auto.class;
            sel.arena = auto.arena;
            if auto.wait > 3.0 {
                run::start(&mut run, &sel);
                next.set(Phase::Playing);
                auto.wait = 0.0;
            }
        }
        Phase::LevelUp if auto.wait > 1.0 => {
            run::pick(0, &mut run, &mut offer, &mut rng, &mut next);
            auto.wait = 0.0;
        }
        Phase::LevelUp => {}
        _ => auto.wait = 0.0,
    }
    auto.shot_t -= dt;
    if auto.shot_t <= 0.0 {
        auto.shot_t = 5.0;
        auto.shots += 1;
        let path = auto.dir.join(format!("shot-{:03}-{:?}.png", auto.shots, phase.get()));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        info!(
            "auto: t={:.0}s {:?} level={} hp={:.0} kills={} abilities=[{}]",
            auto.t,
            phase.get(),
            run.level,
            run.hp,
            run.kills,
            run.abilities.iter().map(|o| format!("{}:{}", o.def.key, o.level)).collect::<Vec<_>>().join(" ")
        );
    }
    if auto.t > auto.quit_after {
        exit.write(AppExit::Success);
    }
}
