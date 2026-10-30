//! The classes, their abilities and talent specs, and how an ability grows with its rank.
//! Every name here is Ember Arena's own.

/// An ability's magic (or lack of it): its colour on screen and its icon's.
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
    /// Linear-ish sRGB, 0..1.
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
pub enum Kind {
    /// A missile at the nearest rivals.
    Bolt,
    /// An instant hit on the nearest rivals in range.
    Strike,
    /// A close hit on the nearest rivals in reach.
    Melee,
    /// Everything in a radius around the hero.
    Nova,
    /// Everything in a cone toward the nearest rival.
    Cone,
    /// An area at the thickest pack in `range` (under the hero when `range` is 0), ticking each
    /// second for `duration`, or striking once when that is 0.
    Ground,
    /// Jumps from rival to rival.
    Chain,
    /// Damage over time on several rivals.
    Dot,
    /// Only at a rival mid-cast: breaks the cast and locks it out. Never fires at nothing.
    Interrupt,
    /// Cuts damage taken by the `damage` fraction for `duration`, when hurt with a rival close.
    Guard,
    /// Raises all damage by the `damage` fraction for `duration`, with a crowd or a champion near.
    Empower,
    /// Immune to all damage for `duration`, breaking every control: the panic button.
    Immune,
    /// Absorbs `damage` for `duration`.
    Shield,
    /// Heals `damage` over `duration`, below 85% health.
    Heal,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    None,
    /// Speed multiplier for seconds.
    Slow(f32, f32),
    Root(f32),
    Stun(f32),
    Fear(f32),
    /// Held for seconds, but any damage breaks it.
    Incap(f32),
    /// No casting for seconds; diminishes.
    Silence(f32),
    /// A kick: the cast breaks and casting locks out for seconds; never diminishes.
    Interrupt(f32),
    /// Outgoing damage multiplier for seconds.
    Weaken(f32, f32),
    /// Heals the hero by this fraction of the damage dealt.
    Drain(f32),
    /// Kills outright below this health fraction.
    Execute(f32),
}

impl Effect {
    /// The seconds a control lasts before diminishing; `None` for everything else.
    pub fn control_secs(self) -> Option<f32> {
        match self {
            Self::Stun(t) | Self::Fear(t) | Self::Incap(t) | Self::Root(t) | Self::Silence(t) => {
                Some(t)
            }
            _ => None,
        }
    }

    /// Controls a smart player spends on the rival that is casting.
    pub fn seeks_casters(self) -> bool {
        matches!(self, Self::Incap(_) | Self::Silence(_) | Self::Fear(_))
    }
}

pub struct Ability {
    pub key: &'static str,
    pub name: &'static str,
    pub school: School,
    pub kind: Kind,
    /// Direct damage; per tick for areas; absorb, heal, or the fraction for Guard and Empower.
    pub damage: f32,
    /// Damage over time, per second, for `duration`.
    pub dot: f32,
    pub cooldown: f32,
    pub range: f32,
    /// Splash for single hits, area for the area kinds, the jump for a chain.
    pub radius: f32,
    pub count: u32,
    pub duration: f32,
    pub effect: Effect,
    /// Damage multiplier on a frozen (rooted or stunned) rival. 0 for none.
    pub shatter: f32,
    /// Aimed at the toughest rival in range rather than the nearest.
    pub toughest: bool,
}

const fn ab(key: &'static str, name: &'static str, school: School, kind: Kind) -> Ability {
    Ability {
        key,
        name,
        school,
        kind,
        damage: 0.0,
        dot: 0.0,
        cooldown: 2.0,
        range: 20.0,
        radius: 0.0,
        count: 1,
        duration: 0.0,
        effect: Effect::None,
        shatter: 0.0,
        toughest: false,
    }
}

macro_rules! ability {
    ($key:literal, $name:literal, $school:ident, $kind:ident { $($f:ident: $v:expr),* $(,)? }) => {
        Ability { $($f: $v,)* ..ab($key, $name, School::$school, Kind::$kind) }
    };
}

pub static ABILITIES: &[Ability] = &[
    // --- Vanguard: sword and board, a shield wall that hits back. ---
    ability!("cleaving_blow", "Cleaving Blow", Physical, Melee {
        damage: 14.0, cooldown: 1.1, range: 5.5, count: 3
    }),
    ability!("rampart_strike", "Rampart Strike", Physical, Melee {
        damage: 10.0, cooldown: 12.0, range: 5.5, effect: Effect::Stun(3.0)
    }),
    ability!("crippling_cut", "Crippling Cut", Physical, Melee {
        damage: 6.0, cooldown: 3.0, range: 5.5, count: 2, effect: Effect::Slow(0.5, 8.0)
    }),
    ability!("pommel_strike", "Pommel Strike", Physical, Interrupt {
        damage: 6.0, cooldown: 8.0, range: 6.0, effect: Effect::Interrupt(4.0)
    }),
    ability!("daunting_roar", "Daunting Roar", Physical, Nova {
        cooldown: 18.0, radius: 8.0, effect: Effect::Fear(4.0)
    }),
    ability!("judging_light", "Judging Light", Holy, Strike {
        damage: 22.0, cooldown: 4.0, range: 14.0, count: 2
    }),
    ability!("aegis", "Aegis", Holy, Immune {
        cooldown: 60.0, duration: 5.0
    }),
    ability!("bastion_slam", "Bastion Slam", Physical, Melee {
        damage: 36.0, cooldown: 3.0, range: 5.5
    }),
    ability!("quake", "Quake", Physical, Cone {
        damage: 18.0, cooldown: 14.0, radius: 9.0, effect: Effect::Stun(3.0)
    }),
    ability!("iron_wall", "Iron Wall", Physical, Guard {
        damage: 0.6, cooldown: 45.0, duration: 10.0
    }),
    ability!("radiant_strike", "Radiant Strike", Holy, Melee {
        damage: 26.0, cooldown: 1.6, range: 5.5
    }),
    ability!("sunburst", "Sunburst", Holy, Nova {
        damage: 18.0, cooldown: 3.0, radius: 6.0, effect: Effect::Drain(0.15)
    }),
    ability!("penitence", "Penitence", Holy, Strike {
        cooldown: 15.0, range: 20.0, effect: Effect::Incap(6.0)
    }),
    ability!("dawnfury", "Dawnfury", Holy, Empower {
        damage: 0.3, cooldown: 50.0, range: 12.0, duration: 15.0
    }),
    ability!("rending_strike", "Rending Strike", Physical, Melee {
        damage: 48.0, cooldown: 3.0, range: 6.0
    }),
    ability!("lacerate", "Lacerate", Physical, Dot {
        dot: 4.0, cooldown: 3.0, range: 6.0, count: 2, duration: 12.0
    }),
    ability!("whirling_steel", "Whirling Steel", Physical, Nova {
        damage: 16.0, cooldown: 4.0, radius: 7.0
    }),
    ability!("battle_trance", "Battle Trance", Physical, Empower {
        damage: 0.3, cooldown: 40.0, range: 10.0, duration: 12.0
    }),
    // --- Berserker: a great axe and nothing held back. ---
    ability!("wide_chop", "Wide Chop", Physical, Melee {
        damage: 16.0, cooldown: 1.2, range: 6.0, count: 3
    }),
    ability!("gash", "Gash", Physical, Melee {
        damage: 8.0, cooldown: 3.0, range: 6.0, count: 2, effect: Effect::Slow(0.5, 6.0)
    }),
    ability!("throat_punch", "Throat Punch", Physical, Interrupt {
        damage: 8.0, cooldown: 8.0, range: 6.0, effect: Effect::Interrupt(4.0)
    }),
    ability!("bellow", "Bellow", Physical, Nova {
        cooldown: 18.0, radius: 9.0, effect: Effect::Fear(4.0)
    }),
    ability!("red_mist", "Red Mist", Physical, Empower {
        damage: 0.3, cooldown: 40.0, range: 10.0, duration: 12.0
    }),
    ability!("hardened_skin", "Hardened Skin", Nature, Guard {
        damage: 0.35, cooldown: 40.0, duration: 12.0
    }),
    ability!("gorge", "Gorge", Physical, Melee {
        damage: 34.0, cooldown: 2.5, range: 6.0, effect: Effect::Drain(0.2)
    }),
    ability!("frenzy_spin", "Frenzy Spin", Physical, Nova {
        damage: 18.0, cooldown: 3.5, radius: 7.5
    }),
    ability!("crimson_whirl", "Crimson Whirl", Physical, Ground {
        damage: 12.0, cooldown: 22.0, range: 0.0, radius: 8.0, duration: 6.0
    }),
    ability!("earthshaker", "Earthshaker", Nature, Cone {
        damage: 20.0, cooldown: 14.0, radius: 10.0, effect: Effect::Stun(3.0)
    }),
    ability!("granite_will", "Granite Will", Physical, Guard {
        damage: 0.6, cooldown: 45.0, duration: 10.0
    }),
    ability!("rockhide", "Rockhide", Nature, Shield {
        damage: 45.0, cooldown: 20.0, duration: 12.0
    }),
    ability!("headtaker", "Headtaker", Physical, Strike {
        damage: 40.0, cooldown: 2.0, range: 8.0, count: 2, effect: Effect::Execute(0.25)
    }),
    ability!("hurl_axe", "Hurl Axe", Physical, Bolt {
        damage: 26.0, cooldown: 2.0, range: 22.0
    }),
    ability!("bonebreaker", "Bonebreaker", Physical, Melee {
        damage: 16.0, cooldown: 12.0, range: 6.0, effect: Effect::Stun(4.0)
    }),
    // --- Arcanist: frost, fire and raw aether from a staff. ---
    ability!("frost_lance", "Frost Lance", Frost, Bolt {
        damage: 12.0, cooldown: 1.3, range: 24.0, effect: Effect::Slow(0.5, 2.5)
    }),
    ability!("flame_burst", "Flame Burst", Fire, Strike {
        damage: 26.0, cooldown: 3.5, range: 18.0
    }),
    ability!("frost_ring", "Frost Ring", Frost, Nova {
        damage: 8.0, cooldown: 7.0, radius: 9.0, effect: Effect::Root(2.5)
    }),
    ability!("spellbreak", "Spellbreak", Arcane, Interrupt {
        damage: 4.0, cooldown: 10.0, range: 30.0, effect: Effect::Interrupt(4.0)
    }),
    ability!("bewilder", "Bewilder", Arcane, Strike {
        cooldown: 10.0, range: 30.0, effect: Effect::Incap(8.0)
    }),
    ability!("crystal_tomb", "Crystal Tomb", Frost, Immune {
        cooldown: 60.0, duration: 6.0
    }),
    ability!("shatter_shard", "Shatter Shard", Frost, Strike {
        damage: 9.0, cooldown: 1.0, range: 26.0, shatter: 3.0
    }),
    ability!("deep_chill", "Deep Chill", Frost, Strike {
        damage: 24.0, cooldown: 12.0, range: 20.0, effect: Effect::Stun(5.0), shatter: 2.0
    }),
    ability!("glacial_barrier", "Glacial Barrier", Frost, Shield {
        damage: 40.0, cooldown: 20.0, duration: 12.0
    }),
    ability!("ice_squall", "Ice Squall", Frost, Ground {
        damage: 7.0, cooldown: 9.0, range: 20.0, radius: 7.0, duration: 6.0,
        effect: Effect::Slow(0.6, 1.5)
    }),
    ability!("living_ember", "Living Ember", Fire, Strike {
        damage: 6.0, dot: 5.0, duration: 8.0, cooldown: 3.0, range: 26.0, radius: 4.0, count: 2
    }),
    ability!("inferno_orb", "Inferno Orb", Fire, Bolt {
        damage: 70.0, dot: 6.0, duration: 6.0, cooldown: 6.0, range: 30.0, toughest: true
    }),
    ability!("searing_cone", "Searing Cone", Fire, Cone {
        damage: 16.0, cooldown: 14.0, radius: 10.0, effect: Effect::Incap(4.0)
    }),
    ability!("kindle", "Kindle", Fire, Empower {
        damage: 0.35, cooldown: 45.0, range: 18.0, duration: 8.0
    }),
    ability!("aether_missiles", "Aether Missiles", Arcane, Bolt {
        damage: 6.0, cooldown: 1.9, range: 22.0, count: 3
    }),
    ability!("rift_barrage", "Rift Barrage", Arcane, Strike {
        damage: 16.0, cooldown: 1.8, range: 26.0, count: 2
    }),
    ability!("gravity_snare", "Gravity Snare", Arcane, Strike {
        damage: 2.0, cooldown: 6.0, range: 26.0, count: 3, effect: Effect::Slow(0.4, 8.0)
    }),
    ability!("overcharge", "Overcharge", Arcane, Empower {
        damage: 0.3, cooldown: 45.0, range: 18.0, duration: 12.0
    }),
    // --- Shadowblade: twin knives from the dark. ---
    ability!("twin_slash", "Twin Slash", Physical, Melee {
        damage: 15.0, cooldown: 0.85, range: 5.5
    }),
    ability!("gut_punch", "Gut Punch", Physical, Melee {
        damage: 12.0, cooldown: 6.0, range: 6.0, effect: Effect::Stun(3.5)
    }),
    ability!("throat_jab", "Throat Jab", Physical, Interrupt {
        damage: 8.0, cooldown: 7.0, range: 6.0, effect: Effect::Interrupt(5.0)
    }),
    ability!("blinding_powder", "Blinding Powder", Nature, Strike {
        cooldown: 16.0, range: 12.0, effect: Effect::Incap(8.0)
    }),
    ability!("finishing_cut", "Finishing Cut", Physical, Melee {
        damage: 42.0, cooldown: 2.8, range: 5.5
    }),
    ability!("shadow_cloak", "Shadow Cloak", Shadow, Immune {
        cooldown: 60.0, duration: 5.0
    }),
    ability!("toxic_stab", "Toxic Stab", Nature, Melee {
        damage: 40.0, cooldown: 1.6, range: 5.5
    }),
    ability!("venom_burst", "Venom Burst", Nature, Melee {
        damage: 28.0, dot: 5.0, duration: 6.0, cooldown: 3.5, range: 5.5, count: 2
    }),
    ability!("bleed_out", "Bleed Out", Physical, Dot {
        dot: 5.0, cooldown: 3.5, range: 10.0, count: 3, duration: 6.0
    }),
    ability!("flurry_dance", "Flurry Dance", Physical, Chain {
        damage: 20.0, cooldown: 12.0, range: 10.0, radius: 10.0, count: 4
    }),
    ability!("parry_stance", "Parry Stance", Physical, Guard {
        damage: 0.5, cooldown: 40.0, duration: 10.0
    }),
    ability!("knife_fan", "Knife Fan", Physical, Nova {
        damage: 12.0, cooldown: 2.5, radius: 8.0
    }),
    ability!("adrenal_surge", "Adrenal Surge", Physical, Empower {
        damage: 0.3, cooldown: 40.0, range: 10.0, duration: 12.0
    }),
    ability!("night_cut", "Night Cut", Shadow, Melee {
        damage: 24.0, cooldown: 1.2, range: 5.5
    }),
    ability!("shadowgrip", "Shadowgrip", Shadow, Melee {
        damage: 4.0, cooldown: 10.0, range: 5.5, effect: Effect::Stun(4.0)
    }),
    ability!("choke_wire", "Choke Wire", Physical, Dot {
        dot: 6.0, cooldown: 8.0, range: 6.0, duration: 9.0, effect: Effect::Silence(3.0)
    }),
    // --- Ranger: a crossbow, traps, and a keen eye. ---
    ability!("quick_bolt", "Quick Bolt", Physical, Bolt {
        damage: 13.0, cooldown: 1.0, range: 28.0
    }),
    ability!("spread_shot", "Spread Shot", Physical, Bolt {
        damage: 10.0, cooldown: 2.0, range: 26.0, count: 3
    }),
    ability!("barbed_bolt", "Barbed Bolt", Nature, Bolt {
        damage: 2.0, dot: 4.0, duration: 9.0, cooldown: 2.6, range: 26.0, count: 2
    }),
    ability!("pinning_shot", "Pinning Shot", Physical, Bolt {
        damage: 8.0, cooldown: 3.0, range: 28.0, effect: Effect::Slow(0.5, 4.0)
    }),
    ability!("snare_trap", "Snare Trap", Frost, Ground {
        cooldown: 16.0, range: 14.0, radius: 4.0, effect: Effect::Incap(8.0)
    }),
    ability!("hushing_shot", "Hushing Shot", Arcane, Interrupt {
        damage: 8.0, cooldown: 10.0, range: 30.0, effect: Effect::Interrupt(3.0)
    }),
    ability!("deflect", "Deflect", Physical, Guard {
        damage: 0.75, cooldown: 50.0, duration: 5.0
    }),
    ability!("hawk_strike", "Hawk Strike", Nature, Strike {
        damage: 40.0, cooldown: 3.0, range: 25.0
    }),
    ability!("pack_fury", "Pack Fury", Nature, Empower {
        damage: 0.3, cooldown: 45.0, range: 25.0, duration: 12.0
    }),
    ability!("arrow_rain", "Arrow Rain", Physical, Ground {
        damage: 8.0, cooldown: 9.0, range: 24.0, radius: 7.0, duration: 5.0
    }),
    ability!("piercing_shot", "Piercing Shot", Physical, Bolt {
        damage: 48.0, cooldown: 4.0, range: 32.0, toughest: true
    }),
    ability!("final_shot", "Final Shot", Physical, Strike {
        damage: 50.0, cooldown: 6.0, range: 30.0, effect: Effect::Execute(0.2)
    }),
    ability!("sleep_dart", "Sleep Dart", Nature, Bolt {
        cooldown: 20.0, range: 28.0, effect: Effect::Incap(6.0)
    }),
    ability!("blast_trap", "Blast Trap", Fire, Ground {
        damage: 22.0, dot: 4.0, cooldown: 8.0, range: 14.0, radius: 5.0
    }),
    ability!("volatile_bolt", "Volatile Bolt", Fire, Bolt {
        damage: 16.0, dot: 6.0, duration: 3.0, cooldown: 2.5, range: 28.0, radius: 3.0
    }),
];

pub fn ability(key: &str) -> &'static Ability {
    ABILITIES
        .iter()
        .find(|a| a.key == key)
        .unwrap_or_else(|| panic!("no ability {key:?}"))
}

/// The character model a class or rival wears; the game maps it to art.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Knight,
    Barbarian,
    Mage,
    Rogue,
    HoodedRogue,
}

/// The class's escape, cast on F once learned.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Move {
    /// Jump this many yards toward the cursor.
    Blink(f32),
    /// Rush at the rival nearest the cursor within this many yards, stunning it.
    Charge(f32),
    /// Rivals lose the hero, and he runs this much faster, for `secs`.
    Vanish(f32),
}

pub struct Mobility {
    pub name: &'static str,
    pub kind: Move,
    pub cooldown: f32,
    pub secs: f32,
    pub desc: &'static str,
}

/// One of a class's three talent specs: its signature ability (learned on the pick) and the
/// abilities it adds at level-up.
pub struct Spec {
    pub name: &'static str,
    pub desc: &'static str,
    /// The signature ability first.
    pub pool: &'static [&'static str],
}

pub struct Class {
    pub key: &'static str,
    pub name: &'static str,
    pub model: Model,
    /// 0xRRGGBB.
    pub color: u32,
    pub start: &'static str,
    /// The class's core abilities, offered from level 1.
    pub pool: &'static [&'static str],
    pub specs: [Spec; 3],
    pub mobility: Mobility,
    /// Extra health over [`BASE_HP`]: the armoured classes stand up to more.
    pub toughness: f32,
}

pub static CLASSES: &[Class] = &[
    Class {
        key: "vanguard",
        name: "Vanguard",
        model: Model::Knight,
        color: 0xC9A35C,
        start: "cleaving_blow",
        pool: &[
            "cleaving_blow",
            "rampart_strike",
            "crippling_cut",
            "pommel_strike",
            "daunting_roar",
            "judging_light",
            "aegis",
        ],
        specs: [
            Spec {
                name: "Bulwark",
                desc: "An unmoving wall of steel that turns every blow back.",
                pool: &["bastion_slam", "quake", "iron_wall"],
            },
            Spec {
                name: "Crusader",
                desc: "A knight of the dawn who heals through the fight.",
                pool: &["radiant_strike", "sunburst", "penitence", "dawnfury"],
            },
            Spec {
                name: "Warlord",
                desc: "A duelist who opens wounds and finishes the fallen.",
                pool: &["rending_strike", "lacerate", "whirling_steel", "battle_trance"],
            },
        ],
        mobility: Mobility {
            name: "Shield Rush",
            kind: Move::Charge(22.0),
            cooldown: 8.0,
            secs: 1.5,
            desc: "Rush the rival nearest the cursor within 22 yards, stunning it.",
        },
        toughness: 40.0,
    },
    Class {
        key: "berserker",
        name: "Berserker",
        model: Model::Barbarian,
        color: 0xB5482D,
        start: "wide_chop",
        pool: &[
            "wide_chop",
            "gash",
            "throat_punch",
            "bellow",
            "red_mist",
            "hardened_skin",
        ],
        specs: [
            Spec {
                name: "Carnage",
                desc: "Feeds on the fight, healing with every heavy blow.",
                pool: &["gorge", "frenzy_spin", "crimson_whirl"],
            },
            Spec {
                name: "Ironhide",
                desc: "Shakes the ground and shrugs off the crowd.",
                pool: &["earthshaker", "granite_will", "rockhide"],
            },
            Spec {
                name: "Headhunter",
                desc: "Throws axes and takes heads.",
                pool: &["headtaker", "hurl_axe", "bonebreaker"],
            },
        ],
        mobility: Mobility {
            name: "Skull Leap",
            kind: Move::Blink(16.0),
            cooldown: 10.0,
            secs: 0.0,
            desc: "Leap 16 yards toward the cursor.",
        },
        toughness: 25.0,
    },
    Class {
        key: "arcanist",
        name: "Arcanist",
        model: Model::Mage,
        color: 0x5FB9F2,
        start: "frost_lance",
        pool: &[
            "frost_lance",
            "flame_burst",
            "frost_ring",
            "spellbreak",
            "bewilder",
            "crystal_tomb",
        ],
        specs: [
            Spec {
                name: "Rime",
                desc: "Freezes rivals in place and shatters them.",
                pool: &["shatter_shard", "deep_chill", "glacial_barrier", "ice_squall"],
            },
            Spec {
                name: "Pyre",
                desc: "Sets the arena alight with living flame.",
                pool: &["living_ember", "inferno_orb", "searing_cone", "kindle"],
            },
            Spec {
                name: "Aether",
                desc: "Bends raw aether into barrages and gravity.",
                pool: &["aether_missiles", "rift_barrage", "gravity_snare", "overcharge"],
            },
        ],
        mobility: Mobility {
            name: "Phase Step",
            kind: Move::Blink(18.0),
            cooldown: 10.0,
            secs: 0.0,
            desc: "Step through the aether 18 yards toward the cursor.",
        },
        toughness: 0.0,
    },
    Class {
        key: "shadowblade",
        name: "Shadowblade",
        model: Model::Rogue,
        color: 0xE8D45C,
        start: "twin_slash",
        pool: &[
            "twin_slash",
            "gut_punch",
            "throat_jab",
            "blinding_powder",
            "finishing_cut",
            "shadow_cloak",
        ],
        specs: [
            Spec {
                name: "Venom",
                desc: "Coats every blade in poison.",
                pool: &["toxic_stab", "venom_burst", "bleed_out"],
            },
            Spec {
                name: "Duelist",
                desc: "Dances through the crowd, parrying as it goes.",
                pool: &["flurry_dance", "parry_stance", "knife_fan", "adrenal_surge"],
            },
            Spec {
                name: "Umbra",
                desc: "Strikes from the dark and chokes the casters.",
                pool: &["night_cut", "shadowgrip", "choke_wire", "knife_fan"],
            },
        ],
        mobility: Mobility {
            name: "Smoke Veil",
            kind: Move::Vanish(1.4),
            cooldown: 18.0,
            secs: 4.0,
            desc: "Vanish in smoke: rivals lose you and you run 40% faster for 4 sec.",
        },
        toughness: 10.0,
    },
    Class {
        key: "ranger",
        name: "Ranger",
        model: Model::HoodedRogue,
        color: 0x7DBE58,
        start: "quick_bolt",
        pool: &[
            "quick_bolt",
            "spread_shot",
            "barbed_bolt",
            "pinning_shot",
            "snare_trap",
            "hushing_shot",
            "deflect",
        ],
        specs: [
            Spec {
                name: "Beastcaller",
                desc: "Calls hawks and the pack to the hunt.",
                pool: &["hawk_strike", "pack_fury", "arrow_rain"],
            },
            Spec {
                name: "Deadeye",
                desc: "One bolt, one fall.",
                pool: &["piercing_shot", "final_shot", "sleep_dart"],
            },
            Spec {
                name: "Trapper",
                desc: "Mines the arena with fire and snares.",
                pool: &["blast_trap", "volatile_bolt", "arrow_rain"],
            },
        ],
        mobility: Mobility {
            name: "Tumble",
            kind: Move::Blink(14.0),
            cooldown: 8.0,
            secs: 0.0,
            desc: "Roll 14 yards toward the cursor.",
        },
        toughness: 15.0,
    },
];

pub fn class(key: &str) -> &'static Class {
    CLASSES
        .iter()
        .find(|c| c.key == key)
        .unwrap_or_else(|| panic!("no class {key:?}"))
}

pub const BASE_HP: f32 = 120.0;
pub const BASE_SPEED: f32 = 7.0;
pub const BASE_MAGNET: f32 = 4.5;
pub const MAX_ABILITY_LEVEL: u8 = 8;
pub const MAX_PASSIVE_LEVEL: u8 = 5;
/// Class picks; a spec's signature ability is a slot of its own on top.
pub const MAX_ABILITIES: usize = 6;
pub const MAX_PASSIVES: usize = 6;
/// The level a run picks its spec at.
pub const SPEC_LEVEL: u32 = 10;
/// The F escape is learned here.
pub const MOBILITY_LEVEL: u32 = 5;
/// The arena trinket, on Q: breaks every control.
pub const TRINKET_COOLDOWN: f32 = 120.0;

/// An ability the hero has, at its rank.
#[derive(Clone, Copy)]
pub struct Owned {
    pub def: &'static Ability,
    pub level: u8,
    /// Seconds to ready.
    pub cd: f32,
}

impl Owned {
    pub fn new(def: &'static Ability) -> Self {
        Self { def, level: 1, cd: 0.3 }
    }
    pub fn damage_mult(&self) -> f32 {
        1.0 + 0.3 * f32::from(self.level - 1)
    }
    pub fn cooldown(&self, stats: &Stats) -> f32 {
        let base = self.def.cooldown * 0.93f32.powi(i32::from(self.level) - 1);
        (base * (1.0 - stats.haste).max(0.35)).max(0.2)
    }
    pub fn count(&self) -> u32 {
        let extra = [3u8, 5, 7].iter().filter(|&&l| self.level >= l).count() as u32;
        // A control or a kick stays on one target: ranks make it come round sooner instead.
        if self.def.effect.control_secs().is_some() || self.def.kind == Kind::Interrupt {
            self.def.count
        } else {
            self.def.count + extra
        }
    }
    pub fn radius(&self, stats: &Stats) -> f32 {
        self.def.radius * (1.0 + 0.08 * f32::from(self.level - 1)) * stats.area
    }
    pub fn duration(&self) -> f32 {
        self.def.duration * (1.0 + 0.1 * f32::from(self.level - 1))
    }
    /// The numbers one cast uses now.
    pub fn cast(&self, stats: &Stats) -> Cast {
        let mult = match self.def.kind {
            // A fraction, not damage: Guard and Empower don't grow with Might.
            Kind::Guard | Kind::Empower => 1.0,
            _ => self.damage_mult() * stats.might,
        };
        Cast {
            dmg: self.def.damage * mult,
            dot: self.def.dot * mult,
            count: self.count(),
            radius: self.radius(stats),
            duration: self.duration(),
            cooldown: self.cooldown(stats),
        }
    }
}

/// An ability's numbers at a rank and the hero's stats.
#[derive(Clone, Copy, Debug)]
pub struct Cast {
    pub dmg: f32,
    pub dot: f32,
    pub count: u32,
    pub radius: f32,
    pub duration: f32,
    pub cooldown: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stat {
    Might,
    MaxHp,
    Speed,
    Regen,
    Haste,
    Area,
    Magnet,
    Growth,
    Armor,
    Crit,
}

pub struct Passive {
    pub key: &'static str,
    pub name: &'static str,
    pub stat: Stat,
    pub per_level: f32,
    pub school: School,
}

pub static PASSIVES: &[Passive] = &[
    Passive { key: "might", name: "Ember Might", stat: Stat::Might, per_level: 0.10, school: School::Fire },
    Passive { key: "vigor", name: "Vigor", stat: Stat::MaxHp, per_level: 0.20, school: School::Nature },
    Passive { key: "swift", name: "Swiftfoot", stat: Stat::Speed, per_level: 0.10, school: School::Physical },
    Passive { key: "renewal", name: "Renewal", stat: Stat::Regen, per_level: 0.5, school: School::Holy },
    Passive { key: "quicken", name: "Quickened Mind", stat: Stat::Haste, per_level: 0.08, school: School::Arcane },
    Passive { key: "reach", name: "Wide Reach", stat: Stat::Area, per_level: 0.10, school: School::Arcane },
    Passive { key: "lodestone", name: "Lodestone", stat: Stat::Magnet, per_level: 0.40, school: School::Physical },
    Passive { key: "insight", name: "Insight", stat: Stat::Growth, per_level: 0.08, school: School::Holy },
    Passive { key: "fortify", name: "Fortify", stat: Stat::Armor, per_level: 1.0, school: School::Physical },
    Passive { key: "precision", name: "Precision", stat: Stat::Crit, per_level: 0.05, school: School::Shadow },
];

/// The hero's derived numbers, from the class, the passives and any offensive cooldown.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub might: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub regen: f32,
    pub haste: f32,
    pub area: f32,
    pub magnet: f32,
    pub growth: f32,
    pub armor: f32,
    pub crit: f32,
}

impl Stats {
    pub fn of(class: &Class, passives: &[(&'static Passive, u8)], empower: f32) -> Self {
        let mut s = Self {
            might: 1.0,
            max_hp: BASE_HP + class.toughness,
            speed: BASE_SPEED + if class.toughness <= 0.0 { 0.5 } else { 0.0 },
            regen: 0.0,
            haste: 0.0,
            area: 1.0,
            magnet: BASE_MAGNET,
            growth: 1.0,
            armor: 0.0,
            crit: 0.05,
        };
        for &(p, level) in passives {
            let v = p.per_level * f32::from(level);
            match p.stat {
                Stat::Might => s.might += v,
                Stat::MaxHp => s.max_hp *= 1.0 + v,
                Stat::Speed => s.speed *= 1.0 + v,
                Stat::Regen => s.regen += v,
                Stat::Haste => s.haste += v,
                Stat::Area => s.area += v,
                Stat::Magnet => s.magnet *= 1.0 + v,
                Stat::Growth => s.growth += v,
                Stat::Armor => s.armor += v,
                Stat::Crit => s.crit += v,
            }
        }
        s.might *= 1.0 + empower;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pool_names_a_real_ability_once() {
        for c in CLASSES {
            assert!(c.pool.contains(&c.start), "{} starts outside its pool", c.name);
            for s in &c.specs {
                assert!(!s.pool.is_empty());
                for k in c.pool.iter().chain(s.pool) {
                    ability(k);
                }
                for k in s.pool {
                    assert!(!c.pool.contains(k), "{k} is in {}'s core and {}", c.name, s.name);
                }
            }
        }
    }

    #[test]
    fn keys_and_names_are_unique() {
        for (i, a) in ABILITIES.iter().enumerate() {
            for b in &ABILITIES[i + 1..] {
                assert_ne!(a.key, b.key);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn every_class_can_interrupt() {
        for c in CLASSES {
            let all = || c.pool.iter().chain(c.specs.iter().flat_map(|s| s.pool.iter()));
            assert!(all().any(|k| ability(k).kind == Kind::Interrupt), "{}", c.name);
            assert!(
                all().any(|k| matches!(ability(k).kind, Kind::Immune | Kind::Guard)),
                "{} has no defensive",
                c.name
            );
        }
        for a in ABILITIES.iter().filter(|a| a.kind == Kind::Interrupt) {
            assert!(matches!(a.effect, Effect::Interrupt(t) | Effect::Silence(t) if t > 0.0));
        }
    }

    #[test]
    fn ranks_grow_damage_and_shorten_cooldowns() {
        let stats = Stats::of(&CLASSES[0], &[], 0.0);
        let mut o = Owned::new(ability("frost_lance"));
        let one = o.cast(&stats);
        o.level = MAX_ABILITY_LEVEL;
        let top = o.cast(&stats);
        assert!(top.dmg > one.dmg && top.cooldown < one.cooldown && top.count > one.count);
        // A control keeps one target at every rank.
        let mut b = Owned::new(ability("bewilder"));
        b.level = MAX_ABILITY_LEVEL;
        assert_eq!(b.count(), 1);
    }

    #[test]
    fn an_offensive_cooldown_raises_might() {
        let c = &CLASSES[0];
        assert!(Stats::of(c, &[], 0.3).might > Stats::of(c, &[], 0.0).might);
    }
}
