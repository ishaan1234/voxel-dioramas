//! Icebergs as crystalline polyhedra: each chunk is the intersection of
//! half-spaces (p·n ≤ d) — a ring of jittered side planes, slanted roof planes
//! meeting in a ridge or peak (or one flat top for tabular bergs), and a keel
//! plane. Pinnacle bergs union a few such chunks; light fbm breaks the edges.
//! Built with the waterline at y = 0 and a submerged keel below it.

use glam::Vec3;
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

const BERG_SCALE: f32 = 0.28;

struct Chunk {
    c: Vec3,
    planes: Vec<(Vec3, f32)>,
}

impl Chunk {
    fn new(rng: &mut Rng, c: Vec3, r: f32, height: f32, keel: f32, flat: bool) -> Self {
        let mut planes = Vec::new();
        let n_side = 6 + (rng.next_f32() * 3.0) as usize;
        let a0 = rng.next_f32() * TAU;
        let squash = 0.7 + rng.next_f32() * 0.5;
        for k in 0..n_side {
            let a = a0 + k as f32 / n_side as f32 * TAU + (rng.next_f32() - 0.5) * 0.5;
            // Sides lean inward a little going up.
            let n = Vec3::new(a.cos(), 0.12 + rng.next_f32() * 0.18, a.sin() * squash).normalize();
            let rr = r * (0.85 + rng.next_f32() * 0.3) * (a.cos().powi(2) + (a.sin() * squash).powi(2)).sqrt();
            planes.push((n, rr));
        }
        if flat {
            planes.push((Vec3::Y, height));
            // Bevel the rim.
            for k in 0..4 {
                let a = a0 + k as f32 * TAU / 4.0 + 0.4;
                let n = Vec3::new(a.cos(), 1.6, a.sin()).normalize();
                planes.push((n, (height + r * 0.75) * n.y));
            }
        } else {
            let n_roof = 3 + (rng.next_f32() * 2.0) as usize;
            let b0 = rng.next_f32() * TAU;
            for k in 0..n_roof {
                let a = b0 + k as f32 / n_roof as f32 * TAU + (rng.next_f32() - 0.5) * 0.6;
                let tilt = 0.9 + rng.next_f32() * 0.5; // steeper = sharper peak
                let n = Vec3::new(a.cos() * tilt, 1.0, a.sin() * tilt).normalize();
                planes.push((n, height * n.y));
            }
        }
        planes.push((-Vec3::Y, keel));
        Chunk { c, planes }
    }

    fn sd(&self, p: Vec3) -> f32 {
        let q = p - self.c;
        self.planes.iter().fold(f32::MIN, |d, &(n, off)| d.max(q.dot(n) - off))
    }
}

/// Build one iceberg archetype. `tabular` gives a flat-topped slab berg,
/// otherwise a jagged pinnacle berg made of several leaning chunks.
pub fn build_berg(seed: u32, tabular: bool, colors: &mut ColorCache) -> (MeshData, u64) {
    let mut rng = Rng(seed);
    let s = BERG_SCALE;
    let keel = 2.2;
    let mut chunks = Vec::new();
    if tabular {
        let r = 5.0 + rng.next_f32() * 1.5;
        let h = 1.8 + rng.next_f32() * 0.8;
        chunks.push(Chunk::new(&mut rng, Vec3::ZERO, r, h, keel, true));
    } else {
        let main_h = 6.0 + rng.next_f32() * 2.5;
        chunks.push(Chunk::new(&mut rng, Vec3::ZERO, 3.6, main_h, keel, false));
        for _ in 0..2 {
            let a = rng.next_f32() * TAU;
            let d = 2.4 + rng.next_f32() * 1.2;
            let c = Vec3::new(a.cos() * d, 0.0, a.sin() * d);
            let h = main_h * (0.35 + rng.next_f32() * 0.3);
            let r = 2.4 + rng.next_f32() * 0.8;
            let flat = rng.next_f32() < 0.4;
            chunks.push(Chunk::new(&mut rng, c, r, h, keel, flat));
        }
    }
    let sd = seed as f32 * 0.37;
    let shape = |p: Vec3| {
        let d = chunks.iter().fold(f32::MAX, |d, c| d.min(c.sd(p)));
        d + (fbm3(p * 0.35 + Vec3::splat(sd), 2) - 0.5) * 0.6
    };

    let ext = 9.5;
    let n = (ext / s).ceil() as i32;
    let y_lo = ((-keel - 0.6) / s).floor() as i32;
    let y_hi = (10.0 / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(n * 2, y_hi - y_lo, n * 2, s, -n, y_lo, -n);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf(lo, hi, shape, |p, _| {
        // Snow only on near-horizontal tops (SDF gradient points up).
        let e = s * 1.5;
        let g = Vec3::new(
            shape(p + Vec3::X * e) - shape(p - Vec3::X * e),
            shape(p + Vec3::Y * e) - shape(p - Vec3::Y * e),
            shape(p + Vec3::Z * e) - shape(p - Vec3::Z * e),
        )
        .normalize_or_zero();
        if g.y > 0.8 && p.y > 0.3 && shape(p + Vec3::Y * (s * 1.5)) > 0.0 {
            return Some((MAT_MATTE, pal::BERG_TOP));
        }
        let band = (p.y * 0.9 + (p.x * 0.3 + p.z * 0.25).sin() * 0.5).floor() as i32;
        let col = if p.y < -0.3 {
            pal::BERG_LO
        } else if band.rem_euclid(5) == 0 {
            pal::BERG_MID
        } else {
            pal::BERG_HI
        };
        Some((MAT_WATER, col))
    });
    vol.compile(colors)
}
