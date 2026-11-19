//! Rope bridges hanging as catenaries between island rims.
//!
//! Built in a local frame (+X from anchor A toward B, horizontal) and placed in
//! the world with a Y rotation, so the voxel volume stays tight.

use glam::Vec3;

use super::pal;
use crate::engine::prelude::*;
use crate::engine::sdf::*;

pub const BRIDGE_SCALE: f32 = 0.15;
const HALF_W: f32 = 1.15;
const RAIL_H: f32 = 1.15;

/// Normalised catenary dip: 0 at s=0 and s=1, -1 at s=0.5.
/// c(s) = (cosh(k(s-½)) - cosh(k/2)) / (cosh(k/2) - 1)
fn catenary(s: f32, k: f32) -> f32 {
    let ch = (k * 0.5).cosh();
    ((k * (s - 0.5)).cosh() - ch) / (ch - 1.0)
}

pub struct BridgeSpec {
    pub a: Vec3,
    pub b: Vec3,
    pub sag: f32,
}

impl BridgeSpec {
    pub fn length(&self) -> f32 {
        Vec3::new(self.b.x - self.a.x, 0.0, self.b.z - self.a.z).length()
    }

    /// Heading so that local +X points from A toward B.
    pub fn yaw(&self) -> f32 {
        let d = self.b - self.a;
        (-d.z).atan2(d.x)
    }

    /// Deck centreline in the local frame.
    pub fn local_point(&self, s: f32) -> Vec3 {
        let l = self.length();
        let dy = self.b.y - self.a.y;
        Vec3::new(s * l, s * dy + self.sag * catenary(s, 3.2), 0.0)
    }

    /// World-space point on the deck (for camera presets, lights, etc.).
    pub fn world_point(&self, s: f32) -> Vec3 {
        let p = self.local_point(s);
        let yaw = self.yaw();
        self.a + Vec3::new(p.x * yaw.cos(), p.y, -p.x * yaw.sin())
    }

    pub fn transform(&self) -> ModelInstance {
        ModelInstance::from_trs(self.a, self.yaw(), Vec3::ONE)
    }

    pub fn build(&self, colors: &mut ColorCache) -> (MeshData, u64) {
        let s = BRIDGE_SCALE;
        let l = self.length();
        let samples: Vec<Vec3> = (0..=200).map(|i| self.local_point(i as f32 / 200.0)).collect();
        let (ymin, ymax) = samples.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let ox = (-2.0 / s) as i32;
        let oy = ((ymin - 1.5) / s).floor() as i32;
        let nx = ((l + 4.0) / s).ceil() as i32;
        let ny = ((ymax - ymin + RAIL_H + 4.5) / s).ceil() as i32;
        let nz = ((HALF_W * 2.0 + 1.6) / s).ceil() as i32;
        let mut vol = MicroVoxelVolume::new(nx, ny, nz, s, ox, oy, -nz / 2);

        // Arc-length parametrisation so planks are evenly spaced along the curve.
        let mut arc = vec![0.0f32];
        for w in samples.windows(2) {
            arc.push(arc.last().unwrap() + w[0].distance(w[1]));
        }
        let total = *arc.last().unwrap();
        let at_len = |d: f32| -> (Vec3, Vec3) {
            let i = arc.partition_point(|&x| x < d).clamp(1, samples.len() - 1);
            let f = ((d - arc[i - 1]) / (arc[i] - arc[i - 1]).max(1e-6)).clamp(0.0, 1.0);
            let p = samples[i - 1].lerp(samples[i], f);
            (p, (samples[i] - samples[i - 1]).normalize())
        };

        // Planks: oriented boxes, every 0.42 m of arc length.
        let pitch = 0.42;
        let n_planks = (total / pitch) as i32;
        for i in 0..=n_planks {
            let (c, t) = at_len(i as f32 * pitch);
            let ang = t.y.atan2(t.x);
            let wobble = (hash2(i as f32, 3.7) - 0.5) * 0.12;
            let col = if i % 3 == 0 { pal::PLANK_DK } else { pal::PLANK };
            let pad = Vec3::splat(HALF_W + 0.5);
            vol.fill_sdf_solid(c - pad, c + pad, |p| box3(rot_z(p - c, -ang), Vec3::new(0.17, 0.07, HALF_W + wobble)), MAT_WOOD, col);
        }

        // Rails & deck ropes: tubes swept along offset catenaries.
        let lp = |s: f32| self.local_point(s);
        for side in [-1.0f32, 1.0] {
            let z = side * (HALF_W + 0.05);
            vol.sweep(|t| lp(t) + Vec3::new(0.0, RAIL_H, z), 160, 0.13, MAT_WOOD, pal::ROPE);
            vol.sweep(|t| lp(t) + Vec3::new(0.0, -0.02, z), 160, 0.12, MAT_WOOD, pal::ROPE);
            // Suspenders every ~1.7 m.
            let n = (total / 1.7) as i32;
            for i in 1..n {
                let (c, _) = at_len(i as f32 * 1.7);
                let a = c + Vec3::new(0.0, 0.0, z);
                let b = a + Vec3::Y * RAIL_H;
                vol.fill_sdf_solid(a - Vec3::splat(0.4), b + Vec3::splat(0.4), |p| capsule(p, a, b, 0.09), MAT_WOOD, pal::ROPE);
            }
            // End posts with lanterns.
            for end in [0.0f32, 1.0] {
                let base = lp(end) + Vec3::new(if end == 0.0 { 0.4 } else { -0.4 }, 0.0, z);
                let top = base + Vec3::Y * (RAIL_H + 0.7);
                vol.fill_sdf_solid(base - Vec3::new(0.5, 1.6, 0.5), top + Vec3::splat(0.5), |p| box3(p - (base + top) * 0.5 + Vec3::Y * 0.6, Vec3::new(0.18, (RAIL_H + 0.7) * 0.5 + 0.6, 0.18)), MAT_WOOD, pal::POST);
                let lamp = top + Vec3::Y * 0.3;
                vol.fill_sdf_solid(lamp - Vec3::splat(0.4), lamp + Vec3::splat(0.4), |p| round_box(p - lamp, Vec3::new(0.2, 0.26, 0.2), 0.06), MAT_LAMP, pal::LANTERN);
            }
        }
        vol.compile(colors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catenary_endpoints_and_midpoint() {
        assert!(catenary(0.0, 3.2).abs() < 1e-5);
        assert!(catenary(1.0, 3.2).abs() < 1e-5);
        assert!((catenary(0.5, 3.2) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn world_point_hits_anchors() {
        let b = BridgeSpec { a: Vec3::new(10.0, 50.0, -4.0), b: Vec3::new(-30.0, 58.0, 25.0), sag: 4.0 };
        assert!(b.world_point(0.0).distance(b.a) < 1e-3);
        assert!(b.world_point(1.0).distance(b.b) < 1e-3);
        // The transform must map local +X onto the A->B direction.
        let m = glam::Mat4::from_cols_array_2d(&b.transform().model);
        let end = m.transform_point3(b.local_point(1.0));
        assert!(end.distance(b.b) < 1e-3, "{end:?}");
    }
}
