//! Floating island generator. An island is defined entirely by functions of
//! (x, z): an outline radius R(θ), a top height-field, and an underside that
//! hangs like an inverted, noise-ridged paraboloid. No voxel is hand-placed.

use glam::Vec3;
use super::pal;
use crate::engine::prelude::*;

pub const ISLAND_SCALE: f32 = 0.4;
/// World Y of the top of the cloud sea (waterfalls fall until here).
pub const CLOUD_TOP_Y: f32 = 10.0;

#[derive(Clone, Copy)]
pub struct Lake {
    /// Direction of the lake from the island centre (radians).
    pub angle: f32,
    /// Distance of the lake centre as a fraction of the island radius.
    pub dist: f32,
    pub rx: f32,
    pub rz: f32,
    pub depth: f32,
    /// Cut a channel to the rim and pour a waterfall off the edge.
    pub waterfall: bool,
}

#[derive(Clone)]
pub struct IslandSpec {
    pub name: &'static str,
    /// World position of the nominal top-surface centre.
    pub center: Vec3,
    pub radius: f32,
    /// How far the rocky underside hangs below the surface.
    pub depth: f32,
    pub seed: f32,
    /// Radius of a flattened plaza at the centre (0 = natural hills).
    pub plaza: f32,
    pub hilliness: f32,
    pub lake: Option<Lake>,
    pub grass: [u32; 3],
    pub crystal: u32,
    /// Winding footpaths from the centre toward these headings (radians).
    pub paths: Vec<f32>,
}

const WATER_Y: f32 = -0.3;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl IslandSpec {
    /// Outline radius in direction of (x, z): low-frequency noise around a circle.
    pub fn edge_radius(&self, x: f32, z: f32) -> f32 {
        let a = z.atan2(x);
        let n = fbm2(a.cos() * 1.4 + self.seed, a.sin() * 1.4 - self.seed * 0.7, 3);
        self.radius * (0.74 + 0.52 * n)
    }

    /// Normalised radial coordinate: 0 at centre, 1 on the rim.
    pub fn e(&self, x: f32, z: f32) -> f32 {
        (x * x + z * z).sqrt() / self.edge_radius(x, z)
    }

    fn lake_center(&self, l: &Lake) -> (f32, f32) {
        (l.angle.cos() * l.dist * self.radius, l.angle.sin() * l.dist * self.radius)
    }

    /// Point on the rim along the lake's direction (where the waterfall pours off).
    pub fn waterfall_lip(&self) -> Option<(Vec3, Vec3)> {
        let l = self.lake?;
        if !l.waterfall {
            return None;
        }
        let dir = Vec3::new(l.angle.cos(), 0.0, l.angle.sin());
        let mut r = l.dist * self.radius;
        while self.e(dir.x * r, dir.z * r) < 1.0 {
            r += 0.2;
        }
        Some((dir * (r - 0.4) + Vec3::Y * WATER_Y, dir))
    }

    /// 0..1 mask of the lake basin + outflow channel (1 = deepest).
    fn water_mask(&self, x: f32, z: f32) -> f32 {
        let Some(l) = self.lake else { return 0.0 };
        let (lx, lz) = self.lake_center(&l);
        let d = (((x - lx) / l.rx).powi(2) + ((z - lz) / l.rz).powi(2)).sqrt();
        let mut m = (1.0 - d * d).max(0.0);
        if l.waterfall {
            // Distance from (x, z) to the ray lake-centre -> rim.
            let dir = (l.angle.cos(), l.angle.sin());
            let t = (x - lx) * dir.0 + (z - lz) * dir.1;
            if t > 0.0 {
                let px = lx + dir.0 * t;
                let pz = lz + dir.1 * t;
                let w = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
                let ch = (1.0 - w / 1.8).max(0.0) * 0.55;
                m = m.max(ch);
            }
        }
        m
    }

    /// Top surface height (local metres, 0 = nominal surface).
    pub fn top_y(&self, x: f32, z: f32) -> f32 {
        let e = self.e(x, z);
        let r = (x * x + z * z).sqrt();
        let hills = (fbm2(x * 0.045 + self.seed * 3.1, z * 0.045 - self.seed, 4) - 0.35) * self.hilliness * 2.0;
        let flat = if self.plaza > 0.0 { smoothstep(self.plaza, self.plaza + 7.0, r) } else { 1.0 };
        let mut y = hills * flat - 1.6 * smoothstep(0.78, 1.0, e);
        let wm = self.water_mask(x, z);
        if wm > 0.0 {
            let lake_depth = self.lake.map(|l| l.depth).unwrap_or(1.0);
            // Basin floor replaces the hills inside the lake / channel mask.
            y = WATER_Y - 0.2 - lake_depth * wm;
        }
        y
    }

    /// Underside height (local metres).
    pub fn bottom_y(&self, x: f32, z: f32) -> f32 {
        let e = self.e(x, z);
        let f = (1.0 - e).max(0.0);
        let ridge = ridged2(x * 0.08 + self.seed * 5.0, z * 0.08, 4);
        let drip = (noise2(x * 0.35 + self.seed, z * 0.35) - 0.5) * 2.0 * f;
        -(1.8 + self.depth * f.powf(0.62) * (0.6 + 0.8 * ridge) + drip)
    }

    /// True on a meandering footpath: distance to a ray, offset by a sine wiggle.
    pub fn on_path(&self, x: f32, z: f32) -> bool {
        self.paths.iter().any(|&a| {
            let (c, s) = (a.cos(), a.sin());
            let t = x * c + z * s;
            if t < self.plaza.max(2.0) - 1.0 {
                return false;
            }
            let perp = -x * s + z * c;
            let u = t / self.edge_radius(x, z);
            let taper = (u * 2.0).min(1.0) * (1.0 - ((u - 0.65) * 3.5).clamp(0.0, 1.0));
            let wiggle = (t * 0.22 + self.seed * 4.0).sin() * 1.6 * taper;
            (perp - wiggle).abs() < 1.15
        })
    }

    pub fn is_water(&self, x: f32, z: f32) -> bool {
        self.water_mask(x, z) > 0.02
    }

    fn rock_color(&self, p: Vec3) -> u32 {
        let band = (p.y * 0.55 + fbm2(p.x * 0.06 + self.seed, p.z * 0.06, 3) * 3.0).floor() as i32;
        pal::ROCK[band.rem_euclid(pal::ROCK.len() as i32) as usize]
    }

    /// Voxelise this island. Returns (opaque volume, water volume).
    pub fn voxelize(&self, rng: &mut Rng) -> (MicroVoxelVolume, MicroVoxelVolume) {
        let s = ISLAND_SCALE;
        let ext = self.radius * 1.32;
        let up = 6.0;
        let down = self.depth * 1.5 + 14.0;
        let (nx, ny) = (((2.0 * ext) / s).ceil() as i32, ((up + down) / s).ceil() as i32);
        let (ox, oy) = (-(ext / s).ceil() as i32, -(down / s).ceil() as i32);
        let mut vol = MicroVoxelVolume::new(nx, ny, nx, s, ox, oy, ox);
        // The water volume must reach all the way down to the cloud sea for the falls.
        let w_down = down.max(self.center.y - CLOUD_TOP_Y + 4.0);
        let w_ny = ((up + w_down) / s).ceil() as i32;
        let mut water = MicroVoxelVolume::new(nx, w_ny, nx, s, ox, -(w_down / s).ceil() as i32, ox);
        let (lo, hi) = vol.local_bounds();

        // 1. Main body: one column per (x, z), painted by depth below the surface.
        vol.fill_columns(
            lo,
            hi,
            |x, z| {
                if self.e(x, z) >= 1.0 {
                    return None;
                }
                Some((self.bottom_y(x, z), self.top_y(x, z)))
            },
            |p, depth| {
                let wet = self.is_water(p.x, p.z);
                if depth < 0.42 {
                    if wet {
                        return Some((MAT_MATTE, pal::LAKEBED));
                    }
                    if self.on_path(p.x, p.z) {
                        let h = hash2(p.x.floor() * 0.7, p.z.floor() * 1.3);
                        return Some((MAT_MATTE, if h < 0.5 { pal::PATH } else { pal::PATH_DK }));
                    }
                    let patch = (fbm2(p.x * 0.11 + self.seed, p.z * 0.11, 3) * 3.0) as usize;
                    return Some((MAT_FOLIAGE, self.grass[patch.min(2)]));
                }
                if depth < 1.7 {
                    return Some((MAT_MATTE, if p.y as i32 % 2 == 0 { pal::DIRT } else { pal::DIRT_DK }));
                }
                // Glowing crystal veins deep in the rock (3D noise threshold).
                if depth > 5.0 && noise3(p * 0.62 + Vec3::splat(self.seed * 9.0)) > 0.83 {
                    return Some((MAT_LAMP, self.crystal));
                }
                if depth < 3.6 && hash3(p.x.floor(), p.y.floor(), p.z.floor()) < 0.35 {
                    return Some((MAT_FOLIAGE, pal::MOSS));
                }
                Some((MAT_MATTE, self.rock_color(p)))
            },
        );

        // 2. Stalactites: tapered capsules hanging from the underside on a golden spiral.
        let n = (self.radius * 0.7) as usize;
        for i in 0..n {
            let (sx, sz) = golden_spiral(i, n, self.radius * 0.8);
            let (sx, sz) = (sx + self.seed.sin() * 2.0, sz);
            if self.e(sx, sz) > 0.85 {
                continue;
            }
            let top = self.bottom_y(sx, sz) + 1.2;
            let f = 1.0 - self.e(sx, sz);
            let len = (2.0 + rng.next_f32() * 6.0) * (0.4 + f);
            let r0 = 0.9 + rng.next_f32() * 1.4 * f;
            let a = Vec3::new(sx, top, sz);
            let b = Vec3::new(sx + (rng.next_f32() - 0.5) * 1.5, top - len, sz + (rng.next_f32() - 0.5) * 1.5);
            let pad = Vec3::splat(r0 + 1.0);
            vol.fill_sdf(a.min(b) - pad, a.max(b) + pad, |p| sdf::tapered_capsule(p, a, b, r0, 0.15), |p, _| {
                Some((MAT_MATTE, self.rock_color(p)))
            });
        }

        // 3. Flowers & grass tufts on top, scattered by a hash (no water, no plaza).
        let (a, b) = (ox, ox + nx - 1);
        for iz in a..=b {
            for ix in a..=b {
                let (x, z) = ((ix as f32 + 0.5) * s, (iz as f32 + 0.5) * s);
                if self.e(x, z) >= 0.95 || self.is_water(x, z) || self.on_path(x, z) || (x * x + z * z).sqrt() < self.plaza + 1.5 {
                    continue;
                }
                let h = hash2(x * 1.7 + self.seed, z * 1.3);
                if h < 0.028 {
                    let iy = (self.top_y(x, z) / s).floor() as i32 + 1;
                    let col = pal::FLOWERS[(h * 1000.0) as usize % pal::FLOWERS.len()];
                    vol.set(ix, iy, iz, MAT_FOLIAGE, col);
                } else if h < 0.09 {
                    let iy = (self.top_y(x, z) / s).floor() as i32 + 1;
                    vol.set(ix, iy, iz, MAT_FOLIAGE, self.grass[2]);
                }
            }
        }

        // 4. Lake water surface + waterfall ribbon (translucent volume).
        if self.lake.is_some() {
            let (wlo, whi) = water.local_bounds();
            water.fill_columns(
                wlo,
                whi,
                |x, z| {
                    if self.e(x, z) >= 0.99 || !self.is_water(x, z) {
                        return None;
                    }
                    let top = self.top_y(x, z);
                    (top < WATER_Y).then_some((top, WATER_Y))
                },
                |_, depth| Some((MAT_WATER, if depth < 0.4 { pal::WATER } else { pal::WATER_DEEP })),
            );
        }
        if let Some((lip, dir)) = self.waterfall_lip() {
            self.pour_waterfall(&mut water, lip, dir);
        }
        (vol, water)
    }

    /// Ballistic waterfall: x(t) = lip + dir·v0·t, y(t) = lip.y − g·t²/2,
    /// swept as a ribbon of capsules until it reaches the cloud sea.
    fn pour_waterfall(&self, water: &mut MicroVoxelVolume, lip: Vec3, dir: Vec3) {
        let (v0, g) = (2.6, 9.8);
        let fall = self.center.y + lip.y - CLOUD_TOP_Y;
        let t_end = (2.0 * fall / g).sqrt();
        let side = Vec3::new(-dir.z, 0.0, dir.x);
        let steps = 90;
        let pos = |t: f32| lip + dir * (v0 * t) - Vec3::Y * (0.5 * g * t * t);
        for i in 0..steps {
            let t0 = t_end * i as f32 / steps as f32;
            let t1 = t_end * (i + 1) as f32 / steps as f32;
            // Ribbon widens slightly as it falls.
            let spread = 1.0 + t0 * 0.35;
            for k in [-1.0f32, -0.33, 0.33, 1.0] {
                let a = pos(t0) + side * (k * spread);
                let b = pos(t1) + side * (k * spread);
                let r = 0.45;
                let pad = Vec3::splat(r + 0.5);
                water.fill_sdf(a.min(b) - pad, a.max(b) + pad, |p| sdf::capsule(p, a, b, r), |p, _| {
                    let streak = noise2(p.x * 1.3 + p.z * 1.3, p.y * 0.25 + self.seed);
                    Some((MAT_WATER, if streak > 0.62 { pal::FOAM } else { pal::FALL }))
                });
            }
        }
    }

    /// Sample candidate spots for props on the island top (world space).
    pub fn scatter(&self, rng: &mut Rng, min_dist: f32, attempts: usize, max_e: f32, keep_out: f32) -> Vec<Vec3> {
        poisson_disc(rng, self.radius * 1.2, min_dist, attempts, |x, z| {
            self.e(x, z) < max_e && !self.is_water(x, z) && !self.on_path(x, z) && (x * x + z * z).sqrt() > keep_out
        })
        .into_iter()
        .map(|(x, z)| self.center + Vec3::new(x, self.top_y(x, z) - 0.15, z))
        .collect()
    }

    /// Anchor for a bridge leaving toward `toward` (world space), on the rim.
    pub fn bridge_anchor(&self, toward: Vec3) -> Vec3 {
        let d = Vec3::new(toward.x - self.center.x, 0.0, toward.z - self.center.z).normalize();
        let mut r = 2.0;
        while self.e(d.x * r, d.z * r) < 0.9 && r < self.radius * 2.0 {
            r += 0.25;
        }
        let (x, z) = (d.x * r, d.z * r);
        self.center + Vec3::new(x, self.top_y(x, z), z)
    }
}

/// Tiny bobbing islets reuse the same generator with small radii.
pub fn islet_spec(seed: f32, radius: f32) -> IslandSpec {
    IslandSpec {
        name: "islet",
        center: Vec3::ZERO,
        radius,
        depth: radius * 1.6,
        seed,
        plaza: 0.0,
        hilliness: 0.6,
        lake: None,
        grass: pal::GRASS_SPRING,
        crystal: pal::CRYSTAL_CYAN,
        paths: Vec::new(),
    }
}
