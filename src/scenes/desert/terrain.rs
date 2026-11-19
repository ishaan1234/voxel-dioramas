//! Canyon terrain: one height-field over a round diorama plinth.
//!
//! * Mesas: a domain-warped fbm mask, boosted toward the back rim, then
//!   terraced through three smoothsteps (stepped buttes with sheer cliffs).
//! * Floor: low fbm undulation + asymmetric sine dunes (gentle windward,
//!   steep lee) faded out around every feature.
//! * The riverbed and oasis basin are carved by distance to the plan curves.
//! * Colour: tilted sandstone strata bands with desert-varnish streaks on
//!   cliffs, caprock on plateaus, rippled sand, cracked mud in the riverbed.

use glam::Vec2;

use super::arch::Arch;
use super::pal;
use super::plan::{smoothstep, Plan, WORLD_R};
use crate::engine::prelude::*;

/// Terrain voxel size (metres).
pub const VS: f32 = 0.5;
/// Bottom of the diorama plinth.
pub const BASE_Y: f32 = -12.0;
/// Oasis water level.
pub const WATER_Y: f32 = -0.45;
/// Columns per side of the terrain grid.
pub const N: i32 = (2.0 * (WORLD_R * 1.12 + 2.0) / VS) as i32;
const G0: i32 = -N / 2;
/// Chunks per side (each voxelised + meshed in parallel).
const CHUNKS: i32 = 4;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Kind {
    #[default]
    Out,
    Sand,
    River,
    Lakebed,
    Shore,
    Grass,
    Mesa,
}

#[derive(Clone, Copy, Default)]
pub struct Column {
    pub kind: Kind,
    pub top: f32,
    /// Floor height before dunes (the sand layer sits on top of it).
    pub base: f32,
    pub mesa: f32,
    pub talus: f32,
    pub dune: f32,
    /// Dune phase coordinate (ripples) and lee-side flag.
    pub dune_u: f32,
    pub lee: bool,
    pub strata_off: f32,
}

pub struct Terrain {
    pub plan: Plan,
    pub cols: Vec<Column>,
}

/// Top of the voxelised surface for an analytic height `h`.
#[inline]
pub fn voxel_top(h: f32) -> f32 {
    ((h / VS - 0.5).floor() + 1.0) * VS
}

impl Terrain {
    pub fn new(plan: Plan) -> Self {
        let bands: Vec<i32> = (0..16).collect();
        let rows = (N + 15) / 16;
        let parts = par_map(&bands, |&b| {
            let mut v = Vec::new();
            for iz in b * rows..((b + 1) * rows).min(N) {
                for ix in 0..N {
                    let (x, z) = (((ix + G0) as f32 + 0.5) * VS, ((iz + G0) as f32 + 0.5) * VS);
                    v.push(sample(&plan, x, z));
                }
            }
            v
        });
        let cols = parts.into_iter().flatten().collect();
        Self { plan, cols }
    }

    #[inline]
    pub fn col_at(&self, ix: i32, iz: i32) -> Column {
        let (i, k) = (ix - G0, iz - G0);
        if i < 0 || k < 0 || i >= N || k >= N {
            return Column::default();
        }
        self.cols[(k * N + i) as usize]
    }

    /// Column under world (x, z).
    pub fn column(&self, x: f32, z: f32) -> Column {
        self.col_at((x / VS).floor() as i32, (z / VS).floor() as i32)
    }

    /// Height of the voxel surface under (x, z), None off the plinth.
    pub fn surface(&self, x: f32, z: f32) -> Option<f32> {
        let c = self.column(x, z);
        (c.kind != Kind::Out).then(|| voxel_top(c.top))
    }

    /// Lowest voxel-surface height over a disc (for seating props without floating).
    pub fn seat(&self, x: f32, z: f32, r: f32) -> f32 {
        let mut lo = f32::MAX;
        for k in 0..9 {
            let (dx, dz) = if k == 0 {
                (0.0, 0.0)
            } else {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                (a.cos() * r, a.sin() * r)
            };
            lo = lo.min(self.surface(x + dx, z + dz).unwrap_or(BASE_Y));
        }
        lo
    }

    /// Max height difference across a disc (rough slope test for props).
    pub fn roughness(&self, x: f32, z: f32, r: f32) -> f32 {
        let c = self.surface(x, z).unwrap_or(BASE_Y);
        [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
            .iter()
            .map(|(dx, dz)| (self.surface(x + dx, z + dz).unwrap_or(BASE_Y) - c).abs())
            .fold(0.0, f32::max)
    }

    /// Voxelise + mesh the whole terrain (plus the arch, which is carved from
    /// the same rock in the same grid) as CHUNKS x CHUNKS parallel meshes.
    pub fn build_meshes(&self, arch: &Arch) -> Vec<(MeshData, u64)> {
        let ids: Vec<(i32, i32)> = (0..CHUNKS * CHUNKS).map(|k| (k % CHUNKS, k / CHUNKS)).collect();
        par_map(&ids, |&(ci, cj)| {
            let vol = self.voxelize_chunk(ci, cj, arch);
            vol.compile(&mut ColorCache::new())
        })
    }

    fn voxelize_chunk(&self, ci: i32, cj: i32, arch: &Arch) -> MicroVoxelVolume {
        let span = (N + CHUNKS - 1) / CHUNKS;
        let (x0, x1) = (G0 + ci * span, (G0 + (ci + 1) * span).min(G0 + N));
        let (z0, z1) = (G0 + cj * span, (G0 + (cj + 1) * span).min(G0 + N));
        let mut ytop = BASE_Y;
        for iz in z0..z1 {
            for ix in x0..x1 {
                ytop = ytop.max(self.col_at(ix, iz).top);
            }
        }
        // Leave room for the arch if it reaches into this chunk.
        let (alo, ahi) = arch.bounds();
        let (lo_w, hi_w) = (Vec2::new(x0 as f32, z0 as f32) * VS, Vec2::new(x1 as f32, z1 as f32) * VS);
        if alo.x < hi_w.x && ahi.x > lo_w.x && alo.z < hi_w.y && ahi.z > lo_w.y {
            ytop = ytop.max(ahi.y);
        }
        let oy = (BASE_Y / VS).floor() as i32;
        let ny = ((ytop / VS).ceil() as i32 + 2 - oy).max(1);
        let mut vol = MicroVoxelVolume::new(x1 - x0, ny, z1 - z0, VS, x0, oy, z0);
        let winter = self.plan.winter;
        let interior = pal::STRATA[0].1;
        for iz in z0..z1 {
            for ix in x0..x1 {
                let c = self.col_at(ix, iz);
                if c.kind == Kind::Out {
                    continue;
                }
                // Voxels deeper than every neighbour's surface can never be seen.
                let mut expose = c.top;
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = self.col_at(ix + dx, iz + dz);
                    expose = expose.min(if n.kind == Kind::Out { BASE_Y - 1.0 } else { n.top });
                }
                let expose = expose - VS * 1.5;
                let (x, z) = ((ix as f32 + 0.5) * VS, (iz as f32 + 0.5) * VS);
                let iy0 = (BASE_Y / VS).floor() as i32;
                let iy1 = (c.top / VS - 0.5).floor() as i32;
                for iy in iy0..=iy1 {
                    let y = (iy as f32 + 0.5) * VS;
                    let (m, col) = if y < expose && iy > iy0 { (MAT_MATTE, interior) } else { paint(&c, x, y, z, winter) };
                    vol.set(ix, iy, iz, m, col);
                }
            }
        }
        arch.fill(&mut vol, winter);
        vol
    }

    /// Translucent oasis water, on the same grid as the terrain.
    pub fn build_water(&self) -> (MeshData, u64) {
        let r = self.plan.oasis_r.max_element() * 1.4 + 4.0;
        let o = self.plan.oasis;
        let (x0, z0) = (((o.x - r) / VS).floor() as i32, ((o.y - r) / VS).floor() as i32);
        let n = (2.0 * r / VS).ceil() as i32;
        let oy = (BASE_Y / VS).floor() as i32;
        let ny = ((WATER_Y / VS).ceil() as i32 - oy + 1).max(1);
        let mut vol = MicroVoxelVolume::new(n, ny, n, VS, x0, oy, z0);
        let wl = (WATER_Y / VS - 0.5).floor() as i32;
        for iz in z0..z0 + n {
            for ix in x0..x0 + n {
                let c = self.col_at(ix, iz);
                if c.kind == Kind::Out || c.top >= WATER_Y {
                    continue;
                }
                let iy1 = (c.top / VS - 0.5).floor() as i32;
                for iy in iy1 + 1..=wl {
                    let col = if wl - iy >= 2 { pal::WATER_DEEP } else { pal::WATER };
                    vol.set(ix, iy, iz, MAT_WATER, col);
                }
            }
        }
        vol.compile(&mut ColorCache::new())
    }
}

/// Outline radius of the plinth in direction `a`.
fn outline(plan: &Plan, a: f32) -> f32 {
    let n = fbm2(a.cos() * 1.6 + plan.noise_off.x * 0.01, a.sin() * 1.6 + plan.noise_off.y * 0.01, 3);
    WORLD_R * (0.9 + 0.22 * n)
}

#[inline]
fn gauss(p: Vec2, c: Vec2, r: f32) -> f32 {
    (-(p - c).length_squared() / (r * r)).exp()
}

#[inline]
fn near(p: Vec2, c: Vec2, r0: f32, r1: f32) -> f32 {
    1.0 - smoothstep(r0, r1, (p - c).length())
}

/// The analytic height-field: everything about one (x, z) column.
pub fn sample(plan: &Plan, x: f32, z: f32) -> Column {
    let p = Vec2::new(x, z);
    let r = p.length();
    let a = z.atan2(x);
    let e = r / outline(plan, a);
    if e >= 1.0 {
        return Column::default();
    }
    let q = p + plan.noise_off;
    let dir = if r > 1e-3 { p / r } else { Vec2::X };

    // ---- mesa mask -------------------------------------------------------
    let warp = Vec2::new(fbm2(q.x * 0.011 + 5.2, q.y * 0.011, 2), fbm2(q.x * 0.011, q.y * 0.011 + 1.3, 2)) - 0.5;
    let qw = q + warp * 34.0;
    let base = fbm2(qw.x * 0.017, qw.y * 0.017, 4);
    let back = dir.dot(plan.back);
    let mut m = smoothstep(0.5, 0.68, base) * 0.8;
    // Rim mesas, broken into separate blocks by a second noise field.
    let gaps = smoothstep(0.4, 0.58, fbm2(qw.x * 0.024 + 3.0, qw.y * 0.024 - 8.0, 3));
    m += smoothstep(0.45, 0.85, e) * smoothstep(-0.4, 0.45, back) * (0.2 + 1.0 * gaps);
    m += (fbm2(qw.x * 0.06, qw.y * 0.06, 3) - 0.5) * 0.3;
    m += gauss(p, plan.arch_spur, 11.0) * 1.1 + gauss(p, plan.butte, 12.0) * 0.95 + gauss(p, plan.overlook, 17.0) * 1.0;
    // Keep the front (camera side) of the diorama low and open.
    m -= smoothstep(0.1, -0.6, back) * smoothstep(0.35, 0.9, e) * 0.45;
    let (rd, rt) = plan.river_dist(p);
    let oe = plan.oasis_e(p);
    let clear = (1.0 - smoothstep(6.0, 18.0, rd))
        .max(1.0 - smoothstep(1.3, 2.6, oe))
        .max(near(p, plan.ruins, 21.0, 34.0))
        .max(near(p, plan.arch, 10.0, 17.5))
        .max(near(p, plan.arch - plan.arch_across * 24.0, 12.0, 22.0))
        .max(near(p, plan.camp, 7.0, 15.0));
    m -= clear * 1.6;
    let m = m.clamp(0.0, 1.3);

    // ---- mesa height: three terraces --------------------------------------
    let hs = 0.85 + 0.35 * noise2(q.x * 0.02, q.y * 0.02);
    let tier = |t0: f32| smoothstep(t0, t0 + 0.035, m);
    let mut mesa = (tier(0.5) * 13.0 + tier(0.66) * 9.0 + tier(0.8) * 7.0) * hs;
    if mesa > 0.5 {
        mesa += (ridged2(q.x * 0.09, q.y * 0.09, 3) - 0.35) * 1.4 * smoothstep(0.5, 0.6, m);
    }
    let talus = smoothstep(0.34, 0.5, m) * 2.4;

    // ---- floor + dunes -----------------------------------------------------
    let calm = clear.max(near(p, plan.ruins, 26.0, 42.0)).max(1.0 - smoothstep(2.0, 3.2, oe));
    let floor = (fbm2(q.x * 0.03, q.y * 0.03, 3) - 0.45) * 1.6 * (1.0 - calm);
    let du = p.dot(plan.dune_dir);
    let dv = p.dot(plan.dune_dir.perp());
    let u = du / 19.0 + (dv * 0.045).sin() * 0.7 + fbm2(q.x * 0.02, q.y * 0.02, 3) * 2.4 + plan.dune_phase;
    let s = u - u.floor();
    let prof = if s < 0.72 { smoothstep(0.0, 0.72, s) } else { 1.0 - smoothstep(0.72, 1.0, s) };
    let zone = smoothstep(0.3, 0.52, fbm2(q.x * 0.012 + 9.0, q.y * 0.012 - 4.0, 2) + smoothstep(0.0, -0.7, back) * 0.25);
    let along = 0.45 + 0.55 * noise2(dv * 0.035 + q.x * 0.004, u.floor() * 3.7);
    let amp = 4.4 * zone * along * (1.0 - smoothstep(0.2, 0.42, m)) * (1.0 - calm) * (1.0 - smoothstep(0.88, 0.99, e));
    let dune = prof * amp;

    // ---- carving: riverbed + oasis basin -----------------------------------
    let w = 4.5 + 2.5 * rt;
    let river = 1.5 * (1.0 - smoothstep(w * 0.55, w, rd)) * smoothstep(0.0, 0.04, rt).max(0.3);
    let basin = (2.9 * (1.0 - oe * oe)).max(0.0);
    let carve = river.max(basin);
    // Round off the plinth rim slightly.
    let rim = 1.2 * smoothstep(0.975, 1.0, e);
    let base_y = floor - carve - rim;
    let top = base_y + dune + talus + mesa;

    let kind = if mesa > 1.0 {
        Kind::Mesa
    } else if basin > 0.0 && top < WATER_Y + 0.05 {
        Kind::Lakebed
    } else if oe < 1.0 {
        Kind::Shore
    } else if oe < 1.35 + 0.5 * noise2(x * 0.25, z * 0.25) {
        Kind::Grass
    } else if river > 0.35 {
        Kind::River
    } else {
        Kind::Sand
    };
    let strata_off = strata_offset(plan.noise_off, plan.back, p);
    Column { kind, top, base: base_y, mesa, talus, dune, dune_u: u, lee: s >= 0.72 && amp > 0.6, strata_off }
}

/// Height offset of the strata bands at (x, z): gently warped and tilted beds.
pub fn strata_offset(noise_off: Vec2, back: Vec2, p: Vec2) -> f32 {
    let q = p + noise_off;
    (fbm2(q.x * 0.013, q.y * 0.013, 2) - 0.5) * 6.0 + p.dot(back) * 0.035
}

/// Sandstone strata at world height y (tilted by `off`), with varnish streaks.
pub fn strata(x: f32, y: f32, z: f32, off: f32, winter: bool) -> u32 {
    const PERIOD: f32 = 17.4;
    let mut k = (y + off).rem_euclid(PERIOD);
    let mut c = pal::STRATA[0].1;
    for &(t, col) in &pal::STRATA {
        if k < t {
            c = col;
            break;
        }
        k -= t;
    }
    // Desert varnish: dark vertical streaks hanging from ledges.
    let streak = noise2((x + z) * 0.45, y * 0.035 + (x - z) * 0.01);
    if streak > 0.7 {
        c = pal::shade(c, 0.8);
    } else if hash3(x.floor(), (y * 2.0).floor(), z.floor()) < 0.1 {
        c = pal::shade(c, 0.93);
    }
    pal::cool(c, winter)
}

fn sand_color(c: &Column, x: f32, z: f32, winter: bool) -> u32 {
    let mut col = if c.lee {
        pal::SAND_LEE
    } else if c.dune > 2.6 {
        pal::SAND_LT
    } else if c.dune < 0.35 && noise2(x * 0.12, z * 0.12) < 0.45 {
        pal::SAND_DK
    } else {
        pal::SAND
    };
    if c.talus > 0.6 && hash2(x * 1.3, z * 1.7) < c.talus * 0.25 {
        col = pal::PEBBLE;
    }
    if winter && (c.dune > 2.0 || noise2(x * 0.15, z * 0.15) > 0.6) {
        return if hash2(x, z) < 0.3 { pal::FROST_DK } else { pal::FROST };
    }
    pal::cool(col, winter)
}

/// Material + colour of one surface-near voxel.
fn paint(c: &Column, x: f32, y: f32, z: f32, winter: bool) -> (u8, u32) {
    let depth = c.top - y;
    let m = MAT_MATTE;
    match c.kind {
        Kind::Mesa if depth < 0.8 => {
            if winter {
                return (m, if noise2(x * 0.3, z * 0.3) < 0.25 { pal::FROST_DK } else { pal::FROST });
            }
            let i = (noise2(x * 0.18, z * 0.18) * 2.99) as usize;
            (m, pal::CAPROCK[i.min(2)])
        }
        Kind::River if depth < 0.6 => {
            // Cracked mud: borders of Voronoi cells (F2 - F1 small).
            let (crack, cell) = worley_edge(x * 0.45, z * 0.45);
            let col = if crack < 0.13 {
                pal::MUD_CRACK
            } else if cell < 0.3 {
                pal::shade(pal::MUD, 0.94)
            } else {
                pal::MUD
            };
            (m, if winter && crack >= 0.13 { pal::FROST_DK } else { pal::cool(col, winter) })
        }
        Kind::Lakebed if depth < 0.6 => (m, pal::LAKEBED),
        Kind::Shore if depth < 0.6 => (m, pal::cool(pal::WET_SAND, winter)),
        Kind::Grass if depth < 0.5 => {
            let i = (hash2(x * 0.7, z * 0.9) * 2.99) as usize;
            (MAT_FOLIAGE, pal::cool(pal::GRASS[i.min(2)], winter))
        }
        Kind::Sand | Kind::Grass | Kind::Shore if y > c.base - 0.9 => (m, sand_color(c, x, z, winter)),
        Kind::River if y > c.base - 0.9 => (m, pal::cool(pal::SAND_DK, winter)),
        _ => (m, strata(x, y, z, c.strata_off, winter)),
    }
}

/// Voronoi edge distance (F2 - F1) and a hash of the nearest cell.
fn worley_edge(x: f32, z: f32) -> (f32, f32) {
    let (ix, iz) = (x.floor(), z.floor());
    let (mut f1, mut f2, mut id) = (f32::MAX, f32::MAX, 0.0);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (cx, cz) = (ix + dx as f32, iz + dz as f32);
            let px = cx + hash2(cx, cz);
            let pz = cz + hash2(cz + 17.3, cx - 4.1);
            let d = ((px - x).powi(2) + (pz - z).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = hash2(cx * 0.37, cz * 1.91);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f2 - f1, id)
}
