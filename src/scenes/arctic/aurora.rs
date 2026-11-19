//! Aurora borealis: long curtains swept along a wavy centre line
//! z(x) = A·(0.6 sin k₁x + 0.4 sin k₂x) + fbm, with ragged vertical rays
//! (per-column noise sets brightness and top height). Colour follows the
//! height fraction: pink lower fringe, bright green core, teal, then a fade
//! to near-sky darkness at the top. Curtains hang far beyond the valley, so
//! daytime fog hides them and they own the night sky. Each curtain is a
//! dynamic instance that drifts and sways with a time-varying shear.

use glam::{Mat4, Vec3, Vec4};
use std::f32::consts::TAU;

use super::pal;
use crate::engine::prelude::*;

const AURORA_SCALE: f32 = 4.2;

#[derive(Clone, Copy)]
pub struct Curtain {
    pub len: f32,
    pub height: f32,
    pub amp: f32,
    pub seed: f32,
}

fn centre_z(c: &Curtain, x: f32) -> f32 {
    let k1 = TAU / (c.len * 0.55);
    let k2 = TAU / (c.len * 0.21);
    c.amp * (0.6 * (x * k1 + c.seed).sin() + 0.4 * (x * k2 + c.seed * 2.3).sin())
        + (fbm2(x * 0.003 + c.seed, c.seed * 1.7, 3) - 0.5) * c.amp
}

pub fn build_curtain(c: &Curtain, colors: &mut ColorCache) -> (MeshData, u64) {
    let s = AURORA_SCALE;
    let hx = ((c.len * 0.5 + s) / s).ceil() as i32;
    let hz = ((c.amp * 1.8 + 6.0) / s).ceil() as i32;
    let y0 = (-18.0 / s).floor() as i32;
    let y1 = ((c.height + 14.0) / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(hx * 2, y1 - y0, hz * 2, s, -hx, y0, -hz);
    let (lo, hi) = vol.local_bounds();
    let k3 = TAU / (c.len * 0.33);
    let column = |x: f32| -> (f32, f32, f32, f32) {
        let zc = centre_z(c, x);
        let slope = (centre_z(c, x + 1.0) - centre_z(c, x - 1.0)) * 0.5;
        let yb = 11.0 * (x * k3 + c.seed * 0.7).sin();
        let edge = 0.5 - x.abs() / c.len;
        let env = (edge / 0.16).clamp(0.0, 1.0);
        let env = env * env * (3.0 - 2.0 * env);
        let ray = noise2(x * 0.08 + c.seed * 5.0, c.seed * 3.0);
        let body = 0.5 + 0.5 * fbm2(x * 0.0045 + c.seed, c.seed * 0.3 + 4.0, 3);
        let top = c.height * env * body * (0.5 + 0.5 * ray);
        (zc, 2.3 + slope.abs() * 2.2, yb, top.max(0.0))
    };
    // Column fill: each (x, z) column inside the sheet spans [yb, yb + top].
    let cur = std::cell::Cell::new((0.0f32, 0.0f32));
    vol.fill_columns(
        lo,
        hi,
        |x, z| {
            let (zc, hw, yb, top) = column(x);
            cur.set((yb, top));
            ((z - zc).abs() <= hw && top > s && x.abs() <= c.len * 0.5).then_some((yb, yb + top))
        },
        |p, _| {
            let (yb, top) = cur.get();
            let f = (p.y - yb) / c.height.max(1.0);
            let ray = noise2(p.x * 0.08 + c.seed * 5.0, c.seed * 3.0);
            let fold = fbm2(p.x * 0.0065 + c.seed * 2.0, 1.5, 2);
            let bright = ray + (fold - 0.5) * 1.2 > 0.5;
            let ft = (p.y - yb) / top.max(1.0);
            let col = if f < 0.028 {
                pal::AUR_PINK
            } else if f < 0.065 {
                pal::AUR_EDGE
            } else if ft > 0.86 {
                pal::AUR_FADE
            } else if ft > 0.7 {
                pal::AUR_TEAL_DIM
            } else if f < 0.38 {
                if bright { pal::AUR_GREEN } else { pal::AUR_GREEN_DIM }
            } else if bright {
                pal::AUR_TEAL
            } else {
                pal::AUR_TEAL_DIM
            };
            Some((MAT_LAMP, col))
        },
    );
    vol.compile(colors)
}

/// Placement + animation state for one curtain.
pub struct AuroraInst {
    pub pos: Vec3,
    pub yaw: f32,
    pub phase: f32,
    pub speed: f32,
}

pub fn push_aurora(items: &[AuroraInst], t: f32, out: &mut Vec<ModelInstance>) {
    for a in items {
        let w = t * a.speed + a.phase;
        let sx = 0.07 * (w * 0.7).sin() + 0.03 * (w * 1.9).sin();
        let sz = 0.16 * (w * 0.5 + 1.3).sin() + 0.05 * (w * 1.4).cos();
        let pulse = 0.07 * (w * 0.9 + 0.4).sin();
        let drift = Vec3::new((w * 0.21).sin() * 30.0, (w * 0.33).sin() * 8.0, (w * 0.17).cos() * 16.0);
        let shear = Mat4::from_cols(Vec4::X, Vec4::new(sx, 1.0 + pulse, sz, 0.0), Vec4::Z, Vec4::W);
        let m = Mat4::from_translation(a.pos + drift) * Mat4::from_rotation_y(a.yaw + (w * 0.13).sin() * 0.03) * shear;
        out.push(ModelInstance::from_mat4(m));
    }
}
