//! The arenas, the rivals who pour into them, and how a run's waves grow: who comes when, how
//! tough, and the champions every couple of minutes.

use crate::Pt;
use crate::kit::{Effect, Model, School};

/// A walled ring. Pillars block movement (and, later, sight lines).
pub struct ArenaDef {
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    /// Yards from the centre to the inside of the wall.
    pub radius: f32,
    /// Where the hero starts.
    pub start: Pt,
    /// `(centre, radius)` of each pillar.
    pub pillars: &'static [(Pt, f32)],
    /// The run's victory line.
    pub victory: &'static str,
}

impl ArenaDef {
    /// `p` pushed back inside the wall and out of every pillar, for a body `r` yards wide.
    pub fn confine(&self, p: Pt, r: f32) -> Pt {
        let [mut x, mut z] = p;
        for &([px, pz], pr) in self.pillars {
            let (dx, dz) = (x - px, z - pz);
            let d = (dx * dx + dz * dz).sqrt();
            let min = pr + r;
            if d < min {
                // Dead centre: out along +x.
                let (nx, nz) = if d > 1e-4 { (dx / d, dz / d) } else { (1.0, 0.0) };
                x = px + nx * min;
                z = pz + nz * min;
            }
        }
        let d = (x * x + z * z).sqrt();
        let max = self.radius - r;
        if d > max {
            x *= max / d;
            z *= max / d;
        }
        [x, z]
    }
}

pub static ARENAS: &[ArenaDef] = &[
    ArenaDef {
        key: "ember_pit",
        name: "The Ember Pit",
        blurb: "A sunken ring of old stone, four pillars to break the line.",
        radius: 26.0,
        start: [0.0, 0.0],
        pillars: &[
            ([10.0, 10.0], 1.4),
            ([-10.0, 10.0], 1.4),
            ([10.0, -10.0], 1.4),
            ([-10.0, -10.0], 1.4),
        ],
        victory: "The Ember Pit roars your name.",
    },
    ArenaDef {
        key: "cinder_ring",
        name: "The Cinder Ring",
        blurb: "A wide dirt ring with a broken shrine at its heart.",
        radius: 32.0,
        start: [0.0, 8.0],
        pillars: &[
            ([0.0, 0.0], 2.2),
            ([16.0, 0.0], 1.4),
            ([-16.0, 0.0], 1.4),
            ([0.0, 16.0], 1.4),
            ([0.0, -16.0], 1.4),
        ],
        victory: "The Cinder Ring falls silent before you.",
    },
];

/// What a rival carries; the game maps it to the model's weapon meshes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gear {
    SwordShield,
    GreatSword,
    GreatAxe,
    DualAxes,
    Staff,
    Wand,
    Knives,
    Crossbow,
}

/// A bolt a caster stops to cast at the hero: interruptible.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cast {
    pub name: &'static str,
    pub school: School,
    /// What it does to the hero beyond its damage.
    pub effect: Effect,
}

/// A champion's control on the hero, every [`CONTROL_EVERY`] seconds when in reach.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Control {
    pub name: &'static str,
    pub school: School,
    pub effect: Effect,
    pub reach: f32,
}

/// A kind of rival, before its tier's numbers.
#[derive(Clone, Copy, Debug)]
pub struct Archetype {
    pub name: &'static str,
    pub model: Model,
    pub gear: Gear,
    pub speed: f32,
    pub cast: Option<Cast>,
}

const fn melee(name: &'static str, model: Model, gear: Gear, speed: f32) -> Archetype {
    Archetype { name, model, gear, speed, cast: None }
}

const fn caster(name: &'static str, model: Model, gear: Gear, cast: Cast) -> Archetype {
    Archetype { name, model, gear, speed: 4.0, cast: Some(cast) }
}

/// The rival kinds in the order each tier meets them: brawlers and casters interleaved.
pub static ARCHETYPES: &[Archetype] = &[
    melee("Shieldbearer", Model::Knight, Gear::SwordShield, 4.4),
    caster(
        "Hexcaster",
        Model::Mage,
        Gear::Wand,
        Cast { name: "Hex Bolt", school: School::Shadow, effect: Effect::None },
    ),
    melee("Cutthroat", Model::Rogue, Gear::Knives, 5.2),
    caster(
        "Bowman",
        Model::HoodedRogue,
        Gear::Crossbow,
        Cast { name: "Aimed Bolt", school: School::Physical, effect: Effect::None },
    ),
    melee("Brute", Model::Barbarian, Gear::GreatAxe, 4.2),
    caster(
        "Frostweaver",
        Model::Mage,
        Gear::Staff,
        Cast { name: "Rime Bolt", school: School::Frost, effect: Effect::Slow(0.6, 3.0) },
    ),
    melee("Executioner", Model::Knight, Gear::GreatSword, 4.0),
    melee("Raider", Model::Barbarian, Gear::DualAxes, 4.8),
    caster(
        "Pyromancer",
        Model::Mage,
        Gear::Staff,
        Cast { name: "Fire Bolt", school: School::Fire, effect: Effect::None },
    ),
];

/// The rank titles, lowest first: each a tier of the ladder.
pub const TITLES: [&str; 3] = ["Initiate", "Contender", "Veteran"];

/// The champions, one every [`CHAMPION_EVERY`] seconds, each with a control on the hero.
pub static CHAMPIONS: &[(Archetype, Control)] = &[
    (
        melee("Champion Warlord", Model::Knight, Gear::SwordShield, 4.6),
        Control { name: "Bulwark Bash", school: School::Physical, effect: Effect::Stun(3.0), reach: 8.0 },
    ),
    (
        melee("Champion Pyromancer", Model::Mage, Gear::Staff, 4.4),
        Control { name: "Bewildering Hex", school: School::Arcane, effect: Effect::Incap(5.0), reach: 22.0 },
    ),
    (
        melee("Champion Reaver", Model::Barbarian, Gear::GreatAxe, 4.8),
        Control { name: "Terror Howl", school: School::Shadow, effect: Effect::Fear(4.0), reach: 9.0 },
    ),
    (
        melee("Champion Nightblade", Model::Rogue, Gear::Knives, 5.4),
        Control { name: "Low Blow", school: School::Physical, effect: Effect::Stun(4.0), reach: 6.0 },
    ),
    (
        melee("Champion Huntmaster", Model::HoodedRogue, Gear::Crossbow, 4.6),
        Control { name: "Snare Net", school: School::Nature, effect: Effect::Root(4.0), reach: 20.0 },
    ),
];

/// A rival with its numbers.
#[derive(Clone, Debug)]
pub struct Rival {
    pub name: String,
    pub kind: Archetype,
    pub hp: f32,
    pub damage: f32,
    pub xp: u32,
    pub scale: f32,
    /// A champion's control on the hero.
    pub control: Option<Control>,
}

/// Survive this long to win.
pub const VICTORY_SECS: f32 = 600.0;
/// A champion every this many seconds; the last of five arrives before the bell.
pub const CHAMPION_EVERY: f32 = 110.0;
/// Seconds between a champion's controls.
pub const CONTROL_EVERY: f32 = 14.0;
/// Experience per orb.
pub const XP_RATE: f32 = 2.0;

/// A caster's cast time, its reach, and the rest between casts.
pub const CAST_SECS: f32 = 2.0;
pub const CAST_RANGE: f32 = 18.0;
pub const CAST_EVERY: f32 = 2.5;
/// A bolt's damage over the caster's melee: it stands off to deal it.
pub const CAST_DAMAGE: f32 = 1.4;

/// The tuned ladder the tiers read their numbers off: `(health, damage, experience)`, weakest
/// first. Bosses have their own.
const LADDER: &[(f32, f32, u32)] = &[
    (8.0, 2.0, 1),
    (10.0, 2.0, 1),
    (16.0, 3.0, 2),
    (18.0, 4.0, 2),
    (22.0, 8.0, 2),
    (26.0, 8.0, 3),
    (36.0, 10.0, 3),
    (40.0, 11.0, 3),
    (34.0, 10.0, 3),
    (44.0, 12.0, 4),
    (70.0, 14.0, 5),
    (55.0, 12.0, 4),
    (60.0, 13.0, 4),
    (85.0, 16.0, 5),
    (75.0, 15.0, 5),
    (90.0, 16.0, 6),
    (95.0, 18.0, 6),
    (120.0, 20.0, 7),
    (150.0, 22.0, 8),
    (220.0, 28.0, 12),
    (160.0, 24.0, 8),
    (180.0, 26.0, 10),
    (90.0, 18.0, 5),
    (240.0, 30.0, 12),
    (200.0, 30.0, 10),
    (280.0, 34.0, 14),
    (300.0, 36.0, 14),
    (420.0, 42.0, 24),
    (380.0, 44.0, 22),
    (360.0, 40.0, 20),
];

const CHAMPION_LADDER: &[(f32, f32, u32)] = &[
    (700.0, 20.0, 60),
    (1800.0, 32.0, 120),
    (4200.0, 45.0, 200),
    (7000.0, 55.0, 300),
    (12000.0, 70.0, 400),
];

/// Read `n` evenly spaced rungs off a ladder, from its running peak so no later rung comes out
/// softer than an earlier one.
fn rungs(ladder: &[(f32, f32, u32)], n: usize) -> Vec<(f32, f32, u32)> {
    let mut peak = (0.0f32, 0.0f32, 0.0f32);
    let peaks: Vec<(f32, f32, f32)> = ladder
        .iter()
        .map(|&(h, d, x)| {
            peak = (peak.0.max(h), peak.1.max(d), peak.2.max(x as f32));
            peak
        })
        .collect();
    let top = (peaks.len() - 1) as f32;
    (0..n)
        .map(|i| {
            let t = if n > 1 { i as f32 / (n - 1) as f32 * top } else { 0.0 };
            let (lo, hi) = (peaks[t.floor() as usize], peaks[t.ceil() as usize]);
            let k = t.fract();
            let mix = |a: f32, b: f32| (a + (b - a) * k).round();
            (mix(lo.0, hi.0), mix(lo.1, hi.1), mix(lo.2, hi.2) as u32)
        })
        .collect()
}

/// Every tier of rival a run meets, weakest first: each title through every archetype.
pub fn rivals() -> Vec<Rival> {
    let n = TITLES.len() * ARCHETYPES.len();
    let numbers = rungs(LADDER, n);
    TITLES
        .iter()
        .flat_map(|t| ARCHETYPES.iter().map(move |a| (t, a)))
        .zip(numbers)
        .map(|((title, &kind), (hp, damage, xp))| Rival {
            name: format!("{title} {}", kind.name),
            kind,
            hp,
            damage,
            xp,
            scale: 1.0,
            control: None,
        })
        .collect()
}

pub fn champions() -> Vec<Rival> {
    CHAMPIONS
        .iter()
        .zip(rungs(CHAMPION_LADDER, CHAMPIONS.len()))
        .map(|(&(kind, control), (hp, damage, xp))| Rival {
            name: kind.name.to_string(),
            kind,
            hp,
            damage,
            xp,
            scale: 1.45,
            control: Some(control),
        })
        .collect()
}

/// How far through the run, 0 at the start and 1 at [`VICTORY_SECS`].
pub fn progress(elapsed: f32) -> f32 {
    (elapsed / VICTORY_SECS).max(0.0)
}

/// The three neighbouring tiers the spawner draws from at a point in the run.
pub fn wave_pool(elapsed: f32, tiers: usize) -> [usize; 3] {
    let steps = 30usize;
    let step = ((progress(elapsed) * steps as f32) as usize).min(steps);
    let span = tiers.saturating_sub(3);
    let lo = (step.saturating_sub(1) * span / (steps - 3)).min(span);
    [lo, lo + 1, lo + 2]
}

/// How many rivals the spawner keeps alive.
pub fn target_alive(elapsed: f32) -> usize {
    ((12.0 + progress(elapsed) * 150.0) as usize).min(140)
}

pub fn hp_scale(elapsed: f32) -> f32 {
    1.0 + progress(elapsed) * 0.75
}

pub fn damage_scale(elapsed: f32) -> f32 {
    1.0 + progress(elapsed) * 0.9
}

/// The experience from `level` to the next: cheap early.
pub fn xp_to_next(level: u32) -> f32 {
    let l = level as f32;
    if level < 20 {
        5.0 + (l - 1.0) * 10.0
    } else if level < 40 {
        195.0 + (l - 20.0) * 13.0
    } else {
        455.0 + (l - 40.0) * 16.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_climbs_and_meets_every_archetype() {
        let r = rivals();
        assert_eq!(r.len(), TITLES.len() * ARCHETYPES.len());
        for w in r.windows(2) {
            assert!(w[1].hp >= w[0].hp && w[1].damage >= w[0].damage);
        }
        assert_eq!(r[0].name, "Initiate Shieldbearer");
        assert!(r.iter().any(|x| x.kind.cast.is_some()));
        let c = champions();
        assert_eq!(c.len(), CHAMPIONS.len());
        assert!(c.iter().all(|x| x.control.is_some() && x.hp > r.last().unwrap().hp));
    }

    #[test]
    fn the_pool_walks_the_whole_ladder() {
        let n = rivals().len();
        assert_eq!(wave_pool(0.0, n), [0, 1, 2]);
        assert_eq!(wave_pool(VICTORY_SECS, n), [n - 3, n - 2, n - 1]);
        let mut last = 0;
        for s in 0..=600 {
            let p = wave_pool(s as f32, n);
            assert!(p[0] >= last && p[2] < n);
            last = p[0];
        }
    }

    #[test]
    fn every_champion_arrives_before_the_bell() {
        let last = CHAMPION_EVERY * CHAMPIONS.len() as f32;
        assert!(last < VICTORY_SECS);
    }

    #[test]
    fn bodies_stay_inside_the_wall_and_out_of_pillars() {
        let a = &ARENAS[0];
        let [x, z] = a.confine([100.0, 0.0], 0.5);
        assert!((x - (a.radius - 0.5)).abs() < 1e-3 && z.abs() < 1e-3);
        let ([px, pz], pr) = a.pillars[0];
        let [x, z] = a.confine([px + 0.1, pz], 0.5);
        assert!(((x - px).powi(2) + (z - pz).powi(2)).sqrt() >= pr + 0.5 - 1e-3);
        assert_eq!(a.confine([1.0, 2.0], 0.5), [1.0, 2.0]);
    }

    #[test]
    fn arenas_keep_their_pillars_inside_the_wall() {
        for a in ARENAS {
            for &([x, z], r) in a.pillars {
                assert!((x * x + z * z).sqrt() + r < a.radius - 4.0, "{}", a.name);
                let [sx, sz] = a.start;
                assert!(((sx - x).powi(2) + (sz - z).powi(2)).sqrt() > r + 2.0, "{}", a.name);
            }
        }
    }
}
