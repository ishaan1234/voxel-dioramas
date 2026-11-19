//! Wildflowers built from SDFs at 3 cm voxels: curved stems (swept capsules),
//! leaf blades (rotated ellipsoids), and heads made with polar repetition
//! (petal rings), shells (tulip cups) and stacked buds (lavender).

use glam::Vec3;
use std::f32::consts::TAU;

use crate::engine::prelude::*;
use crate::engine::sdf::*;

/// Voxel sizes for the three levels of detail (near, mid, far). The shapes are
/// SDFs in metres, so each LOD is the same formula sampled more coarsely.
pub const LOD_SCALES: [f32; 3] = [0.03, 0.06, 0.12];
/// Camera distance (m, at 1080p) up to which each LOD is used: the coarser
/// voxel is then at most ~1.2 px on screen.
pub const LOD_DIST: [f32; 3] = [70.0, 140.0, f32::INFINITY];

const STEM: u32 = 0x4f8f35;
const STEM_DK: u32 = 0x3d7429;
const LEAF: u32 = 0x5da03c;

#[derive(Clone, Copy)]
pub enum Species {
    Daisy { petals: u32, center: u32 },
    Tulip { petals: u32, inner: u32 },
    Poppy { petals: u32 },
    Lavender,
}

/// Every flower variant used on the islands.
pub const VARIANTS: [Species; 8] = [
    Species::Daisy { petals: 0xfbfbf6, center: 0xf2c230 },
    Species::Daisy { petals: 0xffe066, center: 0xd9822b },
    Species::Tulip { petals: 0xe8343a, inner: 0xa81d2a },
    Species::Tulip { petals: 0xffd23f, inner: 0xe0a01c },
    Species::Tulip { petals: 0xff8fb1, inner: 0xd9587f },
    Species::Poppy { petals: 0xff6a2b },
    Species::Poppy { petals: 0xd8b8ff },
    Species::Lavender,
];

/// Snap a height to a voxel centre so 1-voxel-thin parts (petals) never vanish.
fn snap(y: f32, s: f32) -> f32 {
    ((y / s).floor() + 0.5) * s
}

fn stem(vol: &mut MicroVoxelVolume, h: f32, lean: f32) -> Vec3 {
    let s = vol.scale;
    let curve = move |t: f32| Vec3::new((t * 1.4).sin() * lean + s * 0.5, t * h, s * 0.5);
    let r = s * 0.8;
    vol.sweep(curve, 24, r, MAT_FOLIAGE, STEM);
    curve(1.0)
}

fn leaf(vol: &mut MicroVoxelVolume, base_y: f32, len: f32, yaw: f32, tilt: f32) {
    let s = vol.scale;
    let c = Vec3::new(0.0, base_y, 0.0);
    let (lo, hi) = (c - Vec3::splat(len * 1.2), c + Vec3::splat(len * 1.2));
    vol.fill_sdf(lo, hi, |p| {
        // Blade lying along +X from the stem, rotated up by `tilt`, then yawed.
        let q = rot_z(rot_y(p - c, -yaw), -tilt) - Vec3::new(len * 0.5, 0.0, 0.0);
        ellipsoid(q, Vec3::new(len * 0.5, s * 0.6, len * 0.16))
    }, |p, _| Some((MAT_FOLIAGE, if (p.y - base_y) > len * 0.15 { LEAF } else { STEM_DK })));
}

/// Volume big enough for any plant (about 1 m wide, 1 m tall) at voxel size `s`.
fn plant_volume(s: f32) -> MicroVoxelVolume {
    let h = (0.5 / s).ceil() as i32;
    MicroVoxelVolume::new(h * 2, (1.05 / s).ceil() as i32, h * 2, s, -h, 0, -h)
}

pub fn build_flower(species: Species, seed: u32, s: f32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let mut vol = plant_volume(s);
    match species {
        Species::Daisy { petals, center } => {
            let h = snap(0.42 + rng.next_f32() * 0.12, s);
            let top = stem(&mut vol, h, 0.03);
            leaf(&mut vol, 0.06, 0.16, 0.3, 0.5);
            leaf(&mut vol, 0.1, 0.14, 3.6, 0.6);
            let c = Vec3::new(top.x, h, top.z);
            let (lo, hi) = (c - Vec3::new(0.2, 0.06, 0.2), c + Vec3::new(0.2, 0.06, 0.2));
            // Ring of 14 petals by polar repetition, each a flat ellipsoid.
            vol.fill_sdf_solid(lo, hi, |p| {
                let q = repeat_polar(p - c, 14);
                ellipsoid(q - Vec3::new(0.1, 0.0, 0.0), Vec3::new(0.065, s * 0.6, 0.022))
            }, MAT_FOLIAGE, petals);
            vol.fill_sdf_solid(lo, hi, |p| ellipsoid(p - c - Vec3::Y * s * 0.5, Vec3::new(0.045, 0.03, 0.045)), MAT_FOLIAGE, center);
        }
        Species::Tulip { petals, inner } => {
            let h = snap(0.5 + rng.next_f32() * 0.12, s);
            let top = stem(&mut vol, h, 0.02);
            leaf(&mut vol, 0.02, 0.3, 0.8, 1.05);
            leaf(&mut vol, 0.02, 0.26, 4.0, 1.15);
            let c = Vec3::new(top.x, h + 0.08, top.z);
            let (lo, hi) = (c - Vec3::splat(0.15), c + Vec3::splat(0.17));
            // Cup: an egg-shaped shell, opened at the top along a 6-lobed rim
            // (petal tips = rim height following cos(6θ)).
            vol.fill_sdf(lo, hi, |p| {
                let q = p - c;
                let cup = shell(ellipsoid(q, Vec3::new(0.085, 0.12, 0.085)), s * 0.7);
                let a = q.z.atan2(q.x);
                let rim = 0.05 + 0.045 * (a * 3.0).cos().abs();
                intersect(cup, q.y - rim)
            }, |p, _| {
                let r = radial(p - c);
                Some((MAT_FOLIAGE, if r < 0.075 { inner } else { petals }))
            });
        }
        Species::Poppy { petals } => {
            let h = snap(0.45 + rng.next_f32() * 0.15, s);
            let top = stem(&mut vol, h, 0.05);
            leaf(&mut vol, 0.05, 0.15, 1.2, 0.4);
            let c = Vec3::new(top.x, h, top.z);
            let (lo, hi) = (c - Vec3::new(0.2, 0.05, 0.2), c + Vec3::new(0.2, 0.15, 0.2));
            // Five broad petals tilted up into a shallow bowl.
            vol.fill_sdf_solid(lo, hi, |p| {
                let q = repeat_polar(p - c, 5);
                let q = rot_z(q - Vec3::new(0.02, 0.0, 0.0), -0.5);
                ellipsoid(q - Vec3::new(0.08, 0.0, 0.0), Vec3::new(0.08, s * 0.7, 0.07))
            }, MAT_FOLIAGE, petals);
            vol.fill_sdf_solid(lo, hi, |p| sphere(p - c - Vec3::Y * 0.03, 0.03), MAT_FOLIAGE, 0x2b2230);
            vol.fill_sdf_solid(lo, hi, |p| torus(p - c - Vec3::Y * 0.03, 0.04, 0.012), MAT_FOLIAGE, 0xf2d23c);
        }
        Species::Lavender => {
            // Three stalks fanning out, each lined with buds on its top 45%.
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 + rng.next_f32();
                let h = 0.5 + rng.next_f32() * 0.15;
                let lean = Vec3::new(a.cos(), 0.0, a.sin()) * 0.08;
                let base = Vec3::new(s * 0.5, 0.0, s * 0.5);
                let curve = move |t: f32| base + lean * (t * t) + Vec3::Y * (t * h);
                vol.sweep(curve, 20, s * 0.8, MAT_FOLIAGE, STEM);
                for i in 0..9 {
                    let t = 0.55 + i as f32 * 0.05;
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let bud = curve(t) + Vec3::new(-a.sin(), 0.0, a.cos()) * (0.018 * side);
                    let r = 0.03 * (1.15 - (t - 0.55));
                    let col = if i % 3 == 0 { 0x8e6bd6 } else if i % 3 == 1 { 0xa784e8 } else { 0x7a58c4 };
                    vol.fill_sdf_solid(bud - Vec3::splat(0.06), bud + Vec3::splat(0.06), |p| sphere(p - bud, r), MAT_FOLIAGE, col);
                }
            }
            leaf(&mut vol, 0.02, 0.18, 0.5, 0.9);
            leaf(&mut vol, 0.02, 0.16, 2.6, 0.9);
            leaf(&mut vol, 0.02, 0.17, 4.6, 0.9);
        }
    }
    vol.compile(colors)
}

/// A clump of curved grass blades (flat, tapering ellipsoids leaning outward),
/// coloured dark at the root to light at the tip from an island's grass palette.
pub fn build_grass_tuft(grass: [u32; 3], seed: u32, s: f32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let mut vol = plant_volume(s);
    let blades = 7;
    for i in 0..blades {
        let a = i as f32 * TAU / blades as f32 + rng.next_f32() * 0.5;
        let len = 0.26 + rng.next_f32() * 0.2;
        let lean = 0.3 + rng.next_f32() * 0.4; // radians from vertical
        let (lo, hi) = (Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 0.6, 0.5));
        vol.fill_sdf(lo, hi, |p| {
            // Blade frame: rotate so the blade leans along +X, then an
            // ellipsoid that narrows toward its tip.
            let q = rot_z(rot_y(p, -a), lean);
            let t = (q.y / len).clamp(0.0, 1.0);
            let half_w = 0.035 * (1.0 - t * 0.8);
            ellipsoid(q - Vec3::new(0.0, len * 0.5, 0.0), Vec3::new(s * 0.6, len * 0.5, half_w.max(s * 0.6)))
        }, |p, _| {
            let k = if p.y < 0.08 { 1 } else if p.y < 0.2 { 0 } else { 2 };
            Some((MAT_FOLIAGE, grass[k]))
        });
    }
    vol.compile(colors)
}

/// Horizontal distance from a flower's axis.
fn radial(v: Vec3) -> f32 {
    (v.x * v.x + v.z * v.z).sqrt()
}
