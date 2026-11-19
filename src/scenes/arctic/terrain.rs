//! Arctic terrain: a snowy height-field valley ringed by ridged mountains,
//! with an organic frozen lake carved into it. The lake outline is a
//! domain-warped ellipse (fbm warp), the basin falls off smoothly from the
//! shore, and the ice is a single translucent layer flush with the shore line.
//! Building pads flatten the ground locally with smoothstep blends.

use glam::Vec2;

use super::pal;
use crate::engine::prelude::*;

pub const WORLD_HALF: f32 = 150.0;
pub const TERRAIN_SCALE: f32 = 1.0;
const ICE_SCALE: f32 = 0.5;
/// World Y of the lake surface (ice top / water line).
pub const ICE_LEVEL: f32 = 0.0;
const BOTTOM: f32 = -12.0;
/// Height of the snow bank right at the shoreline (one voxel above the ice).
const SHORE: f32 = ICE_LEVEL + 0.5;

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A flattened building site.
#[derive(Clone, Copy)]
pub struct Pad {
    pub c: Vec2,
    pub r: f32,
    pub h: f32,
}

#[derive(Clone)]
pub struct Terrain {
    /// Noise-domain offsets (seed dependent).
    pub off: Vec2,
    pub lake_c: Vec2,
    pub lake_r: Vec2,
    pub lake_rot: f32,
    pub lake_warp: f32,
    pub lake_depth: f32,
    pub season: Season,
    pub pads: Vec<Pad>,
    /// Packed-snow footpaths (segments in world XZ).
    pub paths: Vec<(Vec2, Vec2)>,
}

impl Terrain {
    pub fn new(seed: u32, season: Season) -> Self {
        let mut t = Terrain {
            off: Vec2::new(5.0, -7.0),
            lake_c: Vec2::new(-4.0, -18.0),
            lake_r: Vec2::new(56.0, 38.0),
            lake_rot: 0.12,
            lake_warp: 13.0,
            lake_depth: 4.5,
            season,
            pads: Vec::new(),
            paths: Vec::new(),
        };
        if seed != 0 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B);
            let mut v = |r: f32| (rng.next_f32() - 0.5) * 2.0 * r;
            t.off = Vec2::new(v(60.0), v(60.0));
            t.lake_c = Vec2::new(v(14.0), -18.0 + v(10.0));
            t.lake_r = Vec2::new(56.0 + v(10.0), 38.0 + v(7.0));
            t.lake_rot = v(0.6);
            t.lake_warp = 13.0 + v(5.0);
            t.lake_depth = 4.5 + v(1.5);
        }
        t
    }

    /// Approximate signed distance (metres) to the lake shore, negative inside.
    pub fn lake_d(&self, x: f32, z: f32) -> f32 {
        let w = self.lake_warp;
        let wx = (fbm2(x * 0.021 + self.off.x * 0.3 + 11.0, z * 0.021, 3) - 0.5) * 2.0 * w;
        let wz = (fbm2(x * 0.021 - 7.0, z * 0.021 + self.off.y * 0.3 + 3.0, 3) - 0.5) * 2.0 * w;
        let p = Vec2::new(x + wx, z + wz) - self.lake_c;
        let (s, c) = self.lake_rot.sin_cos();
        let q = Vec2::new(c * p.x + s * p.y, -s * p.x + c * p.y);
        let e = (q / self.lake_r).length();
        if e < 1e-4 {
            return -self.lake_r.min_element();
        }
        (e - 1.0) * q.length() / e
    }

    /// Ground height before building pads are applied.
    pub fn raw_height(&self, x: f32, z: f32) -> f32 {
        let o = self.off;
        let roll = (fbm2(x * 0.011 + o.x, z * 0.011 + o.y, 4) - 0.5) * 13.0 + 3.0;
        let fine = (fbm2(x * 0.06 + 3.1, z * 0.06 - 7.7, 2) - 0.5) * 1.4;
        // Mountains ring the valley (super-ellipse distance), tallest to the north.
        let rb = (x.powi(4) + z.powi(4)).powf(0.25);
        let rim = smoothstep(82.0, 148.0, rb);
        let north = 0.28 + 0.72 * smoothstep(60.0, -120.0, z);
        let peaks = 6.0 + ridged2(x * 0.017 + o.x, z * 0.017 + o.y, 4) * 56.0;
        let land = roll + fine + rim * north * peaks;

        let d = self.lake_d(x, z);
        if d < 0.0 {
            // Keep the bed at least one voxel below the ice so they never share a face.
            let bed = ICE_LEVEL - 1.3 - self.lake_depth * smoothstep(0.0, 18.0, -d);
            bed + (fbm2(x * 0.08 + 2.0, z * 0.08 - 1.0, 2) - 0.5) * 0.5
        } else {
            let t = smoothstep(0.0, 22.0, d);
            SHORE + (land.max(SHORE) - SHORE) * t
        }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let mut h = self.raw_height(x, z);
        let p = Vec2::new(x, z);
        for pad in &self.pads {
            let d = (p - pad.c).length();
            let w = 1.0 - smoothstep(pad.r, pad.r + 9.0, d);
            h += (pad.h - h) * w;
        }
        h
    }

    /// Top of the terrain voxel column at (x, z).
    pub fn ground_top(&self, x: f32, z: f32) -> f32 {
        let s = TERRAIN_SCALE;
        let cx = ((x / s).floor() + 0.5) * s;
        let cz = ((z / s).floor() + 0.5) * s;
        ((self.height(cx, cz) / s).floor() + 1.0) * s
    }

    /// Walkable surface: snow top, or the lake surface over the lake.
    pub fn surface_y(&self, x: f32, z: f32) -> f32 {
        self.ground_top(x, z).max(ICE_LEVEL)
    }

    pub fn slope(&self, x: f32, z: f32) -> f32 {
        let e = 1.2;
        let gx = self.height(x + e, z) - self.height(x - e, z);
        let gz = self.height(x, z + e) - self.height(x, z - e);
        Vec2::new(gx, gz).length() / (2.0 * e)
    }

    fn path_d(&self, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z);
        let mut best = f32::MAX;
        for &(a, b) in &self.paths {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            // Gently meandering: offset by low-frequency noise.
            let wob = (noise2(x * 0.15, z * 0.15) - 0.5) * 0.9;
            best = best.min((p - (a + ab * t)).length() + wob);
        }
        best
    }

    /// Bare rock: only on steep mountain slopes (never the lake rim).
    pub fn rocky(&self, x: f32, z: f32, h: f32, slope: f32) -> bool {
        let n = fbm2(x * 0.045 - 13.0, z * 0.045 + 5.0, 3);
        h > 3.0 && slope > 1.05 + (n - 0.5) * 0.5
    }

    /// Is the ground at (x, z) (height h, slope s) snow covered this season?
    pub fn snowy(&self, x: f32, z: f32, h: f32, slope: f32) -> bool {
        let n = fbm2(x * 0.045 - 13.0, z * 0.045 + 5.0, 3);
        let rocky = self.rocky(x, z, h, slope);
        match self.season {
            Season::Winter => !rocky,
            Season::Spring => {
                // Small melt patches near the shore.
                let m = fbm2(x * 0.13 + 4.0, z * 0.13 - 2.0, 2);
                !rocky && !(self.lake_d(x, z) < 26.0 && m > 0.66 - n * 0.06)
            }
            Season::Autumn => !rocky && (h > 18.0 + n * 10.0 || n > 0.66),
            Season::Summer => !rocky && h > 34.0 + n * 12.0 && slope < 0.8,
        }
    }

    /// The opaque terrain mesh (one world-sized height-field).
    pub fn build_mesh(&self, colors: &mut ColorCache) -> (MeshData, u64) {
        let s = TERRAIN_SCALE;
        let n = (WORLD_HALF / s).ceil() as i32;
        let top = 95.0;
        let ny = ((top - BOTTOM) / s).ceil() as i32;
        let mut vol = MicroVoxelVolume::new(n * 2, ny, n * 2, s, -n, (BOTTOM / s).floor() as i32, -n);
        let (lo, hi) = vol.local_bounds();
        // Per-column state shared between the span and paint closures.
        let col = std::cell::Cell::new((0.0f32, 0u32, 0u32));
        let tundra = match self.season {
            Season::Autumn => pal::TUNDRA_AUTUMN,
            Season::Spring => pal::TUNDRA_SPRING,
            _ => pal::TUNDRA_SUMMER,
        };
        vol.fill_columns(
            lo,
            hi,
            |x, z| {
                let h = self.height(x, z);
                let slope = self.slope(x, z);
                let n = fbm2(x * 0.035 + 40.0, z * 0.035 - 9.0, 3);
                let underwater = h < ICE_LEVEL - 0.1 && self.lake_d(x, z) < 1.0;
                let on_path = self.path_d(x, z) < 1.25;
                let snow = self.snowy(x, z, h, slope);
                let rock = self.rocky(x, z, h, slope);
                let surf = if underwater {
                    if n > 0.5 { pal::LAKEBED } else { pal::LAKEBED_DK }
                } else if rock {
                    pal::ROCK[((n * 31.0) as usize) % 3]
                } else if snow {
                    if on_path {
                        pal::SNOW_PATH
                    } else if n > 0.56 {
                        pal::SNOW_B
                    } else {
                        pal::SNOW
                    }
                } else if on_path {
                    pal::DIRT_PATH
                } else {
                    tundra[((n * 2.6) as usize).min(2)]
                };
                // Snow is a thick blanket; tundra / rock only a skin.
                let skin = if snow && !underwater { 3 } else { 1 };
                col.set((h, surf, skin));
                Some((BOTTOM, h))
            },
            |p, depth| {
                let (_, surf, skin) = col.get();
                let c = if depth < skin as f32 {
                    surf
                } else if surf == pal::SNOW || surf == pal::SNOW_B || surf == pal::SNOW_PATH {
                    if depth < 4.5 { pal::SNOW_DEEP } else { strata(p.y) }
                } else {
                    strata(p.y)
                };
                Some((MAT_MATTE, c))
            },
        );
        vol.compile(colors)
    }

    /// The translucent lake surface: one layer of ice (or open water in summer)
    /// sitting exactly on top of the carved basin, flush with the shore.
    pub fn build_lake(&self, colors: &mut ColorCache) -> (MeshData, u64) {
        let s = ICE_SCALE;
        let ext = self.lake_r.max_element() + self.lake_warp * 1.6 + 6.0;
        let (x0, x1) = (self.lake_c.x - ext, self.lake_c.x + ext);
        let (z0, z1) = (self.lake_c.y - ext, self.lake_c.y + ext);
        let ix0 = (x0 / s).floor() as i32;
        let iz0 = (z0 / s).floor() as i32;
        let nx = ((x1 - x0) / s).ceil() as i32 + 2;
        let nz = ((z1 - z0) / s).ceil() as i32 + 2;
        let oy = ((ICE_LEVEL - s) / s).round() as i32;
        let mut vol = MicroVoxelVolume::new(nx, 2, nz, s, ix0, oy, iz0);
        let (lo, hi) = vol.local_bounds();
        let season = self.season;
        vol.fill_columns(
            lo,
            hi,
            |x, z| {
                // Only where the (coarser) terrain column ends below the surface.
                if self.ground_top(x, z) <= ICE_LEVEL - 0.99 && self.lake_d(x, z) < 2.0 {
                    Some((ICE_LEVEL - s * 0.5, ICE_LEVEL - 0.01))
                } else {
                    None
                }
            },
            |p, _| {
                let d = self.lake_d(p.x, p.z);
                let n = fbm2(p.x * 0.05 + 3.0, p.z * 0.05 - 8.0, 3);
                let crack = (fbm2(p.x * 0.09 + 17.0, p.z * 0.09 - 4.0, 3) - 0.5).abs() < 0.012;
                let melt = fbm2(p.x * 0.03 - 5.0, p.z * 0.03 + 2.0, 2);
                let water = match season {
                    Season::Winter => false,
                    Season::Spring => d > -6.0 + melt * 4.0 || melt > 0.64,
                    Season::Autumn => melt > 0.5,
                    Season::Summer => n < 0.66,
                };
                let c = if water {
                    if d < -14.0 { pal::WATER_DK } else { pal::WATER }
                } else if crack {
                    pal::ICE_CRACK
                } else if n > 0.6 {
                    pal::ICE_SNOWY
                } else if n < 0.4 {
                    pal::ICE_DK
                } else {
                    pal::ICE
                };
                Some((MAT_WATER, c))
            },
        );
        vol.compile(colors)
    }
}

/// Earth / rock layers seen on the diorama's cut edges.
fn strata(y: f32) -> u32 {
    let band = ((y + 40.0) * 0.45 + (y * 0.31).sin() * 0.8).floor() as i32;
    match band.rem_euclid(4) {
        0 => pal::EARTH,
        1 => pal::ROCK[2],
        2 => pal::EARTH_DK,
        _ => pal::ROCK[0],
    }
}
