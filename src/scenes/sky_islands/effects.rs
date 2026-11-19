//! Animated, instanced elements. Every `push_*` function appends exactly the
//! number of instances its batch reserved in `layout.rs`, in the same order.

use glam::{Mat4, Quat, Vec3};
use std::f32::consts::TAU;

use super::island::CLOUD_TOP_Y;
use super::pal;
use crate::engine::prelude::*;

// ------------------------------ cloud sea ----------------------------------

pub const SEA_HALF: f32 = 340.0;
/// Highest point any skimming puff reaches (centre + half its max height).
#[cfg_attr(not(test), allow(dead_code))]
pub const PUFF_MAX_Y: f32 = CLOUD_TOP_Y + 2.5 + 3.4 * 0.6;
const SEA_SCALE: f32 = 1.5;

/// The cloud sea itself: an opaque voxel height-field (so islands cast shadows
/// on it). Billow height = smoothstepped low-frequency fbm + fine detail.
pub fn build_cloud_sea(colors: &mut ColorCache) -> (MeshData, u64) {
    let n = (SEA_HALF / SEA_SCALE).ceil() as i32;
    let base = CLOUD_TOP_Y - 9.0;
    let mut vol = MicroVoxelVolume::new(n * 2, 12, n * 2, SEA_SCALE, -n, (base / SEA_SCALE).floor() as i32, -n);
    let (lo, hi) = vol.local_bounds();
    vol.fill_columns(
        lo,
        hi,
        |x, z| {
            let big = fbm2(x * 0.006 + 40.0, z * 0.006 - 13.0, 4);
            let t = ((big - 0.3) / 0.4).clamp(0.0, 1.0);
            let billow = t * t * (3.0 - 2.0 * t) * 9.0 + fbm2(x * 0.03, z * 0.03, 3) * 4.0;
            Some((base, base + 1.0 + billow))
        },
        |_, depth| {
            let col = if depth < 1.6 { pal::CLOUD } else if depth < 3.2 { pal::CLOUD_MID } else { pal::CLOUD_LOW };
            Some((MAT_CLOUD, col))
        },
    );
    vol.compile(colors)
}

pub struct Puff {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub scale: Vec3,
    pub speed: f32,
}

/// Drifting puffs skimming the sea, plus high wisps well above the islands.
/// Puffs stay below `PUFF_MAX_Y`; islands are kept above it (see tests).
pub fn make_clouds(rng: &mut Rng) -> Vec<Puff> {
    let mut out = Vec::new();
    let step = 16.0;
    let n = (SEA_HALF * 2.0 / step) as i32;
    for iz in 0..n {
        for ix in 0..n {
            let x = -SEA_HALF + (ix as f32 + rng.next_f32()) * step;
            let z = -SEA_HALF + (iz as f32 + rng.next_f32()) * step;
            let v = fbm2(x * 0.017 - 7.0, z * 0.017 + 21.0, 3);
            if v < 0.56 {
                continue;
            }
            let s = 0.8 + rng.next_f32() * 0.9;
            out.push(Puff { x, y: CLOUD_TOP_Y + 0.5 + rng.next_f32() * 2.0, z, scale: Vec3::new(s, 0.6 + rng.next_f32() * 0.6, s * 0.8), speed: 1.6 + rng.next_f32() * 0.8 });
        }
    }
    for _ in 0..14 {
        let cx = (rng.next_f32() - 0.5) * 600.0;
        let cz = (rng.next_f32() - 0.5) * 600.0;
        let cy = 235.0 + rng.next_f32() * 25.0;
        let speed = 2.0 + rng.next_f32() * 1.5;
        let k = 5 + (rng.next_f32() * 7.0) as i32;
        for i in 0..k {
            let a = i as f32 * 2.4;
            let r = (i as f32).sqrt() * 7.0;
            out.push(Puff { x: cx + a.cos() * r * 1.7, y: cy + rng.next_f32() * 2.0, z: cz + a.sin() * r, scale: Vec3::new(1.2, 0.5, 1.2), speed });
        }
    }
    out
}

pub fn push_clouds(puffs: &[Puff], t: f32, out: &mut Vec<ModelInstance>) {
    let span = SEA_HALF * 2.0;
    for p in puffs {
        let x = (p.x + t * p.speed + SEA_HALF).rem_euclid(span) - SEA_HALF;
        let bob = (t * 0.2 + p.z * 0.05).sin() * 0.3;
        out.push(ModelInstance::from_trs(Vec3::new(x, p.y + bob, p.z), 0.0, p.scale));
    }
}

// ----------------------------- waterfall spray -----------------------------

pub struct Spray {
    pub lip: Vec3,
    pub dir: Vec3,
    pub t0: f32,
    pub side: f32,
    pub speed: f32,
    pub scale: f32,
}

pub fn make_spray(lips: &[(Vec3, Vec3)], per_fall: usize, rng: &mut Rng) -> Vec<Spray> {
    let mut out = Vec::new();
    for &(lip, dir) in lips {
        for _ in 0..per_fall {
            out.push(Spray {
                lip,
                dir,
                t0: rng.next_f32(),
                side: (rng.next_f32() - 0.5) * 3.4,
                speed: 0.25 + rng.next_f32() * 0.2,
                scale: 0.5 + rng.next_f32() * 0.9,
            });
        }
    }
    out
}

/// Droplets ride the same ballistic curve as the voxel waterfall, then puff
/// outward where they hit the cloud sea.
pub fn push_spray(spray: &[Spray], t: f32, out: &mut Vec<ModelInstance>) {
    let (v0, g) = (2.6, 9.8);
    for s in spray {
        let fall = s.lip.y - CLOUD_TOP_Y;
        let t_end = (2.0 * fall / g).sqrt();
        let u = (s.t0 + t * s.speed).fract();
        let tt = u * t_end;
        let side = Vec3::new(-s.dir.z, 0.0, s.dir.x);
        let mut p = s.lip + s.dir * (v0 * tt + 0.3) - Vec3::Y * (0.5 * g * tt * tt) + side * (s.side * (1.0 + tt * 0.4));
        let mut sc = s.scale * (0.6 + u * 0.8);
        if u > 0.88 {
            let k = (u - 0.88) / 0.12;
            sc *= 1.0 + k * 3.0;
            p += side * (s.side * k * 2.0) + Vec3::Y * (k * 2.5);
        }
        out.push(ModelInstance::from_trs(p, 0.0, Vec3::splat(sc)));
    }
}

// --------------------------------- islets ----------------------------------

pub struct Islet {
    pub base: Vec3,
    pub phase: f32,
    pub spin: f32,
    pub scale: f32,
}

pub fn push_islets(islets: &[Islet], t: f32, out: &mut Vec<ModelInstance>) {
    for i in islets {
        let y = i.base.y + (t * 0.45 + i.phase).sin() * 1.4;
        let yaw = i.phase + t * i.spin;
        out.push(ModelInstance::from_trs(Vec3::new(i.base.x, y, i.base.z), yaw, Vec3::splat(i.scale)));
    }
}

// --------------------------------- birds -----------------------------------

pub struct Bird {
    pub center: Vec3,
    pub radius: f32,
    pub omega: f32,
    pub phase: f32,
    pub height: f32,
}

pub fn make_birds(center: Vec3, n: usize, rng: &mut Rng) -> Vec<Bird> {
    (0..n)
        .map(|i| Bird {
            center,
            radius: 46.0 + rng.next_f32() * 26.0,
            omega: 0.16 + rng.next_f32() * 0.05,
            phase: i as f32 * 0.35 + rng.next_f32() * 0.3,
            height: 16.0 + rng.next_f32() * 18.0,
        })
        .collect()
}

pub fn push_birds(birds: &[Bird], t: f32, out: &mut Vec<ModelInstance>) {
    for b in birds {
        let a = b.phase + t * b.omega;
        let wob = (t * 0.7 + b.phase * 3.0).sin();
        let r = b.radius + wob * 4.0;
        let pos = b.center + Vec3::new(a.cos() * r, b.height + (t * 0.9 + b.phase).sin() * 2.5, a.sin() * r);
        // Heading = tangent of the circle; local +X (right wing) points outward,
        // so a positive roll about Z lifts it and banks the bird into the turn.
        let heading = -a; // derivative direction of (cos a, sin a) is (-sin a, cos a)
        let flap = (t * 7.0 + b.phase * 11.0).sin() * 0.12;
        let m = Mat4::from_translation(pos) * Mat4::from_quat(Quat::from_rotation_y(heading) * Quat::from_rotation_z(0.35 + flap));
        out.push(ModelInstance::from_mat4(m));
    }
}

/// A gull: capsule body plus two swept-back wing capsules, voxelised at 0.08 m.
pub fn build_bird(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.08;
    let mut vol = MicroVoxelVolume::new(30, 12, 26, s, -15, -6, -13);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sdf::tapered_capsule(p, Vec3::new(0.0, 0.0, -0.45), Vec3::new(0.0, 0.05, 0.5), 0.15, 0.1), MAT_MATTE, pal::BIRD);
    for side in [-1.0f32, 1.0] {
        let a = Vec3::new(0.0, 0.05, 0.05);
        let b = Vec3::new(side * 0.55, 0.18, -0.05);
        let c = Vec3::new(side * 1.1, 0.05, -0.32);
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, a, b, 0.07), MAT_MATTE, pal::BIRD);
        vol.fill_sdf_solid(lo, hi, |p| sdf::capsule(p, b, c, 0.06), MAT_MATTE, pal::BIRD_TIP);
    }
    vol.compile(colors)
}

// ------------------------------- armillary ---------------------------------

/// Three nested rings tumbling on different axes, plus a gently bobbing orb.
pub fn push_armillary(center: Vec3, t: f32, out: &mut Vec<ModelInstance>) {
    let bob = Vec3::Y * ((t * 0.6).sin() * 0.4);
    let c = center + bob;
    let rings = [
        Quat::from_rotation_y(t * 0.15) * Quat::from_rotation_x(0.42),
        Quat::from_rotation_z(t * 0.27) * Quat::from_rotation_x(TAU * 0.25),
        Quat::from_rotation_x(t * 0.4) * Quat::from_rotation_z(TAU * 0.25 + 0.3),
    ];
    for q in rings {
        out.push(ModelInstance::from_mat4(Mat4::from_translation(c) * Mat4::from_quat(q)));
    }
    let pulse = 1.0 + (t * 1.7).sin() * 0.06;
    out.push(ModelInstance::from_mat4(Mat4::from_translation(c) * Mat4::from_scale(Vec3::splat(pulse))));
}
