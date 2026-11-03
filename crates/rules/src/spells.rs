//! The classes and their ten abilities each, cast by hand from an action bar: cast times, a
//! global cooldown, a resource to spend, range, and what each does when it lands. Every name here
//! is Ember Arena's own.

use crate::control::Ctl;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum School {
    Physical,
    Fire,
    Frost,
    Arcane,
    Shadow,
    Nature,
    Holy,
}

impl School {
    /// sRGB, 0..1.
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Physical => [0.85, 0.82, 0.76],
            Self::Fire => [1.0, 0.45, 0.1],
            Self::Frost => [0.45, 0.8, 1.0],
            Self::Arcane => [0.8, 0.45, 1.0],
            Self::Shadow => [0.55, 0.25, 0.75],
            Self::Nature => [0.45, 0.9, 0.3],
            Self::Holy => [1.0, 0.88, 0.45],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    /// 100, regenerating slowly; casters spend it.
    Mana,
    /// 100, regenerating fast; rogues spend it.
    Energy,
    /// Starts empty; built by builders and by being hit.
    Rage,
}

impl Resource {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mana => "Mana",
            Self::Energy => "Energy",
            Self::Rage => "Rage",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Melee,
    Ranged,
    Healer,
}

/// What an ability is aimed at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aim {
    /// The hostile target.
    Enemy,
    /// A friendly target, or yourself when the target isn't friendly.
    Friend,
    /// Yourself.
    Myself,
    /// Every enemy within `radius` of you.
    Around,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AuraKind {
    /// Damage each second.
    Dot(f32),
    /// Healing each second.
    Hot(f32),
    /// Absorbs this much damage.
    Absorb(f32),
    /// Damage taken multiplier.
    Taken(f32),
    /// Damage dealt multiplier.
    Dealt(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AuraDef {
    pub kind: AuraKind,
    pub secs: f32,
}

/// What an ability does beyond damage, healing, an aura and a control.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Special {
    None,
    /// Jump this far toward where you face, out of roots and stuns.
    Blink(f32),
    /// Rush to the target.
    Charge,
    /// Step behind the target.
    Behind,
    /// Leap this far backward.
    Leap(f32),
    /// Immune to all damage and control for seconds, breaking what holds you.
    Immune(f32),
    /// Removes controls (not stuns) and damage over time from a friend.
    Cleanse,
    /// Damage times this on a rooted or stunned target.
    Shatter(f32),
    /// Kills outright a target below this health fraction (else hits normally).
    Execute(f32),
    /// Heals you for this fraction of the damage dealt.
    Drain(f32),
}

/// How the AI thinks of an ability.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Use {
    /// Spam it.
    Filler,
    /// A big hit or dot: use when ready.
    Damage,
    Heal,
    Interrupt,
    /// Control someone, ideally not the kill target.
    Control,
    /// Slow a runner.
    Snare,
    /// A cooldown for when you're dying.
    Defensive,
    /// A cooldown for when the target is going down.
    Burst,
    /// Close the gap.
    Gap,
    /// Get away.
    Escape,
    Cleanse,
    /// The white hit: not on the bar, it swings by itself while attacking.
    Auto,
}

pub struct Spell {
    pub key: &'static str,
    pub name: &'static str,
    pub school: School,
    pub aim: Aim,
    /// Seconds to cast; 0 is instant. A cast breaks if you move.
    pub cast: f32,
    pub cooldown: f32,
    pub cost: f32,
    /// Yards; melee is 5.
    pub range: f32,
    /// For [`Aim::Around`].
    pub radius: f32,
    /// Triggers the global cooldown.
    pub gcd: bool,
    pub damage: f32,
    pub heal: f32,
    /// Resource gained (rage builders).
    pub gain: f32,
    pub aura: Option<AuraDef>,
    pub ctl: Option<Ctl>,
    pub special: Special,
    /// Usable while stunned, feared or incapacitated.
    pub free: bool,
    pub ai: Use,
}

const fn sp(key: &'static str, name: &'static str, school: School, aim: Aim, ai: Use) -> Spell {
    Spell {
        key,
        name,
        school,
        aim,
        cast: 0.0,
        cooldown: 0.0,
        cost: 0.0,
        range: 30.0,
        radius: 0.0,
        gcd: true,
        damage: 0.0,
        heal: 0.0,
        gain: 0.0,
        aura: None,
        ctl: None,
        special: Special::None,
        free: false,
        ai,
    }
}

macro_rules! spell {
    ($key:literal, $name:literal, $school:ident, $aim:ident, $ai:ident { $($f:ident: $v:expr),* $(,)? }) => {
        Spell { $($f: $v,)* ..sp($key, $name, School::$school, Aim::$aim, Use::$ai) }
    };
}

const fn aura(kind: AuraKind, secs: f32) -> Option<AuraDef> {
    Some(AuraDef { kind, secs })
}

const MELEE: f32 = 5.0;
pub const GCD: f32 = 1.5;

/// The model a class wears; the game maps it to art.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Knight,
    Barbarian,
    Mage,
    Rogue,
    HoodedRogue,
}

/// What a fighter carries; the game maps it to the model's weapon meshes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gear {
    SwordShield,
    GreatAxe,
    Staff,
    Wand,
    Knives,
    Crossbow,
}

pub struct Class {
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    pub model: Model,
    pub gear: Gear,
    pub role: Role,
    pub resource: Resource,
    pub health: f32,
    /// 0xRRGGBB.
    pub color: u32,
    /// The white hit or auto shot: `cooldown` is its swing time, `gain` the rage each swing builds.
    /// A caster's is a feeble swing of the staff.
    pub auto: &'static Spell,
    pub spells: &'static [Spell],
}

pub static CLASSES: &[Class] = &[
    Class {
        key: "vanguard",
        name: "Vanguard",
        blurb: "Plate and shield. Builds rage with every blow, stuns and charges.",
        model: Model::Knight,
        gear: Gear::SwordShield,
        role: Role::Melee,
        resource: Resource::Rage,
        health: 620.0,
        color: 0xC9A35C,
        auto: &spell!("white_sword", "Attack", Physical, Enemy, Auto { range: MELEE, damage: 16.0, cooldown: 2.4, gain: 10.0, gcd: false }),
        spells: &[
            spell!("steel_strike", "Steel Strike", Physical, Enemy, Filler { range: MELEE, damage: 20.0, gain: 15.0 }),
            spell!("rending_strike", "Rending Strike", Physical, Enemy, Damage { range: MELEE, damage: 46.0, cost: 30.0, cooldown: 6.0 }),
            spell!("crippling_cut", "Crippling Cut", Physical, Enemy, Snare { range: MELEE, damage: 8.0, cost: 10.0, ctl: Some(Ctl::Slow(0.5, 10.0)) }),
            spell!("shield_rush", "Shield Rush", Physical, Enemy, Gap { range: 25.0, cooldown: 15.0, gain: 20.0, gcd: false, special: Special::Charge, ctl: Some(Ctl::Stun(1.5)) }),
            spell!("pommel_strike", "Pommel Strike", Physical, Enemy, Interrupt { range: MELEE, cost: 10.0, cooldown: 10.0, gcd: false, damage: 6.0, ctl: Some(Ctl::Interrupt(4.0)) }),
            spell!("rampart_strike", "Rampart Strike", Physical, Enemy, Control { range: MELEE, cost: 15.0, cooldown: 30.0, damage: 10.0, ctl: Some(Ctl::Stun(4.0)) }),
            spell!("daunting_roar", "Daunting Roar", Physical, Around, Control { radius: 8.0, cost: 20.0, cooldown: 30.0, ctl: Some(Ctl::Fear(6.0)) }),
            spell!("judging_light", "Judging Light", Holy, Enemy, Damage { range: 20.0, damage: 26.0, cost: 10.0, cooldown: 8.0 }),
            spell!("iron_wall", "Iron Wall", Physical, Myself, Defensive { cooldown: 60.0, gcd: false, free: true, aura: aura(AuraKind::Taken(0.5), 8.0) }),
            spell!("second_wind", "Second Wind", Holy, Myself, Defensive { cooldown: 45.0, cost: 20.0, aura: aura(AuraKind::Hot(18.0), 10.0) }),
        ],
    },
    Class {
        key: "berserker",
        name: "Berserker",
        blurb: "A great axe and no restraint. Hits hardest, heals off the carnage.",
        model: Model::Barbarian,
        gear: Gear::GreatAxe,
        role: Role::Melee,
        resource: Resource::Rage,
        health: 600.0,
        color: 0xB5482D,
        auto: &spell!("white_axe", "Attack", Physical, Enemy, Auto { range: MELEE, damage: 24.0, cooldown: 3.2, gain: 14.0, gcd: false }),
        spells: &[
            spell!("wide_chop", "Wide Chop", Physical, Enemy, Filler { range: MELEE, damage: 22.0, gain: 15.0 }),
            spell!("gorge", "Gorge", Physical, Enemy, Damage { range: MELEE, damage: 42.0, cost: 25.0, cooldown: 6.0, special: Special::Drain(0.3) }),
            spell!("gash", "Gash", Physical, Enemy, Snare { range: MELEE, damage: 8.0, cost: 10.0, ctl: Some(Ctl::Slow(0.5, 10.0)) }),
            spell!("skull_leap", "Skull Leap", Physical, Enemy, Gap { range: 30.0, cooldown: 20.0, gcd: false, special: Special::Charge }),
            spell!("throat_punch", "Throat Punch", Physical, Enemy, Interrupt { range: MELEE, cost: 10.0, cooldown: 10.0, gcd: false, damage: 6.0, ctl: Some(Ctl::Interrupt(4.0)) }),
            spell!("bonebreaker", "Bonebreaker", Physical, Enemy, Control { range: MELEE, cost: 15.0, cooldown: 30.0, damage: 12.0, ctl: Some(Ctl::Stun(4.0)) }),
            spell!("bellow", "Bellow", Physical, Around, Control { radius: 8.0, cost: 20.0, cooldown: 30.0, ctl: Some(Ctl::Fear(6.0)) }),
            spell!("red_mist", "Red Mist", Physical, Myself, Burst { cooldown: 60.0, gcd: false, aura: aura(AuraKind::Dealt(1.3), 15.0) }),
            spell!("hardened_skin", "Hardened Skin", Nature, Myself, Defensive { cooldown: 60.0, gcd: false, free: true, aura: aura(AuraKind::Taken(0.6), 10.0) }),
            spell!("hurl_axe", "Hurl Axe", Physical, Enemy, Snare { range: 25.0, damage: 18.0, cost: 10.0, cooldown: 10.0, ctl: Some(Ctl::Slow(0.6, 6.0)) }),
        ],
    },
    Class {
        key: "arcanist",
        name: "Arcanist",
        blurb: "Frost and fire from range. Roots, transforms, and blinks away.",
        model: Model::Mage,
        gear: Gear::Staff,
        role: Role::Ranged,
        resource: Resource::Mana,
        health: 480.0,
        color: 0x5FB9F2,
        auto: &spell!("white_staff", "Attack", Physical, Enemy, Auto { range: MELEE, damage: 6.0, cooldown: 2.8, gcd: false }),
        spells: &[
            spell!("frost_lance", "Frost Lance", Frost, Enemy, Filler { cast: 2.0, damage: 38.0, cost: 4.0, ctl: Some(Ctl::Slow(0.6, 6.0)) }),
            spell!("flame_burst", "Flame Burst", Fire, Enemy, Damage { range: 20.0, damage: 24.0, cost: 6.0, cooldown: 8.0 }),
            spell!("inferno_orb", "Inferno Orb", Fire, Enemy, Damage { cast: 3.0, damage: 72.0, cost: 9.0, cooldown: 10.0 }),
            spell!("frost_ring", "Frost Ring", Frost, Around, Control { radius: 10.0, damage: 8.0, cost: 6.0, cooldown: 25.0, ctl: Some(Ctl::Root(6.0)) }),
            spell!("spellbreak", "Spellbreak", Arcane, Enemy, Interrupt { cooldown: 24.0, cost: 5.0, gcd: false, ctl: Some(Ctl::Interrupt(5.0)) }),
            spell!("bewilder", "Bewilder", Arcane, Enemy, Control { cast: 1.5, cost: 8.0, ctl: Some(Ctl::Incap(10.0)) }),
            spell!("phase_step", "Phase Step", Arcane, Myself, Escape { cooldown: 15.0, gcd: false, free: false, special: Special::Blink(20.0) }),
            spell!("crystal_tomb", "Crystal Tomb", Frost, Myself, Defensive { cooldown: 120.0, gcd: false, free: true, special: Special::Immune(8.0) }),
            spell!("shatter_shard", "Shatter Shard", Frost, Enemy, Damage { damage: 14.0, cost: 3.0, special: Special::Shatter(3.0) }),
            spell!("deep_chill", "Deep Chill", Frost, Enemy, Control { damage: 20.0, cost: 6.0, cooldown: 30.0, ctl: Some(Ctl::Stun(5.0)) }),
        ],
    },
    Class {
        key: "shadowblade",
        name: "Shadowblade",
        blurb: "Twin knives and energy. Stuns, blinds and kicks, then vanishes behind you.",
        model: Model::Rogue,
        gear: Gear::Knives,
        role: Role::Melee,
        resource: Resource::Energy,
        health: 520.0,
        color: 0xE8D45C,
        auto: &spell!("white_knives", "Attack", Physical, Enemy, Auto { range: MELEE, damage: 9.0, cooldown: 1.3, gcd: false }),
        spells: &[
            spell!("twin_slash", "Twin Slash", Physical, Enemy, Filler { range: MELEE, damage: 22.0, cost: 40.0 }),
            spell!("finishing_cut", "Finishing Cut", Physical, Enemy, Damage { range: MELEE, damage: 52.0, cost: 35.0, cooldown: 8.0 }),
            spell!("venom_burst", "Venom Burst", Nature, Enemy, Damage { range: MELEE, damage: 12.0, cost: 30.0, cooldown: 4.0, aura: aura(AuraKind::Dot(8.0), 10.0) }),
            spell!("hamstring_cut", "Hamstring Cut", Physical, Enemy, Snare { range: MELEE, cost: 25.0, ctl: Some(Ctl::Slow(0.5, 12.0)) }),
            spell!("throat_jab", "Throat Jab", Physical, Enemy, Interrupt { range: MELEE, cost: 25.0, cooldown: 10.0, gcd: false, ctl: Some(Ctl::Interrupt(5.0)) }),
            spell!("gut_punch", "Gut Punch", Physical, Enemy, Control { range: MELEE, cost: 25.0, cooldown: 20.0, ctl: Some(Ctl::Stun(5.0)) }),
            spell!("blinding_powder", "Blinding Powder", Nature, Enemy, Control { range: 10.0, cost: 30.0, cooldown: 60.0, ctl: Some(Ctl::Incap(8.0)) }),
            spell!("shadow_step", "Shadow Step", Shadow, Enemy, Gap { range: 25.0, cooldown: 20.0, gcd: false, special: Special::Behind }),
            spell!("shadow_cloak", "Shadow Cloak", Shadow, Myself, Defensive { cooldown: 90.0, gcd: false, special: Special::Immune(3.0) }),
            spell!("parry_dance", "Parry Dance", Physical, Myself, Defensive { cooldown: 60.0, gcd: false, free: true, aura: aura(AuraKind::Taken(0.5), 10.0) }),
        ],
    },
    Class {
        key: "ranger",
        name: "Ranger",
        blurb: "A crossbow at long range. Traps, snares, and a roll out of trouble.",
        model: Model::HoodedRogue,
        gear: Gear::Crossbow,
        role: Role::Ranged,
        resource: Resource::Mana,
        health: 520.0,
        color: 0x7DBE58,
        auto: &spell!("auto_shot", "Auto Shot", Physical, Enemy, Auto { range: 35.0, damage: 12.0, cooldown: 2.0, gcd: false }),
        spells: &[
            spell!("quick_bolt", "Quick Bolt", Physical, Enemy, Filler { range: 35.0, damage: 22.0, cost: 3.0 }),
            spell!("piercing_shot", "Piercing Shot", Physical, Enemy, Damage { range: 35.0, cast: 2.5, damage: 64.0, cost: 8.0, cooldown: 8.0 }),
            spell!("barbed_bolt", "Barbed Bolt", Nature, Enemy, Damage { range: 35.0, cost: 5.0, cooldown: 6.0, aura: aura(AuraKind::Dot(7.0), 15.0) }),
            spell!("pinning_shot", "Pinning Shot", Physical, Enemy, Snare { range: 35.0, damage: 8.0, cost: 4.0, cooldown: 10.0, ctl: Some(Ctl::Slow(0.5, 8.0)) }),
            spell!("hushing_shot", "Hushing Shot", Arcane, Enemy, Interrupt { range: 35.0, cost: 5.0, cooldown: 20.0, gcd: false, ctl: Some(Ctl::Silence(3.0)) }),
            spell!("snare_trap", "Snare Trap", Frost, Enemy, Control { range: 15.0, cost: 6.0, cooldown: 30.0, ctl: Some(Ctl::Incap(8.0)) }),
            spell!("tumble", "Tumble", Physical, Myself, Escape { cooldown: 20.0, gcd: false, special: Special::Leap(15.0) }),
            spell!("deflect", "Deflect", Physical, Myself, Defensive { cooldown: 90.0, gcd: false, free: true, aura: aura(AuraKind::Taken(0.25), 5.0) }),
            spell!("final_shot", "Final Shot", Physical, Enemy, Burst { range: 35.0, damage: 40.0, cost: 6.0, cooldown: 10.0, special: Special::Execute(0.2) }),
            spell!("sleep_dart", "Sleep Dart", Nature, Enemy, Control { range: 20.0, cost: 6.0, cooldown: 30.0, ctl: Some(Ctl::Incap(4.0)) }),
        ],
    },
    Class {
        key: "mystic",
        name: "Mystic",
        blurb: "The healer. Mends, wards and cleanses friends; fears and silences foes.",
        model: Model::Mage,
        gear: Gear::Wand,
        role: Role::Healer,
        resource: Resource::Mana,
        health: 500.0,
        color: 0xF2F2E6,
        auto: &spell!("white_wand", "Attack", Physical, Enemy, Auto { range: MELEE, damage: 5.0, cooldown: 2.6, gcd: false }),
        spells: &[
            spell!("mending_light", "Mending Light", Holy, Friend, Heal { range: 40.0, cast: 2.0, heal: 60.0, cost: 6.0 }),
            spell!("swift_mend", "Swift Mend", Holy, Friend, Heal { range: 40.0, heal: 32.0, cost: 8.0, cooldown: 8.0 }),
            spell!("renewing_bloom", "Renewing Bloom", Nature, Friend, Heal { range: 40.0, cost: 4.0, aura: aura(AuraKind::Hot(9.0), 12.0) }),
            spell!("ward_of_light", "Ward of Light", Holy, Friend, Heal { range: 40.0, cost: 6.0, cooldown: 6.0, aura: aura(AuraKind::Absorb(50.0), 15.0) }),
            spell!("searing_word", "Searing Word", Holy, Enemy, Filler { cast: 1.5, damage: 26.0, cost: 4.0 }),
            spell!("shadow_lash", "Shadow Lash", Shadow, Enemy, Damage { cost: 4.0, aura: aura(AuraKind::Dot(6.0), 15.0), cooldown: 3.0 }),
            spell!("terrify", "Terrify", Shadow, Around, Control { radius: 8.0, cost: 6.0, cooldown: 30.0, ctl: Some(Ctl::Fear(8.0)) }),
            spell!("silencing_word", "Silencing Word", Shadow, Enemy, Interrupt { cost: 5.0, cooldown: 45.0, gcd: false, ctl: Some(Ctl::Silence(4.0)) }),
            spell!("guardian_ward", "Guardian Ward", Holy, Friend, Defensive { range: 40.0, cooldown: 60.0, gcd: false, free: true, aura: aura(AuraKind::Taken(0.6), 8.0) }),
            spell!("cleanse", "Cleanse", Holy, Friend, Cleanse { range: 40.0, cost: 5.0, special: Special::Cleanse }),
        ],
    },
];

pub fn class(key: &str) -> &'static Class {
    CLASSES.iter().find(|c| c.key == key).unwrap_or_else(|| panic!("no class {key:?}"))
}

/// The arena trinket: breaks every control, once every two minutes.
pub const TRINKET_COOLDOWN: f32 = 120.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_ten_unique_spells() {
        for c in CLASSES {
            assert_eq!(c.spells.len(), 10, "{}", c.name);
            for (i, a) in c.spells.iter().enumerate() {
                assert!(c.spells[i + 1..].iter().all(|b| b.key != a.key && b.name != a.name));
            }
        }
        let all: Vec<&str> = CLASSES.iter().flat_map(|c| c.spells.iter().map(|s| s.key)).collect();
        for (i, k) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(k), "{k} twice");
        }
    }

    #[test]
    fn every_class_has_the_arena_basics() {
        for c in CLASSES {
            let has = |u: Use| c.spells.iter().any(|s| s.ai == u);
            assert!(has(Use::Interrupt), "{} can't interrupt", c.name);
            assert!(has(Use::Control), "{} can't control", c.name);
            assert!(has(Use::Defensive), "{} has no defensive", c.name);
            let first = if c.role == Role::Healer { Use::Heal } else { Use::Filler };
            assert!(c.spells[0].ai == first, "{} slot 1 should be its bread and butter", c.name);
        }
        assert!(CLASSES.iter().any(|c| c.role == Role::Healer));
    }

    #[test]
    fn everyone_has_a_white_hit_and_a_casters_is_feeble() {
        let dps = |c: &Class| c.auto.damage / c.auto.cooldown;
        let is_caster = |c: &Class| c.resource == Resource::Mana && c.key != "ranger";
        let weakest_fighter = CLASSES.iter().filter(|c| !is_caster(c)).map(dps).fold(f32::MAX, f32::min);
        for c in CLASSES {
            let filler = c.spells.iter().find(|s| s.ai == Use::Filler).expect("a filler");
            assert!(dps(c) < filler.damage / GCD, "{} white hits outdo its filler", c.name);
            assert_eq!(c.auto.gain > 0.0, c.resource == Resource::Rage, "{}", c.name);
            if is_caster(c) {
                assert!(dps(c) < weakest_fighter * 0.5, "{} swings too hard", c.name);
            }
        }
    }

    #[test]
    fn interrupts_lock_out_or_silence() {
        for s in CLASSES.iter().flat_map(|c| c.spells.iter()).filter(|s| s.ai == Use::Interrupt) {
            assert!(matches!(s.ctl, Some(Ctl::Interrupt(_) | Ctl::Silence(_))), "{}", s.key);
        }
    }
}
