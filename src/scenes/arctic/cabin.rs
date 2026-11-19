//! Timber cabins built from implicit solids. The body is a box, the gables are
//! the region under the roof line y_u(z) = h + (W/2 - |z|)·tan(pitch), and the
//! roof is a slab that follows the same line (so it always sits on the walls,
//! centred, with eaves overhanging). Snow lies on the roof as a lumpy layer.
//! Local frame: base at y = 0, ridge along X, door on the +X gable end.

use glam::{Vec2, Vec3};

use super::pal;
use crate::engine::prelude::*;

const CABIN_SCALE: f32 = 0.125;

#[derive(Clone, Copy, PartialEq)]
pub enum Siding {
    Logs,
    Boards,
}

#[derive(Clone, Copy)]
pub struct CabinStyle {
    pub len: f32,
    pub wid: f32,
    pub h: f32,
    pub pitch: f32,
    pub overhang: f32,
    pub siding: Siding,
    pub wall: u32,
    pub wall_dk: u32,
    pub windows: usize,
    /// Stone chimney (false = thin stove pipe).
    pub chimney: bool,
    pub lanterns: bool,
}

pub const COTTAGE_LOG: CabinStyle = CabinStyle {
    len: 7.6,
    wid: 5.6,
    h: 3.0,
    pitch: 0.68,
    overhang: 0.7,
    siding: Siding::Logs,
    wall: pal::LOG,
    wall_dk: pal::LOG_DK,
    windows: 2,
    chimney: true,
    lanterns: true,
};

pub const COTTAGE_RED: CabinStyle = CabinStyle {
    len: 7.0,
    wid: 5.4,
    h: 3.2,
    pitch: 0.72,
    overhang: 0.6,
    siding: Siding::Boards,
    wall: pal::FALU,
    wall_dk: pal::FALU_DK,
    windows: 2,
    chimney: true,
    lanterns: true,
};

pub const COTTAGE_OCHRE: CabinStyle = CabinStyle {
    len: 6.4,
    wid: 5.0,
    h: 2.9,
    pitch: 0.7,
    overhang: 0.6,
    siding: Siding::Boards,
    wall: pal::OCHRE,
    wall_dk: pal::OCHRE_DK,
    windows: 1,
    chimney: true,
    lanterns: false,
};

pub const LODGE: CabinStyle = CabinStyle {
    len: 13.0,
    wid: 8.4,
    h: 4.2,
    pitch: 0.62,
    overhang: 0.9,
    siding: Siding::Logs,
    wall: pal::LOG,
    wall_dk: pal::LOG_DK,
    windows: 3,
    chimney: true,
    lanterns: true,
};

pub const FISHING_HUT: CabinStyle = CabinStyle {
    len: 3.2,
    wid: 2.6,
    h: 2.2,
    pitch: 0.6,
    overhang: 0.35,
    siding: Siding::Boards,
    wall: pal::FALU,
    wall_dk: pal::FALU_DK,
    windows: 1,
    chimney: false,
    lanterns: false,
};

pub struct BuiltCabin {
    pub mesh: MeshData,
    pub voxels: u64,
    /// Local position of the chimney / stove-pipe mouth (smoke source).
    pub smoke: Vec3,
}

pub fn build_cabin(st: &CabinStyle, snowy: bool, seed: u32, colors: &mut ColorCache) -> BuiltCabin {
    let s = CABIN_SCALE;
    let tanp = st.pitch.tan();
    let (hl, hw) = (st.len * 0.5, st.wid * 0.5);
    let plinth = if st.chimney { 0.35 } else { 0.2 };
    let ridge = st.h + hw * tanp;
    let roof_t = 0.28;
    let snow_t = if snowy { 0.5 } else { 0.0 };
    let ext_x = hl + st.overhang + 1.2;
    let ext_z = hw + st.overhang + 1.0;
    let nx = (ext_x / s).ceil() as i32;
    let nz = (ext_z / s).ceil() as i32;
    let y0 = (-1.6 / s).floor() as i32;
    let y1 = ((ridge + 2.6) / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(nx * 2, y1 - y0, nz * 2, s, -nx, y0, -nz);
    let (lo, hi) = vol.local_bounds();
    let sd = seed as f32 * 0.17;
    let roof_under = |x: f32, z: f32| -> f32 {
        let _ = x;
        st.h + (hw - z.abs()) * tanp
    };

    // Stone plinth (reaches below ground so the cabin never floats on a slope).
    vol.fill_sdf(
        lo,
        hi,
        |p| sdf::box3(p - Vec3::Y * ((plinth - 1.6) * 0.5), Vec3::new(hl + 0.22, (plinth + 1.6) * 0.5, hw + 0.22)),
        |p, _| {
            let b = ((p.x * 2.2).floor() + (p.y * 3.5).floor() * 1.7 + (p.z * 2.2).floor() * 1.3) as i32;
            Some((MAT_MATTE, pal::STONE[b.rem_euclid(3) as usize]))
        },
    );

    // Walls + gables.
    let body = |p: Vec3| -> f32 {
        let b = sdf::box3(p - Vec3::new(0.0, (st.h + plinth) * 0.5, 0.0), Vec3::new(hl, (st.h - plinth) * 0.5, hw));
        let gable = (p.x.abs() - hl).max(p.z.abs() - hw).max(st.h - 0.01 - p.y).max(p.y - roof_under(p.x, p.z));
        b.min(gable)
    };
    vol.fill_sdf(lo, hi, body, |p, _| {
        let gable = p.y > st.h;
        let c = match st.siding {
            Siding::Logs if !gable => {
                let f = ((p.y - plinth) / 0.42).fract();
                if f < 0.22 { st.wall_dk } else { st.wall }
            }
            _ => {
                // Vertical boards with dark battens; white corner trim.
                let u = if p.x.abs() > hl - 0.05 { p.z } else { p.x };
                let corner = p.x.abs() > hl - 0.3 && p.z.abs() > hw - 0.3;
                if corner && !gable {
                    pal::TRIM
                } else if (u / 0.36).rem_euclid(1.0) < 0.2 {
                    st.wall_dk
                } else {
                    st.wall
                }
            }
        };
        Some((MAT_WOOD, c))
    });
    // Log cabins: crossed log ends stick out at the corners.
    if st.siding == Siding::Logs {
        let rows = ((st.h - plinth) / 0.42) as i32;
        for k in 0..rows {
            let y = plinth + 0.21 + k as f32 * 0.42;
            for sx in [-1.0f32, 1.0] {
                for sz in [-1.0f32, 1.0] {
                    let c = Vec3::new(sx * (hl - 0.18), y, sz * (hw - 0.18));
                    let (a, b) = if k % 2 == 0 {
                        (c, c + Vec3::X * sx * 0.55)
                    } else {
                        (c, c + Vec3::Z * sz * 0.55)
                    };
                    vol.fill_sdf_solid(a.min(b) - Vec3::splat(0.4), a.max(b) + Vec3::splat(0.4), |p| sdf::capsule(p, a, b, 0.2), MAT_WOOD, st.wall);
                }
            }
        }
    }

    // Windows on the long walls (±Z), the back gable (-X) and beside the door.
    let wy = plinth + (st.h - plinth) * 0.56;
    let (ww, wh) = (0.5, 0.58);
    let mut windows: Vec<(Vec3, Vec3)> = Vec::new(); // (centre on wall, outward normal)
    for i in 0..st.windows {
        let fx = (i as f32 + 0.5) / st.windows as f32 * 2.0 - 1.0;
        let wx = fx * (hl - 1.2).max(0.3) * if st.windows == 1 { 0.0 } else { 1.0 };
        for sz in [-1.0f32, 1.0] {
            windows.push((Vec3::new(wx, wy, sz * hw), Vec3::Z * sz));
        }
    }
    windows.push((Vec3::new(-hl, wy, 0.0), -Vec3::X));
    if st.h + (hw - 0.0) * tanp > st.h + 1.6 {
        windows.push((Vec3::new(-hl, st.h + 0.75, 0.0), -Vec3::X));
        windows.push((Vec3::new(hl, st.h + 0.75, 0.0), Vec3::X));
    }
    for &(c, n) in &windows {
        let t = Vec3::new(n.z.abs(), 0.0, n.x.abs()); // along-wall axis
        let local = |p: Vec3| -> (f32, f32, f32) { ((p - c).dot(t), p.y - c.y, (p - c).dot(n)) };
        // Frame (protrudes slightly) then glass recessed into the wall.
        vol.fill_sdf(
            c - Vec3::splat(1.2),
            c + Vec3::splat(1.2),
            |p| {
                let (u, v, w) = local(p);
                (u.abs() - (ww + 0.16)).max(v.abs() - (wh + 0.16)).max(w - 0.1).max(-w - 0.3)
            },
            |_, _| Some((MAT_WOOD, pal::TRIM)),
        );
        vol.carve_sdf(c - Vec3::splat(1.2), c + Vec3::splat(1.2), |p| {
            let (u, v, w) = local(p);
            (u.abs() - ww).max(v.abs() - wh).max(w - 0.2).max(-w - 0.12)
        });
        vol.fill_sdf(
            c - Vec3::splat(1.2),
            c + Vec3::splat(1.2),
            |p| {
                let (u, v, w) = local(p);
                (u.abs() - ww).max(v.abs() - wh).max((w + 0.2).abs() - 0.08)
            },
            |p, _| {
                let (u, v, _) = local(p);
                let mullion = u.abs() < 0.06 || v.abs() < 0.06;
                Some(if mullion { (MAT_WOOD, pal::TRIM) } else { (MAT_WIN, pal::WINDOW) })
            },
        );
        // Sill with a little snow.
        let sill_c = c + n * 0.18 - Vec3::Y * (wh + 0.2);
        vol.fill_sdf(
            sill_c - Vec3::splat(1.0),
            sill_c + Vec3::splat(1.0),
            |p| {
                let (u, v, w) = local(p);
                let sw = (p - sill_c).dot(n);
                let _ = w;
                (u.abs() - (ww + 0.25)).max((v + wh + 0.2).abs() - 0.07 - if snowy { 0.08 } else { 0.0 }).max(sw.abs() - 0.2)
            },
            |p, _| Some(if snowy && p.y > sill_c.y + 0.03 { (MAT_MATTE, pal::SNOW) } else { (MAT_WOOD, pal::TRIM) }),
        );
    }

    // Door on the +X gable, with a step and lanterns.
    let door_h = (st.h - plinth) * 0.78;
    let door_w = if st.chimney { 0.6 } else { 0.45 };
    let dc = Vec3::new(hl, plinth + door_h * 0.5, 0.0);
    vol.fill_sdf(
        dc - Vec3::splat(2.0),
        dc + Vec3::splat(2.0),
        |p| sdf::box3(p - dc, Vec3::new(0.1, door_h * 0.5 + 0.15, door_w + 0.15)),
        |_, _| Some((MAT_WOOD, pal::TRIM)),
    );
    vol.carve_sdf(dc - Vec3::splat(2.0), dc + Vec3::splat(2.0), |p| sdf::box3(p - dc, Vec3::new(0.25, door_h * 0.5, door_w)));
    vol.fill_sdf(
        dc - Vec3::splat(2.0),
        dc + Vec3::splat(2.0),
        |p| sdf::box3(p - (dc - Vec3::X * 0.16), Vec3::new(0.08, door_h * 0.5, door_w)),
        |p, _| Some((MAT_WOOD, if ((p.z / 0.3).rem_euclid(1.0)) < 0.2 { pal::LOG_DK } else { pal::DOOR })),
    );
    let step = Vec3::new(hl + 0.55, plinth * 0.5 - 0.2, 0.0);
    vol.fill_sdf(step - Vec3::splat(1.5), step + Vec3::splat(1.5), |p| sdf::box3(p - step, Vec3::new(0.5, plinth * 0.5 + 0.2, door_w + 0.3)), |_, _| {
        Some((MAT_MATTE, pal::STONE[1]))
    });
    if st.lanterns {
        for sz in [-1.0f32, 1.0] {
            let lc = Vec3::new(hl + 0.22, plinth + door_h + 0.05, sz * (door_w + 0.45));
            vol.fill_sdf(lc - Vec3::splat(0.5), lc + Vec3::splat(0.5), |p| sdf::box3(p - lc, Vec3::new(0.14, 0.2, 0.14)), |p, _| {
                Some(if p.y > lc.y + 0.13 { (MAT_METAL, pal::ROOF) } else { (MAT_LAMP, pal::LANTERN) })
            });
        }
    }

    // Roof slab following the gable line, eaves overhanging on all sides.
    let in_roof_xz = |p: Vec3, pad: f32| (p.x.abs() - (hl + st.overhang + pad)).max(p.z.abs() - (hw + st.overhang + pad));
    vol.fill_sdf(
        lo,
        hi,
        |p| {
            let yu = roof_under(p.x, p.z);
            in_roof_xz(p, 0.0).max(yu - p.y).max(p.y - (yu + roof_t))
        },
        |p, _| {
            // Summer: Nordic turf roofs on the cottages.
            let top = p.y > roof_under(p.x, p.z) + roof_t * 0.5;
            let c = if !snowy && st.chimney && top { pal::TUNDRA_SUMMER[1] } else { pal::ROOF };
            Some((MAT_WOOD, c))
        },
    );
    if snowy {
        // Lumpy snow blanket, thinner and rounded toward the eaves.
        vol.fill_sdf(
            lo,
            hi,
            |p| {
                let yu = roof_under(p.x, p.z) + roof_t;
                let edge = -in_roof_xz(p, 0.08);
                let lump = (fbm2(p.x * 0.45 + sd, p.z * 0.45, 2) - 0.5) * 0.14;
                let th = (snow_t + lump) * smoothstep_f(0.0, 0.45, edge).max(0.25);
                in_roof_xz(p, 0.08).max(yu - 0.02 - p.y).max(p.y - (yu + th))
            },
            |_, _| Some((MAT_MATTE, pal::SNOW)),
        );
    }

    // Chimney (stone) or stove pipe (metal), always piercing the roof.
    let (smoke, chim_half) = if st.chimney {
        let cx = -hl * 0.45;
        let cz = -hw * 0.38;
        let top = roof_under(cx, cz) + 1.3;
        (Vec3::new(cx, top + 0.1, cz), Vec3::new(0.42, 0.0, 0.42))
    } else {
        let cx = -hl * 0.3;
        let cz = hw * 0.3;
        (Vec3::new(cx, roof_under(cx, cz) + 1.0, cz), Vec3::new(0.14, 0.0, 0.14))
    };
    let base_y = st.h - 0.6;
    let cc = Vec3::new(smoke.x, (smoke.y - 0.1 + base_y) * 0.5, smoke.z);
    let ch = Vec3::new(chim_half.x, (smoke.y - 0.1 - base_y) * 0.5, chim_half.z);
    vol.fill_sdf(cc - ch - Vec3::splat(0.5), cc + ch + Vec3::splat(0.5), |p| sdf::box3(p - cc, ch), |p, _| {
        if !st.chimney {
            return Some((MAT_METAL, pal::ROOF));
        }
        let b = ((p.y * 3.5).floor() as i32 + ((p.x + p.z) * 2.4).floor() as i32).rem_euclid(3);
        Some((MAT_MATTE, pal::STONE[b as usize]))
    });
    if st.chimney {
        let mouth = Vec3::new(smoke.x, smoke.y - 0.15, smoke.z);
        vol.carve_sdf(mouth - Vec3::splat(0.5), mouth + Vec3::splat(0.5), |p| sdf::box3(p - mouth, Vec3::new(0.22, 0.2, 0.22)));
    }

    // Snow drifted against the walls (not on the hut out on the ice).
    if snowy && st.chimney {
        vol.fill_sdf(
            lo,
            hi,
            |p| {
                let q = Vec2::new((p.x.abs() - hl).max(0.0), (p.z.abs() - hw).max(0.0));
                let outside = q.length();
                let drift = 0.55 + (fbm2(p.x * 0.7 + sd, p.z * 0.7 - sd, 2) - 0.5) * 0.5 - outside * 0.45;
                let door_gap = if p.x > hl - 0.1 && p.z.abs() < door_w + 0.5 { 1.0 } else { 0.0 };
                (p.y - drift).max(-p.y - 1.0).max(outside - 1.6).max(door_gap - 0.5)
            },
            |_, _| Some((MAT_MATTE, pal::SNOW)),
        );
    }

    let (mesh, voxels) = vol.compile(colors);
    BuiltCabin { mesh, voxels, smoke }
}

fn smoothstep_f(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A wooden lamp post with a glowing lantern. Origin at ground level.
pub fn build_lamp_post(snowy: bool, colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.08;
    let mut vol = MicroVoxelVolume::new(16, 50, 16, s, -6, -12, -8);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sdf::box3(p - Vec3::Y * 1.1, Vec3::new(0.09, 2.0, 0.09)), MAT_WOOD, pal::LOG_DK);
    vol.fill_sdf_solid(lo, hi, |p| sdf::box3(p - Vec3::new(0.25, 2.95, 0.0), Vec3::new(0.32, 0.05, 0.05)), MAT_WOOD, pal::LOG_DK);
    let lc = Vec3::new(0.5, 2.62, 0.0);
    vol.fill_sdf(lo, hi, |p| sdf::box3(p - lc, Vec3::new(0.15, 0.22, 0.15)), |_, _| Some((MAT_LAMP, pal::LANTERN)));
    let cap = lc + Vec3::Y * 0.27;
    vol.fill_sdf(lo, hi, |p| sdf::box3(p - cap, Vec3::new(0.2, 0.05, 0.2)), |_, _| Some((MAT_METAL, pal::ROOF)));
    if snowy {
        let sc = cap + Vec3::Y * 0.1;
        vol.fill_sdf(lo, hi, |p| sdf::box3(p - sc, Vec3::new(0.17, 0.05, 0.17)), |_, _| Some((MAT_MATTE, pal::SNOW)));
    }
    vol.compile(colors)
}
