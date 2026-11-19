use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use super::color::ColorCache;
use super::math::Rng;

#[allow(dead_code)]
pub const MAT_AIR: u8 = 0;
pub const MAT_MATTE: u8 = 1;
pub const MAT_WOOD: u8 = 2;
pub const MAT_METAL: u8 = 3;
pub const MAT_FOLIAGE: u8 = 4;
pub const MAT_WIN: u8 = 5;
pub const MAT_LAMP: u8 = 6;
pub const MAT_WATER: u8 = 7;
pub const MAT_CLOUD: u8 = 8;
pub const MAT_MIST: u8 = 9;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct VoxelVertex {
    pub pos: [f32; 3],
    pub norm: [f32; 3],
    pub color: [f32; 3],
    pub mat_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ModelInstance {
    pub model: [[f32; 4]; 4],
}

impl ModelInstance {
    pub fn identity() -> Self {
        Self {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn from_mat4(m: glam::Mat4) -> Self {
        Self {
            model: m.to_cols_array_2d(),
        }
    }

    pub fn from_trs(pos: Vec3, rot_y: f32, scale: Vec3) -> Self {
        let m = glam::Mat4::from_translation(pos)
            * glam::Mat4::from_rotation_y(rot_y)
            * glam::Mat4::from_scale(scale);
        Self::from_mat4(m)
    }
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn tri_count(&self) -> usize {
        self.indices.len() / 3
    }
}

struct FaceDef {
    axis: usize,
    sign: i32,
    u_axis: usize,
    v_axis: usize,
    shade: f32,
}

const FACES: [FaceDef; 6] = [
    FaceDef { axis: 0, sign:  1, u_axis: 1, v_axis: 2, shade: 0.94 }, // +X
    FaceDef { axis: 0, sign: -1, u_axis: 2, v_axis: 1, shade: 0.91 }, // -X
    FaceDef { axis: 1, sign:  1, u_axis: 2, v_axis: 0, shade: 1.00 }, // +Y
    FaceDef { axis: 1, sign: -1, u_axis: 0, v_axis: 2, shade: 0.78 }, // -Y
    FaceDef { axis: 2, sign:  1, u_axis: 0, v_axis: 1, shade: 0.96 }, // +Z
    FaceDef { axis: 2, sign: -1, u_axis: 1, v_axis: 0, shade: 0.89 }, // -Z
];

const AO_FACTORS: [f32; 4] = [0.56, 0.72, 0.86, 1.00];

pub struct MicroVoxelVolume {
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    pub scale: f32,
    pub ox: i32,
    pub oy: i32,
    pub oz: i32,
    mat: Vec<u8>,
    col: Vec<u32>,
    pub placed_count: u64,
}

impl MicroVoxelVolume {
    pub fn new(nx: i32, ny: i32, nz: i32, scale: f32, ox: i32, oy: i32, oz: i32) -> Self {
        let n = (nx * ny * nz) as usize;
        Self {
            nx,
            ny,
            nz,
            scale,
            ox,
            oy,
            oz,
            mat: vec![0; n],
            col: vec![0; n],
            placed_count: 0,
        }
    }

    #[inline]
    pub fn set(&mut self, wx: i32, wy: i32, wz: i32, mat: u8, col: u32) {
        let ix = wx - self.ox;
        let iy = wy - self.oy;
        let iz = wz - self.oz;
        if ix < 0 || ix >= self.nx || iy < 0 || iy >= self.ny || iz < 0 || iz >= self.nz {
            return;
        }
        let i = ((iy * self.nz + iz) * self.nx + ix) as usize;
        if self.mat[i] == 0 && mat != 0 {
            self.placed_count += 1;
        }
        self.mat[i] = mat;
        self.col[i] = col;
    }

    /// Remove a voxel (set to air), keeping `placed_count` accurate.
    #[inline]
    pub fn clear(&mut self, wx: i32, wy: i32, wz: i32) {
        let ix = wx - self.ox;
        let iy = wy - self.oy;
        let iz = wz - self.oz;
        if ix < 0 || ix >= self.nx || iy < 0 || iy >= self.ny || iz < 0 || iz >= self.nz {
            return;
        }
        let i = ((iy * self.nz + iz) * self.nx + ix) as usize;
        if self.mat[i] != 0 {
            self.placed_count -= 1;
            self.mat[i] = 0;
        }
    }

    /// Material at a world voxel coordinate (0 = air / out of bounds).
    #[inline]
    pub fn get(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        self.get_mat_raw(wx - self.ox, wy - self.oy, wz - self.oz)
    }

    #[inline]
    fn get_mat_raw(&self, ix: i32, iy: i32, iz: i32) -> u8 {
        if ix < 0 || ix >= self.nx || iy < 0 || iy >= self.ny || iz < 0 || iz >= self.nz {
            return 0;
        }
        self.mat[((iy * self.nz + iz) * self.nx + ix) as usize]
    }

    pub fn box_fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, col: u32) {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    self.set(x, y, z, mat, col);
                }
            }
        }
    }

    pub fn box_fn<F>(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, mut col_fn: F)
    where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    self.set(x, y, z, mat, col_fn(x, y, z));
                }
            }
        }
    }

    pub fn hollow_box(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, col: u32) {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    if x == ax || x == bx || y == ay || y == by || z == az || z == bz {
                        self.set(x, y, z, mat, col);
                    }
                }
            }
        }
    }

    pub fn hollow_box_fn<F>(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, mut col_fn: F)
    where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    if x == ax || x == bx || y == ay || y == by || z == az || z == bz {
                        self.set(x, y, z, mat, col_fn(x, y, z));
                    }
                }
            }
        }
    }

    pub fn ellipsoid_fn<F>(
        &mut self,
        cx: f32,
        cy: f32,
        cz: f32,
        rx: f32,
        ry: f32,
        rz: f32,
        mat: u8,
        mut col_fn: F,
        jitter: f32,
        rng: &mut Rng,
    ) where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ix0, ix1) = ((cx - rx).floor() as i32, (cx + rx).ceil() as i32);
        let (iy0, iy1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
        let (iz0, iz1) = ((cz - rz).floor() as i32, (cz + rz).ceil() as i32);
        for y in iy0..=iy1 {
            for z in iz0..=iz1 {
                for x in ix0..=ix1 {
                    let dx = (x as f32 - cx) / (rx + 0.01);
                    let dy = (y as f32 - cy) / (ry + 0.01);
                    let dz = (z as f32 - cz) / (rz + 0.01);
                    let d = dx * dx + dy * dy + dz * dz;
                    if d <= 1.0 && !(jitter > 0.0 && d > 1.0 - jitter && rng.next_f32() < 0.4) {
                        self.set(x, y, z, mat, col_fn(x, y, z));
                    }
                }
            }
        }
    }

    pub fn ellipsoid(&mut self, cx: f32, cy: f32, cz: f32, rx: f32, ry: f32, rz: f32, mat: u8, col: u32, jitter: f32, rng: &mut Rng) {
        self.ellipsoid_fn(cx, cy, cz, rx, ry, rz, mat, |_, _, _| col, jitter, rng);
    }

    pub fn cylinder(&mut self, cx: f32, y0: i32, cz: f32, r: f32, h: i32, mat: u8, col: u32) {
        let rr = r * r + 0.15;
        let (iz0, iz1) = ((cz - r).floor() as i32, (cz + r).ceil() as i32);
        let (ix0, ix1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        for y in y0..=(y0 + h) {
            for z in iz0..=iz1 {
                for x in ix0..=ix1 {
                    let dx = x as f32 - cx;
                    let dz = z as f32 - cz;
                    if dx * dx + dz * dz <= rr {
                        self.set(x, y, z, mat, col);
                    }
                }
            }
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, radius: f32, mat: u8, col: u32) {
        let dist = ((x1 - x0).powi(2) + (y1 - y0).powi(2) + (z1 - z0).powi(2)).sqrt();
        let steps = (dist * 1.5).ceil().max(1.0) as i32;
        let ir = radius.ceil() as i32;
        let rr = radius * radius + 0.15;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = x0 + (x1 - x0) * t;
            let cy = y0 + (y1 - y0) * t;
            let cz = z0 + (z1 - z0) * t;
            if radius <= 0.55 {
                self.set(cx.round() as i32, cy.round() as i32, cz.round() as i32, mat, col);
            } else {
                for dy in -ir..=ir {
                    for dz in -ir..=ir {
                        for dx in -ir..=ir {
                            if (dx * dx + dy * dy + dz * dz) as f32 <= rr {
                                self.set(
                                    (cx + dx as f32).round() as i32,
                                    (cy + dy as f32).round() as i32,
                                    (cz + dz as f32).round() as i32,
                                    mat,
                                    col,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn compile(&self, colors: &mut ColorCache) -> (MeshData, u64) {
        let dims = [self.nx, self.ny, self.nz];
        let offs = [self.ox, self.oy, self.oz];
        let s = self.scale;
        let max_slice = (self.nx * self.ny).max(self.ny * self.nz).max(self.nx * self.nz) as usize;
        let mut mask_mat = vec![0u8; max_slice];
        let mut mask_col = vec![0u32; max_slice];
        let mut mask_ao = vec![0u8; max_slice];
        let mut mesh = MeshData::default();

        for f in &FACES {
            let axis = f.axis;
            let sign = f.sign;
            let u_axis = f.u_axis;
            let v_axis = f.v_axis;
            let shade = f.shade;

            let n_slices = dims[axis];
            let n_u = dims[u_axis];
            let n_v = dims[v_axis];
            let mut n_vec = [0.0f32; 3];
            n_vec[axis] = sign as f32;

            let stride = |ax: usize| -> i32 {
                match ax {
                    0 => 1,
                    1 => self.nx * self.nz,
                    _ => self.nx,
                }
            };
            let stride_a = stride(axis);
            let stride_u = stride(u_axis);
            let stride_v = stride(v_axis);

            for slice in 0..n_slices {
                let adj_slice = slice + sign;
                let out_of_slice = adj_slice < 0 || adj_slice >= n_slices;
                let mut any_visible = false;

                for v in 0..n_v {
                    for u in 0..n_u {
                        let k = (v * n_u + u) as usize;
                        let v_idx = (slice * stride_a + v * stride_v + u * stride_u) as usize;
                        let m = self.mat[v_idx];
                        if m == 0 {
                            mask_mat[k] = 0;
                            continue;
                        }
                        let nm = if out_of_slice {
                            0
                        } else {
                            self.mat[(adj_slice * stride_a + v * stride_v + u * stride_u) as usize]
                        };
                        if nm != 0 {
                            mask_mat[k] = 0;
                            continue;
                        }

                        any_visible = true;
                        mask_mat[k] = m;
                        mask_col[k] = self.col[v_idx];

                        if out_of_slice || m == MAT_WIN || m == MAT_LAMP {
                            mask_ao[k] = 255;
                        } else {
                            let mut coord = [0i32; 3];
                            coord[axis] = adj_slice;
                            coord[u_axis] = u;
                            coord[v_axis] = v;
                            let g = |du: i32, dv: i32| -> u8 {
                                let mut nc = coord;
                                nc[u_axis] += du;
                                nc[v_axis] += dv;
                                if self.get_mat_raw(nc[0], nc[1], nc[2]) != 0 { 1 } else { 0 }
                            };
                            let ao_val = |s1: u8, s2: u8, cn: u8| -> u8 {
                                if s1 != 0 && s2 != 0 { 0 } else { 3 - (s1 + s2 + cn) }
                            };
                            let um = g(-1, 0);
                            let up = g(1, 0);
                            let vm = g(0, -1);
                            let vp = g(0, 1);
                            let a00 = ao_val(um, vm, g(-1, -1));
                            let a10 = ao_val(up, vm, g(1, -1));
                            let a11 = ao_val(up, vp, g(1, 1));
                            let a01 = ao_val(um, vp, g(-1, 1));
                            mask_ao[k] = a00 | (a10 << 2) | (a11 << 4) | (a01 << 6);
                        }
                    }
                }
                if !any_visible {
                    continue;
                }

                let face_plane = (slice + if sign > 0 { 1 } else { 0 } + offs[axis]) as f32 * s;
                for v in 0..n_v {
                    let mut u = 0;
                    while u < n_u {
                        let k = (v * n_u + u) as usize;
                        let m = mask_mat[k];
                        if m == 0 {
                            u += 1;
                            continue;
                        }
                        let c = mask_col[k];
                        let ao = mask_ao[k];
                        let mut w = 1;
                        while u + w < n_u {
                            let kw = k + w as usize;
                            if mask_mat[kw] == m && mask_col[kw] == c && mask_ao[kw] == ao {
                                w += 1;
                            } else {
                                break;
                            }
                        }
                        let mut h = 1;
                        'outer: while v + h < n_v {
                            let row_base = ((v + h) * n_u + u) as usize;
                            for du in 0..w as usize {
                                let kr = row_base + du;
                                if mask_mat[kr] != m || mask_col[kr] != c || mask_ao[kr] != ao {
                                    break 'outer;
                                }
                            }
                            h += 1;
                        }
                        for dv in 0..h {
                            let row_base = ((v + dv) * n_u + u) as usize;
                            for du in 0..w as usize {
                                mask_mat[row_base + du] = 0;
                            }
                        }

                        let base_v = mesh.vertices.len() as u32;
                        let u0 = (u + offs[u_axis]) as f32 * s;
                        let u1 = (u + w + offs[u_axis]) as f32 * s;
                        let v0 = (v + offs[v_axis]) as f32 * s;
                        let v1 = (v + h + offs[v_axis]) as f32 * s;
                        let rgb = colors.get(c);

                        let mut push_pt = |pu: f32, pv: f32, ao_bits: u8| {
                            let mut p = [0.0f32; 3];
                            p[axis] = face_plane;
                            p[u_axis] = pu;
                            p[v_axis] = pv;
                            let mult = shade * AO_FACTORS[(ao_bits & 3) as usize];
                            mesh.vertices.push(VoxelVertex {
                                pos: p,
                                norm: n_vec,
                                color: [rgb[0] * mult, rgb[1] * mult, rgb[2] * mult],
                                mat_id: m as u32,
                            });
                        };

                        let a00 = ao & 3;
                        let a10 = (ao >> 2) & 3;
                        let a11 = (ao >> 4) & 3;
                        let a01 = (ao >> 6) & 3;
                        push_pt(u0, v0, a00);
                        push_pt(u1, v0, a10);
                        push_pt(u1, v1, a11);
                        push_pt(u0, v1, a01);

                        if a00 + a11 > a10 + a01 {
                            mesh.indices.extend_from_slice(&[
                                base_v, base_v + 1, base_v + 2,
                                base_v, base_v + 2, base_v + 3,
                            ]);
                        } else {
                            mesh.indices.extend_from_slice(&[
                                base_v + 1, base_v + 2, base_v + 3,
                                base_v + 1, base_v + 3, base_v,
                            ]);
                        }
                        u += w;
                    }
                }
            }
        }

        (mesh, self.placed_count)
    }
}
