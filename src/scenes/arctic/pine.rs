//! Snow-laden spruces. Each tier is a cone minus the same cone shifted down
//! (a hollow "skirt"), with a radial sine for jagged branch tips and 3D noise.
//! Snow settles wherever the voxel just above lies outside the tree (an
//! up-facing surface test on the SDF), scaled by the season's snow amount.

use glam::{Vec2, Vec3};

use super::pal;
use crate::engine::prelude::*;

pub struct PineShape {
    pub h: f32,
    pub r: f32,
    pub tiers: usize,
}

pub const SHAPES: [PineShape; 3] = [
    PineShape { h: 11.0, r: 3.0, tiers: 6 },
    PineShape { h: 8.5, r: 3.1, tiers: 5 },
    PineShape { h: 13.5, r: 2.6, tiers: 7 },
];

pub fn build_pine(shape: &PineShape, seed: u32, scale: f32, snow: f32, colors: &mut ColorCache) -> (MeshData, u64) {
    let s = scale;
    let (h, base_r) = (shape.h, shape.r);
    let half = ((base_r + 0.8) / s).ceil() as i32;
    let y0 = (-1.6 / s).floor() as i32;
    let ny = ((h + 1.0) / s).ceil() as i32 - y0;
    let mut vol = MicroVoxelVolume::new(half * 2, ny, half * 2, s, -half, y0, -half);
    let (lo, hi) = vol.local_bounds();
    let sd = seed as f32 * 0.31;

    vol.fill_sdf_solid(lo, hi, |q| sdf::cylinder(q - Vec3::Y * (h * 0.2 - 0.8), 0.3, h * 0.2 + 0.8), MAT_WOOD, pal::PINE_TRUNK);

    let n = shape.tiers;
    let tiers: Vec<(Vec3, f32, f32)> = (0..n)
        .map(|t| {
            let f = t as f32 / n as f32;
            let y = 1.0 + f * (h - 2.6);
            let r = base_r * (1.0 - f * 0.8);
            let ch = (r * 1.15 + 0.9).min(h - y);
            (Vec3::new(0.0, y, 0.0), r, ch)
        })
        .collect();
    let tree = |q: Vec3| -> f32 {
        let mut d = f32::MAX;
        for (i, &(c, r, ch)) in tiers.iter().enumerate() {
            let l = q - c;
            let a = l.z.atan2(l.x);
            let rr = Vec2::new(l.x, l.z).length() / r.max(0.1);
            let teeth = (a * 8.0 + i as f32 * 1.7 + sd).sin() * 0.2 * rr;
            let cone = sdf::cone(l, r, ch) + teeth;
            let under = sdf::cone(l + Vec3::Y * 0.55, r * 0.92, ch);
            d = d.min(cone.max(-under));
        }
        d + (noise3(q * 0.7 + Vec3::splat(sd)) - 0.5) * 0.25
    };
    let tip = Vec3::new(0.0, h - 1.4, 0.0);
    let shape_d = |q: Vec3| tree(q).min(sdf::cone(q - tip, 0.35, 1.5));
    vol.fill_sdf(lo, hi, shape_d, |q, d| {
        // Snow lies on the upper part of each tier's visible skirt: above a
        // noisy height over the tier rim, so the branch tips stay green.
        let mut best = (f32::MAX, 0.0f32);
        for &(c, r, ch) in &tiers {
            let cone = sdf::cone(q - c, r, ch);
            if cone < best.0 {
                best = (cone, q.y - c.y);
            }
        }
        let rim_h = best.1;
        let patch = noise3(q * 0.45 + Vec3::splat(sd + 3.0));
        let thr = 0.95 - snow * 0.75 + (patch - 0.5) * 0.6;
        if snow > 0.0 && rim_h > thr {
            return Some((MAT_MATTE, pal::PINE_SNOW));
        }
        let pal_ = if snow > 0.05 { pal::PINE } else { pal::PINE_SUMMER };
        let k = if d < -0.45 { 1 } else if patch > 0.62 { 2 } else { 0 };
        Some((MAT_FOLIAGE, pal_[k]))
    });
    vol.compile(colors)
}
