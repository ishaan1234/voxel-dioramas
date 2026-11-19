//! Small animated elements: campfire flames and chimney smoke. Every `push_*`
//! appends exactly the number of instances reserved in `layout.rs`.

use glam::{Mat4, Vec3};
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

/// A soft smoke puff: a noise-lumpy voxel sphere (unit diameter).
pub fn build_puff(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.1;
    let mut vol = MicroVoxelVolume::new(12, 12, 12, s, -6, -6, -6);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sdf::sphere(p, 0.48) + (noise3(p * 6.0) - 0.5) * 0.18, MAT_MIST, pal::SMOKE);
    vol.compile(colors)
}

pub struct Flame {
    pub base: Vec3,
    pub phase: f32,
    pub scale: f32,
    pub lean: f32,
}

pub fn make_flames(fire: Vec3) -> Vec<Flame> {
    (0..4)
        .map(|i| {
            let a = i as f32 * TAU / 4.0 + 0.3;
            let r = if i == 0 { 0.0 } else { 0.32 };
            Flame {
                base: fire + Vec3::new(a.cos() * r, 0.05, a.sin() * r),
                phase: i as f32 * 1.7,
                scale: if i == 0 { 1.15 } else { 0.75 },
                lean: a,
            }
        })
        .collect()
}

pub fn push_flames(flames: &[Flame], t: f32, out: &mut Vec<ModelInstance>) {
    for f in flames {
        let w = t * 9.0 + f.phase;
        let sy = f.scale * (0.85 + 0.18 * w.sin() + 0.08 * (w * 2.7).sin());
        let sxz = f.scale * (0.95 - 0.08 * (w * 1.3).cos());
        let tilt = 0.08 * (w * 0.7).sin();
        let m = Mat4::from_translation(f.base)
            * Mat4::from_rotation_y(f.lean + t * 0.6)
            * Mat4::from_rotation_z(tilt)
            * Mat4::from_scale(Vec3::new(sxz, sy, sxz));
        out.push(ModelInstance::from_mat4(m));
    }
}

pub struct Puff {
    pub src: Vec3,
    pub phase: f32,
}

pub const PUFFS_PER_CHIMNEY: usize = 7;

pub fn make_smoke(chimneys: &[Vec3]) -> Vec<Puff> {
    let mut v = Vec::new();
    for (ci, &c) in chimneys.iter().enumerate() {
        for k in 0..PUFFS_PER_CHIMNEY {
            v.push(Puff { src: c, phase: k as f32 / PUFFS_PER_CHIMNEY as f32 + ci as f32 * 0.137 });
        }
    }
    v
}

/// Puffs rise, grow and drift downwind, looping every ~9 seconds.
pub fn push_smoke(puffs: &[Puff], t: f32, out: &mut Vec<ModelInstance>) {
    let wind = Vec3::new(0.8, 0.0, 0.35);
    for p in puffs {
        let u = (p.phase + t * 0.11).fract();
        let wob = Vec3::new((u * 9.0 + p.phase * 20.0).sin() * 0.3, 0.0, (u * 7.0 + p.phase * 13.0).cos() * 0.3);
        let pos = p.src + Vec3::Y * (u * 8.0 + 0.2) + wind * (u * u * 6.0) + wob * u;
        let sc = 0.45 + u * 1.9;
        // Shrink to nothing at the end of the loop instead of popping.
        let fade = if u > 0.8 { (1.0 - u) / 0.2 } else { 1.0 };
        out.push(ModelInstance::from_trs(pos, u * 2.0 + p.phase * 5.0, Vec3::splat(sc * fade)));
    }
}
