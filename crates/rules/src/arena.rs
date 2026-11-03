//! The arenas: walled rings whose pillars block both movement and line of sight, and where each
//! team starts.

use crate::Pt;

pub struct ArenaDef {
    pub key: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    /// Yards from the centre to the inside of the wall.
    pub radius: f32,
    /// `(centre, radius)` of each pillar.
    pub pillars: &'static [(Pt, f32)],
}

pub static ARENAS: &[ArenaDef] = &[
    ArenaDef {
        key: "ember_pit",
        name: "The Ember Pit",
        blurb: "A sunken ring of old stone, four great pillars to break the line.",
        radius: 42.0,
        pillars: &[([15.0, 15.0], 3.0), ([-15.0, 15.0], 3.0), ([15.0, -15.0], 3.0), ([-15.0, -15.0], 3.0)],
    },
    ArenaDef {
        key: "cinder_ring",
        name: "The Cinder Ring",
        blurb: "A wide ring around a broken shrine: one huge pillar at the heart, four around it.",
        radius: 46.0,
        pillars: &[
            ([0.0, 0.0], 5.0),
            ([25.0, 0.0], 2.5),
            ([-25.0, 0.0], 2.5),
            ([0.0, 25.0], 2.5),
            ([0.0, -25.0], 2.5),
        ],
    },
];

impl ArenaDef {
    /// `p` pushed back inside the wall and out of every pillar, for a body `r` yards wide.
    pub fn confine(&self, p: Pt, r: f32) -> Pt {
        let [mut x, mut z] = p;
        for &([px, pz], pr) in self.pillars {
            let (dx, dz) = (x - px, z - pz);
            let d = (dx * dx + dz * dz).sqrt();
            let min = pr + r;
            if d < min {
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

    /// Whether `a` can see `b`: no pillar crosses the line between them.
    pub fn sight(&self, a: Pt, b: Pt) -> bool {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dz * dz;
        self.pillars.iter().all(|&([px, pz], pr)| {
            let t = if len2 > 0.0 { (((px - a[0]) * dx + (pz - a[1]) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let (cx, cz) = (a[0] + dx * t - px, a[1] + dz * t - pz);
            cx * cx + cz * cz > pr * pr * 0.8
        })
    }

    /// A team's pen at the gates: `(centre, radius)`, where it waits behind the fence.
    pub fn pen(&self, team: usize) -> (Pt, f32) {
        let z = (self.radius - 7.0) * if team == 0 { -1.0 } else { 1.0 };
        ([0.0, z], 10.0)
    }

    /// `p` held inside a team's pen, for a body `r` yards wide.
    pub fn confine_pen(&self, p: Pt, team: usize, r: f32) -> Pt {
        let (c, pr) = self.pen(team);
        let (dx, dz) = (p[0] - c[0], p[1] - c[1]);
        let d = (dx * dx + dz * dz).sqrt();
        let max = pr - r;
        if d > max { [c[0] + dx / d * max, c[1] + dz / d * max] } else { p }
    }

    /// Where member `i` of a team of `n` starts: team 0 at the south gate, team 1 at the north.
    pub fn start(&self, team: usize, i: usize, n: usize) -> Pt {
        let z = (self.radius - 11.0) * if team == 0 { -1.0 } else { 1.0 };
        let x = (i as f32 - (n as f32 - 1.0) / 2.0) * 3.0;
        [x, z]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_stay_inside_the_wall_and_out_of_pillars() {
        let a = &ARENAS[0];
        let [x, z] = a.confine([100.0, 0.0], 0.5);
        assert!((x - (a.radius - 0.5)).abs() < 1e-3 && z.abs() < 1e-3);
        let ([px, pz], pr) = a.pillars[0];
        let [x, z] = a.confine([px + 0.1, pz], 0.5);
        assert!(((x - px).powi(2) + (z - pz).powi(2)).sqrt() >= pr + 0.5 - 1e-3);
    }

    #[test]
    fn the_pens_hold_their_teams_clear_of_each_other_and_the_pillars() {
        for a in ARENAS {
            let (c0, r0) = a.pen(0);
            let (c1, r1) = a.pen(1);
            assert!((c1[1] - c0[1]).abs() > r0 + r1 + 10.0, "{}", a.name);
            for team in 0..2 {
                for i in 0..3 {
                    let s = a.start(team, i, 3);
                    assert_eq!(a.confine_pen(s, team, 0.6), s, "{}", a.name);
                }
                let (c, r) = a.pen(team);
                for &(p, pr) in a.pillars {
                    let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
                    assert!(d > r + pr, "{} has a pillar in a pen", a.name);
                }
                let held = a.confine_pen([0.0, 0.0], team, 0.6);
                assert!(((held[0] - c[0]).powi(2) + (held[1] - c[1]).powi(2)).sqrt() <= r);
            }
        }
    }

    #[test]
    fn pillars_block_sight() {
        let a = &ARENAS[1];
        assert!(!a.sight([-20.0, 0.0], [20.0, 0.0]), "the shrine is in the way");
        assert!(a.sight([-20.0, 10.0], [20.0, 10.0]));
    }

    #[test]
    fn teams_start_apart_with_the_heart_between_them() {
        for a in ARENAS {
            let (s, n) = (a.start(0, 0, 3), a.start(1, 2, 3));
            assert!(s[1] < 0.0 && n[1] > 0.0);
            assert_eq!(a.confine(s, 0.6), s);
            for &(p, r) in a.pillars {
                assert!((p[0] * p[0] + p[1] * p[1]).sqrt() + r < a.radius - 6.0, "{}", a.name);
            }
        }
    }
}
