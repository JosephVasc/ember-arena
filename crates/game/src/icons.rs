//! The ability icons, painted by this code at startup so they're the game's own: a glyph built from
//! signed-distance shapes, lit in the ability's school colour over a brushed background, in a
//! bevelled frame. Swap in painted art later by replacing [`Icons`]' images.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rules::spells::{CLASSES, School};

pub fn plugin(app: &mut App) {
    app.add_systems(PreStartup, build);
}

const SIZE: usize = 96;

/// The trinket's key in [`Icons`].
pub const TRINKET: &str = "trinket";

#[derive(Resource, Default)]
pub struct Icons(HashMap<&'static str, Handle<Image>>);

impl Icons {
    /// An ability's icon by its key.
    pub fn get(&self, key: &str) -> Handle<Image> {
        self.0.get(key).cloned().unwrap_or_default()
    }
}

fn build(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut icons = Icons::default();
    let mut add = |key: &'static str, color: [f32; 3], images: &mut Assets<Image>| {
        let (g, turn) = look(key).unwrap_or((Glyph::Orb, 0.0));
        let image = Image::new(
            Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            paint(g, turn, color, seed(key)),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        icons.0.insert(key, images.add(image));
    };
    for c in CLASSES {
        for s in c.spells {
            add(s.key, tint(s.school, c.color), &mut images);
        }
        add(c.auto.key, tint(c.auto.school, c.color), &mut images);
        for b in c.buffs {
            add(b.key, tint(b.school, c.color), &mut images);
        }
    }
    add(TRINKET, School::Holy.color(), &mut images);
    commands.insert_resource(icons);
}

/// An icon's colour: its school's, or for plain physical blows the class's own.
fn tint(school: School, class: u32) -> [f32; 3] {
    if school == School::Physical {
        let c = |shift: u32| ((class >> shift) & 0xff) as f32 / 255.0;
        [c(16), c(8), c(0)]
    } else {
        school.color()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Glyph {
    Sword,
    CrossedSwords,
    Dagger,
    Daggers,
    Axe,
    ThrownAxe,
    Shield,
    ShieldCross,
    Arrow,
    BarbedArrow,
    Spear,
    Flame,
    Orb,
    Snowflake,
    Skull,
    Eye,
    Fist,
    Chevrons,
    Burst,
    Stars,
    Ring,
    Cross,
    CrossFast,
    Leaf,
    Drop,
    Swirl,
    Trap,
    Crystal,
    Shards,
    Ban,
    Shout,
    Slash,
    Chain,
    Moon,
    Heart,
    Fang,
    Bone,
    Hammer,
    Sparkle,
    Medallion,
}

/// Each ability's glyph, and how far it's turned (radians).
fn look(key: &str) -> Option<(Glyph, f32)> {
    use Glyph::*;
    const Q: f32 = std::f32::consts::FRAC_PI_4;
    Some(match key {
        // White hits.
        "white_sword" => (CrossedSwords, 0.0),
        "white_axe" => (Axe, 0.45),
        "white_knives" => (Daggers, 0.0),
        "auto_shot" => (Arrow, Q),
        "white_staff" | "white_wand" => (Hammer, Q),
        // Buffs.
        "battle_hymn" => (Shout, 0.0),
        "steel_resolve" => (ShieldCross, 0.0),
        "war_drums" => (Medallion, 0.0),
        "savage_fury" => (Flame, 0.0),
        "aether_insight" => (Eye, 0.0),
        "frost_mantle" => (Shield, 0.0),
        "venom_coating" => (Dagger, Q),
        "keen_edge" => (Sword, Q),
        "hawks_eye" => (Eye, 0.0),
        "pack_instinct" => (Fang, 0.0),
        "vitality" => (Heart, 0.0),
        "inner_calm" => (Moon, 0.0),
        // Vanguard.
        "steel_strike" => (Sword, Q),
        "rending_strike" => (Slash, 0.0),
        "crippling_cut" => (Chain, 0.0),
        "shield_rush" => (Chevrons, 0.0),
        "pommel_strike" => (Fist, 0.0),
        "rampart_strike" => (Hammer, -Q),
        "daunting_roar" => (Shout, 0.0),
        "judging_light" => (Burst, 0.0),
        "iron_wall" => (Shield, 0.0),
        "second_wind" => (Heart, 0.0),
        // Berserker.
        "wide_chop" => (Axe, -0.45),
        "gorge" => (Fang, 0.0),
        "gash" => (Slash, 0.0),
        "skull_leap" => (Chevrons, -Q),
        "throat_punch" => (Fist, 0.0),
        "bonebreaker" => (Bone, Q),
        "bellow" => (Shout, 0.0),
        "red_mist" => (Drop, 0.0),
        "hardened_skin" => (Shield, 0.0),
        "hurl_axe" => (ThrownAxe, 0.0),
        // Arcanist.
        "frost_lance" => (Spear, -Q),
        "flame_burst" => (Burst, 0.0),
        "inferno_orb" => (Flame, 0.0),
        "frost_ring" => (Ring, 0.0),
        "spellbreak" => (Ban, 0.0),
        "bewilder" => (Swirl, 0.0),
        "phase_step" => (Sparkle, 0.0),
        "crystal_tomb" => (Crystal, 0.0),
        "shatter_shard" => (Shards, 0.0),
        "deep_chill" => (Snowflake, 0.0),
        // Shadowblade.
        "twin_slash" => (Daggers, 0.0),
        "finishing_cut" => (Dagger, Q),
        "venom_burst" => (Drop, 0.0),
        "hamstring_cut" => (Chain, 0.0),
        "throat_jab" => (Fist, 0.0),
        "gut_punch" => (Stars, 0.0),
        "blinding_powder" => (Eye, 0.0),
        "shadow_step" => (Chevrons, Q),
        "shadow_cloak" => (Moon, 0.0),
        "parry_dance" => (CrossedSwords, 0.0),
        // Ranger.
        "quick_bolt" => (Arrow, -Q),
        "piercing_shot" => (Spear, Q),
        "barbed_bolt" => (BarbedArrow, -Q),
        "pinning_shot" => (Chain, 0.0),
        "hushing_shot" => (Ban, 0.0),
        "snare_trap" => (Trap, 0.0),
        "tumble" => (Chevrons, 2.0 * Q),
        "deflect" => (Shield, 0.0),
        "final_shot" => (Skull, 0.0),
        "sleep_dart" => (Moon, 0.0),
        // Mystic.
        "mending_light" => (Cross, 0.0),
        "swift_mend" => (CrossFast, 0.0),
        "renewing_bloom" => (Leaf, 0.0),
        "ward_of_light" => (Ring, 0.0),
        "searing_word" => (Burst, 0.0),
        "shadow_lash" => (Slash, 0.0),
        "terrify" => (Skull, 0.0),
        "silencing_word" => (Ban, 0.0),
        "guardian_ward" => (ShieldCross, 0.0),
        "cleanse" => (Sparkle, 0.0),
        TRINKET => (Medallion, 0.0),
        _ => return None,
    })
}

fn seed(key: &str) -> u32 {
    key.bytes().fold(2166136261u32, |h, b| (h ^ u32::from(b)).wrapping_mul(16777619))
}

type P = [f32; 2];

fn turned(p: P, a: f32) -> P {
    let (s, c) = a.sin_cos();
    [p[0] * c + p[1] * s, -p[0] * s + p[1] * c]
}

fn circle(p: P, c: P, r: f32) -> f32 {
    (p[0] - c[0]).hypot(p[1] - c[1]) - r
}

fn seg(p: P, a: P, b: P, r: f32) -> f32 {
    let (px, py, bx, by) = (p[0] - a[0], p[1] - a[1], b[0] - a[0], b[1] - a[1]);
    let t = ((px * bx + py * by) / (bx * bx + by * by).max(1e-6)).clamp(0.0, 1.0);
    (px - bx * t).hypot(py - by * t) - r
}

/// Signed distance to a polygon (negative inside).
fn poly(p: P, v: &[P]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    let mut j = v.len() - 1;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[j]);
        d = d.min(seg(p, a, b, 0.0));
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    if inside { -d } else { d }
}

fn star4(p: P, c: P, r: f32) -> f32 {
    let k = r * 0.28;
    poly(p, &[
        [c[0], c[1] + r],
        [c[0] + k, c[1] + k],
        [c[0] + r, c[1]],
        [c[0] + k, c[1] - k],
        [c[0], c[1] - r],
        [c[0] - k, c[1] - k],
        [c[0] - r, c[1]],
        [c[0] - k, c[1] + k],
    ])
}

fn sword(p: P, len: f32) -> f32 {
    let blade = poly(p, &[[-0.09, -0.3], [0.09, -0.3], [0.09, len - 0.2], [0.0, len], [-0.09, len - 0.2]]);
    let guard = seg(p, [-0.28, -0.32], [0.28, -0.32], 0.05);
    let grip = seg(p, [0.0, -0.34], [0.0, -0.62], 0.05);
    blade.min(guard).min(grip).min(circle(p, [0.0, -0.68], 0.08))
}

/// An axe's blade on one side of the haft.
fn axe_bit(p: P, side: f32) -> f32 {
    let q = [p[0] * side, p[1]];
    poly(q, &[[0.03, 0.68], [0.34, 0.74], [0.72, 0.9], [0.8, 0.38], [0.72, -0.1], [0.34, 0.08], [0.03, 0.2]])
}

fn arrow(p: P) -> f32 {
    let shaft = seg(p, [0.0, -0.75], [0.0, 0.45], 0.04);
    let head = poly(p, &[[-0.2, 0.42], [0.2, 0.42], [0.0, 0.85]]);
    let fl = seg(p, [0.0, -0.5], [-0.2, -0.78], 0.04).min(seg(p, [0.0, -0.5], [0.2, -0.78], 0.04));
    shaft.min(head).min(fl)
}

fn shield(p: P) -> f32 {
    let outer = poly(p, &[[-0.62, 0.68], [0.62, 0.68], [0.62, 0.05], [0.32, -0.45], [0.0, -0.8], [-0.32, -0.45], [-0.62, 0.05]]);
    let rim = poly(p, &[[-0.46, 0.53], [0.46, 0.53], [0.46, 0.08], [0.22, -0.34], [0.0, -0.6], [-0.22, -0.34], [-0.46, 0.08]]);
    // A rim, and a boss in the middle.
    outer.max(-rim).min(circle(p, [0.0, 0.08], 0.2))
}

fn drop(p: P, s: f32) -> f32 {
    let q = [p[0] / s, p[1] / s];
    circle(q, [0.0, -0.28], 0.5).min(poly(q, &[[-0.44, -0.08], [0.44, -0.08], [0.0, 0.85]])) * s
}

/// The glyph's signed distance at `p` (in -1..1, y up).
fn shape(g: Glyph, p: P) -> f32 {
    use Glyph::*;
    match g {
        Sword => sword(p, 0.85),
        Dagger => sword(p, 0.5),
        CrossedSwords => sword(turned(p, 0.785), 0.85).min(sword(turned(p, -0.785), 0.85)),
        Daggers => {
            let blade = |x: f32, a: f32| sword(turned([p[0] - x, p[1] + 0.12], a), 0.72);
            blade(-0.36, -0.2).min(blade(0.36, 0.2))
        }
        Axe => seg(p, [0.0, -0.88], [0.0, 0.82], 0.06).min(axe_bit(p, 1.0)).min(axe_bit(p, -1.0)),
        ThrownAxe => {
            let q = turned([p[0] - 0.2, p[1] - 0.14], 0.9);
            let axe = seg(q, [0.0, -0.58], [0.0, 0.66], 0.055).min(axe_bit(q, 1.0));
            let streaks = seg(p, [-0.88, 0.08], [-0.42, 0.08], 0.04)
                .min(seg(p, [-0.88, -0.2], [-0.32, -0.2], 0.04))
                .min(seg(p, [-0.88, -0.48], [-0.42, -0.48], 0.04));
            axe.min(streaks)
        }
        Shield => shield(p),
        ShieldCross => shield(p).min(seg(p, [0.0, -0.3], [0.0, 0.45], 0.06)).min(seg(p, [-0.25, 0.12], [0.25, 0.12], 0.06)),
        Arrow => arrow(p),
        BarbedArrow => {
            let barbs = seg(p, [0.0, 0.1], [0.2, -0.08], 0.035)
                .min(seg(p, [0.0, 0.1], [-0.2, -0.08], 0.035))
                .min(seg(p, [0.0, -0.15], [0.2, -0.33], 0.035))
                .min(seg(p, [0.0, -0.15], [-0.2, -0.33], 0.035));
            arrow(p).min(barbs)
        }
        Spear => {
            let head = poly(p, &[[-0.17, 0.3], [0.0, 0.2], [0.17, 0.3], [0.0, 0.92]]);
            seg(p, [0.0, -0.88], [0.0, 0.3], 0.045).min(head)
        }
        Flame => {
            let body = drop([p[0], p[1] + 0.12], 0.92);
            let lick = drop([p[0] + 0.42, p[1] + 0.32], 0.45).min(drop([p[0] - 0.42, p[1] + 0.36], 0.4));
            body.min(lick).max(-drop([p[0], p[1] + 0.42], 0.34))
        }
        Orb => circle(p, [0.0, 0.0], 0.6),
        Snowflake => {
            let mut d = f32::MAX;
            for k in 0..3 {
                let q = turned(p, k as f32 * std::f32::consts::FRAC_PI_3);
                d = d.min(seg(q, [0.0, -0.8], [0.0, 0.8], 0.05));
                for y in [0.45f32, -0.45] {
                    let tip = if y > 0.0 { 0.72 } else { -0.72 };
                    d = d.min(seg(q, [0.0, y], [0.2, tip], 0.04)).min(seg(q, [0.0, y], [-0.2, tip], 0.04));
                }
            }
            d
        }
        Skull => {
            let head = circle(p, [0.0, 0.14], 0.58);
            let jaw = seg(p, [-0.16, -0.5], [0.16, -0.5], 0.2);
            let eyes = circle(p, [-0.23, 0.12], 0.16).min(circle(p, [0.23, 0.12], 0.16));
            let nose = poly(p, &[[-0.07, -0.2], [0.07, -0.2], [0.0, -0.04]]);
            let teeth = seg(p, [-0.11, -0.72], [-0.11, -0.42], 0.018).min(seg(p, [0.11, -0.72], [0.11, -0.42], 0.018));
            head.min(jaw).max(-eyes).max(-nose).max(-teeth)
        }
        Eye => {
            let lens = circle(p, [0.0, -0.55], 0.95).max(circle(p, [0.0, 0.55], 0.95));
            let white = circle(p, [0.0, -0.55], 0.82).max(circle(p, [0.0, 0.55], 0.82));
            lens.max(-white).min(circle(p, [0.0, 0.0], 0.22))
        }
        Fist => {
            let mut d = seg(p, [-0.3, -0.25], [0.3, -0.25], 0.26);
            for k in 0..4 {
                let x = -0.36 + k as f32 * 0.24;
                d = d.min(seg(p, [x, 0.1], [x, 0.42], 0.105));
            }
            d.min(seg(p, [0.52, -0.05], [0.28, 0.22], 0.1)).min(seg(p, [-0.22, -0.55], [0.22, -0.55], 0.14))
        }
        Chevrons => {
            let mut d = f32::MAX;
            for k in 0..3 {
                let y = -0.55 + k as f32 * 0.5;
                d = d.min(seg(p, [-0.55, y], [0.0, y + 0.42], 0.09)).min(seg(p, [0.55, y], [0.0, y + 0.42], 0.09));
            }
            d
        }
        Burst => {
            let mut d = circle(p, [0.0, 0.0], 0.32);
            for k in 0..8 {
                let q = turned(p, k as f32 * std::f32::consts::FRAC_PI_4);
                let far = if k % 2 == 0 { 0.88 } else { 0.66 };
                d = d.min(poly(q, &[[-0.1, 0.34], [0.1, 0.34], [0.0, far]]));
            }
            d
        }
        Stars => star4(p, [0.0, 0.34], 0.42).min(star4(p, [-0.48, -0.26], 0.3)).min(star4(p, [0.48, -0.26], 0.3)),
        Ring => {
            let mut d = circle(p, [0.0, 0.0], 0.68).abs() - 0.1;
            for k in 0..8 {
                let q = turned(p, k as f32 * std::f32::consts::FRAC_PI_4);
                d = d.min(poly(q, &[[-0.07, 0.48], [0.07, 0.48], [0.0, 0.26]]));
            }
            d
        }
        Cross => seg(p, [0.0, -0.66], [0.0, 0.66], 0.2).min(seg(p, [-0.66, 0.0], [0.66, 0.0], 0.2)),
        CrossFast => {
            let plus = seg(p, [0.1, -0.5], [0.1, 0.5], 0.15).min(seg(p, [-0.4, 0.0], [0.6, 0.0], 0.15));
            plus.min(seg(p, [-0.8, 0.45], [-0.5, 0.45], 0.05)).min(seg(p, [-0.85, -0.45], [-0.45, -0.45], 0.05))
        }
        Leaf => {
            let q = turned(p, -0.6);
            let blade = circle(q, [-0.42, 0.0], 0.82).max(circle(q, [0.42, 0.0], 0.82));
            blade.max(-seg(q, [0.0, -0.7], [0.0, 0.5], 0.025)).min(seg(q, [0.0, -0.95], [0.0, -0.6], 0.04))
        }
        Drop => drop(p, 0.95).max(-seg(p, [-0.26, -0.38], [-0.1, -0.56], 0.05)),
        Swirl => {
            let r = p[0].hypot(p[1]);
            let th = p[1].atan2(p[0]) + std::f32::consts::PI;
            let k = 0.27;
            let u = (r - th / std::f32::consts::TAU * k).rem_euclid(k);
            ((u - k / 2.0).abs() - 0.045).max(r - 0.82)
        }
        Trap => {
            let mut d = circle(p, [0.0, 0.0], 0.76).abs() - 0.07;
            for k in 0..10 {
                let q = turned(p, k as f32 * std::f32::consts::TAU / 10.0);
                d = d.min(poly(q, &[[-0.12, 0.72], [0.12, 0.72], [0.0, 0.36]]));
            }
            d
        }
        Crystal => {
            let gem = poly(p, &[[0.0, 0.9], [0.42, 0.3], [0.3, -0.55], [0.0, -0.9], [-0.3, -0.55], [-0.42, 0.3]]);
            let facets = seg(p, [0.0, 0.9], [0.0, -0.9], 0.016).min(seg(p, [-0.42, 0.3], [0.42, 0.3], 0.016));
            gem.max(-facets)
        }
        Shards => {
            let mut d = f32::MAX;
            for k in 0..5 {
                let q = turned(p, k as f32 * std::f32::consts::TAU / 5.0 + 0.2);
                let far = if k % 2 == 0 { 0.88 } else { 0.68 };
                d = d.min(poly(q, &[[-0.12, 0.2], [0.12, 0.2], [0.05, far - 0.2], [0.0, far]]));
            }
            d
        }
        Ban => (circle(p, [0.0, 0.0], 0.66).abs() - 0.12).min(seg(p, [-0.47, 0.47], [0.47, -0.47], 0.11)),
        Shout => {
            let mouth = poly(p, &[[-0.82, 0.14], [-0.5, 0.14], [-0.25, 0.42], [-0.25, -0.42], [-0.5, -0.14], [-0.82, -0.14]]);
            let (dx, dy) = (p[0] + 0.45, p[1]);
            let (r, a) = (dx.hypot(dy), dy.atan2(dx).abs());
            let mut d = mouth;
            for wave in [0.5f32, 0.85, 1.2] {
                d = d.min(((r - wave).abs() - 0.055).max((a - 0.75) * r));
            }
            d
        }
        Slash => {
            let claw = |x: f32| poly(p, &[[x + 0.34, 0.86], [x + 0.06, 0.0], [x - 0.3, -0.84], [x - 0.08, 0.02]]);
            claw(-0.42).min(claw(0.0)).min(claw(0.42))
        }
        Chain => {
            let link = |c: P, a: f32| {
                let q = turned([p[0] - c[0], p[1] - c[1]], a);
                seg(q, [0.0, -0.17], [0.0, 0.17], 0.17).abs() - 0.055
            };
            link([-0.42, -0.42], -0.785).min(link([0.0, 0.0], 0.785)).min(link([0.42, 0.42], -0.785))
        }
        Moon => circle(p, [0.0, 0.0], 0.72).max(-circle(p, [0.32, 0.2], 0.62)).min(star4(p, [0.34, 0.2], 0.22)),
        Heart => circle(p, [-0.3, 0.26], 0.36)
            .min(circle(p, [0.3, 0.26], 0.36))
            .min(poly(p, &[[-0.62, 0.1], [0.62, 0.1], [0.0, -0.76]])),
        Fang => {
            let gum = seg(p, [-0.66, 0.55], [0.66, 0.55], 0.12);
            let tooth = |x: f32, l: f32| poly(p, &[[x - 0.2, 0.5], [x + 0.2, 0.5], [x, l]]);
            gum.min(tooth(-0.42, -0.42)).min(tooth(0.42, -0.42)).min(tooth(0.0, 0.05))
        }
        Bone => {
            let knob = |x: f32, y: f32| circle(p, [x - 0.1, y], 0.15).min(circle(p, [x + 0.1, y], 0.15));
            seg(p, [0.0, -0.62], [0.0, 0.62], 0.11).min(knob(0.0, 0.7)).min(knob(0.0, -0.7))
        }
        Hammer => {
            let head = poly(p, &[[-0.48, 0.34], [0.48, 0.34], [0.48, 0.8], [-0.48, 0.8]]);
            seg(p, [0.0, -0.85], [0.0, 0.4], 0.07).min(head)
        }
        Sparkle => star4(p, [0.0, 0.0], 0.82).min(star4(p, [0.52, 0.52], 0.24)).min(star4(p, [-0.52, -0.52], 0.2)),
        Medallion => (circle(p, [0.0, 0.0], 0.74).abs() - 0.09).min(star4(p, [0.0, 0.0], 0.52)),
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Cheap value noise, for the brushed background.
fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let hash = |i: i32, j: i32| {
        let h = (i as u32).wrapping_mul(374761393) ^ (j as u32).wrapping_mul(668265263) ^ seed;
        let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
    };
    let (i, j) = (x.floor(), y.floor());
    let (fx, fy) = (x - i, y - j);
    let (i, j) = (i as i32, j as i32);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let top = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * u;
    let bot = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * u;
    top + (bot - top) * v
}

/// One icon's RGBA, sRGB bytes.
fn paint(g: Glyph, turn: f32, color: [f32; 3], seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIZE * SIZE * 4);
    let px = 2.0 / SIZE as f32;
    let bright = mix(color, [1.0, 1.0, 1.0], 0.72);
    let deep = mix(color, [0.0, 0.0, 0.0], 0.76);
    let dark = mix(color, [0.0, 0.0, 0.0], 0.93);
    for row in 0..SIZE {
        for col in 0..SIZE {
            let x = (col as f32 + 0.5) * px - 1.0;
            let y = 1.0 - (row as f32 + 0.5) * px;
            // A dark, brushed field, lit from the top left.
            let light = smooth(1.5, -0.3, (x + 0.6).hypot(y - 0.6));
            let streak = noise((x + y) * 2.5, (x - y) * 11.0, seed) * 0.55 + noise(x * 7.0, y * 7.0, seed ^ 77) * 0.45;
            let mut c = mix(dark, deep, (light * 0.75 + streak * 0.5).min(1.0));
            // The glyph: a soft glow behind, a dark outline, then lit from the top with a bright
            // edge fading to the school's colour in its heart.
            let q = turned([x / 0.86, y / 0.86], -turn);
            let d = shape(g, q) * 0.86;
            let glow = (-d.max(0.0) * 7.0).exp();
            c = mix(c, color, glow * 0.5);
            c = mix(c, [0.02, 0.015, 0.02], smooth(0.045 + px, 0.045 - px, d) * 0.9);
            let fill = smooth(px, -px, d);
            let shade = mix(mix(color, [0.0, 0.0, 0.0], 0.12), bright, smooth(-0.8, 0.85, y - x * 0.25));
            let edge = smooth(-0.06, -0.005, d) * 0.35;
            c = mix(c, mix(shade, [1.0, 1.0, 1.0], edge), fill);
            // A bevelled frame: dark outside, a thin light line on the top and left.
            let frame = 1.0 - x.abs().max(y.abs());
            let lit = if y > -x { 0.42 } else { 0.1 };
            c = mix(c, [lit, lit * 0.95, lit * 0.85], smooth(px * 1.5, 0.0, (frame - 0.05).abs()) * 0.8);
            c = mix(c, [0.0, 0.0, 0.0], smooth(0.05, 0.03, frame));
            // Corners fall away into shadow.
            c = mix(c, [0.0, 0.0, 0.0], smooth(1.0, 1.4, x.hypot(y)) * 0.6);
            for v in c {
                out.push((v.clamp(0.0, 1.0) * 255.0) as u8);
            }
            out.push(255);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ability_has_its_own_icon_within_its_class() {
        for c in CLASSES {
            let mut seen: Vec<(String, School)> = Vec::new();
            for s in c.spells.iter().chain(c.buffs) {
                let (g, _) = look(s.key).unwrap_or_else(|| panic!("{} has no icon", s.key));
                let id = (format!("{g:?}"), s.school);
                assert!(!seen.contains(&id), "{}: {} looks like another {} ability", c.name, s.key, id.0);
                seen.push(id);
            }
        }
        assert!(look(TRINKET).is_some());
        for c in CLASSES {
            assert!(look(c.auto.key).is_some(), "{} has no icon", c.auto.key);
        }
    }

    #[test]
    fn a_glyph_covers_a_fair_part_of_its_icon() {
        // Blank or solid glyphs would be a broken shape.
        let all = CLASSES.iter().flat_map(|c| c.spells.iter()).map(|s| s.key).chain([TRINKET]);
        for key in all {
            let (g, _) = look(key).unwrap();
            let mut inside = 0;
            for j in 0..40 {
                for i in 0..40 {
                    let p = [i as f32 / 20.0 - 0.975, j as f32 / 20.0 - 0.975];
                    inside += usize::from(shape(g, p) < 0.0);
                }
            }
            assert!((60..1100).contains(&inside), "{key}: {g:?} covers {inside} of 1600");
        }
    }

    /// `EMBER_ICON_SHEET=<file.ppm> cargo test -p game contact_sheet -- --ignored` draws every icon
    /// on one sheet, a class to a row, for looking at them.
    #[test]
    #[ignore]
    fn contact_sheet() {
        let path = std::env::var("EMBER_ICON_SHEET").expect("EMBER_ICON_SHEET");
        let (w, h) = (SIZE * 10, SIZE * CLASSES.len());
        let mut px = vec![0u8; w * h * 3];
        for (r, c) in CLASSES.iter().enumerate() {
            for (k, s) in c.spells.iter().enumerate() {
                let (g, t) = look(s.key).unwrap();
                let img = paint(g, t, s.school.color(), seed(s.key));
                for y in 0..SIZE {
                    for x in 0..SIZE {
                        let o = (y * SIZE + x) * 4;
                        let d = ((r * SIZE + y) * w + k * SIZE + x) * 3;
                        px[d..d + 3].copy_from_slice(&img[o..o + 3]);
                    }
                }
            }
        }
        let mut file = format!("P6\n{w} {h}\n255\n").into_bytes();
        file.extend(px);
        std::fs::write(path, file).unwrap();
    }
}
