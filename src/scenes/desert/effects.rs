//! Animated elements. Every `push_*` appends exactly the number of instances
//! its batch reserved in `layout.rs`, in the same order.

use glam::{Mat4, Quat, Vec3};

use super::plan::WORLD_R;
use crate::engine::prelude::*;

pub struct Vulture {
    pub center: Vec3,
    pub radius: f32,
    pub omega: f32,
    pub phase: f32,
    pub height: f32,
}

/// Vultures riding a thermal: slow circles at staggered heights.
pub fn make_vultures(center: Vec3, n: usize, rng: &mut Rng) -> Vec<Vulture> {
    (0..n)
        .map(|i| Vulture {
            center,
            radius: 16.0 + rng.next_f32() * 18.0,
            omega: (0.11 + rng.next_f32() * 0.05) * if i % 4 == 3 { -1.0 } else { 1.0 },
            phase: i as f32 * 1.7 + rng.next_f32(),
            height: 38.0 + rng.next_f32() * 22.0,
        })
        .collect()
}

pub fn push_vultures(birds: &[Vulture], t: f32, out: &mut Vec<ModelInstance>) {
    for b in birds {
        let a = b.phase + t * b.omega;
        let r = b.radius + (t * 0.3 + b.phase).sin() * 3.0;
        let pos = b.center + Vec3::new(a.cos() * r, b.height + (t * 0.5 + b.phase).sin() * 1.5, a.sin() * r);
        // Fly along the circle tangent, banked into the turn.
        let heading = -a + if b.omega < 0.0 { std::f32::consts::PI } else { 0.0 };
        let bank = 0.32 * b.omega.signum() + (t * 1.3 + b.phase).sin() * 0.05;
        let m = Mat4::from_translation(pos) * Mat4::from_quat(Quat::from_rotation_y(heading) * Quat::from_rotation_z(bank));
        out.push(ModelInstance::from_mat4(m));
    }
}

/// Flickering flame tongues above every torch / fire (`base` = flame foot).
pub fn push_flames(flames: &[(Vec3, f32)], t: f32, out: &mut Vec<ModelInstance>) {
    for (i, &(p, size)) in flames.iter().enumerate() {
        let k = i as f32 * 1.93;
        let f = 1.0 + 0.18 * (t * 9.0 + k).sin() + 0.1 * (t * 15.3 + k * 2.1).sin();
        let w = 1.0 + 0.08 * (t * 11.0 + k * 0.7).sin();
        let m = Mat4::from_translation(p)
            * Mat4::from_rotation_y(t * 0.8 + k)
            * Mat4::from_rotation_z(0.06 * (t * 5.0 + k).sin())
            * Mat4::from_scale(Vec3::new(w * size, f * size, w * size));
        out.push(ModelInstance::from_mat4(m));
    }
}

/// Fine sand grains carried on the wind.
pub fn sand() -> ParticleConfig {
    ParticleConfig {
        count: 1500,
        seed: 1913,
        bmin: Vec3::new(-WORLD_R, -1.0, -WORLD_R),
        bmax: Vec3::new(WORLD_R, 26.0, WORLD_R),
        fall: (0.08, 0.35),
        drift: (1.6, 3.8),
        size: Vec3::new(0.09, 0.05, 0.09),
        colors: [0xf2d6a0, 0xe8c48e, 0xd9b07a, 0xfbe6c0],
        glow: 1.05,
    }
}
