//! The Sun Temple: a ruined round colonnade (tholos). A three-tier stepped
//! platform with crumbling edges, fluted columns placed by polar repetition
//! (each one whole, broken along a noisy tilted plane, or a stump), surviving
//! lintels on neighbouring whole columns, fallen drums and rubble, an altar
//! with a fire bowl, and wind-blown sand drifts half burying the steps.

use glam::{Quat, Vec2, Vec3};
use std::f32::consts::{PI, TAU};

use super::pal;
use super::plan::Plan;
use crate::engine::prelude::*;

/// Voxel size of the temple.
pub const RS: f32 = 0.25;
/// (radius, top height) of the three platform tiers.
const TIERS: [(f32, f32); 3] = [(12.0, 0.75), (10.6, 1.5), (9.2, 2.25)];
const TOP: f32 = 2.25;
const RING: f32 = 7.4;
const N_COLS: usize = 12;
const SHAFT_H: f32 = 7.0;
const COL_R: f32 = 0.62;
/// Instance scale of the whole temple.
pub const SCALE: f32 = 1.25;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Whole,
    Broken(f32),
    Stump(f32),
}

pub struct Ruins {
    /// World position of the platform centre at ground level.
    pub pos: Vec3,
    pub rot: f32,
    states: [State; N_COLS],
    seed: u32,
    winter: bool,
}

fn col_angle(i: usize) -> f32 {
    i as f32 * TAU / N_COLS as f32 + PI / 12.0
}

fn col_pos(i: usize) -> Vec2 {
    let a = col_angle(i);
    Vec2::new(a.cos(), a.sin()) * RING
}

/// Local ground height under the platform (tier tops, 0 outside).
fn ground(r: f32) -> f32 {
    TIERS.iter().rev().find(|(tr, _)| r < *tr - 0.3).map(|t| t.1).unwrap_or(0.0)
}

impl Ruins {
    pub fn new(plan: &Plan, ground_y: f32) -> Self {
        use State::*;
        let states = if plan.seed == 0 {
            [Whole, Whole, Whole, Broken(4.4), Stump(0.9), Broken(2.6), Whole, Whole, Broken(5.4), Stump(0.6), Broken(3.2), Stump(1.2)]
        } else {
            let mut rng = Rng(plan.seed.wrapping_mul(0x2545_F491) ^ 0x7E3B);
            let mut s = [Whole; N_COLS];
            for (i, st) in s.iter_mut().enumerate().skip(3) {
                let h = rng.next_f32();
                let k = rng.next_f32();
                *st = if h < 0.33 { Whole } else if h < 0.72 { Broken(2.2 + k * 3.6) } else { Stump(0.5 + k * 0.8) };
                let _ = i;
            }
            s
        };
        // Offset off the terrain grid so no face is ever coplanar with the sand.
        let pos = Vec3::new(plan.ruins.x + 0.13, ground_y, plan.ruins.y + 0.11);
        Self { pos, rot: plan.ruins_rot, states, seed: plan.seed, winter: plan.winter }
    }

    pub fn transform(&self) -> ModelInstance {
        ModelInstance::from_trs(self.pos, self.rot, Vec3::splat(SCALE))
    }

    pub fn to_world(&self, local: Vec3) -> Vec3 {
        self.pos + Quat::from_rotation_y(self.rot) * (local * SCALE)
    }

    /// Torch bases (world): two flank the stairs, four stand between columns.
    pub fn torches(&self) -> Vec<Vec3> {
        let mut v = vec![Vec3::new(-3.1, 0.0, 14.2), Vec3::new(3.1, 0.0, 14.4)];
        for deg in [30.0f32, 150.0, 210.0, 330.0] {
            let a = deg.to_radians();
            v.push(Vec3::new(a.cos() * 8.6, TOP, a.sin() * 8.6));
        }
        v.into_iter().map(|p| self.to_world(p)).collect()
    }

    /// The altar fire (world).
    pub fn fire(&self) -> Vec3 {
        self.to_world(Vec3::new(0.0, TOP + 1.7, 0.0))
    }

    pub fn top_center(&self) -> Vec3 {
        self.to_world(Vec3::new(0.0, TOP, 0.0))
    }

    fn stone(&self, p: Vec3) -> u32 {
        let i = (noise3(p * 0.7) * 2.99) as usize;
        let c = if hash3(p.x.floor(), (p.y * 2.0).floor(), p.z.floor()) < 0.12 { pal::TEMPLE_DK } else { pal::TEMPLE[i.min(2)] };
        pal::cool(c, self.winter)
    }

    pub fn build(&self) -> (MeshData, u64) {
        let n = (16.5 / RS) as i32;
        let (oy, ny) = ((-1.75 / RS) as i32, ((11.75 + 1.75) / RS) as i32);
        let mut vol = MicroVoxelVolume::new(2 * n, ny, 2 * n, RS, -n, oy, -n);
        let (lo, hi) = vol.local_bounds();
        let s = self.seed as f32 * 0.37;

        // ---- stepped platform with crumbling edges + stairs ----------------
        let platform = |p: Vec3| {
            let r = Vec2::new(p.x, p.z).length();
            let mut d = f32::MAX;
            for (tr, ty) in TIERS {
                let wob = (fbm2(p.x * 0.3 + s, p.z * 0.3, 2) - 0.5) * 0.6;
                d = d.min((r - tr - wob).max(p.y - ty).max(-1.5 - p.y));
            }
            for k in 0..3 {
                let z1 = 13.4 - k as f32 * 1.45;
                let ty = (k + 1) as f32 * 0.75;
                let b = sdf::box3(p - Vec3::new(0.0, (ty - 1.5) * 0.5, (9.0 + z1) * 0.5), Vec3::new(2.3, (ty + 1.5) * 0.5, (z1 - 9.0) * 0.5));
                d = d.min(b);
            }
            // Bites out of the rims: strong noise near the surface removes chunks.
            let bite = (noise3(p * 0.42 + Vec3::splat(s)) - 0.72).max(0.0) * 5.0;
            d + bite + (noise3(p * 1.7) - 0.5) * 0.25
        };
        vol.fill_sdf(lo, Vec3::new(hi.x, TOP + 0.5, hi.z), platform, |p, _| {
            let top_face = p.y > TOP - RS * 1.5 && Vec2::new(p.x, p.z).length() < TIERS[2].0 - 0.6;
            if top_face {
                // Paving slabs: running-bond joints.
                let row = (p.z / 1.3).floor();
                let fx = (p.x / 1.6 + row * 0.5).rem_euclid(1.0);
                let fz = (p.z / 1.3).rem_euclid(1.0);
                if fx < 0.1 || fz < 0.12 {
                    return Some((MAT_MATTE, pal::cool(pal::JOINT, self.winter)));
                }
                if self.winter && hash2((p.x / 1.6 + row * 0.5).floor(), row) < 0.5 {
                    return Some((MAT_MATTE, pal::FROST));
                }
                let k = (hash2((p.x / 1.6 + row * 0.5).floor(), row) * 2.99) as usize;
                return Some((MAT_MATTE, pal::cool(pal::TEMPLE[k], self.winter)));
            }
            Some((MAT_MATTE, self.stone(p)))
        });

        // ---- wind-blown sand drifts burying part of the platform -------------
        let drifts = [(Vec3::new(-8.5, -0.6, -5.5), Vec3::new(7.5, 3.0, 5.5)), (Vec3::new(7.5, -0.6, -7.5), Vec3::new(4.5, 2.1, 3.8))];
        for (c, r) in drifts {
            vol.fill_sdf(c - r - Vec3::ONE, c + r + Vec3::ONE, |p| sdf::ellipsoid(p - c, r) + (fbm3(p * 0.3, 2) - 0.5) * 1.4, |p, _| {
                let col = if self.winter && p.y > 1.2 { pal::FROST } else if noise2(p.x * 0.8, p.z * 0.8) < 0.3 { pal::SAND_DK } else { pal::SAND };
                Some((MAT_MATTE, pal::cool(col, self.winter)))
            });
        }

        // ---- columns -----------------------------------------------------------
        let mut rng = Rng(0xC0_1A55 ^ self.seed);
        for i in 0..N_COLS {
            let c2 = col_pos(i);
            let base = Vec3::new(c2.x, TOP, c2.y);
            let st = self.states[i];
            let (h, tilt) = match st {
                State::Whole => (SHAFT_H, Vec3::Y),
                State::Broken(h) => (h, Vec3::new(rng.next_f32() - 0.5, 1.6, rng.next_f32() - 0.5).normalize()),
                State::Stump(h) => (h, Vec3::new(rng.next_f32() - 0.5, 1.2, rng.next_f32() - 0.5).normalize()),
            };
            let lean = if st == State::Whole { Vec2::ZERO } else { Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * 0.04 };
            let cs = i as f32 * 3.1 + s;
            let shaft = |p: Vec3| {
                let mut q = p - base;
                q.x -= lean.x * q.y;
                q.z -= lean.y * q.y;
                let th = q.z.atan2(q.x);
                let r = COL_R * (1.0 - 0.06 * q.y / SHAFT_H) + 0.07 * (12.0 * th).cos().max(-0.3);
                let mut d = (Vec2::new(q.x, q.z).length() - r).max(-q.y - 0.3);
                // Base mouldings.
                d = d.min(sdf::cylinder(q - Vec3::Y * 0.15, 0.86, 0.15)).min(sdf::cylinder(q - Vec3::Y * 0.4, 0.74, 0.1));
                if st == State::Whole {
                    d = d.max(q.y - h);
                    let t = ((q.y - h) / 0.4).clamp(0.0, 1.0);
                    let echinus = (Vec2::new(q.x, q.z).length() - (0.6 + 0.32 * t)).max(h - q.y).max(q.y - h - 0.4);
                    let abacus = sdf::box3(q - Vec3::Y * (h + 0.58), Vec3::new(0.95, 0.18, 0.95));
                    d.min(echinus).min(abacus)
                } else {
                    let cut = (q - Vec3::Y * h).dot(tilt) + (noise3(p * 1.5 + Vec3::splat(cs)) - 0.5) * 0.9;
                    d.max(cut)
                }
            };
            vol.fill_sdf(base - Vec3::new(1.2, 0.4, 1.2), base + Vec3::new(1.2, SHAFT_H + 1.0, 1.2), shaft, |p, _| {
                if self.winter && p.y > TOP + 0.3 && shaft(p + Vec3::Y * RS) > 0.0 {
                    return Some((MAT_MATTE, pal::FROST));
                }
                Some((MAT_MATTE, self.stone(p)))
            });
        }

        // ---- lintels on neighbouring whole columns ------------------------------
        for i in 0..N_COLS {
            let j = (i + 1) % N_COLS;
            if self.states[i] != State::Whole || self.states[j] != State::Whole {
                continue;
            }
            let (a, b) = (col_pos(i), col_pos(j));
            let mid = (a + b) * 0.5;
            let ax = (b - a).normalize();
            let half = (b - a).length() * 0.5 + 0.95;
            let c = Vec3::new(mid.x, TOP + SHAFT_H + 0.76 + 0.42, mid.y);
            let lintel = |p: Vec3| {
                let d = Vec2::new(p.x - c.x, p.z - c.z);
                let q = Vec3::new(d.dot(ax), p.y - c.y, d.dot(ax.perp()));
                sdf::round_box(q, Vec3::new(half, 0.42, 0.62), 0.08) + (noise3(p * 1.3) - 0.5) * 0.3
            };
            let r = Vec3::splat(half + 1.0);
            vol.fill_sdf(c - r, c + r, lintel, |p, _| {
                if self.winter && lintel(p + Vec3::Y * RS) > 0.0 {
                    return Some((MAT_MATTE, pal::FROST));
                }
                Some((MAT_MATTE, self.stone(p)))
            });
        }

        // ---- fallen drums + a toppled lintel ------------------------------------
        let mut drums: Vec<(Vec3, Vec3, f32, f32)> = Vec::new();
        for i in 0..N_COLS {
            let k = match self.states[i] {
                State::Whole => 0,
                State::Broken(h) => ((SHAFT_H - h) / 2.2).ceil() as usize,
                State::Stump(_) => 3,
            };
            let out = col_pos(i).normalize();
            let side = out.perp();
            for d in 0..k.min(3) {
                let dist = RING + 1.8 + d as f32 * 1.7 + rng.next_f32() * 0.8;
                let c2 = out * dist + side * ((rng.next_f32() - 0.5) * 2.6);
                let ang = (rng.next_f32() - 0.5) * 1.6;
                let axis2 = Vec2::from_angle(ang).rotate(side);
                let r = COL_R * (0.95 - d as f32 * 0.04);
                let hl = 0.55 + rng.next_f32() * 0.3;
                // Rest on the lowest ground under either end (never hover).
                let g = [c2 + axis2 * hl, c2 - axis2 * hl, c2].iter().map(|q| ground(q.length())).fold(f32::MAX, f32::min);
                drums.push((Vec3::new(c2.x, g + r * 0.82, c2.y), Vec3::new(axis2.x, 0.0, axis2.y), r, hl));
            }
        }
        for (c, u, r, hl) in drums {
            let drum = |p: Vec3| {
                let v = p - c;
                let along = v.dot(u);
                let rad = (v - u * along).length();
                let q = Vec2::new(rad - r, along.abs() - hl);
                q.x.max(q.y).min(0.0) + q.max(Vec2::ZERO).length() + (noise3(p * 1.4) - 0.5) * 0.25
            };
            let e = Vec3::splat(r + hl + 0.6);
            vol.fill_sdf(c - e, c + e, drum, |p, _| Some((MAT_MATTE, self.stone(p))));
        }
        let fall_c = Vec3::new(-3.6, TOP + 0.4, -4.4);
        let fall_ax = Vec2::from_angle(0.5 + s).normalize();
        vol.fill_sdf(fall_c - Vec3::splat(4.0), fall_c + Vec3::splat(4.0), |p| {
            let d = Vec2::new(p.x - fall_c.x, p.z - fall_c.z);
            let q = Vec3::new(d.dot(fall_ax), p.y - fall_c.y, d.dot(fall_ax.perp()));
            sdf::round_box(sdf::rot_x(q, 0.12), Vec3::new(2.3, 0.42, 0.62), 0.08) + (noise3(p * 1.3) - 0.5) * 0.35
        }, |p, _| Some((MAT_MATTE, self.stone(p))));

        // ---- rubble around the base ---------------------------------------------
        for k in 0..16 {
            let a = k as f32 * 2.399 + s;
            if (a.sin() - 1.0).abs() < 0.35 {
                continue; // keep the stair approach clear
            }
            let r = 12.6 + rng.next_f32() * 2.6;
            let c = Vec3::new(a.cos() * r, 0.15, a.sin() * r);
            let size = Vec3::new(0.35 + rng.next_f32() * 0.4, 0.25 + rng.next_f32() * 0.25, 0.3 + rng.next_f32() * 0.35);
            let yaw = rng.next_f32() * TAU;
            vol.fill_sdf(c - Vec3::splat(1.2), c + Vec3::splat(1.2), |p| sdf::round_box(sdf::rot_y(p - c, yaw), size, 0.08), |p, _| Some((MAT_MATTE, self.stone(p))));
        }

        // ---- altar + fire bowl -----------------------------------------------------
        let alt = Vec3::new(0.0, TOP + 0.6, 0.0);
        vol.fill_sdf(alt - Vec3::splat(2.0), alt + Vec3::splat(2.0), |p| sdf::round_box(p - alt, Vec3::new(1.3, 0.6, 0.95), 0.12) + (noise3(p * 1.2) - 0.5) * 0.2, |p, _| {
            Some((MAT_MATTE, if (p.y - alt.y).abs() < 0.12 { pal::TEMPLE_DK } else { self.stone(p) }))
        });
        let bowl = Vec3::new(0.0, TOP + 1.35, 0.0);
        vol.fill_sdf(bowl - Vec3::splat(1.0), bowl + Vec3::splat(1.0), |p| sdf::shell(sdf::sphere(p - bowl - Vec3::Y * 0.35, 0.75), 0.09).max(p.y - bowl.y - 0.3), |_, _| Some((MAT_METAL, pal::BRONZE)));
        vol.fill_sdf(bowl - Vec3::splat(1.0), bowl + Vec3::splat(1.0), |p| sdf::ellipsoid(p - bowl - Vec3::Y * 0.2, Vec3::new(0.55, 0.22, 0.55)), |p, _| {
            Some((MAT_LAMP, if hash3(p.x * 4.0, p.y * 4.0, p.z * 4.0) < 0.5 { pal::EMBER } else { pal::FLAME }))
        });
        vol.compile(&mut ColorCache::new())
    }
}
