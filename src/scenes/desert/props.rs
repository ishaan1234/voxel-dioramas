//! Small instanced props: sandstone boulders, torches, the nomad camp and the
//! meshes for animated flames and vultures.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

fn vol_for(lo: Vec3, hi: Vec3, s: f32) -> MicroVoxelVolume {
    let a = (lo / s).floor();
    let b = (hi / s).ceil();
    MicroVoxelVolume::new((b.x - a.x) as i32, (b.y - a.y) as i32, (b.z - a.z) as i32, s, a.x as i32, a.y as i32, a.z as i32)
}

/// A weathered sandstone boulder: noisy ellipsoid sliced by a few random
/// planes (angular joints), banded like the canyon walls. Origin = ground.
pub fn build_rock(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let s = 0.2;
    let r = Vec3::new(1.4 + rng.next_f32() * 1.0, 0.9 + rng.next_f32() * 0.7, 1.2 + rng.next_f32() * 0.9);
    let c = Vec3::Y * (r.y * 0.55);
    let mut cuts = Vec::new();
    for _ in 0..4 {
        let a = rng.next_f32() * TAU;
        let n = Vec3::new(a.cos(), rng.next_f32() * 1.2 - 0.2, a.sin()).normalize();
        cuts.push((n, r.min_element() * (0.55 + rng.next_f32() * 0.3)));
    }
    let off = rng.next_f32() * 10.0;
    let rock = |p: Vec3| {
        let q = p - c;
        let mut d = sdf::ellipsoid(q, r) + (fbm3(p * 0.9 + Vec3::splat(off), 2) - 0.5) * 0.7;
        for &(n, k) in &cuts {
            d = d.max(q.dot(n) - k);
        }
        d
    };
    let mut vol = vol_for(-r - Vec3::ONE + c, r + Vec3::ONE + c, s);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf(lo, hi, rock, |p, _| {
        if winter && rock(p + Vec3::Y * s) > 0.0 {
            return Some((MAT_MATTE, pal::FROST));
        }
        let band = ((p.y * 2.2 + off + noise2(p.x, p.z) * 0.8).floor() as i32).rem_euclid(4) as usize;
        let col = [pal::STRATA[2].1, pal::STRATA[4].1, pal::STRATA[1].1, pal::STRATA[8].1][band];
        Some((MAT_MATTE, pal::cool(col, winter)))
    });
    vol.compile(&mut ColorCache::new())
}

/// Height of a torch's flame base above its foot.
pub const TORCH_H: f32 = 2.45;

/// A wooden torch pole with a bronze brazier of glowing embers.
pub fn build_torch() -> (MeshData, u64) {
    let s = 0.08;
    let mut vol = vol_for(Vec3::new(-0.6, -0.5, -0.6), Vec3::new(0.6, 2.8, 0.6), s);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, Vec3::new(0.0, -0.45, 0.0), Vec3::new(0.0, 2.15, 0.0), 0.09), MAT_WOOD, pal::POLE);
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0;
        let foot = Vec3::new(a.cos() * 0.35, 0.0, a.sin() * 0.35);
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, foot, Vec3::new(0.0, 0.8, 0.0), 0.05), MAT_WOOD, pal::POLE);
    }
    let b = Vec3::Y * 2.4;
    vol.fill_sdf_solid(lo, hi, |p| sdf::shell(sdf::sphere(p - b - Vec3::Y * 0.12, 0.3), 0.05).max(p.y - b.y - 0.1), MAT_METAL, pal::BRONZE);
    vol.fill_sdf(lo, hi, |p| sdf::ellipsoid(p - b, Vec3::new(0.22, 0.09, 0.22)), |p, _| {
        Some((MAT_LAMP, if hash3(p.x * 9.0, p.y * 9.0, p.z * 9.0) < 0.5 { pal::EMBER } else { pal::FLAME }))
    });
    vol.compile(&mut ColorCache::new())
}

/// A flame tongue (instanced + flickered every frame).
pub fn build_flame() -> (MeshData, u64) {
    let s = 0.06;
    let mut vol = vol_for(Vec3::new(-0.35, -0.1, -0.35), Vec3::new(0.35, 0.9, 0.35), s);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf(lo, hi, |p| sdf::tapered_capsule(p, Vec3::Y * 0.12, Vec3::Y * 0.7, 0.22, 0.02), |p, _| {
        Some((MAT_LAMP, if p.y < 0.3 && Vec2::new(p.x, p.z).length() < 0.12 { pal::BLOOM } else if p.y < 0.4 { pal::FLAME } else { pal::EMBER }))
    });
    vol.compile(&mut ColorCache::new())
}

/// Instance scale of the camp.
pub const CAMP_SCALE: f32 = 1.4;
/// Local position of the camp fire (relative to the camp origin, unscaled).
pub const CAMPFIRE: Vec3 = Vec3::new(0.0, 0.0, 3.4);

/// Nomad camp: an A-frame canvas tent, a stone-ringed fire with crossed logs
/// and a few clay jars. Origin = ground at the tent centre.
pub fn build_camp(winter: bool) -> (MeshData, u64) {
    let s = 0.12;
    let mut vol = vol_for(Vec3::new(-3.0, -0.6, -2.4), Vec3::new(3.0, 2.4, 5.0), s);
    let (lo, hi) = vol.local_bounds();
    // Tent: triangular prism with an open front, slightly sagging ridge.
    let (half_len, half_w, h) = (1.7, 1.5, 1.8);
    let tent = |p: Vec3| {
        let sag = 0.12 * (1.0 - (p.x / half_len).powi(2));
        let roof = (p.z.abs() * h / half_w + p.y - h + sag) * 0.65;
        let solid = roof.max(p.x.abs() - half_len).max(-p.y - 0.3);
        let inner = (p.z.abs() * h / half_w + p.y - h + sag + 0.3) * 0.65;
        // Hollow, with the +x end open (the back wall at -x stays).
        let cavity = inner.max(-p.y).max(p.x - half_len - 1.0).max(-p.x - half_len + 0.15);
        solid.max(-cavity)
    };
    vol.fill_sdf(lo, hi, tent, |p, _| {
        if winter && p.y > 0.9 {
            return Some((MAT_MATTE, pal::FROST));
        }
        let band = (p.y > 0.25 && p.y < 0.5) || (p.x.abs() > half_len - 0.25 && p.y < 1.0);
        Some((MAT_MATTE, if band { pal::CANVAS_DK } else { pal::CANVAS }))
    });
    for x in [-half_len - 0.05, half_len + 0.05] {
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, Vec3::new(x, -0.4, 0.0), Vec3::new(x, h + 0.25, 0.0), 0.06), MAT_WOOD, pal::POLE);
    }
    // Fire ring.
    let f = CAMPFIRE;
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0 + 0.2;
        let c = f + Vec3::new(a.cos() * 0.8, 0.08, a.sin() * 0.8);
        let r = Vec3::new(0.24, 0.17 + 0.05 * (k % 3) as f32, 0.2);
        vol.fill_sdf_solid(c - Vec3::splat(0.4), c + Vec3::splat(0.4), |p| sdf::ellipsoid(sdf::rot_y(p - c, a), r), MAT_MATTE, pal::CAMP_STONE);
    }
    for (a, b) in [(Vec3::new(-0.5, 0.1, -0.3), Vec3::new(0.5, 0.25, 0.3)), (Vec3::new(0.45, 0.1, -0.4), Vec3::new(-0.4, 0.3, 0.35))] {
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, f + a, f + b, 0.09), MAT_WOOD, pal::LOG);
    }
    vol.fill_sdf(f - Vec3::ONE, f + Vec3::ONE, |p| sdf::ellipsoid(p - f, Vec3::new(0.42, 0.14, 0.42)), |p, _| {
        Some((MAT_LAMP, if hash3(p.x * 8.0, p.y * 8.0, p.z * 8.0) < 0.5 { pal::EMBER } else { pal::FLAME }))
    });
    // Clay jars beside the tent.
    for (k, c) in [Vec3::new(-2.2, 0.0, 1.3), Vec3::new(-2.55, 0.0, 0.7), Vec3::new(2.3, 0.0, -1.2)].into_iter().enumerate() {
        let hh = 0.45 + 0.12 * k as f32;
        let cc = c + Vec3::Y * hh;
        vol.fill_sdf_solid(lo, hi, |p| sdf::ellipsoid(p - cc, Vec3::new(0.34, hh, 0.34)).min(sdf::cylinder(p - cc - Vec3::Y * hh, 0.17, 0.12)), MAT_MATTE, 0xb8653a);
    }
    vol.compile(&mut ColorCache::new())
}

/// A vulture: body capsule, broad fingered wings, pale head.
pub fn build_vulture() -> (MeshData, u64) {
    let s = 0.08;
    let mut vol = vol_for(Vec3::new(-1.5, -0.4, -0.8), Vec3::new(1.5, 0.5, 0.9), s);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sdf::tapered_capsule(p, Vec3::new(0.0, 0.0, -0.5), Vec3::new(0.0, 0.05, 0.4), 0.17, 0.12), MAT_MATTE, pal::VULTURE);
    vol.fill_sdf_solid(lo, hi, |p| sdf::sphere(p - Vec3::new(0.0, 0.1, 0.62), 0.1), MAT_MATTE, pal::VULTURE_HEAD);
    for side in [-1.0f32, 1.0] {
        let a = Vec3::new(0.0, 0.05, 0.05);
        let b = Vec3::new(side * 0.7, 0.14, 0.0);
        let c = Vec3::new(side * 1.35, 0.1, -0.1);
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, a, b, 0.14).min(sdf::capsule(p, b, c, 0.11)), MAT_MATTE, pal::VULTURE);
        for k in 0..3 {
            let tip = c + Vec3::new(side * 0.12, 0.0, -0.12 - k as f32 * 0.1);
            vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, c - Vec3::Z * (k as f32 * 0.1), tip, 0.05), MAT_MATTE, pal::VULTURE);
        }
    }
    vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, Vec3::new(0.0, 0.0, -0.5), Vec3::new(0.0, 0.0, -0.8), 0.1).max(p.y.abs() - 0.05), MAT_MATTE, pal::VULTURE);
    vol.compile(&mut ColorCache::new())
}
