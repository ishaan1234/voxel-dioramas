//! Igloo camp pieces. The igloo is a hemispherical snow shell (sphere ∩ upper
//! half-space minus an inner sphere) with a half-cylinder entrance vault along
//! local +X. Block seams follow a rising spiral (how real igloos are built):
//! course = floor(elevation / course_h + azimuth / 2π), with staggered joints.
//! The inner surface glows warm so the entrance reads as lit at night.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

const IGLOO_SCALE: f32 = 0.15;

pub fn build_igloo(seed: u32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let r = 3.4 + rng.next_f32() * 0.5;
    let t = 0.6;
    let ri = r - t;
    let (tr_o, tr_i) = (1.25, 0.8);
    let (tx0, tx1) = (r * 0.5, r + 1.7);
    let s = IGLOO_SCALE;
    let nx0 = ((r + 0.8) / s).ceil() as i32;
    let nx1 = ((tx1 + 0.6) / s).ceil() as i32;
    let nz = ((r + 0.8) / s).ceil() as i32;
    let ny = ((r + 0.6) / s).ceil() as i32;
    let ny0 = (1.6 / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(nx0 + nx1, ny + ny0, nz * 2, s, -nx0, -ny0, -nz);
    let (lo, hi) = vol.local_bounds();

    // Packed-snow base mound so the igloo sits into any slope.
    vol.fill_sdf(
        lo,
        hi,
        |p| {
            let rr = Vec2::new(p.x, p.z).length() - (r + 0.45 + (-p.y).max(0.0) * 0.5);
            rr.max(p.y - 0.05).max(-p.y - 1.5)
        },
        |p, _| Some((MAT_MATTE, if p.y > -0.3 { pal::IGLOO_B } else { pal::SNOW_DEEP })),
    );
    let tunnel_outer = |p: Vec3| {
        let yz = Vec2::new(p.y, p.z).length() - tr_o;
        yz.max(-p.y).max((p.x - (tx0 + tx1) * 0.5).abs() - (tx1 - tx0) * 0.5)
    };
    let dome_outer = |p: Vec3| sdf::sphere(p, r).max(-p.y);
    vol.fill_sdf(
        lo,
        hi,
        |p| dome_outer(p).min(tunnel_outer(p)),
        |p, _| {
            let in_dome = dome_outer(p) <= 0.0;
            if in_dome && p.length() < ri + 0.2 {
                return Some((MAT_WIN, pal::IGLOO_GLOW));
            }
            let c = if in_dome {
                let len = p.length().max(1e-3);
                let phi = (p.y / len).clamp(-1.0, 1.0).asin();
                let theta = p.z.atan2(p.x);
                let u = phi / 0.24 + theta / TAU;
                let course = u.floor();
                let n = ((phi.cos() * 13.0).round()).max(3.0);
                let v = theta / TAU * n + course * 0.5;
                if u.fract() < 0.13 || v.rem_euclid(1.0) < 0.07 {
                    pal::IGLOO_SEAM
                } else if (course as i32 + v.floor() as i32).rem_euclid(3) == 0 {
                    pal::IGLOO_B
                } else {
                    pal::IGLOO
                }
            } else {
                let a = p.y.atan2(p.z);
                let ring = (p.x / 0.75).floor();
                if (p.x / 0.75).fract() < 0.16 || ((a / 0.7 + ring * 0.5).rem_euclid(1.0)) < 0.09 {
                    pal::IGLOO_SEAM
                } else {
                    pal::IGLOO
                }
            };
            Some((MAT_MATTE, c))
        },
    );
    // Hollow interior + tunnel bore (open at the far end).
    vol.carve_sdf(lo, hi, |p| sdf::sphere(p, ri).max(-p.y));
    vol.carve_sdf(lo, hi, |p| {
        let yz = Vec2::new(p.y, p.z).length() - tr_i;
        yz.max(-p.y).max(p.x - (tx1 + 1.0)).max(r * 0.2 - p.x)
    });
    // Warm floor inside.
    vol.fill_sdf(
        lo,
        hi,
        |p| (Vec2::new(p.x, p.z).length() - (ri - 0.05)).max((p.y + 0.06).abs() - 0.07),
        |_, _| Some((MAT_WIN, pal::IGLOO_GLOW)),
    );
    vol.compile(colors)
}

/// Campfire: a ring of rounded stones, crossed logs on a bed of embers, and
/// three snow-capped log benches around it. Origin at ground level.
pub fn build_fire_pit(seed: u32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let s: f32 = 0.12;
    let half = (5.2 / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(half * 2, (2.4 / s) as i32, half * 2, s, -half, (-0.8 / s) as i32, -half);
    let (lo, hi) = vol.local_bounds();
    let stones = 11;
    for i in 0..stones {
        let a = i as f32 / stones as f32 * TAU + rng.next_f32() * 0.2;
        let c = Vec3::new(a.cos() * 1.25, 0.12, a.sin() * 1.25);
        let rad = Vec3::new(0.34, 0.26, 0.3) * (0.85 + rng.next_f32() * 0.35);
        let col = pal::STONE[i % 3];
        vol.fill_sdf(c - Vec3::splat(0.6), c + Vec3::splat(0.6), |p| sdf::ellipsoid(sdf::rot_y(p - c, a), rad), |_, _| {
            Some((MAT_MATTE, col))
        });
    }
    // Ember bed and crossed logs.
    vol.fill_sdf(lo, hi, |p| sdf::ellipsoid(p - Vec3::Y * 0.02, Vec3::new(0.95, 0.22, 0.95)), |p, _| {
        let hot = (p.x * 5.0).sin() * (p.z * 4.0).cos() > 0.1;
        Some((MAT_LAMP, if hot { pal::EMBER } else { pal::FLAME }))
    });
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 0.4;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let p0 = d * 0.85 + Vec3::Y * 0.1;
        let p1 = -d * 0.15 + Vec3::Y * 0.55;
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, p0, p1, 0.13), MAT_WOOD, pal::LOG_DK);
    }
    // Log benches facing the fire.
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 1.1 + rng.next_f32() * 0.4;
        let c = Vec3::new(a.cos() * 3.6, 0.25, a.sin() * 3.6);
        let tdir = Vec3::new(-a.sin(), 0.0, a.cos());
        let (p0, p1) = (c - tdir * 1.1, c + tdir * 1.1);
        vol.fill_sdf(lo, hi, |p| sdf::capsule(p, p0, p1, 0.3), |p, _| {
            let top = sdf::capsule(p + Vec3::Y * 0.2, p0, p1, 0.3) > 0.0;
            Some(if top { (MAT_MATTE, pal::SNOW) } else { (MAT_WOOD, pal::LOG) })
        });
    }
    vol.compile(colors)
}

/// A flame tongue (instanced and flickered in `update_dynamic_instances`).
pub fn build_flame(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.1;
    let mut vol = MicroVoxelVolume::new(14, 22, 14, s, -7, 0, -7);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf(
        lo,
        hi,
        |p| sdf::tapered_capsule(p, Vec3::Y * 0.3, Vec3::Y * 1.6, 0.42, 0.04) + (noise3(p * 3.0) - 0.5) * 0.2,
        |p, _| Some((MAT_LAMP, if p.y < 0.75 { pal::FLAME_HOT } else { pal::FLAME })),
    );
    vol.compile(colors)
}
