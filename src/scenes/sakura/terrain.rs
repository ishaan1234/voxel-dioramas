use glam::Vec3;
use super::kit::{
    fbm2, hash2, noise2, pal, ridged2, CatmullRomCurve3, ColorCache, MeshData, VoxelVertex,
    MAT_MATTE, MAT_WATER,
};

pub const WORLD_HALF: f32 = 224.0;
pub const T_CELL: f32 = 0.5;
pub const GRID_N: usize = 896; // 448 / 0.5
pub const CHUNK_SZ: usize = 112; // 8 x 8 = 64 terrain chunks
pub const WATER_MICRO_Y: i16 = 16; // 8.0m water level

#[inline]
pub fn g_idx(gx: usize, gz: usize) -> usize {
    gz * GRID_N + gx
}

#[inline]
pub fn w_to_g(w: f32) -> i32 {
    ((w + WORLD_HALF) / T_CELL).floor() as i32
}

#[inline]
pub fn river_z(x: f32) -> f32 {
    38.0 + 14.0 * (x * 0.021).sin() + 6.0 * (x * 0.048).cos()
}

#[inline]
pub fn river_hw(x: f32) -> f32 {
    8.4 + 1.8 * (x * 0.06).sin()
}

#[inline]
pub fn is_harbor(x: f32, z: f32) -> bool {
    ((x - 144.0) * 0.86).hypot((z - 58.0) * 1.08) < 25.5
}

#[inline]
pub fn is_lagoon(x: f32, z: f32) -> bool {
    ((x - 132.0) * 0.92).hypot((z + 6.0) * 1.02) < 23.5
}

#[inline]
pub fn is_east_stream(x: f32, z: f32) -> bool {
    z >= -32.0 && z <= 42.0 && (x - (132.0 + 4.5 * (z * 0.11).sin())).abs() < 4.5
}

#[inline]
pub fn is_west_canal(x: f32, z: f32) -> bool {
    if x < -142.0 || x > -68.0 || z < 6.0 || z > 44.0 {
        return false;
    }
    let t = (x + 140.0) / 72.0;
    let cz = 38.0 * (1.0 - t) + 19.0 * t - 24.0 * (std::f32::consts::PI * t).sin();
    (z - cz).abs() < 3.6
}

#[inline]
pub fn is_water_body(x: f32, z: f32) -> bool {
    (z - river_z(x)).abs() <= river_hw(x)
        || is_harbor(x, z)
        || is_lagoon(x, z)
        || is_east_stream(x, z)
        || is_west_canal(x, z)
}

pub fn make_senbon_curve() -> CatmullRomCurve3 {
    CatmullRomCurve3::new(
        vec![
            Vec3::new(-124.0, 10.0, -14.0),
            Vec3::new(-130.0, 17.0, -30.0),
            Vec3::new(-139.0, 25.0, -48.0),
            Vec3::new(-145.0, 34.0, -68.0),
            Vec3::new(-146.0, 42.0, -84.0),
        ],
        false,
        0.5,
    )
}

pub struct FlatPad {
    pub cx: f32,
    pub cz: f32,
    pub hw: f32,
    pub hd: f32,
    pub y_micro: i16,
    pub border_stone: bool,
}

pub struct TerrainGrid {
    pub top_y: Vec<i16>,
    pub water_y: Vec<i16>,
    pub top_col: Vec<u32>,
    pub sub_col: Vec<u32>,
    pub flags: Vec<u8>, // bit 1: cobble/plaza, bit 2: stone wall/ishigaki, bit 4: waterfall
    pub flat_pads: Vec<FlatPad>,
}

impl TerrainGrid {
    pub fn new() -> Self {
        let n = GRID_N * GRID_N;
        Self {
            top_y: vec![0; n],
            water_y: vec![0; n],
            top_col: vec![0; n],
            sub_col: vec![0; n],
            flags: vec![0; n],
            flat_pads: Vec::new(),
        }
    }

    pub fn register_pad(&mut self, cx: f32, cz: f32, hw: f32, hd: f32, target_y: f32, border_stone: bool) {
        self.flat_pads.push(FlatPad {
            cx,
            cz,
            hw,
            hd,
            y_micro: (target_y / T_CELL).round() as i16,
            border_stone,
        });
    }

    pub fn get_surface_world_y(&self, wx: f32, wz: f32) -> f32 {
        let gx = w_to_g(wx).clamp(0, GRID_N as i32 - 1) as usize;
        let gz = w_to_g(wz).clamp(0, GRID_N as i32 - 1) as usize;
        self.top_y[g_idx(gx, gz)] as f32 * T_CELL
    }

    pub fn generate(&mut self, senbon_curve: &CatmullRomCurve3) {
        let peaks: [(f32, f32, f32, f32); 9] = [
            (-165.0, -172.0, 86.0, 62.0),
            (-85.0, -186.0, 98.0, 68.0),
            (-8.0, -194.0, 104.0, 72.0),
            (76.0, -185.0, 94.0, 68.0),
            (162.0, -170.0, 88.0, 62.0),
            (-194.0, -115.0, 70.0, 52.0),
            (194.0, -110.0, 68.0, 52.0),
            (-168.0, -60.0, 40.0, 40.0),
            (175.0, -48.0, 46.0, 44.0),
        ];

        let mut senbon_samples = Vec::with_capacity(81);
        for i in 0..=80 {
            senbon_samples.push(senbon_curve.get_point_at(i as f32 / 80.0));
        }

        for gz in 0..GRID_N {
            let z = -WORLD_HALF + (gz as f32 + 0.5) * T_CELL;
            for gx in 0..GRID_N {
                let x = -WORLD_HALF + (gx as f32 + 0.5) * T_CELL;
                let idx = g_idx(gx, gz);

                let r_c = river_z(x);
                let r_w = river_hw(x);
                let d_riv = (z - r_c).abs();
                let in_harbor = is_harbor(x, z);
                let in_lagoon = is_lagoon(x, z);
                let in_stream = is_east_stream(x, z);
                let in_canal = is_west_canal(x, z);
                let wf_x = 132.0 + 2.5 * (z * 0.14).sin();
                let in_wf_channel = (x - wf_x).abs() <= 5.2 && z >= -124.0 && z <= -28.0;

                let mut top_y: i16;
                let mut water_y: i16 = 0;
                let mut col = pal::GRASS_1;
                let mut sub_col = pal::MTN_BASE;
                let mut flag: u8 = 0;

                // 1. Alpine Mountain Range & East/West Spurs
                if z <= -104.0 || (z <= -24.0 && x.abs() >= 102.0) {
                    let mut m_h = 42.0f32;
                    for &(px, pz, ph, pr) in &peaks {
                        let d = ((x - px) * 0.92).hypot((z - pz) * 1.08);
                        if d < pr {
                            m_h = m_h.max(42.0 + ph * (1.0 - d / pr).powf(1.28) * 1.85);
                        }
                    }
                    let detail = (ridged2(x * 0.032 + 5.0, z * 0.032 + 9.0, 4) - 0.35) * 34.0
                        + (fbm2(x * 0.07, z * 0.07, 3) - 0.5) * 14.0;
                    top_y = (m_h + detail).max(38.0).round() as i16;

                    if z > -114.0 && x.abs() <= 108.0 {
                        let t = (z + 114.0) / 10.0;
                        top_y = (top_y as f32 * (1.0 - t) + 56.0 * t).round() as i16;
                    }

                    // East Waterfall Cliff Tiered Plunge Gorge
                    if x >= 114.0 && x <= 152.0 && z >= -124.0 && z <= -26.0 {
                        let cliff_y: i16 = if z <= -84.0 {
                            128
                        } else if z <= -64.0 {
                            (128.0 - ((z + 84.0) / 20.0) * 56.0).round() as i16
                        } else if z <= -48.0 {
                            72
                        } else {
                            (72.0 - ((z + 48.0) / 20.0) * 56.0).round() as i16
                        };

                        if in_wf_channel {
                            top_y = cliff_y - 2;
                            water_y = cliff_y + 2;
                            flag |= 4;
                        } else {
                            let bank_bump = ((x - wf_x).abs() * 1.4
                                + fbm2(x * 0.12, z * 0.12, 2) * 8.0)
                                .round() as i16;
                            top_y = (cliff_y + bank_bump).max(20);
                        }
                    }

                    // Kiyomizu Cliff Promontory Pad
                    if (x - 110.0).hypot(z + 44.0) < 16.0 {
                        let cap = (44.0 + (118.0 - x).max(0.0) * 1.2).round() as i16;
                        top_y = top_y.min(cap);
                    }

                    // Western Senbon Torii Mountain Ridge & Summit Shrine
                    if x <= -108.0 && z >= -98.0 && z <= -8.0 {
                        let mut min_d = 999.0f32;
                        let mut target_path_y = 20i16;
                        for sp in &senbon_samples {
                            let d = (x - sp.x).hypot(z - sp.z);
                            if d < min_d {
                                min_d = d;
                                target_path_y = (sp.y / T_CELL).round() as i16;
                            }
                        }
                        if min_d < 6.5 {
                            let blend = ((min_d - 3.2) / 3.3).max(0.0);
                            top_y = (target_path_y as f32 * (1.0 - blend) + top_y as f32 * blend)
                                .round() as i16;
                            if min_d <= 3.0 {
                                col = pal::STONE_MID;
                                flag |= 1;
                            }
                        }
                        if (x + 146.0).hypot(z + 88.0) < 14.0 {
                            top_y = 84;
                            col = pal::COURTYARD;
                            flag |= 1;
                        }
                    }

                    if (flag & 1) == 0 && (flag & 4) == 0 {
                        let snow_line = 104.0 + fbm2(x * 0.04, z * 0.04, 3) * 18.0;
                        if top_y as f32 >= snow_line {
                            col = if (gx + gz) % 3 == 0 { pal::SNOW_SHADE } else { pal::SNOW };
                            sub_col = pal::MTN_MID;
                        } else if top_y <= 52 && noise2(x * 0.08, z * 0.08) > 0.36 {
                            col = pal::GRASS_2;
                            sub_col = pal::DIRT;
                        } else {
                            let pk = hash2(gx as f32 * 0.7, gz as f32 * 0.7);
                            col = if pk > 0.65 {
                                pal::MTN_MID
                            } else if pk > 0.3 {
                                pal::MTN_BASE
                            } else {
                                pal::MTN_DARK
                            };
                            sub_col = pal::MTN_DARK;
                        }
                    }
                    if (flag & 4) != 0 {
                        col = pal::STONE_DARK;
                        sub_col = pal::MTN_DARK;
                    }
                }
                // 2. Dual-Tier Sacred Citadel Plateau
                else if z >= -106.0 && z <= -24.0 && x.abs() <= 104.0 {
                    let upper = z <= -58.0 && x.abs() <= 92.0;
                    top_y = if upper { 56 } else { 44 };
                    let is_parapet = x.abs() >= 103.5
                        || z >= -24.5
                        || (upper && (x.abs() >= 91.5 || z >= -58.5));
                    if is_parapet {
                        col = pal::STONE_MID;
                    } else if !upper && x >= 24.0 && x <= 74.0 && z >= -52.0 && z <= -30.0 {
                        let d_rock1 = (x - 38.0).hypot(z + 41.0);
                        let d_rock2 = (x - 60.0).hypot(z + 38.0);
                        let d_min = d_rock1.min(d_rock2);
                        let ring = if d_min < 9.5 {
                            (d_min * 3.1).sin() > 0.0
                        } else {
                            (z * 3.14).sin() > 0.0
                        };
                        col = if ring { pal::ZEN_GRAVEL } else { pal::ZEN_GRAVEL_DK };
                    } else {
                        let tile = ((gx >> 1) + (gz >> 1)) % 4 == 0;
                        col = if tile {
                            pal::COURTYARD_ALT
                        } else if (gx * gz) % 11 == 0 {
                            pal::COURTYARD_LT
                        } else {
                            pal::COURTYARD
                        };
                    }
                    sub_col = pal::STONE_MID;
                    flag |= 2;
                }
                // 3. River, Harbor, Waterfall Lagoon, East Stream & West Canal
                else if d_riv <= r_w + 2.5 || in_harbor || in_lagoon || in_stream || in_canal {
                    if d_riv <= r_w || in_harbor || in_lagoon || in_stream || in_canal {
                        let depth: i16 = if in_lagoon {
                            8
                        } else if in_harbor {
                            9
                        } else if in_stream || in_canal {
                            11
                        } else if d_riv < r_w * 0.45 {
                            8
                        } else {
                            11
                        };
                        top_y = depth;
                        water_y = WATER_MICRO_Y;
                        col = if (gx + gz) % 5 == 0 { 0x8e8674 } else { pal::SAND };
                        sub_col = pal::MTN_DARK;
                    } else {
                        top_y = 18;
                        col = if noise2(x * 0.2, z * 0.2) > 0.45 { pal::GRASS_2 } else { pal::MOSS };
                        sub_col = pal::STONE_MID;
                    }
                }
                // 4. South-East Contoured Rice & Tea Terraces
                else if x >= 84.0 && x <= 204.0 && z >= 90.0 && z <= 204.0 {
                    let contour = (x - 84.0) * 0.045
                        + (z - 90.0) * 0.032
                        + fbm2(x * 0.025, z * 0.025, 2) * 2.2;
                    let step = contour.floor().min(12.0) as i16;
                    let frac = contour - step as f32;
                    top_y = 20 + step;
                    if frac < 0.14 {
                        top_y += 1;
                        col = if (gx + gz) % 3 == 0 { pal::GRASS_2 } else { pal::DIRT };
                    } else {
                        water_y = top_y + 1;
                        let is_rice_row = ((x + z * 0.4) * 2.2).sin().abs() > 0.45;
                        col = if is_rice_row { pal::GRASS_RICE } else { pal::DIRT };
                        if is_rice_row && (gx + gz) % 2 == 0 {
                            top_y += 1;
                        }
                    }
                    sub_col = pal::DIRT;
                }
                // 5. Rolling Village Plains & Gently Sloped Outer Foothills
                else {
                    let dist_edge = x.abs().max(z.abs());
                    if dist_edge > 186.0 && x.abs() > 42.0 {
                        let hill = (dist_edge - 186.0) * 0.42 + fbm2(x * 0.04, z * 0.04, 3) * 8.0;
                        top_y = 20 + hill.round() as i16;
                        col = if top_y > 28 && noise2(x * 0.1, z * 0.1) > 0.55 {
                            pal::MTN_BASE
                        } else {
                            pal::GRASS_2
                        };
                        sub_col = pal::DIRT;
                    } else {
                        let n = fbm2(x * 0.045, z * 0.045, 2);
                        top_y = 20 + if n > 0.72 { 1 } else { 0 };
                        col = if n > 0.62 {
                            pal::GRASS_3
                        } else if n < 0.35 {
                            pal::GRASS_2
                        } else {
                            pal::GRASS_1
                        };
                        sub_col = pal::DIRT;
                    }
                }

                let edge = x.abs().max(z.abs());
                if edge > 218.0 {
                    top_y = top_y.min(14);
                    col = pal::BEDROCK;
                    sub_col = pal::BEDROCK;
                }

                self.top_y[idx] = top_y;
                self.water_y[idx] = water_y;
                self.top_col[idx] = col;
                self.sub_col[idx] = sub_col;
                self.flags[idx] = flag;
            }
        }
    }

    pub fn paint_road(&mut self, x0: f32, z0: f32, x1: f32, z1: f32, half_w: f32) {
        let dist = (x1 - x0).hypot(z1 - z0);
        let steps = (dist / 0.4).ceil().max(1.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = x0 + (x1 - x0) * t;
            let cz = z0 + (z1 - z0) * t;
            let gx0 = w_to_g(cx - half_w).max(0) as usize;
            let gx1 = w_to_g(cx + half_w).min(GRID_N as i32 - 1) as usize;
            let gz0 = w_to_g(cz - half_w).max(0) as usize;
            let gz1 = w_to_g(cz + half_w).min(GRID_N as i32 - 1) as usize;
            let cgx = w_to_g(cx);
            let cgz = w_to_g(cz);
            for gz in gz0..=gz1 {
                for gx in gx0..=gx1 {
                    let idx = g_idx(gx, gz);
                    if self.water_y[idx] > 0 {
                        continue;
                    }
                    let edge = (gx as i32 - cgx).abs() as f32 >= half_w * 2.0 - 1.0
                        || (gz as i32 - cgz).abs() as f32 >= half_w * 2.0 - 1.0;
                    let st = hash2(gx as f32 * 1.3, gz as f32 * 1.7);
                    self.top_col[idx] = if edge {
                        pal::STONE_MID
                    } else if st > 0.66 {
                        0x454a52
                    } else if st > 0.32 {
                        pal::STONE_DARK
                    } else {
                        0x3b3f46
                    };
                    self.flags[idx] |= 1;
                }
            }
        }
    }

    pub fn apply_pads_and_roads(&mut self) {
        for pad in &self.flat_pads {
            let gx0 = w_to_g(pad.cx - pad.hw).max(0) as usize;
            let gx1 = w_to_g(pad.cx + pad.hw).min(GRID_N as i32 - 1) as usize;
            let gz0 = w_to_g(pad.cz - pad.hd).max(0) as usize;
            let gz1 = w_to_g(pad.cz + pad.hd).min(GRID_N as i32 - 1) as usize;
            for gz in gz0..=gz1 {
                for gx in gx0..=gx1 {
                    let idx = g_idx(gx, gz);
                    if self.water_y[idx] > 0 {
                        continue;
                    }
                    self.top_y[idx] = pad.y_micro;
                    if pad.border_stone {
                        let is_edge = gx == gx0 || gx == gx1 || gz == gz0 || gz == gz1;
                        self.top_col[idx] = if is_edge { pal::STONE_MID } else { pal::COURTYARD_ALT };
                        self.sub_col[idx] = pal::STONE_MID;
                        self.flags[idx] |= 2;
                    }
                }
            }
        }

        // Grand Avenue
        self.paint_road(0.0, 58.0, 0.0, 206.0, 5.5);
        self.paint_road(0.0, -4.0, 0.0, 30.0, 5.5);
        // Market Plaza
        self.paint_road(-36.0, 117.0, 36.0, 117.0, 12.5);
        // East-West Town Streets
        self.paint_road(-164.0, 78.0, 134.0, 78.0, 3.5);
        self.paint_road(-158.0, 106.0, 82.0, 106.0, 3.0);
        self.paint_road(-158.0, 140.0, 82.0, 140.0, 3.2);
        self.paint_road(-138.0, 174.0, 82.0, 174.0, 3.0);
        // Cross Streets
        for sx in [-130.0, -96.0, -62.0, -30.0, 30.0, 62.0, 104.0] {
            self.paint_road(sx, 68.0, sx, 182.0, 2.5);
        }
        // North Bank Embankment Road & Canal Loop
        self.paint_road(-138.0, 6.0, 104.0, 6.0, 3.0);
        self.paint_road(-122.0, 6.0, -122.0, 24.0, 2.2);
        self.paint_road(92.0, 6.0, 92.0, 54.0, 2.4);
        // Citadel Courtyard Walkways
        self.paint_road(0.0, -56.0, 0.0, -25.0, 3.5);
        self.paint_road(-78.0, -40.0, 78.0, -40.0, 2.5);
        self.paint_road(-56.0, -80.0, 36.0, -80.0, 3.2);
    }

    #[inline]
    fn get_y(&self, gx: i32, gz: i32) -> i16 {
        if gx < 0 || gx >= GRID_N as i32 || gz < 0 || gz >= GRID_N as i32 {
            0
        } else {
            self.top_y[g_idx(gx as usize, gz as usize)]
        }
    }

    #[inline]
    fn get_w(&self, gx: i32, gz: i32) -> i16 {
        if gx < 0 || gx >= GRID_N as i32 || gz < 0 || gz >= GRID_N as i32 {
            0
        } else {
            self.water_y[g_idx(gx as usize, gz as usize)]
        }
    }

    pub fn build_chunks(&self, colors: &mut ColorCache) -> (MeshData, MeshData) {
        let chunks_per_side = GRID_N / CHUNK_SZ;
        let ao_factors = [0.60f32, 0.74, 0.87, 1.0];
        let mut opaque = MeshData::default();
        let mut water = MeshData::default();

        let mut visited = vec![0u8; CHUNK_SZ * CHUNK_SZ];
        let mut cell_ao = vec![0u8; CHUNK_SZ * CHUNK_SZ];

        for cz in 0..chunks_per_side {
            for cx in 0..chunks_per_side {
                let x_start = cx * CHUNK_SZ;
                let z_start = cz * CHUNK_SZ;
                visited.fill(0);

                for lz in 0..CHUNK_SZ {
                    let gz = z_start + lz;
                    for lx in 0..CHUNK_SZ {
                        let gx = x_start + lx;
                        let y = self.top_y[g_idx(gx, gz)];
                        let occ = |dx: i32, dz: i32| -> u8 {
                            if self.get_y(gx as i32 + dx, gz as i32 + dz) > y { 1 } else { 0 }
                        };
                        let ao_calc = |s1: u8, s2: u8, cn: u8| -> u8 {
                            if s1 != 0 && s2 != 0 { 0 } else { 3 - (s1 + s2 + cn) }
                        };
                        let xm = occ(-1, 0);
                        let xp = occ(1, 0);
                        let zm = occ(0, -1);
                        let zp = occ(0, 1);
                        let a00 = ao_calc(xm, zp, occ(-1, 1));
                        let a10 = ao_calc(xp, zp, occ(1, 1));
                        let a11 = ao_calc(xp, zm, occ(1, -1));
                        let a01 = ao_calc(xm, zm, occ(-1, -1));
                        cell_ao[lz * CHUNK_SZ + lx] = a00 | (a10 << 2) | (a11 << 4) | (a01 << 6);
                    }
                }

                for lz in 0..CHUNK_SZ {
                    let mut lx = 0usize;
                    while lx < CHUNK_SZ {
                        let lk = lz * CHUNK_SZ + lx;
                        if visited[lk] != 0 {
                            lx += 1;
                            continue;
                        }
                        let gx = x_start + lx;
                        let gz = z_start + lz;
                        let gi = g_idx(gx, gz);
                        let y = self.top_y[gi];
                        let c = self.top_col[gi];
                        let ao = cell_ao[lk];

                        let mut w = 1usize;
                        while lx + w < CHUNK_SZ {
                            let lkw = lk + w;
                            let giw = g_idx(gx + w, gz);
                            if visited[lkw] == 0
                                && self.top_y[giw] == y
                                && self.top_col[giw] == c
                                && cell_ao[lkw] == ao
                            {
                                w += 1;
                            } else {
                                break;
                            }
                        }
                        let mut h = 1usize;
                        'outer: while lz + h < CHUNK_SZ {
                            let row_off = (lz + h) * CHUNK_SZ + lx;
                            for dx in 0..w {
                                let lkr = row_off + dx;
                                let gir = g_idx(gx + dx, gz + h);
                                if visited[lkr] != 0
                                    || self.top_y[gir] != y
                                    || self.top_col[gir] != c
                                    || cell_ao[lkr] != ao
                                {
                                    break 'outer;
                                }
                            }
                            h += 1;
                        }
                        for dz in 0..h {
                            let row_off = (lz + dz) * CHUNK_SZ + lx;
                            for dx in 0..w {
                                visited[row_off + dx] = 1;
                            }
                        }

                        let wx0 = -WORLD_HALF + gx as f32 * T_CELL;
                        let wx1 = wx0 + w as f32 * T_CELL;
                        let wz0 = -WORLD_HALF + gz as f32 * T_CELL;
                        let wz1 = wz0 + h as f32 * T_CELL;
                        let wy = y as f32 * T_CELL;
                        let rgb = colors.get(c);
                        let base_v = opaque.vertices.len() as u32;
                        let a00 = ao & 3;
                        let a10 = (ao >> 2) & 3;
                        let a11 = (ao >> 4) & 3;
                        let a01 = (ao >> 6) & 3;

                        let mut push_v = |px: f32, pz: f32, a: u8| {
                            let m = ao_factors[a as usize];
                            opaque.vertices.push(VoxelVertex {
                                pos: [px, wy, pz],
                                norm: [0.0, 1.0, 0.0],
                                color: [rgb[0] * m, rgb[1] * m, rgb[2] * m],
                                mat_id: MAT_MATTE as u32,
                            });
                        };
                        push_v(wx0, wz1, a00);
                        push_v(wx1, wz1, a10);
                        push_v(wx1, wz0, a11);
                        push_v(wx0, wz0, a01);
                        if a00 + a11 > a10 + a01 {
                            opaque.indices.extend_from_slice(&[
                                base_v, base_v + 1, base_v + 2,
                                base_v, base_v + 2, base_v + 3,
                            ]);
                        } else {
                            opaque.indices.extend_from_slice(&[
                                base_v + 1, base_v + 2, base_v + 3,
                                base_v + 1, base_v + 3, base_v,
                            ]);
                        }
                        lx += w;
                    }
                }

                let emit_side = |opaque: &mut MeshData, colors: &mut ColorCache, x0: f32, z0: f32, x1: f32, z1: f32, y_bot: i16, y_top: i16, nx: f32, nz: f32, shade: f32, c_hex: u32, is_ishigaki: bool| {
                    let step_h = if is_ishigaki { 2 } else { 6 };
                    let mut ys = y_bot;
                    while ys < y_top {
                        let ye = y_top.min(ys + step_h);
                        let band_col = if is_ishigaki {
                            if (ys as i32 + (x0 + z0).round() as i32).rem_euclid(4) == 0 {
                                pal::STONE_DARK
                            } else {
                                pal::STONE_MID
                            }
                        } else if ys < y_top - 6 && ((ys >> 2) & 1) != 0 {
                            pal::MTN_DARK
                        } else {
                            c_hex
                        };
                        let rgb = colors.get(band_col);
                        let bv = opaque.vertices.len() as u32;
                        let py0 = ys as f32 * T_CELL;
                        let py1 = ye as f32 * T_CELL;
                        let b_shade = shade * (0.85 + 0.15 * (ye as f32 / y_top.max(1) as f32));
                        let coords = [[x0, py0, z0], [x1, py0, z1], [x1, py1, z1], [x0, py1, z0]];
                        for (k, &pos) in coords.iter().enumerate() {
                            let k_ao = if k < 2 { b_shade * 0.84 } else { b_shade };
                            opaque.vertices.push(VoxelVertex {
                                pos,
                                norm: [nx, 0.0, nz],
                                color: [rgb[0] * k_ao, rgb[1] * k_ao, rgb[2] * k_ao],
                                mat_id: MAT_MATTE as u32,
                            });
                        }
                        opaque.indices.extend_from_slice(&[bv, bv + 1, bv + 2, bv, bv + 2, bv + 3]);
                        ys += step_h;
                    }
                };

                for lz in 0..CHUNK_SZ {
                    let gz = z_start + lz;
                    for lx in 0..CHUNK_SZ {
                        let gx = x_start + lx;
                        let gi = g_idx(gx, gz);
                        let y = self.top_y[gi];
                        let sc = self.sub_col[gi];
                        let ish = (self.flags[gi] & 2) != 0;
                        let wx0 = -WORLD_HALF + gx as f32 * T_CELL;
                        let wx1 = wx0 + T_CELL;
                        let wz0 = -WORLD_HALF + gz as f32 * T_CELL;
                        let wz1 = wz0 + T_CELL;

                        let ny_xp = self.get_y(gx as i32 + 1, gz as i32);
                        if y > ny_xp {
                            emit_side(&mut opaque, colors, wx1, wz1, wx1, wz0, ny_xp, y, 1.0, 0.0, 0.92, sc, ish);
                        }
                        let ny_xm = self.get_y(gx as i32 - 1, gz as i32);
                        if y > ny_xm {
                            emit_side(&mut opaque, colors, wx0, wz0, wx0, wz1, ny_xm, y, -1.0, 0.0, 0.89, sc, ish);
                        }
                        let ny_zp = self.get_y(gx as i32, gz as i32 + 1);
                        if y > ny_zp {
                            emit_side(&mut opaque, colors, wx0, wz1, wx1, wz1, ny_zp, y, 0.0, 1.0, 0.95, sc, ish);
                        }
                        let ny_zm = self.get_y(gx as i32, gz as i32 - 1);
                        if y > ny_zm {
                            emit_side(&mut opaque, colors, wx1, wz0, wx0, wz0, ny_zm, y, 0.0, -1.0, 0.87, sc, ish);
                        }

                        let wy = self.water_y[gi];
                        if wy > y {
                            let is_wf = (self.flags[gi] & 4) != 0;
                            let w_hex = if is_wf {
                                if (gx + gz) % 3 == 0 { pal::FOAM } else { 0x52c8ca }
                            } else {
                                pal::WATER
                            };
                            let wrgb = colors.get(w_hex);
                            let wy_world = wy as f32 * T_CELL - 0.1;
                            let bv = water.vertices.len() as u32;
                            for pos in [
                                [wx0, wy_world, wz1],
                                [wx1, wy_world, wz1],
                                [wx1, wy_world, wz0],
                                [wx0, wy_world, wz0],
                            ] {
                                water.vertices.push(VoxelVertex {
                                    pos,
                                    norm: [0.0, 1.0, 0.0],
                                    color: wrgb,
                                    mat_id: MAT_WATER as u32,
                                });
                            }
                            water.indices.extend_from_slice(&[bv, bv + 1, bv + 2, bv, bv + 2, bv + 3]);

                            let nw_z = self.get_w(gx as i32, gz as i32 + 1);
                            if is_wf && nw_z > 0 && nw_z < wy {
                                let bv2 = water.vertices.len() as u32;
                                let y_lo = nw_z as f32 * T_CELL - 0.1;
                                for pos in [
                                    [wx0, y_lo, wz1],
                                    [wx1, y_lo, wz1],
                                    [wx1, wy_world, wz1],
                                    [wx0, wy_world, wz1],
                                ] {
                                    water.vertices.push(VoxelVertex {
                                        pos,
                                        norm: [0.0, 0.0, 1.0],
                                        color: wrgb,
                                        mat_id: MAT_WATER as u32,
                                    });
                                }
                                water.indices.extend_from_slice(&[bv2, bv2 + 1, bv2 + 2, bv2, bv2 + 2, bv2 + 3]);
                            }
                        }
                    }
                }
            }
        }

        (opaque, water)
    }
}
