//! Crowd control, arena style: every control belongs to a diminishing-returns category, and each
//! one landed in a category inside [`DR_RESET`] seconds of the last lasts half as long, then a
//! quarter, then not at all, until the category has rested. Kicks lock out casting without
//! diminishing; snares never diminish.

use crate::Pt;

/// Seconds a category must rest, after its last control wears off, to be whole again.
pub const DR_RESET: f32 = 18.0;

/// A control or snare one unit lays on another.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ctl {
    /// Speed multiplier for seconds.
    Slow(f32, f32),
    Root(f32),
    Stun(f32),
    Fear(f32),
    /// Held for seconds; any damage breaks it.
    Incap(f32),
    /// No spellcasting for seconds; diminishes.
    Silence(f32),
    /// A kick: the cast breaks and spells lock out for seconds; never diminishes.
    Interrupt(f32),
}

impl Ctl {
    /// The seconds a diminishing control lasts before diminishing.
    pub fn secs(self) -> Option<f32> {
        match self {
            Self::Stun(t) | Self::Fear(t) | Self::Incap(t) | Self::Root(t) | Self::Silence(t) => Some(t),
            _ => None,
        }
    }

    /// What a status line calls it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Slow(..) => "Slowed",
            Self::Root(_) => "Rooted",
            Self::Stun(_) => "Stunned",
            Self::Fear(_) => "Feared",
            Self::Incap(_) => "Incapacitated",
            Self::Silence(_) => "Silenced",
            Self::Interrupt(_) => "Interrupted",
        }
    }
}

/// The diminishing-returns categories.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dr {
    Stun,
    Fear,
    /// Broken by any damage.
    Incap,
    Root,
    Silence,
}

impl Dr {
    const ALL: usize = 5;

    /// The category an effect diminishes in, `None` for the rest (snares and kicks included).
    pub fn of(ctl: Ctl) -> Option<Self> {
        match ctl {
            Ctl::Stun(_) => Some(Self::Stun),
            Ctl::Fear(_) => Some(Self::Fear),
            Ctl::Incap(_) => Some(Self::Incap),
            Ctl::Root(_) => Some(Self::Root),
            Ctl::Silence(_) => Some(Self::Silence),
            _ => None,
        }
    }
}

/// One unit's diminishing returns: per category, the controls landed and the seconds left
/// before the count resets.
#[derive(Clone, Copy, Default, Debug)]
pub struct Diminish([(u8, f32); Dr::ALL]);

impl Diminish {
    /// The seconds `secs` of control in `cat` actually last now, counted. 0 means immune.
    pub fn land(&mut self, cat: Dr, secs: f32) -> f32 {
        let (count, reset) = &mut self.0[cat as usize];
        let scale = match *count {
            0 => 1.0,
            1 => 0.5,
            2 => 0.25,
            _ => 0.0,
        };
        let lasts = secs * scale;
        if lasts > 0.0 {
            *count += 1;
            // The window runs from when this control wears off; an immune attempt adds nothing.
            *reset = lasts + DR_RESET;
        }
        lasts
    }

    pub fn tick(&mut self, dt: f32) {
        for (count, reset) in &mut self.0 {
            if *count > 0 {
                *reset -= dt;
                if *reset <= 0.0 {
                    *count = 0;
                    *reset = 0.0;
                }
            }
        }
    }
}

/// How a control landed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Landed {
    /// Not a control: the effect does something else.
    Not,
    /// Diminished to nothing.
    Immune,
    /// Held this many seconds.
    For(f32),
}

/// What holds a unit, rival or hero: the timed controls, the snare, and their diminishing returns.
#[derive(Clone, Copy, Debug)]
pub struct Held {
    /// Speed multiplier while `slow_t` runs.
    pub slow: f32,
    pub slow_t: f32,
    pub root_t: f32,
    pub stun_t: f32,
    pub fear_t: f32,
    pub incap_t: f32,
    pub silence_t: f32,
    /// Where a fear runs from.
    pub from: Pt,
    pub dr: Diminish,
}

impl Default for Held {
    fn default() -> Self {
        Self {
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            incap_t: 0.0,
            silence_t: 0.0,
            from: [0.0, 0.0],
            dr: Diminish::default(),
        }
    }
}

impl Held {
    /// Land `effect` for `scale` of its seconds (a champion shrugs most of it off), through the
    /// diminishing returns.
    pub fn land(&mut self, ctl: Ctl, scale: f32, from: Pt) -> Landed {
        match ctl {
            Ctl::Slow(m, t) => {
                self.slow = if self.slow_t > 0.0 { self.slow.min(m) } else { m };
                self.slow_t = self.slow_t.max(t * scale);
                return Landed::Not;
            }
            Ctl::Interrupt(t) => {
                self.silence_t = self.silence_t.max(t * scale);
                return Landed::For(t * scale);
            }
            _ => {}
        }
        let (Some(cat), Some(secs)) = (Dr::of(ctl), ctl.secs()) else {
            return Landed::Not;
        };
        let secs = self.dr.land(cat, secs * scale);
        if secs <= 0.0 {
            return Landed::Immune;
        }
        let slot = match cat {
            Dr::Stun => &mut self.stun_t,
            Dr::Fear => &mut self.fear_t,
            Dr::Incap => &mut self.incap_t,
            Dr::Root => &mut self.root_t,
            Dr::Silence => &mut self.silence_t,
        };
        *slot = slot.max(secs);
        if cat == Dr::Fear {
            self.from = from;
        }
        Landed::For(secs)
    }

    pub fn tick(&mut self, dt: f32) {
        for t in [
            &mut self.slow_t,
            &mut self.root_t,
            &mut self.stun_t,
            &mut self.fear_t,
            &mut self.incap_t,
            &mut self.silence_t,
        ] {
            *t = (*t - dt).max(0.0);
        }
        if self.slow_t <= 0.0 {
            self.slow = 1.0;
        }
        self.dr.tick(dt);
    }

    /// Stunned, rooted or incapacitated: going nowhere of its own will.
    pub fn held(&self) -> bool {
        self.stun_t > 0.0 || self.root_t > 0.0 || self.incap_t > 0.0
    }

    /// Stunned, feared or incapacitated: no swing, no spell.
    pub fn out(&self) -> bool {
        self.stun_t > 0.0 || self.fear_t > 0.0 || self.incap_t > 0.0
    }

    /// No spellcasting: out of the fight, or silenced.
    pub fn silenced(&self) -> bool {
        self.out() || self.silence_t > 0.0
    }

    /// Rooted or stunned, for Shatter.
    pub fn frozen(&self) -> bool {
        self.root_t > 0.0 || self.stun_t > 0.0
    }

    pub fn speed(&self) -> f32 {
        if self.slow_t > 0.0 { self.slow } else { 1.0 }
    }

    /// Damage breaks an incapacitate.
    pub fn hurt(&mut self) {
        self.incap_t = 0.0;
    }

    /// Every control and snare gone (a trinket, an immunity); the diminishing returns stay.
    pub fn clear(&mut self) {
        *self = Self {
            dr: self.dr,
            ..Self::default()
        };
    }

    /// Anything at all holding the unit: what a trinket has to break.
    pub fn any(&self) -> bool {
        self.out() || self.held() || self.silence_t > 0.0 || self.slow_t > 0.0
    }

    /// The control holding the unit now, for a status line: the strongest first.
    pub fn status(&self) -> Option<(&'static str, f32)> {
        [
            ("Stunned", self.stun_t),
            ("Incapacitated", self.incap_t),
            ("Feared", self.fear_t),
            ("Rooted", self.root_t),
            ("Silenced", self.silence_t),
        ]
        .into_iter()
        .find(|&(_, t)| t > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: Pt = [0.0, 0.0];

    #[test]
    fn controls_in_one_category_halve_then_quarter_then_miss() {
        let mut d = Diminish::default();
        assert_eq!(d.land(Dr::Stun, 4.0), 4.0);
        assert_eq!(d.land(Dr::Stun, 4.0), 2.0);
        assert_eq!(d.land(Dr::Stun, 4.0), 1.0);
        assert_eq!(d.land(Dr::Stun, 4.0), 0.0);
    }

    #[test]
    fn categories_diminish_apart() {
        let mut d = Diminish::default();
        d.land(Dr::Stun, 4.0);
        d.land(Dr::Stun, 4.0);
        assert_eq!(d.land(Dr::Fear, 8.0), 8.0);
    }

    #[test]
    fn a_rested_category_is_whole_again() {
        let mut d = Diminish::default();
        d.land(Dr::Root, 4.0);
        assert_eq!(d.land(Dr::Root, 4.0), 2.0);
        d.tick(2.0 + DR_RESET - 0.1);
        assert_eq!(d.land(Dr::Root, 4.0), 1.0);
        d.tick(1.0 + DR_RESET + 0.1);
        assert_eq!(d.land(Dr::Root, 4.0), 4.0);
    }

    #[test]
    fn an_immune_attempt_does_not_hold_the_window_open() {
        let mut d = Diminish::default();
        for _ in 0..3 {
            d.land(Dr::Fear, 8.0);
        }
        d.tick(DR_RESET);
        assert_eq!(d.land(Dr::Fear, 8.0), 0.0);
        d.tick(2.1);
        assert_eq!(d.land(Dr::Fear, 8.0), 8.0);
    }

    #[test]
    fn a_trinket_clears_controls_but_not_the_diminishing() {
        let mut h = Held::default();
        assert_eq!(h.land(Ctl::Stun(4.0), 1.0, O), Landed::For(4.0));
        assert!(h.out() && h.held() && h.frozen());
        h.clear();
        assert!(!h.any());
        assert_eq!(h.land(Ctl::Stun(4.0), 1.0, O), Landed::For(2.0));
    }

    #[test]
    fn damage_breaks_an_incapacitate_but_not_a_stun() {
        let mut h = Held::default();
        h.land(Ctl::Incap(8.0), 1.0, O);
        h.land(Ctl::Stun(2.0), 1.0, O);
        h.hurt();
        assert_eq!(h.incap_t, 0.0);
        assert!(h.stun_t > 0.0);
        assert_eq!(h.status().map(|s| s.0), Some("Stunned"));
    }

    #[test]
    fn a_snare_keeps_the_stronger_and_never_diminishes() {
        let mut h = Held::default();
        for _ in 0..5 {
            assert_eq!(h.land(Ctl::Slow(0.5, 3.0), 1.0, O), Landed::Not);
        }
        h.land(Ctl::Slow(0.7, 1.0), 1.0, O);
        assert_eq!(h.speed(), 0.5);
        h.tick(3.0);
        assert_eq!(h.speed(), 1.0);
    }

    #[test]
    fn kicks_lock_out_without_diminishing_and_spare_the_feet() {
        let mut h = Held::default();
        for _ in 0..5 {
            assert_eq!(h.land(Ctl::Interrupt(4.0), 1.0, O), Landed::For(4.0));
        }
        assert!(h.silenced() && !h.out() && !h.held());
        assert_eq!(h.land(Ctl::Silence(4.0), 1.0, O), Landed::For(4.0));
    }

    #[test]
    fn a_fear_remembers_what_to_run_from() {
        let mut h = Held::default();
        h.land(Ctl::Fear(3.0), 1.0, [5.0, -2.0]);
        assert_eq!(h.from, [5.0, -2.0]);
    }
}
