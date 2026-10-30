//! What an ability does at its numbers, in a tooltip's voice: read off the same numbers the game
//! casts with, so the text can never drift from the effect.

use crate::kit::{Ability, Cast, Effect, Kind, Stat};

pub fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

pub fn pct(v: f32) -> String {
    format!("{}%", num(v * 100.0))
}

pub fn effect(e: Effect) -> Option<String> {
    Some(match e {
        Effect::None => return None,
        Effect::Slow(m, t) => format!("slowing them by {} for {} sec", pct(1.0 - m), num(t)),
        Effect::Root(t) => format!("rooting them for {} sec", num(t)),
        Effect::Stun(t) => format!("stunning them for {} sec", num(t)),
        Effect::Fear(t) => format!("sending them fleeing for {} sec", num(t)),
        Effect::Incap(t) => format!("taking them out for {} sec (damage breaks it)", num(t)),
        Effect::Silence(t) => format!("silencing them for {} sec", num(t)),
        Effect::Interrupt(t) => format!("locking them out of casting for {} sec", num(t)),
        Effect::Weaken(m, t) => format!("cutting their damage by {} for {} sec", pct(1.0 - m), num(t)),
        Effect::Drain(f) => format!("healing you for {} of the damage", pct(f)),
        Effect::Execute(f) => format!("finishing them below {} health", pct(f)),
    })
}

/// The tooltip body for `a` casting with `c`.
pub fn ability(a: &Ability, c: &Cast) -> String {
    let d = num(c.dmg);
    let who = |noun: &str| {
        if a.effect.seeks_casters() && c.count == 1 {
            "a casting rival (else the nearest free one)".to_string()
        } else if a.toughest {
            match c.count {
                1 => format!("the toughest {noun}"),
                n => format!("the {n} toughest {noun}s"),
            }
        } else {
            match c.count {
                1 => format!("the nearest {noun}"),
                n => format!("the {n} nearest {noun}s"),
            }
        }
    };
    let mut s = match a.kind {
        Kind::Bolt | Kind::Strike | Kind::Melee => {
            let verb = if a.kind == Kind::Melee { "Strikes" } else { "Hits" };
            let mut s = if c.dmg > 0.0 {
                format!("{verb} {} for {d} damage", who("rival"))
            } else {
                format!("{verb} {}", who("rival"))
            };
            if c.radius > 0.0 && a.kind != Kind::Melee {
                s += &format!(" and everything within {} yd of the hit", num(c.radius));
            }
            if a.shatter > 0.0 {
                s += &format!(" ({}× on a frozen target)", num(a.shatter));
            }
            s
        }
        Kind::Nova => format!("Deals {d} damage to every rival within {} yd", num(c.radius)),
        Kind::Cone => format!("Deals {d} damage in a {} yd cone toward the nearest rival", num(c.radius)),
        Kind::Ground => {
            let at = if a.range <= 0.0 { "around you" } else { "on the thickest pack" };
            if a.duration <= 0.0 {
                format!("Strikes a {} yd area {at} for {d} damage", num(c.radius))
            } else {
                format!(
                    "Covers a {} yd area {at} for {} sec, dealing {d} damage every second",
                    num(c.radius),
                    num(c.duration)
                )
            }
        }
        Kind::Chain => format!(
            "Strikes the nearest rival for {d} damage, then leaps to {} more within {} yd, 15% weaker each time",
            c.count,
            num(c.radius)
        ),
        Kind::Dot => format!(
            "Afflicts {} rivals with {} damage every second for {} sec",
            c.count,
            num(c.dot),
            num(c.duration)
        ),
        Kind::Interrupt => format!("Strikes a rival mid-cast within {} yd", num(a.range)),
        Kind::Guard => format!(
            "Below 60% health with a rival close, cuts the damage you take by {} for {} sec",
            pct(a.damage),
            num(c.duration)
        ),
        Kind::Empower => format!(
            "With a crowd or a champion within {} yd, raises all your damage by {} for {} sec",
            num(a.range),
            pct(a.damage),
            num(c.duration)
        ),
        Kind::Immune => format!(
            "Below 40% health or held, with a rival close: breaks every control and makes you immune to all damage for {} sec",
            num(c.duration)
        ),
        Kind::Shield => format!("Absorbs the next {d} damage you take within {} sec", num(c.duration)),
        Kind::Heal => format!("Below 85% health, heals you for {d} over {} sec", num(c.duration)),
    };
    let ground = a.kind == Kind::Ground;
    if c.dot > 0.0 && a.kind != Kind::Dot && (!ground || a.duration <= 0.0) {
        let secs = if ground { 5.0 } else { c.duration };
        s += &format!(", then {} damage every second for {} sec", num(c.dot), num(secs));
    }
    if let Some(e) = effect(a.effect) {
        s += ", ";
        s += &e;
    }
    s.push('.');
    s
}

/// A stat's bonus at `v`, as a passive card shows it.
pub fn stat(stat: Stat, v: f32) -> String {
    match stat {
        Stat::Might => format!("All damage +{}", pct(v)),
        Stat::MaxHp => format!("Maximum health +{}", pct(v)),
        Stat::Speed => format!("Movement speed +{}", pct(v)),
        Stat::Regen => format!("Regenerate {} health per second", num(v)),
        Stat::Haste => format!("Cooldowns -{}", pct(v)),
        Stat::Area => format!("Area of effect +{}", pct(v)),
        Stat::Magnet => format!("Pickup range +{}", pct(v)),
        Stat::Growth => format!("Experience +{}", pct(v)),
        Stat::Armor => format!("Damage taken -{} per hit", num(v)),
        Stat::Crit => format!("Critical strike +{}", pct(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{ABILITIES, CLASSES, Owned, Stats};

    #[test]
    fn every_ability_reads_as_one_sentence() {
        let stats = Stats::of(&CLASSES[0], &[], 0.0);
        for a in ABILITIES {
            let s = ability(a, &Owned::new(a).cast(&stats));
            assert!(s.ends_with('.') && !s.contains(",,") && !s.contains("  "), "{}: {s}", a.key);
        }
    }

    #[test]
    fn a_control_names_its_target_and_effect() {
        let stats = Stats::of(&CLASSES[0], &[], 0.0);
        let a = crate::kit::ability("bewilder");
        let s = ability(a, &Owned::new(a).cast(&stats));
        assert_eq!(s, "Hits a casting rival (else the nearest free one), taking them out for 8 sec (damage breaks it).");
    }
}
