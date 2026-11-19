//! The Great Arch: a sandstone fin (elliptic hump intersected with a slab whose
//! thickness tapers with height) with an elliptic window subtracted, bent in
//! plan, then eroded by two octaves of 3D noise and banded ledges. One foot
//! grows out of a mesa spur (see `Plan::arch_spur`). It is voxelised straight
//! into the terrain chunks so rock and arch share one grid, colour and AO.

use glam::{Vec2, Vec3};

use super::plan::Plan;
use super::terrain::{strata, strata_offset, VS};
use super::pal;
use crate::engine::prelude::*;

pub struct Arch {
    pub c: Vec2,
    pub dir: Vec2,
    pub half_span: f32,
    pub height: f32,
    seed: f32,
    noise_off: Vec2,
    back: Vec2,
}

impl Arch {
    pub fn new(plan: &Plan) -> Self {
        Self {
            c: plan.arch,
            dir: plan.arch_dir,
            half_span: 17.0,
            height: 27.0,
            seed: (plan.seed % 1000) as f32 * 1.37,
            noise_off: plan.noise_off,
            back: plan.back,
        }
    }

    /// World-space AABB of the rock.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let r = self.half_span + 7.0;
        (Vec3::new(self.c.x - r, -4.0, self.c.y - r), Vec3::new(self.c.x + r, self.height + 3.0, self.c.y + r))
    }

    /// Top of the crown (world), for landmarks and cameras.
    pub fn crown(&self) -> Vec3 {
        Vec3::new(self.c.x, self.height - 1.0, self.c.y)
    }

    pub fn sdf(&self, p: Vec3) -> f32 {
        let d = Vec2::new(p.x, p.z) - self.c;
        let lx = d.dot(self.dir);
        // Gentle bend in plan view so the fin is not a straight wall.
        let lz = d.dot(self.dir.perp()) - 0.012 * lx * lx + 1.5;
        let ly = p.y;
        let (l, h) = (self.half_span, self.height);
        let outer = (Vec2::new(lx / (l + 3.0), (ly + 4.0) / (h + 4.0)).length() - 1.0) * (l + 3.0) * 0.8;
        let (a, b) = (l - 5.0, h - 9.0);
        let hole = (Vec2::new(lx / a, (ly + 1.0) / b).length() - 1.0) * a.min(b) * 0.8;
        let thick = 2.6 + 2.2 * (1.0 - (ly / h).clamp(0.0, 1.0)) + 0.8 * (lx * 0.15).sin();
        let slab = lz.abs() - thick;
        let mut s = sdf::smax(sdf::smax(outer, slab, 1.6), -hole, 1.8);
        let q = p * 0.15 + Vec3::splat(self.seed);
        s += (fbm3(q, 3) - 0.5) * 3.4 + (noise3(p * 0.45 + Vec3::splat(self.seed)) - 0.5) * 0.55;
        // Softer layers weather back into ledges.
        s += ((ly + self.seed) * 1.1).sin().max(0.0) * 0.35;
        s
    }

    /// Voxelise the arch into `vol` (clipped to its bounds automatically).
    pub fn fill(&self, vol: &mut MicroVoxelVolume, winter: bool) {
        let (lo, hi) = self.bounds();
        vol.fill_sdf(lo, hi, |p| self.sdf(p), |p, _| {
            let top = self.sdf(p + Vec3::Y * VS) > 0.0;
            if winter && top {
                return Some((MAT_MATTE, pal::FROST));
            }
            let col = if top && p.y > 4.0 { pal::CAPROCK[(hash2(p.x, p.z) * 2.99) as usize] } else { strata(p.x, p.y, p.z, strata_offset(self.noise_off, self.back, Vec2::new(p.x, p.z)), winter) };
            Some((MAT_MATTE, col))
        });
    }
}
