//! World plan: where every feature sits. Seed 0 is the hand-tuned canonical
//! layout; other seeds rotate / mirror it, jitter every feature, bend the
//! river differently and resample all terrain noise, so each seed is a new
//! but always valid canyon (the terrain is generated *around* the plan).

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use crate::engine::prelude::*;

/// Nominal radius of the diorama plinth (metres).
pub const WORLD_R: f32 = 116.0;

pub struct Plan {
    pub seed: u32,
    pub winter: bool,
    /// Seed transform: rotation (radians) and optional mirror of canonical coords.
    rot: f32,
    mirror: bool,
    /// Offset into every terrain noise field.
    pub noise_off: Vec2,
    /// Unit direction (from the centre) where the tall canyon walls stand.
    pub back: Vec2,
    pub oasis: Vec2,
    pub oasis_r: Vec2,
    pub oasis_rot: f32,
    /// Dense polyline of the dry riverbed, from the rim down to the oasis.
    pub river: Vec<Vec2>,
    pub ruins: Vec2,
    pub ruins_rot: f32,
    pub arch: Vec2,
    /// Unit span axis of the arch; its +axis foot grows out of a mesa spur.
    pub arch_dir: Vec2,
    pub arch_spur: Vec2,
    /// Unit vector through the arch window, pointing toward the diorama centre.
    pub arch_across: Vec2,
    pub butte: Vec2,
    pub overlook: Vec2,
    pub camp: Vec2,
    pub camp_rot: f32,
    pub dune_dir: Vec2,
    pub dune_phase: f32,
}

impl Plan {
    pub fn new(seed: u32, winter: bool) -> Self {
        let mut rng = Rng(0x0DE5_E127 ^ seed.wrapping_mul(0x9E37_79B9));
        let canonical = seed == 0;
        let mut j = |amp: f32| if canonical { Vec2::ZERO } else { Vec2::new(rng_f(&mut rng) - 0.5, rng_f(&mut rng) - 0.5) * 2.0 * amp };
        let jit = [j(6.0), j(7.0), j(5.0), j(4.0), j(8.0), j(6.0), j(4.0)];
        let mut rng = Rng(0x51DE_0000 ^ seed.wrapping_mul(0x85EB_CA6B));
        let (rot, mirror) = if canonical { (0.0, false) } else { (rng.next_f32() * TAU, rng.next_f32() < 0.5) };
        let noise_off = if canonical { Vec2::new(13.0, 71.0) } else { Vec2::new(rng.next_f32() * 900.0 + 40.0, rng.next_f32() * 900.0 + 40.0) };
        let mut p = Plan {
            seed,
            winter,
            rot,
            mirror,
            noise_off,
            back: Vec2::ZERO,
            oasis: Vec2::ZERO,
            oasis_r: Vec2::new(15.0, 11.0),
            oasis_rot: 0.3,
            river: Vec::new(),
            ruins: Vec2::ZERO,
            ruins_rot: 0.21,
            arch: Vec2::ZERO,
            arch_dir: Vec2::X,
            arch_spur: Vec2::ZERO,
            arch_across: Vec2::Y,
            butte: Vec2::ZERO,
            overlook: Vec2::ZERO,
            camp: Vec2::ZERO,
            camp_rot: 0.6,
            dune_dir: Vec2::new(1.0, 0.32).normalize(),
            dune_phase: 0.0,
        };
        // Canonical coordinates: +x east, +z south; walls stand to the north.
        p.back = p.xf_dir(Vec2::new(-0.35, -1.0).normalize());
        p.oasis = p.xf(Vec2::new(-30.0, 36.0) + jit[0]);
        let arch_c = Vec2::new(-10.0, -24.0) + jit[1];
        let arch_axis = Vec2::new(0.72, -0.69).normalize();
        p.arch = p.xf(arch_c);
        p.arch_dir = p.xf_dir(arch_axis);
        p.arch_spur = p.xf(arch_c + arch_axis * 24.0);
        let across = p.arch_dir.perp();
        p.arch_across = if across.dot(-p.arch) < 0.0 { -across } else { across };
        p.ruins = p.xf(Vec2::new(40.0, -6.0) + jit[2]);
        p.butte = p.xf(Vec2::new(74.0, -46.0) + jit[3]);
        p.overlook = p.xf(Vec2::new(-46.0, -86.0) + jit[4] * 0.5);
        p.camp = p.xf(Vec2::new(-9.0, 47.0) + jit[5] * 0.5);
        if !canonical {
            p.oasis_r = Vec2::new(13.5 + rng.next_f32() * 3.5, 10.0 + rng.next_f32() * 2.5);
            p.oasis_rot = rng.next_f32() * TAU;
            p.ruins_rot = rng.next_f32() * TAU;
            p.camp_rot = rng.next_f32() * TAU;
            let a = rng.next_f32() * TAU;
            p.dune_dir = Vec2::new(a.cos(), a.sin());
            p.dune_phase = rng.next_f32() * 10.0;
        }
        // River: rim -> oasis through the western canyon, meandering.
        let mut ctrl = vec![
            Vec2::new(-112.0, -34.0),
            Vec2::new(-84.0, -30.0),
            Vec2::new(-64.0, -9.0),
            Vec2::new(-60.0, 16.0),
            Vec2::new(-46.0, 31.0),
        ];
        for (k, c) in ctrl.iter_mut().enumerate().skip(1) {
            if !canonical {
                *c += Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * (9.0 + k as f32 * 2.0);
            }
        }
        let mut pts: Vec<Vec3> = ctrl.iter().map(|&c| p.xf(c)).map(|v| Vec3::new(v.x, 0.0, v.y)).collect();
        let o = p.oasis;
        pts.push(Vec3::new(o.x, 0.0, o.y));
        let curve = CatmullRomCurve3::new(pts, false, 0.5);
        p.river = (0..=160).map(|i| curve.get_point_at(i as f32 / 160.0)).map(|v| Vec2::new(v.x, v.z)).collect();
        p
    }

    /// Canonical -> world coordinates for this seed.
    pub fn xf(&self, v: Vec2) -> Vec2 {
        self.xf_dir(v)
    }

    pub fn xf_dir(&self, v: Vec2) -> Vec2 {
        let v = if self.mirror { Vec2::new(-v.x, v.y) } else { v };
        Vec2::from_angle(self.rot).rotate(v)
    }

    /// Canonical 3D offset (x, y, z) -> world offset.
    pub fn xf3(&self, v: Vec3) -> Vec3 {
        let h = self.xf(Vec2::new(v.x, v.z));
        Vec3::new(h.x, v.y, h.y)
    }

    /// Distance to the riverbed and the normalised position along it (0 rim, 1 oasis).
    pub fn river_dist(&self, p: Vec2) -> (f32, f32) {
        let mut best = (f32::MAX, 0.0);
        let n = self.river.len() - 1;
        for i in 0..n {
            let (a, b) = (self.river[i], self.river[i + 1]);
            let ab = b - a;
            let h = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d2 = (p - a - ab * h).length_squared();
            if d2 < best.0 {
                best = (d2, (i as f32 + h) / n as f32);
            }
        }
        (best.0.sqrt(), best.1)
    }

    /// Oasis-normalised radius (1 = nominal shoreline), with a wobbly outline.
    pub fn oasis_e(&self, p: Vec2) -> f32 {
        let d = Vec2::from_angle(-self.oasis_rot).rotate(p - self.oasis);
        let e = (d / self.oasis_r).length();
        let a = d.y.atan2(d.x);
        let wob = fbm2(a.cos() * 1.3 + 7.0 + self.noise_off.x * 0.01, a.sin() * 1.3 - 3.0, 3);
        e / (0.86 + 0.28 * wob)
    }
}

fn rng_f(r: &mut Rng) -> f32 {
    r.next_f32()
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
