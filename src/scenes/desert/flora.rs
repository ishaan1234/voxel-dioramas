//! Desert plants, each grown once in its own volume and instanced:
//! date palms (curved trunk sweep + fronds of leaflet capsules), saguaros
//! (ribbed capsules with elbowed arms), prickly pears (stacked pad ellipsoids),
//! barrel cacti, sagebrush shrubs and oasis reeds.

use glam::Vec3;
use std::f32::consts::{PI, TAU};

use super::pal;
use crate::engine::prelude::*;

/// Voxel size of plants.
pub const FS: f32 = 0.15;

fn vol_for(lo: Vec3, hi: Vec3, s: f32) -> MicroVoxelVolume {
    let a = (lo / s).floor();
    let b = (hi / s).ceil();
    MicroVoxelVolume::new((b.x - a.x) as i32, (b.y - a.y) as i32, (b.z - a.z) as i32, s, a.x as i32, a.y as i32, a.z as i32)
}

fn capsule_fill(vol: &mut MicroVoxelVolume, a: Vec3, b: Vec3, ra: f32, rb: f32, mut paint: impl FnMut(Vec3) -> (u8, u32)) {
    let pad = Vec3::splat(ra.max(rb) + 0.3);
    vol.fill_sdf(a.min(b) - pad, a.max(b) + pad, |p| sdf::tapered_capsule(p, a, b, ra, rb), |p, _| Some(paint(p)));
}

// ------------------------------------------------------------------ palms

/// A date palm leaning toward local +X. Returns the mesh and the crown height.
pub fn build_palm(seed: u32, height: f32, lean: f32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let mut vol = vol_for(Vec3::new(-6.5, -1.0, -6.5), Vec3::new(lean + 6.5, height + 3.5, 6.5), FS);
    let trunk = |t: f32| Vec3::new(lean * t.powf(1.7), height * t, (t * 2.3).sin() * lean * 0.12);
    // Root flare.
    capsule_fill(&mut vol, Vec3::new(0.0, -0.9, 0.0), Vec3::new(0.0, 0.7, 0.0), 0.7, 0.42, |_| (MAT_WOOD, pal::PALM_BARK_DK));
    let segs = 28;
    for i in 0..segs {
        let (t0, t1) = (i as f32 / segs as f32, (i + 1) as f32 / segs as f32);
        let r0 = 0.42 - 0.15 * t0;
        let r1 = 0.42 - 0.15 * t1;
        capsule_fill(&mut vol, trunk(t0), trunk(t1), r0, r1, |p| {
            // Leaf-scar rings.
            let ring = ((p.y / 0.32).floor() as i32).rem_euclid(2) == 0;
            (MAT_WOOD, pal::cool(if ring { pal::PALM_BARK } else { pal::PALM_BARK_DK }, winter))
        });
    }
    let crown = trunk(1.0) + Vec3::Y * 0.15;
    capsule_fill(&mut vol, crown - Vec3::Y * 0.4, crown + Vec3::Y * 0.2, 0.38, 0.3, |_| (MAT_WOOD, pal::PALM_BARK_DK));
    // Coconuts / dates under the crown.
    for k in 0..5 {
        let a = k as f32 * 2.4 + rng.next_f32();
        let c = crown + Vec3::new(a.cos() * 0.42, -0.45 - rng.next_f32() * 0.25, a.sin() * 0.42);
        vol.fill_sdf_solid(c - Vec3::splat(0.4), c + Vec3::splat(0.4), |p| sdf::sphere(p - c, 0.22), MAT_MATTE, pal::COCONUT);
    }
    let n = 12;
    for k in 0..n {
        let dead = k >= n - 2;
        let az = k as f32 * TAU / (n - 2) as f32 + (rng.next_f32() - 0.5) * 0.4 + if dead { 0.9 } else { 0.0 };
        let elev = if dead { -1.15 } else { 0.25 + rng.next_f32() * 0.6 };
        let len = if dead { 2.6 } else { 4.0 + rng.next_f32() * 1.3 };
        let droop = if dead { 0.0 } else { 0.13 + rng.next_f32() * 0.08 };
        let h = Vec3::new(az.cos(), 0.0, az.sin());
        let spine = |t: f32| {
            let s = len * t;
            crown + h * (s * elev.cos()) + Vec3::Y * (s * elev.sin() - droop * s * s)
        };
        let steps = 18;
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let (a, b) = (spine(t0), spine(t1));
            let col = if dead { pal::FROND_DEAD } else { pal::FROND[if t0 < 0.3 { 1 } else if t0 < 0.75 { 0 } else { 2 }] };
            let col = pal::cool(col, winter);
            capsule_fill(&mut vol, a, b, 0.11, 0.09, |_| (MAT_FOLIAGE, col));
            if i < 2 {
                continue;
            }
            // Leaflets: pairs of thin capsules swept back and drooping.
            let tan = (b - a).normalize_or(h);
            let side = tan.cross(Vec3::Y).normalize_or(Vec3::Z);
            let w = 1.35 * (PI * t0).sin().powf(0.7) * if dead { 0.5 } else { 1.0 };
            for sgn in [-1.0f32, 1.0] {
                let end = a + side * (w * sgn) - Vec3::Y * (0.45 * w) + tan * (0.35 * w);
                capsule_fill(&mut vol, a, end, 0.1, 0.07, |_| (MAT_FOLIAGE, col));
            }
        }
    }
    vol.compile(&mut ColorCache::new())
}

// ----------------------------------------------------------------- cacti

fn ribbed(p: Vec3, c: Vec3, winter: bool, top: bool) -> (u8, u32) {
    if winter && top {
        return (MAT_MATTE, pal::FROST);
    }
    let th = (p.z - c.z).atan2(p.x - c.x);
    let col = if (12.0 * th).cos() < -0.35 { pal::CACTUS_DK } else if hash3(p.x * 7.0, p.y * 7.0, p.z * 7.0) < 0.06 { pal::CACTUS_LT } else { pal::CACTUS };
    (MAT_FOLIAGE, pal::cool(col, winter))
}

/// Saguaro: ribbed trunk plus 0-3 arms that elbow upward along a quarter circle.
pub fn build_saguaro(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let h = 5.0 + rng.next_f32() * 2.8;
    let mut vol = vol_for(Vec3::new(-3.5, -0.8, -3.5), Vec3::new(3.5, h + 1.0, 3.5), FS);
    let r = 0.5;
    let trunk = |p: Vec3| {
        let th = p.z.atan2(p.x);
        sdf::capsule(p, Vec3::new(0.0, -0.6, 0.0), Vec3::new(0.0, h, 0.0), r + 0.06 * (12.0 * th).cos())
    };
    vol.fill_sdf(Vec3::new(-1.0, -0.8, -1.0), Vec3::new(1.0, h + 0.8, 1.0), trunk, |p, _| {
        Some(ribbed(p, Vec3::ZERO, winter, p.y > h - 0.1))
    });
    let mut tips = vec![Vec3::Y * (h + 0.45)];
    let arms = (rng.next_f32() * 3.4) as usize;
    let az0 = rng.next_f32() * TAU;
    for k in 0..arms {
        let az = az0 + k as f32 * (TAU / arms.max(1) as f32) + (rng.next_f32() - 0.5) * 0.8;
        let d = Vec3::new(az.cos(), 0.0, az.sin());
        let ha = h * (0.35 + rng.next_f32() * 0.3);
        let out = 0.55 + rng.next_f32() * 0.5;
        let up = (1.2 + rng.next_f32() * 1.6).min(h - ha - 0.2);
        let rr = 0.36;
        let p0 = Vec3::Y * ha;
        let p1 = p0 + d * out + Vec3::Y * 0.12;
        let mut pts = vec![p0, p1];
        let er = 0.55;
        for i in 1..=6 {
            let phi = i as f32 / 6.0 * PI * 0.5;
            pts.push(p1 + d * (er * phi.sin()) + Vec3::Y * (er * (1.0 - phi.cos())));
        }
        let elbow = *pts.last().unwrap();
        pts.push(elbow + Vec3::Y * up);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let c = (a + b) * 0.5;
            capsule_fill(&mut vol, a, b, rr, rr, |p| ribbed(p, Vec3::new(c.x, 0.0, c.z), winter, p.y > elbow.y + up - 0.1));
        }
        tips.push(elbow + Vec3::Y * (up + rr));
    }
    if !winter {
        for t in tips {
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 + rng.next_f32();
                let c = t + Vec3::new(a.cos() * 0.3, -0.12, a.sin() * 0.3);
                vol.fill_sdf_solid(c - Vec3::splat(0.3), c + Vec3::splat(0.3), |p| sdf::sphere(p - c, 0.09), MAT_MATTE, pal::BLOOM);
            }
        }
    }
    vol.compile(&mut ColorCache::new())
}

/// Prickly pear: flat oval pads stacked on the rims of the ones below.
pub fn build_prickly_pear(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let mut vol = vol_for(Vec3::new(-2.6, -0.4, -2.6), Vec3::new(2.6, 3.4, 2.6), FS);
    // (centre, yaw, tilt, size)
    let mut pads: Vec<(Vec3, f32, f32, f32)> = Vec::new();
    for k in 0..3 {
        let a = k as f32 * 2.1 + rng.next_f32();
        pads.push((Vec3::new(a.cos() * 0.5, 0.6, a.sin() * 0.5), a + PI * 0.5, (rng.next_f32() - 0.5) * 0.6, 0.75));
    }
    for gen in 0..2 {
        let parents: Vec<_> = pads.iter().filter(|p| p.0.y > 0.4 + gen as f32 * 0.9).cloned().collect();
        for (c, yaw, tilt, s) in parents {
            let k = 1 + (rng.next_f32() * 1.6) as usize;
            for _ in 0..k {
                let lat = (rng.next_f32() - 0.5) * 1.2;
                let rim = Vec3::new(yaw.cos() * lat * s * 0.6, s * 0.85, yaw.sin() * lat * s * 0.6);
                let ny = yaw + (rng.next_f32() - 0.5) * 1.4;
                pads.push((c + rim + Vec3::Y * s * 0.6, ny, tilt + (rng.next_f32() - 0.5) * 0.7, s * 0.85));
            }
        }
    }
    for &(c, yaw, tilt, s) in &pads {
        let pad = |p: Vec3| {
            let q = sdf::rot_x(sdf::rot_y(p - c, yaw), tilt);
            sdf::ellipsoid(q, Vec3::new(s * 0.62, s * 0.8, 0.17))
        };
        vol.fill_sdf(c - Vec3::splat(s + 0.4), c + Vec3::splat(s + 0.4), pad, |p, _| {
            let top = pad(p + Vec3::Y * FS) > 0.0;
            if winter && top {
                return Some((MAT_MATTE, pal::FROST));
            }
            let dot = hash3((p.x * 5.0).floor(), (p.y * 5.0).floor(), (p.z * 5.0).floor()) < 0.05;
            Some((MAT_FOLIAGE, pal::cool(if dot { pal::CACTUS_LT } else { pal::CACTUS }, winter)))
        });
        if !winter && c.y > 1.0 && rng.next_f32() < 0.6 {
            let f = c + Vec3::new(yaw.cos(), 0.0, yaw.sin()) * (s * 0.3) + Vec3::Y * (s * 0.78);
            vol.fill_sdf_solid(f - Vec3::splat(0.3), f + Vec3::splat(0.3), |p| sdf::ellipsoid(p - f, Vec3::new(0.12, 0.16, 0.12)), MAT_MATTE, pal::PEAR_FRUIT);
        }
    }
    vol.compile(&mut ColorCache::new())
}

/// Barrel cactus: a ribbed, squat ellipsoid with a crown of flowers.
pub fn build_barrel(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let h = 0.8 + rng.next_f32() * 0.4;
    let mut vol = vol_for(Vec3::new(-1.0, -0.4, -1.0), Vec3::new(1.0, 2.0 * h + 0.5, 1.0), FS);
    let c = Vec3::Y * (h * 0.85);
    let body = |p: Vec3| {
        let th = (p.z - c.z).atan2(p.x - c.x);
        sdf::ellipsoid(p - c, Vec3::new(0.62, h, 0.62)) - 0.06 * (14.0 * th).cos()
    };
    vol.fill_sdf(Vec3::new(-1.0, -0.4, -1.0), Vec3::new(1.0, 2.0 * h, 1.0), body, |p, _| Some(ribbed(p, c, winter, p.y > c.y + h * 0.85)));
    if !winter {
        for k in 0..7 {
            let a = k as f32 * TAU / 7.0;
            let f = c + Vec3::new(a.cos() * 0.25, h * 0.95, a.sin() * 0.25);
            vol.fill_sdf_solid(f - Vec3::splat(0.3), f + Vec3::splat(0.3), |p| sdf::sphere(p - f, 0.12), MAT_MATTE, pal::BARREL_FLOWER);
        }
    }
    vol.compile(&mut ColorCache::new())
}

// ------------------------------------------------------------ shrubs, reeds

/// Sagebrush: a twiggy base under a cluster of noise-roughened grey-green blobs.
pub fn build_shrub(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let mut vol = vol_for(Vec3::new(-1.6, -0.3, -1.6), Vec3::new(1.6, 1.8, 1.6), FS);
    for k in 0..5 {
        let a = k as f32 * 1.3 + rng.next_f32();
        let tip = Vec3::new(a.cos() * 0.6, 0.6, a.sin() * 0.6);
        capsule_fill(&mut vol, Vec3::new(0.0, -0.2, 0.0), tip, 0.08, 0.05, |_| (MAT_WOOD, pal::LOG));
    }
    let n = 6 + (rng.next_f32() * 3.0) as usize;
    for k in 0..n {
        let a = k as f32 * 2.4 + rng.next_f32();
        let r = 0.2 + rng.next_f32() * 0.7;
        let c = Vec3::new(a.cos() * r, 0.6 + rng.next_f32() * 0.5, a.sin() * r);
        let rad = 0.35 + rng.next_f32() * 0.25;
        let sd = seed as f32 + k as f32;
        vol.fill_sdf(c - Vec3::splat(rad + 0.4), c + Vec3::splat(rad + 0.4), |p| sdf::sphere(p - c, rad) + (noise3(p * 3.0 + Vec3::splat(sd)) - 0.5) * 0.35, |p, _| {
            if winter && p.y > c.y + rad * 0.4 {
                return Some((MAT_FOLIAGE, pal::FROST_DK));
            }
            let i = (hash3(p.x * 6.0, p.y * 6.0, p.z * 6.0) * 2.99) as usize;
            Some((MAT_FOLIAGE, pal::cool(pal::SHRUB[i], winter)))
        });
    }
    vol.compile(&mut ColorCache::new())
}

/// A clump of reeds / rushes, some with cattail heads.
pub fn build_reeds(seed: u32, winter: bool) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let s = 0.1;
    let mut vol = vol_for(Vec3::new(-1.3, -0.3, -1.3), Vec3::new(1.3, 2.4, 1.3), s);
    for k in 0..16 {
        let a = k as f32 * 2.4 + rng.next_f32();
        let r0 = rng.next_f32() * 0.3;
        let base = Vec3::new(a.cos() * r0, -0.2, a.sin() * r0);
        let h = 1.1 + rng.next_f32() * 0.9;
        let tip = base + Vec3::new(a.cos() * (0.3 + rng.next_f32() * 0.5), h, a.sin() * (0.3 + rng.next_f32() * 0.5));
        let mid = base.lerp(tip, 0.5) + Vec3::new(a.cos(), 0.0, a.sin()) * -0.08;
        for (p0, p1, r) in [(base, mid, 0.07), (mid, tip, 0.05)] {
            capsule_fill(&mut vol, p0, p1, r, r * 0.8, |p| {
                let t = ((p.y + 0.2) / h).clamp(0.0, 1.0);
                (MAT_FOLIAGE, pal::cool(pal::REED[if t < 0.4 { 0 } else if t < 0.8 { 1 } else { 2 }], winter))
            });
        }
        if k % 4 == 0 {
            let c = base.lerp(tip, 0.8);
            vol.fill_sdf_solid(c - Vec3::splat(0.3), c + Vec3::splat(0.3), |p| sdf::capsule(p, c - Vec3::Y * 0.12, c + Vec3::Y * 0.12, 0.08), MAT_MATTE, pal::COCONUT);
        }
    }
    vol.compile(&mut ColorCache::new())
}
