//! One run's state, the pick that starts it, and the level-up cards.

use bevy::prelude::*;
use rules::control::Held;
use rules::kit::{self, Ability, Class, Owned, Passive, Spec, Stats};
use rules::waves::{self, ArenaDef, Rival};

use crate::{Phase, Rng, RunEntity};

pub fn plugin(app: &mut App) {
    app.init_resource::<Rng>()
        .init_resource::<Selection>()
        .init_resource::<Run>()
        .init_resource::<Offer>()
        .insert_resource(Roster {
            rivals: waves::rivals(),
            champions: waves::champions(),
        })
        .add_systems(OnEnter(Phase::Playing), unpause)
        .add_systems(OnEnter(Phase::LevelUp), (pause, deal))
        .add_systems(OnEnter(Phase::Paused), pause)
        .add_systems(OnEnter(Phase::GameOver), unpause)
        .add_systems(OnEnter(Phase::Menu), (unpause, clear_run));
}

/// The class and arena picked at the menu.
#[derive(Resource)]
pub struct Selection {
    pub class: usize,
    pub arena: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Self { class: 2, arena: 0 }
    }
}

impl Selection {
    pub fn class(&self) -> &'static Class {
        &kit::CLASSES[self.class]
    }
    pub fn arena(&self) -> &'static ArenaDef {
        &waves::ARENAS[self.arena]
    }
}

/// Every rival tier and champion, with their numbers.
#[derive(Resource)]
pub struct Roster {
    pub rivals: Vec<Rival>,
    pub champions: Vec<Rival>,
}

/// An enemy bolt flying at the hero.
pub struct Incoming {
    pub at: f32,
    pub damage: f32,
    pub cast: waves::Cast,
    pub from: rules::Pt,
}

#[derive(Resource)]
pub struct Run {
    pub class: &'static Class,
    pub arena: &'static ArenaDef,
    pub spec: Option<&'static Spec>,
    pub elapsed: f32,
    pub kills: u32,
    pub level: u32,
    pub xp: f32,
    pub hp: f32,
    pub abilities: Vec<Owned>,
    pub passives: Vec<(&'static Passive, u8)>,
    pub shield: f32,
    pub shield_t: f32,
    pub immune_t: f32,
    pub guard: f32,
    pub guard_t: f32,
    pub empower: f32,
    pub empower_t: f32,
    /// Heals over time, `(per second, seconds left)`; they stack.
    pub heals: Vec<(f32, f32)>,
    pub cc: Held,
    pub trinket_cd: f32,
    pub mobility_cd: f32,
    /// Rivals have lost the hero (Smoke Veil), and his speed multiplier meanwhile.
    pub vanish_t: f32,
    pub haste: f32,
    pub incoming: Vec<Incoming>,
    pub pending_levels: u32,
    pub champions: u32,
    pub rings: u32,
    pub won: bool,
    /// Seconds since the hero fell.
    pub dead_t: Option<f32>,
    pub damage_done: f64,
    /// Bumped when abilities or passives change, so the HUD rebuilds its slots.
    pub version: u32,
    pub banner: Option<(String, f32)>,
}

impl Default for Run {
    fn default() -> Self {
        Self::new(&kit::CLASSES[0], &waves::ARENAS[0])
    }
}

impl Run {
    pub fn new(class: &'static Class, arena: &'static ArenaDef) -> Self {
        let mut run = Self {
            class,
            arena,
            spec: None,
            elapsed: 0.0,
            kills: 0,
            level: 1,
            xp: 0.0,
            hp: 0.0,
            abilities: vec![Owned::new(kit::ability(class.start))],
            passives: Vec::new(),
            shield: 0.0,
            shield_t: 0.0,
            immune_t: 0.0,
            guard: 0.0,
            guard_t: 0.0,
            empower: 0.0,
            empower_t: 0.0,
            heals: Vec::new(),
            cc: Held::default(),
            trinket_cd: 0.0,
            mobility_cd: 0.0,
            vanish_t: 0.0,
            haste: 1.0,
            incoming: Vec::new(),
            pending_levels: 0,
            champions: 0,
            rings: 0,
            won: false,
            dead_t: None,
            damage_done: 0.0,
            version: 1,
            banner: None,
        };
        run.hp = run.stats().max_hp;
        run
    }

    pub fn stats(&self) -> Stats {
        let empower = if self.empower_t > 0.0 { self.empower } else { 0.0 };
        Stats::of(self.class, &self.passives, empower)
    }

    /// The abilities on offer: the class's core, and the spec's once picked.
    pub fn pool(&self) -> impl Iterator<Item = &'static str> + '_ {
        let spec: &'static [&'static str] = self.spec.map_or(&[], |s| s.pool);
        self.class.pool.iter().chain(spec).copied()
    }

    pub fn max_abilities(&self) -> usize {
        kit::MAX_ABILITIES + usize::from(self.spec.is_some())
    }

    pub fn has_mobility(&self) -> bool {
        self.level >= kit::MOBILITY_LEVEL
    }
}

/// A level-up choice.
#[derive(Clone, Copy)]
pub enum Choice {
    New(&'static Ability),
    Rank(usize),
    NewPassive(&'static Passive),
    RankPassive(usize),
    Spec(&'static Spec),
    Mend,
}

#[derive(Resource, Default)]
pub struct Offer(pub Vec<Choice>);

fn choices(run: &Run, rng: &mut Rng) -> Vec<Choice> {
    if run.spec.is_none() && run.level >= kit::SPEC_LEVEL {
        return run.class.specs.iter().map(Choice::Spec).collect();
    }
    let mut pool: Vec<(Choice, f32)> = Vec::new();
    for (i, o) in run.abilities.iter().enumerate() {
        if o.level < kit::MAX_ABILITY_LEVEL {
            pool.push((Choice::Rank(i), 1.2));
        }
    }
    if run.abilities.len() < run.max_abilities() {
        let mut seen: Vec<&str> = Vec::new();
        for key in run.pool() {
            if !run.abilities.iter().any(|o| o.def.key == key) && !seen.contains(&key) {
                seen.push(key);
                pool.push((Choice::New(kit::ability(key)), 1.0));
            }
        }
    }
    for (i, (_, level)) in run.passives.iter().enumerate() {
        if *level < kit::MAX_PASSIVE_LEVEL {
            pool.push((Choice::RankPassive(i), 0.8));
        }
    }
    if run.passives.len() < kit::MAX_PASSIVES {
        for p in kit::PASSIVES {
            if !run.passives.iter().any(|(q, _)| q.key == p.key) {
                pool.push((Choice::NewPassive(p), 0.5));
            }
        }
    }
    let mut out = Vec::with_capacity(3);
    while out.len() < 3 && !pool.is_empty() {
        let total: f32 = pool.iter().map(|(_, w)| w).sum();
        let mut roll = rng.f32() * total;
        let mut idx = pool.len() - 1;
        for (i, (_, w)) in pool.iter().enumerate() {
            if roll < *w {
                idx = i;
                break;
            }
            roll -= w;
        }
        out.push(pool.swap_remove(idx).0);
    }
    if out.is_empty() {
        out.push(Choice::Mend);
    }
    out
}

fn deal(run: Res<Run>, mut rng: ResMut<Rng>, mut offer: ResMut<Offer>) {
    offer.0 = choices(&run, &mut rng);
}

/// Take card `i`; deal again while levels are banked, else back to the fight.
pub fn pick(i: usize, run: &mut Run, offer: &mut Offer, rng: &mut Rng, next: &mut NextState<Phase>) {
    let Some(&choice) = offer.0.get(i) else { return };
    match choice {
        Choice::New(def) => run.abilities.push(Owned::new(def)),
        Choice::Rank(k) => run.abilities[k].level += 1,
        Choice::NewPassive(p) => run.passives.push((p, 1)),
        Choice::RankPassive(k) => run.passives[k].1 += 1,
        Choice::Mend => run.hp = run.stats().max_hp,
        Choice::Spec(spec) => {
            run.spec = Some(spec);
            let signature = kit::ability(spec.pool[0]);
            if !run.abilities.iter().any(|o| o.def.key == signature.key) {
                run.abilities.push(Owned::new(signature));
            }
            run.banner = Some((format!("{} {}", spec.name, run.class.name), 3.0));
        }
    }
    run.version += 1;
    run.pending_levels = run.pending_levels.saturating_sub(1);
    if run.pending_levels > 0 {
        offer.0 = choices(run, rng);
    } else {
        offer.0.clear();
        next.set(Phase::Playing);
    }
}

/// Begin a run with the menu's pick.
pub fn start(run: &mut Run, sel: &Selection) {
    *run = Run::new(sel.class(), sel.arena());
}

fn pause(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn clear_run(mut commands: Commands, spawned: Query<Entity, With<RunEntity>>) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spec_is_dealt_alone_at_its_level_and_grants_its_signature() {
        let mut run = Run::new(kit::class("arcanist"), &waves::ARENAS[0]);
        let mut rng = Rng(7);
        run.level = kit::SPEC_LEVEL;
        let offer = choices(&run, &mut rng);
        assert!(offer.len() == 3 && offer.iter().all(|c| matches!(c, Choice::Spec(_))));
        let mut offer = Offer(offer);
        let mut next = NextState::<Phase>::default();
        run.pending_levels = 1;
        pick(0, &mut run, &mut offer, &mut rng, &mut next);
        let spec = run.spec.expect("picked");
        assert!(run.abilities.iter().any(|o| o.def.key == spec.pool[0]));
        assert!(choices(&run, &mut rng).iter().all(|c| !matches!(c, Choice::Spec(_))));
    }

    #[test]
    fn a_full_run_is_offered_no_duplicates() {
        let mut rng = Rng(11);
        for class in kit::CLASSES {
            let run = Run::new(class, &waves::ARENAS[0]);
            for _ in 0..50 {
                let offer = choices(&run, &mut rng);
                let keys: Vec<&str> = offer
                    .iter()
                    .filter_map(|c| match c {
                        Choice::New(a) => Some(a.key),
                        _ => None,
                    })
                    .collect();
                for (i, k) in keys.iter().enumerate() {
                    assert!(!keys[i + 1..].contains(k));
                }
            }
        }
    }
}
