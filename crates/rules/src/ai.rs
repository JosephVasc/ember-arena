//! How a computer fighter plays: a target to kill, its abilities by an arena player's priorities
//! (trinket out of control, defensives when low, kick casts, heal and cleanse, control the healer,
//! burst a low target, close or open the gap), and where to stand.

use crate::Pt;
use crate::arena::ArenaDef;
use crate::spells::{Aim, Role, Use};
use crate::unit::{Situation, TargetInfo, Unit};

/// What the AI knows of each fighter this moment.
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub team: usize,
    pub pos: Pt,
    pub hp: f32,
    pub dead: bool,
    pub role: Role,
    /// How far through a cast, 0..1.
    pub casting: Option<f32>,
    /// Stunned, feared or incapacitated.
    pub out: bool,
    /// Held by a control a cleanse can lift (not a stun).
    pub cleansable: bool,
    pub slowed: bool,
}

/// What it remembers between thoughts.
#[derive(Default, Clone, Copy, Debug)]
pub struct Brain {
    pub target: Option<usize>,
    hold: f32,
}

/// What it does this moment.
#[derive(Default, Clone, Copy, Debug)]
pub struct Plan {
    /// `(slot, target index)`; the target is itself for its own spells.
    pub cast: Option<(usize, usize)>,
    pub trinket: bool,
    /// Where to go; `None` to stand still.
    pub goal: Option<Pt>,
}

fn dist(a: Pt, b: Pt) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Think for fighter `me` (index into `all`) once; `dt` since the last thought, `roll` in 0..1.
pub fn think(me: usize, unit: &Unit, all: &[Seen], arena: &ArenaDef, brain: &mut Brain, dt: f32, roll: f32) -> Plan {
    let mut plan = Plan::default();
    let s = all[me];
    if s.dead {
        return plan;
    }
    let enemies: Vec<usize> = (0..all.len()).filter(|&i| all[i].team != s.team && !all[i].dead).collect();
    let allies: Vec<usize> = (0..all.len()).filter(|&i| all[i].team == s.team && !all[i].dead).collect();
    if enemies.is_empty() {
        return plan;
    }

    // The kill target: kept a while, then the weakest, nearest breaking ties.
    brain.hold -= dt;
    let valid = brain.target.is_some_and(|t| !all[t].dead && all[t].team != s.team);
    if !valid || brain.hold <= 0.0 {
        let score = |i: usize| all[i].hp * 100.0 + dist(s.pos, all[i].pos) * 0.5;
        brain.target = enemies.iter().copied().min_by(|&a, &b| score(a).total_cmp(&score(b)));
        brain.hold = 6.0;
    }
    let target = brain.target.unwrap_or(enemies[0]);

    if unit.cc.out() {
        plan.trinket = s.hp < 0.6 && unit.trinket_cd <= 0.0;
    }

    let info = |i: usize| {
        let t = all[i];
        Situation {
            target: Some(TargetInfo {
                hostile: t.team != s.team,
                dist: dist(s.pos, t.pos),
                sight: arena.sight(s.pos, t.pos),
                ahead: true,
                dead: t.dead,
            }),
            moving: false,
        }
    };
    let spells = unit.class.spells;
    let ready = |slot: usize, on: usize| unit.check(slot, info(on)).is_ok();
    let first = |u: Use, on: usize| (0..spells.len()).find(|&k| spells[k].ai == u && ready(k, on));
    let lowest_ally = allies.iter().copied().min_by(|&a, &b| all[a].hp.total_cmp(&all[b].hp)).unwrap_or(me);
    let near_enemies = |r: f32| enemies.iter().filter(|&&e| dist(s.pos, all[e].pos) <= r).count();

    let mut pick: Option<(usize, usize)> = None;
    let mut want = |p: Option<(usize, usize)>| {
        if pick.is_none() {
            pick = p;
        }
    };

    // Low: a defensive on yourself, or a guarding ward on a friend about to die.
    if s.hp < 0.4 {
        want(first(Use::Defensive, me).filter(|&k| spells[k].aim == Aim::Myself).map(|k| (k, me)));
    }
    if all[lowest_ally].hp < 0.35 {
        want(first(Use::Defensive, lowest_ally).filter(|&k| spells[k].aim == Aim::Friend).map(|k| (k, lowest_ally)));
    }
    // Kick a cast half way through.
    for &e in &enemies {
        if all[e].casting.is_some_and(|p| p > 0.35) {
            want(first(Use::Interrupt, e).map(|k| (k, e)));
        }
    }
    // A healer heals: fast when it's dire, a cleanse for a held friend.
    if s.role == Role::Healer {
        if let Some(&held) = allies.iter().find(|&&a| all[a].cleansable) {
            want(first(Use::Cleanse, held).map(|k| (k, held)));
        }
        let hurt = all[lowest_ally].hp;
        if hurt < 0.9 {
            let heals: Vec<usize> = (0..spells.len())
                .filter(|&k| spells[k].ai == Use::Heal && ready(k, lowest_ally))
                .collect();
            // Instant heals first when it's dire, the big cast when there's time.
            let pick_heal = if hurt < 0.5 {
                heals.iter().copied().find(|&k| spells[k].cast == 0.0).or(heals.first().copied())
            } else {
                heals.iter().copied().find(|&k| spells[k].cast > 0.0 && roll < 0.6).or(heals.first().copied())
            };
            want(pick_heal.map(|k| (k, lowest_ally)));
        }
    }
    // Control: around you when they're on you, else the healer or another off-target.
    if roll < 0.35 {
        for k in 0..spells.len() {
            if spells[k].ai != Use::Control {
                continue;
            }
            if spells[k].aim == Aim::Around {
                if near_enemies(spells[k].radius) > 0 && ready(k, me) {
                    want(Some((k, me)));
                }
            } else {
                let off = enemies
                    .iter()
                    .copied()
                    .filter(|&e| !all[e].out)
                    .min_by_key(|&e| (all[e].role != Role::Healer, e == target));
                if let Some(e) = off.filter(|&e| ready(k, e)) {
                    want(Some((k, e)));
                }
            }
        }
    }
    let tdist = dist(s.pos, all[target].pos);
    if all[target].hp < 0.6 {
        want(first(Use::Burst, me).filter(|&k| spells[k].aim == Aim::Myself).map(|k| (k, me)));
        want(first(Use::Burst, target).map(|k| (k, target)));
    }
    if s.role == Role::Melee && tdist > 8.0 {
        want(first(Use::Gap, target).map(|k| (k, target)));
    }
    if s.role != Role::Melee && near_enemies(6.0) > 0 && s.role != Role::Healer {
        want(first(Use::Escape, me).map(|k| (k, me)));
    }
    if !all[target].slowed && (s.role != Role::Melee || tdist > 3.0) {
        want(first(Use::Snare, target).map(|k| (k, target)));
    }
    if s.role != Role::Healer || all[lowest_ally].hp > 0.7 {
        want(first(Use::Damage, target).map(|k| (k, target)));
        want(first(Use::Filler, target).map(|k| (k, target)));
    }
    plan.cast = pick;

    // Where to stand: melee on the target; casters in range and in sight, away from melee.
    let anchor = if s.role == Role::Healer { all[lowest_ally].pos } else { all[target].pos };
    let reach = match s.role {
        Role::Melee => 3.5,
        Role::Ranged => 26.0,
        Role::Healer => 22.0,
    };
    let sees = arena.sight(s.pos, anchor);
    let d = dist(s.pos, anchor);
    let threat = enemies
        .iter()
        .copied()
        .find(|&e| all[e].role == Role::Melee && dist(s.pos, all[e].pos) < 6.0);
    plan.goal = if unit.casting.is_some() {
        None
    } else if s.role != Role::Melee && threat.is_some() && pick.is_none() {
        let t = all[threat.unwrap_or(target)].pos;
        let (ax, az) = (s.pos[0] - t[0], s.pos[1] - t[1]);
        let l = (ax * ax + az * az).sqrt().max(0.1);
        Some([s.pos[0] + ax / l * 10.0, s.pos[1] + az / l * 10.0])
    } else if d > reach || !sees {
        Some(anchor)
    } else {
        None
    };
    // A cast with a cast time needs standing still.
    if let Some((k, _)) = plan.cast {
        if spells[k].cast > 0.0 {
            plan.goal = None;
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ARENAS;
    use crate::spells::class;

    fn seen(team: usize, pos: Pt, role: Role) -> Seen {
        Seen { team, pos, hp: 1.0, dead: false, role, casting: None, out: false, cleansable: false, slowed: false }
    }

    #[test]
    fn a_melee_closes_in_and_a_caster_casts() {
        let a = &ARENAS[0];
        let all = [seen(0, [0.0, -30.0], Role::Melee), seen(1, [0.0, 30.0], Role::Ranged)];
        let rogue = Unit::new(class("shadowblade"));
        let plan = think(0, &rogue, &all, a, &mut Brain::default(), 0.1, 0.9);
        assert_eq!(plan.goal, Some([0.0, 30.0]));
        let all = [seen(0, [0.0, -10.0], Role::Ranged), seen(1, [0.0, 10.0], Role::Melee)];
        let mage = Unit::new(class("arcanist"));
        let plan = think(0, &mage, &all, a, &mut Brain::default(), 0.1, 0.9);
        let (slot, on) = plan.cast.expect("casts");
        assert_eq!(on, 1);
        assert_eq!(mage.class.spells[slot].ai, Use::Damage, "its big hits before the filler");
    }

    #[test]
    fn kicks_a_cast_and_heals_a_friend() {
        let a = &ARENAS[0];
        let mut all = [seen(0, [0.0, 0.0], Role::Melee), seen(1, [0.0, 3.0], Role::Ranged)];
        all[1].casting = Some(0.6);
        let rogue = Unit::new(class("shadowblade"));
        let plan = think(0, &rogue, &all, a, &mut Brain::default(), 0.1, 0.9);
        assert_eq!(rogue.class.spells[plan.cast.unwrap().0].key, "throat_jab");

        let mut all = [seen(0, [0.0, 0.0], Role::Healer), seen(0, [5.0, 0.0], Role::Melee), seen(1, [0.0, 30.0], Role::Melee)];
        all[1].hp = 0.3;
        let healer = Unit::new(class("mystic"));
        let plan = think(0, &healer, &all, a, &mut Brain::default(), 0.1, 0.9);
        let (slot, on) = plan.cast.expect("heals");
        assert_eq!(on, 1);
        assert!(matches!(healer.class.spells[slot].ai, Use::Heal | Use::Defensive));
    }
}
