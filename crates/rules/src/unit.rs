//! One fighter's state and the rules of casting: the global cooldown, cooldowns, the resource,
//! cast times broken by movement, controls, and what a landed ability does to its target through
//! auras, absorbs, immunity and diminishing returns.

use crate::Pt;
use crate::control::{Ctl, Held, Landed};
use crate::spells::{Aim, AuraDef, AuraKind, BASE_CRIT, Class, GCD, Resource, School, Special, Spell, TRINKET_COOLDOWN, Use};

/// An aura on a unit: a dot, hot, absorb or modifier, from an ability.
#[derive(Clone, Copy, Debug)]
pub struct Aura {
    pub def: AuraDef,
    /// The ability it came from.
    pub key: &'static str,
    pub name: &'static str,
    pub school: School,
    pub left: f32,
    tick: f32,
    /// What's left of an absorb.
    pub pool: f32,
    /// Placed by an enemy: a cleanse removes it.
    pub harmful: bool,
}

/// A control on a unit and the ability that laid it, for showing it; the seconds left are always
/// the unit's own timers.
#[derive(Clone, Copy, Debug)]
struct Mark {
    ctl: Ctl,
    key: &'static str,
    name: &'static str,
    school: School,
    total: f32,
}

/// Something harmful on a unit, as a player sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Debuff {
    /// The ability behind it.
    pub key: &'static str,
    pub name: &'static str,
    pub school: School,
    pub left: f32,
    pub total: f32,
    /// The control, or `None` for damage over time.
    pub ctl: Option<Ctl>,
}

/// A cast under way.
#[derive(Clone, Copy, Debug)]
pub struct Casting {
    pub slot: usize,
    pub left: f32,
    pub total: f32,
}

/// Why an ability can't go off now: the line a player sees.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fail {
    Dead,
    NotReady,
    Busy,
    Held,
    Silenced,
    NoResource,
    NoTarget,
    WrongTarget,
    OutOfRange,
    NoSight,
    Facing,
    Moving,
}

impl Fail {
    pub fn text(self) -> &'static str {
        match self {
            Self::Dead => "You are dead",
            Self::NotReady => "Ability is not ready yet",
            Self::Busy => "Another action is in progress",
            Self::Held => "You can't do that while held",
            Self::Silenced => "Can't do that while silenced",
            Self::NoResource => "Not enough resource",
            Self::NoTarget => "You have no target",
            Self::WrongTarget => "Invalid target",
            Self::OutOfRange => "Out of range",
            Self::NoSight => "Target not in line of sight",
            Self::Facing => "Target needs to be in front of you",
            Self::Moving => "Can't do that while moving",
        }
    }
}

/// What the world says about a cast's target, for [`Unit::check`].
#[derive(Clone, Copy, Debug)]
pub struct Situation {
    /// The target, if any: hostile, its distance, line of sight and whether it's in front.
    pub target: Option<TargetInfo>,
    pub moving: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TargetInfo {
    pub hostile: bool,
    pub dist: f32,
    pub sight: bool,
    pub ahead: bool,
    pub dead: bool,
}

/// What landing did, for floating text and logs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Feedback {
    Damage { amount: f32, crit: bool, school: School },
    Absorbed(f32),
    Heal(f32),
    Immune,
    Ctl(Ctl),
    Interrupted,
    Cleansed,
}

pub struct Unit {
    pub class: &'static Class,
    pub hp: f32,
    pub max_hp: f32,
    pub res: f32,
    pub cc: Held,
    marks: Vec<Mark>,
    pub auras: Vec<Aura>,
    pub cds: Vec<f32>,
    pub gcd: f32,
    pub casting: Option<Casting>,
    pub immune_t: f32,
    pub trinket_cd: f32,
    /// Swinging (or shooting) at the target by itself.
    pub attacking: bool,
    /// Seconds to the next white hit.
    pub swing: f32,
    pub dead: bool,
    pub damage_done: f32,
    pub healing_done: f32,
}

pub const MAX_RES: f32 = 100.0;

impl Unit {
    pub fn new(class: &'static Class) -> Self {
        Self {
            class,
            hp: class.health,
            max_hp: class.health,
            res: if class.resource == Resource::Rage { 0.0 } else { MAX_RES },
            cc: Held::default(),
            marks: Vec::new(),
            auras: Vec::new(),
            cds: vec![0.0; class.spells.len() + class.buffs.len()],
            gcd: 0.0,
            casting: None,
            immune_t: 0.0,
            trinket_cd: 0.0,
            attacking: false,
            swing: 0.0,
            dead: false,
            damage_done: 0.0,
            healing_done: 0.0,
        }
    }

    /// An ability by bar slot: the ten abilities, then the buffs.
    pub fn spell(&self, slot: usize) -> &'static Spell {
        let n = self.class.spells.len();
        if slot < n { &self.class.spells[slot] } else { &self.class.buffs[slot - n] }
    }

    /// The chance of a critical strike now.
    pub fn crit_chance(&self) -> f32 {
        BASE_CRIT + self.auras.iter().map(|a| if let AuraKind::Crit(c) = a.def.kind { c } else { 0.0 }).sum::<f32>()
    }

    /// Maximum health follows the buffs; gaining some heals by as much, losing it never kills.
    fn refresh_health(&mut self) {
        let max = self.class.health * self.modifier(|k| if let AuraKind::Health(m) = k { Some(m) } else { None });
        if !self.dead {
            self.hp = (self.hp + (max - self.max_hp).max(0.0)).min(max).max(1.0);
        }
        self.max_hp = max;
    }

    /// The gates open: full health and resource (rage starts from nothing), cooldowns ready.
    pub fn prepare(&mut self) {
        self.hp = self.max_hp;
        if self.class.resource != Resource::Rage {
            self.res = MAX_RES;
        }
        self.cds.iter_mut().for_each(|c| *c = 0.0);
        self.gcd = 0.0;
        self.swing = 0.0;
    }

    /// Everything helpful on the unit now.
    pub fn buffs(&self) -> Vec<Debuff> {
        if self.dead {
            return Vec::new();
        }
        self.auras
            .iter()
            .filter(|a| !a.harmful)
            .map(|a| Debuff { key: a.key, name: a.name, school: a.school, left: a.left, total: a.def.secs, ctl: None })
            .collect()
    }

    pub fn hp_frac(&self) -> f32 {
        self.hp / self.max_hp
    }

    fn modifier(&self, pick: fn(AuraKind) -> Option<f32>) -> f32 {
        self.auras.iter().filter_map(|a| pick(a.def.kind)).product()
    }

    /// Damage dealt multiplier.
    pub fn dealt(&self) -> f32 {
        self.modifier(|k| if let AuraKind::Dealt(m) = k { Some(m) } else { None })
    }

    fn taken(&self) -> f32 {
        self.modifier(|k| if let AuraKind::Taken(m) = k { Some(m) } else { None })
    }

    /// Whether `slot` could go off now, given the target and the unit's own state.
    pub fn check(&self, slot: usize, s: Situation) -> Result<(), Fail> {
        let spell = self.spell(slot);
        if self.dead {
            return Err(Fail::Dead);
        }
        if self.cds[slot] > 0.0 || (spell.gcd && self.gcd > 0.0) {
            return Err(Fail::NotReady);
        }
        if self.casting.is_some() {
            return Err(Fail::Busy);
        }
        if self.cc.out() && !spell.free {
            return Err(Fail::Held);
        }
        if self.cc.silence_t > 0.0 && !spell.free && spell.school != School::Physical {
            return Err(Fail::Silenced);
        }
        if spell.cost > self.res {
            return Err(Fail::NoResource);
        }
        if spell.cast > 0.0 && s.moving {
            return Err(Fail::Moving);
        }
        match spell.aim {
            Aim::Myself | Aim::Around | Aim::Party => Ok(()),
            Aim::Friend => match s.target {
                // A friendly ability on no friend lands on yourself.
                Some(t) if !t.hostile && !t.dead => {
                    if t.dist > spell.range {
                        Err(Fail::OutOfRange)
                    } else if !t.sight {
                        Err(Fail::NoSight)
                    } else {
                        Ok(())
                    }
                }
                _ => Ok(()),
            },
            Aim::Enemy => {
                let t = s.target.ok_or(Fail::NoTarget)?;
                if !t.hostile || t.dead {
                    Err(Fail::WrongTarget)
                } else if t.dist > spell.range {
                    Err(Fail::OutOfRange)
                } else if !t.sight {
                    Err(Fail::NoSight)
                } else if !t.ahead {
                    Err(Fail::Facing)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Begin `slot` (already checked): the global cooldown, and a cast bar for a cast time.
    /// Returns true when it goes off now.
    pub fn begin(&mut self, slot: usize) -> bool {
        let spell = self.spell(slot);
        if spell.gcd {
            self.gcd = GCD;
        }
        if spell.cast > 0.0 {
            self.casting = Some(Casting { slot, left: spell.cast, total: spell.cast });
            false
        } else {
            self.pay(slot);
            true
        }
    }

    /// The cost and cooldown, when an ability goes off.
    pub fn pay(&mut self, slot: usize) {
        let spell = self.spell(slot);
        self.res = (self.res - spell.cost + spell.gain).clamp(0.0, MAX_RES);
        self.cds[slot] = spell.cooldown;
    }

    /// Advance a cast. Moving, a stun or a silence breaks it; returns the slot when it completes.
    pub fn advance(&mut self, dt: f32, moving: bool) -> Option<usize> {
        let c = self.casting.as_mut()?;
        if moving || self.cc.silenced() || self.dead {
            self.casting = None;
            return None;
        }
        c.left -= dt;
        if c.left <= 0.0 {
            let slot = c.slot;
            self.casting = None;
            self.pay(slot);
            Some(slot)
        } else {
            None
        }
    }

    /// Whether a white hit could reach its target now, whatever the swing timer says. A stun or a
    /// fear stops it, a cast pauses it, and an auto shot needs standing still.
    pub fn check_auto(&self, s: Situation) -> Result<(), Fail> {
        let a = self.class.auto;
        if self.dead {
            return Err(Fail::Dead);
        }
        if self.cc.out() {
            return Err(Fail::Held);
        }
        if self.casting.is_some() {
            return Err(Fail::Busy);
        }
        if a.range > 6.0 && s.moving {
            return Err(Fail::Moving);
        }
        let t = s.target.ok_or(Fail::NoTarget)?;
        if !t.hostile || t.dead {
            Err(Fail::WrongTarget)
        } else if t.dist > a.range {
            Err(Fail::OutOfRange)
        } else if !t.sight {
            Err(Fail::NoSight)
        } else if !t.ahead {
            Err(Fail::Facing)
        } else {
            Ok(())
        }
    }

    /// Time for a white hit: attacking, the swing timer run down, and the target in reach.
    pub fn can_swing(&self, s: Situation) -> bool {
        self.attacking && self.swing <= 0.0 && self.check_auto(s).is_ok()
    }

    /// A white hit goes off: the timer restarts, and rage builds.
    pub fn swung(&mut self) {
        let a = self.class.auto;
        self.swing = a.cooldown;
        self.res = (self.res + a.gain).clamp(0.0, MAX_RES);
    }

    /// Everything harmful on the unit now: controls first, strongest first, then the rest.
    pub fn debuffs(&self) -> Vec<Debuff> {
        if self.dead {
            return Vec::new();
        }
        let mut out: Vec<Debuff> = self
            .marks
            .iter()
            .filter_map(|m| {
                let left = self.cc.left_of(m.ctl);
                (left > 0.0).then_some(Debuff {
                    key: m.key,
                    name: m.name,
                    school: m.school,
                    left,
                    total: m.total.max(left),
                    ctl: Some(m.ctl),
                })
            })
            .collect();
        out.sort_by_key(|d| d.ctl.map_or(9, severity));
        out.extend(self.auras.iter().filter(|a| a.harmful).map(|a| Debuff {
            key: a.key,
            name: a.name,
            school: a.school,
            left: a.left,
            total: a.def.secs,
            ctl: None,
        }));
        out
    }

    /// The control that matters most on the unit now; a snare doesn't count.
    pub fn loss_of_control(&self) -> Option<Debuff> {
        self.debuffs().into_iter().find(|d| d.ctl.is_some_and(|c| !matches!(c, Ctl::Slow(..))))
    }

    /// Time passes: cooldowns, controls, resource, and auras ticking. Returns what the ticks did.
    pub fn tick(&mut self, dt: f32) -> Vec<Feedback> {
        let mut out = Vec::new();
        if self.dead {
            return out;
        }
        for cd in &mut self.cds {
            *cd = (*cd - dt).max(0.0);
        }
        self.gcd = (self.gcd - dt).max(0.0);
        self.immune_t = (self.immune_t - dt).max(0.0);
        self.trinket_cd = (self.trinket_cd - dt).max(0.0);
        self.swing = (self.swing - dt).max(0.0);
        self.cc.tick(dt);
        let bonus = self.auras.iter().map(|a| if let AuraKind::Regen(r) = a.def.kind { r } else { 0.0 }).sum::<f32>();
        let regen = match self.class.resource {
            Resource::Mana => 1.2 + bonus,
            Resource::Energy => 10.0,
            Resource::Rage => -0.5,
        };
        self.res = (self.res + regen * dt).clamp(0.0, MAX_RES);
        let mut dot = 0.0;
        let mut hot = 0.0;
        let mut school = School::Physical;
        for a in &mut self.auras {
            a.left -= dt;
            a.tick -= dt;
            if a.tick <= 0.0 && a.left > -1e-3 {
                a.tick += 1.0;
                match a.def.kind {
                    AuraKind::Dot(d) => {
                        dot += d;
                        school = a.school;
                    }
                    AuraKind::Hot(h) => hot += h,
                    _ => {}
                }
            }
        }
        self.auras.retain(|a| a.left > 0.0 && !(matches!(a.def.kind, AuraKind::Absorb(_)) && a.pool <= 0.0));
        self.refresh_health();
        if dot > 0.0 {
            out.extend(self.take(dot, school, false));
        }
        if hot > 0.0 {
            out.push(Feedback::Heal(self.mend(hot)));
        }
        out
    }

    /// Take damage through immunity, modifiers and absorbs; it breaks incapacitates, builds rage
    /// and may kill.
    pub fn take(&mut self, raw: f32, school: School, crit: bool) -> Vec<Feedback> {
        if self.dead || raw <= 0.0 {
            return Vec::new();
        }
        if self.immune_t > 0.0 {
            return vec![Feedback::Immune];
        }
        let mut dmg = raw * self.taken();
        let mut absorbed = 0.0;
        for a in &mut self.auras {
            if let AuraKind::Absorb(_) = a.def.kind {
                let take = dmg.min(a.pool);
                a.pool -= take;
                dmg -= take;
                absorbed += take;
            }
        }
        let mut out = Vec::new();
        if absorbed > 0.0 {
            out.push(Feedback::Absorbed(absorbed));
        }
        if dmg > 0.0 {
            self.hp -= dmg;
            self.cc.hurt();
            if self.class.resource == Resource::Rage {
                self.res = (self.res + dmg * 0.25).min(MAX_RES);
            }
            out.push(Feedback::Damage { amount: dmg, crit, school });
            if self.hp <= 0.0 {
                self.hp = 0.0;
                self.dead = true;
                self.casting = None;
                self.auras.clear();
            }
        }
        out
    }

    /// Heal, capped at full; returns what took.
    pub fn mend(&mut self, amount: f32) -> f32 {
        if self.dead {
            return 0.0;
        }
        let before = self.hp;
        self.hp = (self.hp + amount).min(self.max_hp);
        self.hp - before
    }

    /// The trinket: every control and snare gone.
    pub fn trinket(&mut self) -> bool {
        if self.dead || self.trinket_cd > 0.0 || !self.cc.any() {
            return false;
        }
        self.cc.clear();
        self.trinket_cd = TRINKET_COOLDOWN;
        true
    }

    /// Effects that land on yourself as the ability goes off: immunity, the blink's freeing.
    pub fn on_self(&mut self, spell: &Spell) {
        match spell.special {
            Special::Immune(t) => {
                self.immune_t = t;
                self.cc.clear();
                self.auras.retain(|a| !a.harmful);
                self.casting = None;
            }
            Special::Blink(_) | Special::Leap(_) => {
                self.cc.stun_t = 0.0;
                self.cc.root_t = 0.0;
            }
            _ => {}
        }
    }
}

/// How hard a control is, lowest first: what a player most needs to see.
fn severity(c: Ctl) -> u8 {
    match c {
        Ctl::Stun(_) => 0,
        Ctl::Incap(_) => 1,
        Ctl::Fear(_) => 2,
        Ctl::Silence(_) | Ctl::Interrupt(_) => 3,
        Ctl::Root(_) => 4,
        Ctl::Slow(..) => 5,
    }
}

/// The caster's side of a landing.
#[derive(Clone, Copy, Debug)]
pub struct Source {
    pub dealt: f32,
    pub crit: bool,
    pub from: Pt,
    pub hostile: bool,
}

/// Land `spell` on `target`. Returns the feedback and any healing owed to the caster (a drain).
pub fn land(spell: &'static Spell, src: Source, target: &mut Unit) -> (Vec<Feedback>, f32) {
    let mut out = Vec::new();
    let mut drain = 0.0;
    if target.dead {
        return (out, 0.0);
    }
    if src.hostile && target.immune_t > 0.0 {
        return (vec![Feedback::Immune], 0.0);
    }
    if spell.damage > 0.0 {
        let mut d = spell.damage * src.dealt;
        match spell.special {
            Special::Shatter(m) if target.cc.frozen() => d *= m,
            Special::Execute(f) if target.hp_frac() <= f => d = d.max(target.hp / target.taken().max(0.01)),
            _ => {}
        }
        // A white hit crits for double, an ability for half again.
        if src.crit {
            d *= if spell.ai == Use::Auto { 2.0 } else { 1.5 };
        }
        let fb = target.take(d, spell.school, src.crit);
        if let Special::Drain(f) = spell.special {
            drain = fb
                .iter()
                .map(|f| if let Feedback::Damage { amount, .. } = f { *amount } else { 0.0 })
                .sum::<f32>()
                * f;
        }
        out.extend(fb);
    }
    if spell.heal > 0.0 {
        let h = spell.heal * if src.crit { 1.5 } else { 1.0 };
        out.push(Feedback::Heal(target.mend(h)));
    }
    if let Some(def) = spell.aura {
        target.auras.retain(|a| a.name != spell.name);
        let pool = if let AuraKind::Absorb(p) = def.kind { p } else { 0.0 };
        target.auras.push(Aura {
            def,
            key: spell.key,
            name: spell.name,
            school: spell.school,
            left: def.secs,
            tick: 1.0,
            pool,
            harmful: src.hostile,
        });
        target.refresh_health();
    }
    if let Some(ctl) = spell.ctl {
        if !target.dead {
            let was_casting = target.casting.is_some();
            let landed = target.cc.land(ctl, 1.0, src.from);
            let total = match (landed, ctl) {
                (Landed::For(t), _) | (Landed::Not, Ctl::Slow(_, t)) => Some(t),
                _ => None,
            };
            if let Some(total) = total {
                // One mark a timer: a silence and a kick share theirs.
                let left = target.cc.left_of(ctl);
                let same_timer = |m: &Mark| std::mem::discriminant(&m.ctl) == std::mem::discriminant(&ctl)
                    || (matches!(m.ctl, Ctl::Silence(_) | Ctl::Interrupt(_)) && matches!(ctl, Ctl::Silence(_) | Ctl::Interrupt(_)));
                target.marks.retain(|m| !same_timer(m));
                target.marks.push(Mark { ctl, key: spell.key, name: spell.name, school: spell.school, total: total.max(left) });
            }
            match landed {
                Landed::Immune => out.push(Feedback::Immune),
                Landed::For(_) => {
                    if was_casting && target.cc.silenced() {
                        target.casting = None;
                        out.push(Feedback::Interrupted);
                    } else {
                        out.push(Feedback::Ctl(ctl));
                    }
                }
                Landed::Not => out.push(Feedback::Ctl(ctl)),
            }
        }
    }
    if spell.special == Special::Cleanse {
        let held = target.cc;
        target.cc.fear_t = 0.0;
        target.cc.incap_t = 0.0;
        target.cc.root_t = 0.0;
        target.cc.silence_t = 0.0;
        target.cc.slow_t = 0.0;
        let before = target.auras.len();
        target.auras.retain(|a| !a.harmful);
        if held.any() || target.auras.len() != before {
            out.push(Feedback::Cleansed);
        }
    }
    (out, drain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::class;

    fn at(dist: f32) -> Situation {
        Situation {
            target: Some(TargetInfo { hostile: true, dist, sight: true, ahead: true, dead: false }),
            moving: false,
        }
    }

    fn slot(u: &Unit, key: &str) -> usize {
        u.class.spells.iter().position(|s| s.key == key).unwrap()
    }

    const HIT: Source = Source { dealt: 1.0, crit: false, from: [0.0, 0.0], hostile: true };

    #[test]
    fn casting_needs_range_sight_facing_and_stillness() {
        let mage = Unit::new(class("arcanist"));
        let lance = slot(&mage, "frost_lance");
        assert_eq!(mage.check(lance, at(10.0)), Ok(()));
        assert_eq!(mage.check(lance, at(45.0)), Err(Fail::OutOfRange));
        let mut s = at(10.0);
        s.moving = true;
        assert_eq!(mage.check(lance, s), Err(Fail::Moving));
        let mut s = at(10.0);
        s.target.as_mut().unwrap().sight = false;
        assert_eq!(mage.check(lance, s), Err(Fail::NoSight));
        let mut s = at(10.0);
        s.target.as_mut().unwrap().ahead = false;
        assert_eq!(mage.check(lance, s), Err(Fail::Facing));
        assert_eq!(mage.check(lance, Situation { target: None, moving: false }), Err(Fail::NoTarget));
    }

    #[test]
    fn a_cast_completes_pays_and_respects_the_gcd() {
        let mut mage = Unit::new(class("arcanist"));
        let lance = slot(&mage, "frost_lance");
        assert!(!mage.begin(lance));
        assert_eq!(mage.check(lance, at(10.0)), Err(Fail::NotReady));
        assert_eq!(mage.advance(1.0, false), None);
        assert_eq!(mage.advance(1.1, false), Some(lance));
        assert!(mage.res < MAX_RES);
    }

    #[test]
    fn moving_or_a_kick_breaks_a_cast() {
        let mut mage = Unit::new(class("arcanist"));
        let lance = slot(&mage, "frost_lance");
        mage.begin(lance);
        assert_eq!(mage.advance(0.5, true), None);
        assert!(mage.casting.is_none());
        mage.gcd = 0.0;
        mage.begin(lance);
        let rogue = Unit::new(class("shadowblade"));
        let kick = rogue.spell(slot(&rogue, "throat_jab"));
        let (fb, _) = land(kick, HIT, &mut mage);
        assert!(fb.contains(&Feedback::Interrupted));
        assert!(mage.casting.is_none());
        mage.gcd = 0.0;
        assert_eq!(mage.check(lance, at(10.0)), Err(Fail::Silenced));
    }

    #[test]
    fn absorbs_and_immunity_soak_damage() {
        let mut u = Unit::new(class("vanguard"));
        let healer = Unit::new(class("mystic"));
        let ward = healer.spell(slot(&healer, "ward_of_light"));
        land(ward, Source { hostile: false, ..HIT }, &mut u);
        let fb = u.take(30.0, School::Fire, false);
        assert_eq!(fb, vec![Feedback::Absorbed(30.0)]);
        assert_eq!(u.hp, u.max_hp);
        u.immune_t = 1.0;
        assert_eq!(u.take(500.0, School::Fire, false), vec![Feedback::Immune]);
        u.immune_t = 0.0;
        u.take(5000.0, School::Fire, false);
        assert!(u.dead);
    }

    #[test]
    fn dots_tick_and_a_cleanse_lifts_them() {
        let mut u = Unit::new(class("vanguard"));
        let rogue = Unit::new(class("shadowblade"));
        let venom = rogue.spell(slot(&rogue, "venom_burst"));
        land(venom, HIT, &mut u);
        let hp = u.hp;
        u.tick(1.0);
        assert!(u.hp < hp);
        let healer = Unit::new(class("mystic"));
        let cleanse = healer.spell(slot(&healer, "cleanse"));
        let (fb, _) = land(cleanse, Source { hostile: false, ..HIT }, &mut u);
        assert!(fb.contains(&Feedback::Cleansed));
        assert!(u.auras.is_empty());
    }

    #[test]
    fn a_stunned_unit_can_only_use_what_is_free() {
        let mut mage = Unit::new(class("arcanist"));
        mage.cc.land(Ctl::Stun(4.0), 1.0, [0.0; 2]);
        assert_eq!(mage.check(slot(&mage, "frost_lance"), at(10.0)), Err(Fail::Held));
        let tomb = slot(&mage, "crystal_tomb");
        assert_eq!(mage.check(tomb, at(10.0)), Ok(()));
        mage.on_self(mage.spell(tomb));
        assert!(!mage.cc.any() && mage.immune_t > 0.0);
        assert!(!mage.trinket(), "nothing left to break");
    }

    #[test]
    fn debuffs_name_their_source_and_end_with_their_timers() {
        let mut u = Unit::new(class("vanguard"));
        let rogue = Unit::new(class("shadowblade"));
        land(rogue.spell(slot(&rogue, "venom_burst")), HIT, &mut u);
        land(rogue.spell(slot(&rogue, "hamstring_cut")), HIT, &mut u);
        assert!(u.loss_of_control().is_none(), "a snare and a dot aren't loss of control");
        land(rogue.spell(slot(&rogue, "gut_punch")), HIT, &mut u);
        let d = u.debuffs();
        assert_eq!(d[0].key, "gut_punch", "the stun leads");
        assert_eq!(d.len(), 3);
        assert_eq!(u.loss_of_control().map(|d| d.name), Some("Gut Punch"));
        assert!((d[0].left - 5.0).abs() < 1e-3 && d[0].total == 5.0);
        u.tick(2.0);
        assert!((u.debuffs()[0].left - 3.0).abs() < 1e-3);
        // A trinket ends the controls at once, whoever laid them; the dot stays.
        assert!(u.trinket());
        let d = u.debuffs();
        assert_eq!((d.len(), d[0].key), (1, "venom_burst"));
    }

    #[test]
    fn white_hits_swing_on_a_timer_build_rage_and_crit_for_double() {
        let mut v = Unit::new(class("vanguard"));
        let auto = v.class.auto;
        assert!(!v.can_swing(at(3.0)), "not until attacking");
        v.attacking = true;
        assert!(v.can_swing(at(3.0)));
        assert_eq!(v.check_auto(at(20.0)), Err(Fail::OutOfRange));
        v.swung();
        assert_eq!(v.res, auto.gain);
        assert!(!v.can_swing(at(3.0)), "the timer runs first");
        v.tick(auto.cooldown);
        assert!(v.can_swing(at(3.0)));

        let mut t = Unit::new(class("arcanist"));
        let hp = t.hp;
        land(auto, Source { crit: true, ..HIT }, &mut t);
        assert_eq!(hp - t.hp, auto.damage * 2.0);
    }

    #[test]
    fn a_stun_stops_a_white_hit_and_moving_stops_a_shot_not_a_blade() {
        let mut v = Unit::new(class("vanguard"));
        v.attacking = true;
        let mut moving = at(3.0);
        moving.moving = true;
        assert!(v.can_swing(moving), "melee swings on the run");
        v.cc.land(Ctl::Stun(2.0), 1.0, [0.0; 2]);
        assert_eq!(v.check_auto(at(3.0)), Err(Fail::Held));
        let mut r = Unit::new(class("ranger"));
        r.attacking = true;
        assert!(r.can_swing(at(30.0)));
        assert_eq!(r.check_auto(Situation { moving: true, ..at(30.0) }), Err(Fail::Moving));
    }

    #[test]
    fn a_caster_swings_only_up_close() {
        let mut m = Unit::new(class("arcanist"));
        m.attacking = true;
        assert!(m.can_swing(at(3.0)));
        assert_eq!(m.check_auto(at(20.0)), Err(Fail::OutOfRange));
    }

    #[test]
    fn buffs_raise_health_damage_crit_and_regen() {
        let mut u = Unit::new(class("arcanist"));
        let friend = Source { hostile: false, ..HIT };
        let base = u.max_hp;
        let healer = class("mystic");
        land(&healer.buffs[0], friend, &mut u);
        assert_eq!(u.max_hp, base * 1.12);
        assert_eq!(u.hp, u.max_hp, "the extra health comes filled");
        assert_eq!(u.buffs()[0].key, "vitality");
        assert!(u.debuffs().is_empty());
        // The same buff twice doesn't stack.
        land(&healer.buffs[0], friend, &mut u);
        assert_eq!(u.max_hp, base * 1.12);
        let r = class("ranger");
        land(&r.buffs[1], friend, &mut u);
        assert!((u.crit_chance() - (BASE_CRIT + 0.04)).abs() < 1e-5);
        let d = u.dealt();
        land(&r.buffs[0], friend, &mut u);
        assert!(u.dealt() > d);
        u.res = 0.0;
        u.tick(1.0);
        assert!(u.res > 1.2 - 1e-3);
    }

    #[test]
    fn a_cleanse_or_immunity_leaves_buffs_alone() {
        let mut u = Unit::new(class("vanguard"));
        let friend = Source { hostile: false, ..HIT };
        land(&class("mystic").buffs[0], friend, &mut u);
        let cleanse = Unit::new(class("mystic"));
        land(cleanse.spell(slot(&cleanse, "cleanse")), friend, &mut u);
        assert_eq!(u.buffs().len(), 1);
    }

    #[test]
    fn the_buff_slots_follow_the_abilities() {
        let u = Unit::new(class("mystic"));
        assert_eq!(u.spell(10).key, "vitality");
        assert_eq!(u.spell(11).key, "inner_calm");
        assert_eq!(u.check(10, Situation { target: None, moving: true }), Ok(()));
    }

    #[test]
    fn the_gates_opening_refills_all_but_rage() {
        let mut m = Unit::new(class("arcanist"));
        m.res = 10.0;
        m.hp = 5.0;
        m.gcd = 1.0;
        m.prepare();
        assert_eq!((m.res, m.hp, m.gcd), (MAX_RES, m.max_hp, 0.0));
        let mut v = Unit::new(class("vanguard"));
        v.prepare();
        assert_eq!(v.res, 0.0);
    }

    #[test]
    fn rage_builds_from_blows_and_hits() {
        let mut v = Unit::new(class("vanguard"));
        assert_eq!(v.res, 0.0);
        let strike = slot(&v, "steel_strike");
        v.begin(strike);
        assert!(v.res > 0.0);
        let r = v.res;
        v.take(40.0, School::Physical, false);
        assert!(v.res > r);
    }
}
