//! Signed-distance-field toolkit + voxelisers.
//!
//! Shapes are plain functions `Fn(Vec3) -> f32` (negative = inside), composed
//! with CSG operators, then rasterised into a [`MicroVoxelVolume`]. All
//! coordinates passed to SDFs are in *local metres* (voxel index * scale), so a
//! shape keeps its size if the voxel resolution changes.

use glam::{Vec2, Vec3};

use super::voxel::MicroVoxelVolume;

// ----------------------------- primitives ---------------------------------

#[inline]
pub fn sphere(p: Vec3, r: f32) -> f32 {
    p.length() - r
}

#[inline]
pub fn ellipsoid(p: Vec3, r: Vec3) -> f32 {
    // Bound-correct approximation (iq).
    let k0 = (p / r).length();
    let k1 = (p / (r * r)).length();
    if k1 < 1e-6 { -r.min_element() } else { k0 * (k0 - 1.0) / k1 }
}

#[inline]
pub fn box3(p: Vec3, half: Vec3) -> f32 {
    let q = p.abs() - half;
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
}

#[inline]
pub fn round_box(p: Vec3, half: Vec3, r: f32) -> f32 {
    box3(p, half - Vec3::splat(r)) - r
}

#[inline]
pub fn capsule(p: Vec3, a: Vec3, b: Vec3, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba).max(1e-9)).clamp(0.0, 1.0);
    (pa - ba * h).length() - r
}

/// Capsule whose radius tapers linearly from `ra` (at a) to `rb` (at b).
#[inline]
pub fn tapered_capsule(p: Vec3, a: Vec3, b: Vec3, ra: f32, rb: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba).max(1e-9)).clamp(0.0, 1.0);
    (pa - ba * h).length() - (ra + (rb - ra) * h)
}

/// Torus lying in the XZ plane: major radius `big`, tube radius `small`.
#[inline]
pub fn torus(p: Vec3, big: f32, small: f32) -> f32 {
    Vec2::new(Vec2::new(p.x, p.z).length() - big, p.y).length() - small
}

/// Vertical capped cylinder centred at origin with half-height `hh`.
#[inline]
pub fn cylinder(p: Vec3, r: f32, hh: f32) -> f32 {
    let d = Vec2::new(Vec2::new(p.x, p.z).length() - r, p.y.abs() - hh);
    d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
}

/// Vertical cone, apex at y = h, base radius `r` at y = 0.
#[inline]
pub fn cone(p: Vec3, r: f32, h: f32) -> f32 {
    let q = Vec2::new(Vec2::new(p.x, p.z).length(), p.y);
    let tip = Vec2::new(0.0, h);
    let base = Vec2::new(r, 0.0);
    // Distance to the slanted segment, signed by inside test.
    let e = base - tip;
    let w = q - tip;
    let t = (w.dot(e) / e.dot(e)).clamp(0.0, 1.0);
    let d_side = (w - e * t).length();
    let d_base = if q.x <= r { (q.y).abs() } else { (q - base).length() };
    let inside = q.y >= 0.0 && q.y <= h && q.x <= r * (1.0 - q.y / h);
    let d = d_side.min(d_base);
    if inside { -d } else { d }
}

// ------------------------------ operators ---------------------------------

#[inline]
pub fn union(a: f32, b: f32) -> f32 {
    a.min(b)
}
#[inline]
pub fn subtract(a: f32, b: f32) -> f32 {
    a.max(-b)
}
#[inline]
pub fn intersect(a: f32, b: f32) -> f32 {
    a.max(b)
}
/// Smooth union with blend radius `k`.
#[inline]
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}
/// Smooth subtraction of `b` from `a`.
#[inline]
pub fn smax(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, -b, k)
}
/// Hollow out a solid, leaving a shell of thickness `t`.
#[inline]
pub fn shell(d: f32, t: f32) -> f32 {
    d.abs() - t
}

// --------------------------- domain transforms ----------------------------

#[inline]
pub fn rot_x(p: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}
#[inline]
pub fn rot_y(p: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(c * p.x + s * p.z, p.y, -s * p.x + c * p.z)
}
#[inline]
pub fn rot_z(p: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z)
}

/// Polar repetition around the Y axis: folds space into one of `n` wedges,
/// returning the point expressed in the wedge's local frame (wedge axis = +X).
#[inline]
pub fn repeat_polar(p: Vec3, n: u32) -> Vec3 {
    let sector = std::f32::consts::TAU / n as f32;
    let a = p.z.atan2(p.x);
    let a = (a + sector * 0.5).rem_euclid(sector) - sector * 0.5;
    let r = Vec2::new(p.x, p.z).length();
    Vec3::new(r * a.cos(), p.y, r * a.sin())
}

/// Twist around Y by `k` radians per metre.
#[inline]
pub fn twist_y(p: Vec3, k: f32) -> Vec3 {
    rot_y(p, k * p.y)
}

// ------------------------------- voxelisers -------------------------------

/// What a paint callback decides for one voxel inside the shape.
pub type Paint = Option<(u8, u32)>;

impl MicroVoxelVolume {
    #[inline]
    fn local_center(&self, x: i32, y: i32, z: i32) -> Vec3 {
        Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) * self.scale
    }

    fn bounds_to_voxels(&self, lo: Vec3, hi: Vec3) -> ([i32; 3], [i32; 3]) {
        let s = self.scale;
        let a = [
            ((lo.x / s).floor() as i32).max(self.ox),
            ((lo.y / s).floor() as i32).max(self.oy),
            ((lo.z / s).floor() as i32).max(self.oz),
        ];
        let b = [
            ((hi.x / s).ceil() as i32).min(self.ox + self.nx - 1),
            ((hi.y / s).ceil() as i32).min(self.oy + self.ny - 1),
            ((hi.z / s).ceil() as i32).min(self.oz + self.nz - 1),
        ];
        (a, b)
    }

    /// Rasterise `sdf` inside the local-metre box [lo, hi]. For each voxel whose
    /// centre is inside (d <= 0), `paint(p, d)` chooses material + colour, or
    /// returns `None` to leave the voxel untouched.
    pub fn fill_sdf<D, P>(&mut self, lo: Vec3, hi: Vec3, sdf: D, mut paint: P)
    where
        D: Fn(Vec3) -> f32,
        P: FnMut(Vec3, f32) -> Paint,
    {
        let (a, b) = self.bounds_to_voxels(lo, hi);
        for y in a[1]..=b[1] {
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    let p = self.local_center(x, y, z);
                    let d = sdf(p);
                    if d <= 0.0 {
                        if let Some((m, c)) = paint(p, d) {
                            self.set(x, y, z, m, c);
                        }
                    }
                }
            }
        }
    }

    /// Single-material convenience wrapper around [`fill_sdf`].
    pub fn fill_sdf_solid<D: Fn(Vec3) -> f32>(&mut self, lo: Vec3, hi: Vec3, sdf: D, mat: u8, col: u32) {
        self.fill_sdf(lo, hi, sdf, |_, _| Some((mat, col)));
    }

    /// Clear every voxel inside `sdf` (CSG subtraction applied to the volume).
    pub fn carve_sdf<D: Fn(Vec3) -> f32>(&mut self, lo: Vec3, hi: Vec3, sdf: D) {
        let (a, b) = self.bounds_to_voxels(lo, hi);
        for y in a[1]..=b[1] {
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    if sdf(self.local_center(x, y, z)) <= 0.0 {
                        self.clear(x, y, z);
                    }
                }
            }
        }
    }

    /// Column voxeliser for height-field-like solids: for every (x, z) column
    /// `span(x, z)` returns the solid's [bottom, top] in local metres (or None),
    /// then `paint(p, depth_below_top)` colours each voxel. Much faster than a
    /// full 3D SDF for terrain / islands.
    pub fn fill_columns<S, P>(&mut self, lo: Vec3, hi: Vec3, mut span: S, mut paint: P)
    where
        S: FnMut(f32, f32) -> Option<(f32, f32)>,
        P: FnMut(Vec3, f32) -> Paint,
    {
        let (a, b) = self.bounds_to_voxels(lo, hi);
        let s = self.scale;
        for z in a[2]..=b[2] {
            for x in a[0]..=b[0] {
                let cx = (x as f32 + 0.5) * s;
                let cz = (z as f32 + 0.5) * s;
                let Some((y_lo, y_hi)) = span(cx, cz) else { continue };
                let iy0 = ((y_lo / s).floor() as i32).max(a[1]);
                let iy1 = ((y_hi / s).floor() as i32).min(b[1]);
                for y in iy0..=iy1 {
                    let p = Vec3::new(cx, (y as f32 + 0.5) * s, cz);
                    if let Some((m, c)) = paint(p, y_hi - p.y) {
                        self.set(x, y, z, m, c);
                    }
                }
            }
        }
    }

    /// Sweep a tube of radius `r` along a parametric curve `f(t)`, t in [0, 1].
    pub fn sweep<F: Fn(f32) -> Vec3>(&mut self, f: F, segments: usize, r: f32, mat: u8, col: u32) {
        let pad = Vec3::splat(r + self.scale);
        for i in 0..segments {
            let a = f(i as f32 / segments as f32);
            let b = f((i + 1) as f32 / segments as f32);
            self.fill_sdf_solid(a.min(b) - pad, a.max(b) + pad, |p| capsule(p, a, b, r), mat, col);
        }
    }

    /// Local-metre bounds of the whole volume.
    pub fn local_bounds(&self) -> (Vec3, Vec3) {
        let s = self.scale;
        (
            Vec3::new(self.ox as f32, self.oy as f32, self.oz as f32) * s,
            Vec3::new((self.ox + self.nx) as f32, (self.oy + self.ny) as f32, (self.oz + self.nz) as f32) * s,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::voxel::MAT_MATTE;

    #[test]
    fn primitives_have_correct_sign() {
        assert!(sphere(Vec3::ZERO, 1.0) < 0.0);
        assert!(sphere(Vec3::new(2.0, 0.0, 0.0), 1.0) > 0.0);
        assert!((box3(Vec3::new(2.0, 0.0, 0.0), Vec3::ONE) - 1.0).abs() < 1e-5);
        assert!(torus(Vec3::new(3.0, 0.0, 0.0), 3.0, 0.5) < 0.0);
        assert!(cylinder(Vec3::new(0.0, 0.9, 0.0), 1.0, 1.0) < 0.0);
        assert!(cone(Vec3::new(0.0, 0.5, 0.0), 1.0, 2.0) < 0.0);
        assert!(cone(Vec3::new(0.0, 2.5, 0.0), 1.0, 2.0) > 0.0);
        assert!(capsule(Vec3::new(0.5, 0.0, 0.0), Vec3::ZERO, Vec3::X, 0.1) < 0.0);
    }

    #[test]
    fn sphere_voxel_count_matches_volume() {
        let mut v = MicroVoxelVolume::new(64, 64, 64, 0.25, -32, -32, -32);
        let r = 6.0;
        v.fill_sdf_solid(Vec3::splat(-7.0), Vec3::splat(7.0), |p| sphere(p, r), MAT_MATTE, 0xffffff);
        let expected = 4.0 / 3.0 * std::f32::consts::PI * r * r * r / (0.25f32).powi(3);
        let got = v.placed_count as f32;
        assert!((got - expected).abs() / expected < 0.03, "got {got}, expected {expected}");
        v.carve_sdf(Vec3::splat(-7.0), Vec3::splat(7.0), |p| sphere(p, r));
        assert_eq!(v.placed_count, 0);
    }
}
