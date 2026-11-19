//! Procedural trees: recursive branching (each branch is a tapered-capsule SDF),
//! noise-displaced foliage blobs, and cone-stack pines.

use glam::Vec3;
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

pub const TREE_SCALE: f32 = 0.2;

pub struct TreeStyle {
    pub trunk_len: f32,
    pub trunk_r: f32,
    pub depth: u32,
    pub spread: f32,
    pub leaf_r: f32,
    pub bark: u32,
    pub leaves: [u32; 3],
    /// Chance per leaf cluster of a hanging glowing lantern.
    pub lanterns: f32,
}

/// Any unit vector perpendicular to `d`.
fn perpendicular(d: Vec3) -> Vec3 {
    let a = if d.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    d.cross(a).normalize()
}

/// Rotate `v` around unit axis `k` by angle `a` (Rodrigues).
fn rotate_about(v: Vec3, k: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    v * c + k.cross(v) * s + k * k.dot(v) * (1.0 - c)
}

fn grow(
    vol: &mut MicroVoxelVolume,
    rng: &mut Rng,
    style: &TreeStyle,
    p: Vec3,
    dir: Vec3,
    len: f32,
    r: f32,
    depth: u32,
    leaves: &mut Vec<Vec3>,
) {
    let end = p + dir * len;
    let r_end = r * 0.7;
    let pad = Vec3::splat(r + TREE_SCALE * 2.0);
    let bark = style.bark;
    vol.fill_sdf(p.min(end) - pad, p.max(end) + pad, |q| sdf::tapered_capsule(q, p, end, r, r_end), |q, _| {
        // Bark ridges: darker stripes following the branch.
        let stripe = (q.x * 6.0 + q.z * 6.0).sin() > 0.6;
        Some((MAT_WOOD, if stripe { pal::BARK } else { bark }))
    });
    if depth == 0 {
        leaves.push(end);
        return;
    }
    let n = if rng.next_f32() < 0.4 { 3 } else { 2 };
    let phase = rng.next_f32() * TAU;
    let side = perpendicular(dir);
    for i in 0..n {
        let az = phase + i as f32 * TAU / n as f32 + (rng.next_f32() - 0.5) * 0.6;
        let tilt = style.spread * (0.75 + rng.next_f32() * 0.5);
        let axis = rotate_about(side, dir, az);
        let nd = (rotate_about(dir, axis, tilt) + Vec3::Y * 0.18).normalize();
        let nl = len * (0.66 + rng.next_f32() * 0.14);
        grow(vol, rng, style, end, nd, nl, r_end * 0.95, depth - 1, leaves);
    }
}

fn paint_foliage(vol: &mut MicroVoxelVolume, leaves: &[Vec3], style: &TreeStyle, rng: &mut Rng, seed: f32) {
    for &c in leaves {
        let rad = style.leaf_r * (0.8 + rng.next_f32() * 0.4);
        let pad = Vec3::splat(rad * 1.5);
        vol.fill_sdf(
            c - pad,
            c + pad,
            |q| sdf::ellipsoid(q - c, Vec3::new(rad, rad * 0.75, rad)) + (noise3(q * 1.6 + Vec3::splat(seed)) - 0.5) * 0.9,
            |q, _| {
                let h = ((q.y - c.y) / rad + 1.0) * 0.5; // 0 bottom .. 1 top
                let idx = if h > 0.62 { 2 } else if h > 0.3 { 0 } else { 1 };
                Some((MAT_FOLIAGE, style.leaves[idx]))
            },
        );
        if style.lanterns > 0.0 && rng.next_f32() < style.lanterns {
            let top = c - Vec3::Y * (rad * 0.6);
            let bot = top - Vec3::Y * (1.2 + rng.next_f32() * 1.4);
            vol.fill_sdf_solid(bot - Vec3::splat(0.5), top + Vec3::splat(0.2), |q| sdf::capsule(q, top, bot + Vec3::Y * 0.5, 0.13), MAT_WOOD, pal::ROPE);
            vol.fill_sdf_solid(bot - Vec3::splat(0.6), bot + Vec3::splat(0.6), |q| sdf::round_box(q - bot, Vec3::new(0.28, 0.4, 0.28), 0.12), MAT_LAMP, pal::LANTERN);
        }
    }
}

/// Grow one tree archetype in its own volume (origin at the trunk base).
pub fn build_tree(style: &TreeStyle, seed: u32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let reach = style.trunk_len * 2.6 + style.leaf_r * 2.0;
    let half = (reach / TREE_SCALE).ceil() as i32;
    let height = ((style.trunk_len * 3.6 + style.leaf_r * 2.5) / TREE_SCALE).ceil() as i32;
    let roots = (2.0 / TREE_SCALE) as i32;
    let mut vol = MicroVoxelVolume::new(half * 2, height + roots, half * 2, TREE_SCALE, -half, -roots, -half);

    let lean = Vec3::new((rng.next_f32() - 0.5) * 0.15, 1.0, (rng.next_f32() - 0.5) * 0.15).normalize();
    let mut leaves = Vec::new();
    grow(&mut vol, &mut rng, style, Vec3::new(0.0, -1.0, 0.0), lean, style.trunk_len + 1.0, style.trunk_r, style.depth, &mut leaves);

    // Root flare: a few tapered capsules splaying out and down.
    for i in 0..5 {
        let a = i as f32 * TAU / 5.0 + rng.next_f32() * 0.5;
        let p0 = Vec3::new(0.0, 0.6, 0.0);
        let p1 = Vec3::new(a.cos() * style.trunk_r * 3.2, -1.2, a.sin() * style.trunk_r * 3.2);
        let pad = Vec3::splat(style.trunk_r + 0.4);
        vol.fill_sdf_solid(p0.min(p1) - pad, p0.max(p1) + pad, |q| sdf::tapered_capsule(q, p0, p1, style.trunk_r * 0.7, 0.12), MAT_WOOD, style.bark);
    }
    paint_foliage(&mut vol, &leaves, style, &mut rng, seed as f32 * 0.013);
    vol.compile(colors)
}

/// Pine: trunk cylinder + stacked, noise-roughened cones.
pub fn build_pine(seed: u32, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let h = 9.0 + rng.next_f32() * 4.0;
    let base_r = 3.0 + rng.next_f32() * 0.8;
    let half = ((base_r + 1.0) / TREE_SCALE).ceil() as i32;
    let ny = ((h + 2.0) / TREE_SCALE).ceil() as i32 + 10;
    let mut vol = MicroVoxelVolume::new(half * 2, ny, half * 2, TREE_SCALE, -half, -10, -half);
    vol.fill_sdf_solid(Vec3::new(-1.0, -2.0, -1.0), Vec3::new(1.0, h, 1.0), |q| sdf::cylinder(q - Vec3::Y * (h * 0.3), 0.38, h * 0.35 + 1.0), MAT_WOOD, pal::BARK);
    let tiers = 4;
    for t in 0..tiers {
        let f = t as f32 / tiers as f32;
        let y0 = 2.0 + f * (h - 3.5);
        let r = base_r * (1.0 - f * 0.62);
        let ch = (h - y0) * 0.55 + 1.2;
        let c = Vec3::new(0.0, y0, 0.0);
        let s = seed as f32;
        vol.fill_sdf(
            c - Vec3::new(r + 1.0, 0.5, r + 1.0),
            c + Vec3::new(r + 1.0, ch + 0.5, r + 1.0),
            |q| sdf::cone(q - c, r, ch) + (noise3(q * 1.4 + Vec3::splat(s)) - 0.5) * 0.7,
            |q, _| {
                let k = ((q.y - y0) / ch * 2.9) as usize;
                Some((MAT_FOLIAGE, pal::PINE[k.min(2)]))
            },
        );
    }
    vol.compile(colors)
}

pub struct FloraKit {
    pub oaks: Vec<MeshData>,
    pub blossoms: Vec<MeshData>,
    pub golds: Vec<MeshData>,
    pub pines: Vec<MeshData>,
    pub spirit_tree: MeshData,
    pub voxels: u64,
}

/// Every tree archetype is independent, so they are grown in parallel.
pub fn build_flora() -> FloraKit {
    let oak = TreeStyle { trunk_len: 2.6, trunk_r: 0.42, depth: 3, spread: 0.62, leaf_r: 1.7, bark: pal::BARK_LT, leaves: pal::LEAF, lanterns: 0.0 };
    let blossom = TreeStyle { trunk_len: 2.2, trunk_r: 0.36, depth: 3, spread: 0.75, leaf_r: 1.5, bark: pal::BARK, leaves: pal::BLOSSOM, lanterns: 0.0 };
    let gold = TreeStyle { trunk_len: 3.0, trunk_r: 0.34, depth: 3, spread: 0.5, leaf_r: 1.4, bark: pal::BARK_PALE, leaves: pal::LEAF_GOLD, lanterns: 0.0 };
    let spirit = TreeStyle { trunk_len: 6.5, trunk_r: 1.5, depth: 5, spread: 0.58, leaf_r: 2.6, bark: pal::BARK_LT, leaves: pal::SPIRIT_LEAF, lanterns: 0.45 };

    enum Job<'a> {
        Tree(&'a TreeStyle, u32),
        Pine(u32),
    }
    let jobs = [
        Job::Tree(&oak, 11),
        Job::Tree(&oak, 23),
        Job::Tree(&oak, 37),
        Job::Tree(&blossom, 41),
        Job::Tree(&blossom, 53),
        Job::Tree(&gold, 61),
        Job::Tree(&gold, 79),
        Job::Pine(101),
        Job::Pine(131),
        Job::Tree(&spirit, 2027),
    ];
    let built = par_map(&jobs, |j| match j {
        Job::Tree(style, seed) => build_tree(style, *seed, &mut ColorCache::new()),
        Job::Pine(seed) => build_pine(*seed, &mut ColorCache::new()),
    });
    let voxels = built.iter().map(|(_, c)| c).sum();
    let mut it = built.into_iter().map(|(m, _)| m);
    let mut take = |n: usize| (0..n).map(|_| it.next().unwrap()).collect::<Vec<_>>();
    let oaks = take(3);
    let blossoms = take(2);
    let golds = take(2);
    let pines = take(2);
    let spirit_tree = take(1).pop().unwrap();
    FloraKit { oaks, blossoms, golds, pines, spirit_tree, voxels }
}
