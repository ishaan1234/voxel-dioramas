//! The Observatory on the hero island: a single composite SDF (plaza, tiered
//! base, windowed tower, ribbed copper dome with a slit, telescope) plus a
//! floating armillary sphere whose rings are separate meshes so they can spin.

use glam::Vec3;
use std::f32::consts::{PI, TAU};

use super::pal;
use crate::engine::prelude::*;
use crate::engine::sdf::*;

pub const OBS_SCALE: f32 = 0.2;
pub const PLAZA_R: f32 = 15.0;
pub const TOWER_R: f32 = 6.0;
pub const TOWER_TOP: f32 = 11.0;
pub const DOME_R: f32 = 6.4;
/// Height of the armillary sphere centre above the plaza.
pub const ARMILLARY_Y: f32 = 27.0;
pub const LANTERN_POSTS: u32 = 8;
pub const LANTERN_RING_R: f32 = 13.4;

/// Angle within the current polar wedge (for ribs, windows, etc.).
fn wedge(p: Vec3, n: u32) -> Vec3 {
    repeat_polar(p, n)
}

/// `openings`: headings (radians, atan2(z, x)) where the balustrade is open for paths.
pub fn build_observatory(openings: &[f32], colors: &mut ColorCache) -> (MeshData, u64) {
    let s = OBS_SCALE;
    let half = ((PLAZA_R + 1.0) / s).ceil() as i32;
    let (y0, y1) = ((-1.2 / s) as i32, ((TOWER_TOP + DOME_R + 9.0) / s).ceil() as i32);
    let mut vol = MicroVoxelVolume::new(half * 2, y1 - y0, half * 2, s, -half, y0, -half);
    let (lo, hi) = vol.local_bounds();

    // --- Plaza: concentric paving rings and radial seams, gold compass inlay.
    vol.fill_sdf(lo, hi, |p| cylinder(p + Vec3::Y * 0.45, PLAZA_R, 0.55), |p, _| {
        let r = (p.x * p.x + p.z * p.z).sqrt();
        let a = p.z.atan2(p.x);
        let ring = (r / 1.6).floor() as i32;
        let seg = ((a + PI) / TAU * (8.0 + ring as f32 * 4.0)).floor() as i32;
        let compass = r > 7.5 && r < 12.5 && (a * 4.0).sin().abs() < 0.05 * 12.0 / r;
        if compass && p.y > -0.1 {
            return Some((MAT_METAL, pal::GOLD));
        }
        if r > PLAZA_R - 0.6 {
            return Some((MAT_MATTE, pal::STONE_DK));
        }
        Some((MAT_MATTE, if (ring + seg) % 2 == 0 { pal::STONE_LT } else { pal::STONE_MID }))
    });

    // --- Balustrade: low ring wall, opened wherever a path leaves the plaza.
    vol.fill_sdf(lo, hi, |p| {
        let wall = box3(Vec3::new((p.x * p.x + p.z * p.z).sqrt() - (PLAZA_R - 0.35), p.y - 0.55, 0.0), Vec3::new(0.3, 0.5, 1e3));
        let gap = openings
            .iter()
            .map(|&a| box3(rot_y(p, a) - Vec3::new(PLAZA_R, 0.6, 0.0), Vec3::new(2.0, 2.0, 1.7)))
            .fold(f32::MAX, f32::min);
        subtract(wall, gap)
    }, |p, _| {
        let q = wedge(p, 24);
        Some((MAT_MATTE, if q.z.abs() < 0.25 || p.y > 0.9 { pal::STONE_LT } else { pal::STONE_MID }))
    });

    // --- Tiered base: three stepped drums.
    for (i, (r, top)) in [(10.5f32, 0.5f32), (8.6, 1.0), (7.1, 1.5)].iter().enumerate() {
        let (r, top) = (*r, *top);
        vol.fill_sdf_solid(lo, hi, |p| cylinder(p - Vec3::Y * (top * 0.5), r, top * 0.5), MAT_MATTE, if i % 2 == 0 { pal::STONE_MID } else { pal::STONE_LT });
    }

    // --- Tower: cylinder with pilasters and arched windows (polar repetition).
    let tower_h = (TOWER_TOP - 1.5) * 0.5;
    let tower_c = Vec3::Y * (1.5 + tower_h);
    vol.fill_sdf(lo, hi, |p| {
        let body = cylinder(p - tower_c, TOWER_R, tower_h);
        let q = wedge(p, 10);
        // Pilasters: thin boxes pushed out on every wedge boundary.
        let q2 = rot_y(q, PI / 10.0);
        let pil = box3(q2 - Vec3::new(TOWER_R, tower_c.y, 0.0), Vec3::new(0.35, tower_h, 0.3));
        union(body, pil)
    }, |p, _| {
        let q = wedge(p, 10);
        let r = (p.x * p.x + p.z * p.z).sqrt();
        let in_window = q.z.abs() < 0.6 && p.y > 4.0 && p.y < 8.0 + (0.6 - q.z.abs()) * 1.2;
        if in_window && r > TOWER_R - 0.35 {
            let mullion = q.z.abs() < 0.08 || (p.y - 6.0).abs() < 0.08;
            return Some(if mullion { (MAT_WOOD, pal::BRONZE) } else { (MAT_WIN, pal::WINDOW) });
        }
        if r > TOWER_R + 0.05 {
            return Some((MAT_MATTE, pal::STONE_LT));
        }
        let band = (p.y - 2.5).abs() < 0.25 || (p.y - 9.0).abs() < 0.25;
        Some((MAT_MATTE, if band { pal::STONE_DK } else { pal::STONE_MID }))
    });

    // Door on the +Z side.
    let door_c = Vec3::new(0.0, 3.2, TOWER_R);
    vol.fill_sdf_solid(lo, hi, |p| box3(p - door_c, Vec3::new(1.1, 1.7, 0.5)), MAT_WOOD, pal::PLANK_DK);
    vol.fill_sdf_solid(lo, hi, |p| sphere(p - (door_c + Vec3::new(0.6, -0.2, 0.55)), 0.14), MAT_METAL, pal::BRASS);

    // --- Cornice ring + dome (ribbed copper shell with an observation slit).
    vol.fill_sdf_solid(lo, hi, |p| torus(p - Vec3::Y * TOWER_TOP, TOWER_R + 0.15, 0.45), MAT_MATTE, pal::STONE_LT);
    let dc = Vec3::Y * TOWER_TOP;
    vol.fill_sdf(lo, hi, |p| {
        let dome = intersect(shell(sphere(p - dc, DOME_R), 0.35), -(p.y - TOWER_TOP));
        let slit = box3(p - Vec3::new(0.0, TOWER_TOP + DOME_R, DOME_R * 0.5), Vec3::new(0.9, DOME_R * 0.75, DOME_R * 0.6));
        subtract(dome, slit)
    }, |p, _| {
        let q = wedge(p, 16);
        let rib = q.z.abs() < 0.18;
        Some((MAT_METAL, if rib { pal::BRONZE } else if (p.y * 1.5) as i32 % 2 == 0 { pal::COPPER } else { pal::COPPER_DK }))
    });
    // Finial.
    vol.fill_sdf_solid(lo, hi, |p| sphere(p - Vec3::Y * (TOWER_TOP + DOME_R + 0.3), 0.45), MAT_METAL, pal::GOLD);

    // --- Telescope poking out of the slit.
    let ta = Vec3::new(0.0, TOWER_TOP + 1.0, 0.0);
    let tb = Vec3::new(0.0, TOWER_TOP + 6.2, 5.6);
    vol.fill_sdf_solid(lo, hi, |p| tapered_capsule(p, ta, tb, 0.85, 0.6), MAT_METAL, pal::BRASS);
    vol.fill_sdf_solid(lo, hi, |p| torus(rot_x(p - tb, -0.75), 0.62, 0.16), MAT_METAL, pal::GOLD);

    // --- Lantern posts around the plaza.
    for i in 0..LANTERN_POSTS {
        let a = (i as f32 + 0.5) * TAU / LANTERN_POSTS as f32;
        let b = Vec3::new(a.cos() * LANTERN_RING_R, 0.0, a.sin() * LANTERN_RING_R);
        vol.fill_sdf_solid(b - Vec3::new(1.0, 0.2, 1.0), b + Vec3::new(1.0, 3.5, 1.0), |p| cylinder(p - b - Vec3::Y * 1.3, 0.16, 1.3), MAT_METAL, pal::BRASS);
        let l = b + Vec3::Y * 2.95;
        vol.fill_sdf_solid(l - Vec3::splat(0.6), l + Vec3::splat(0.6), |p| round_box(p - l, Vec3::new(0.32, 0.42, 0.32), 0.1), MAT_LAMP, pal::LANTERN);
        vol.fill_sdf_solid(l - Vec3::splat(0.7), l + Vec3::splat(0.9), |p| cone(p - l - Vec3::Y * 0.4, 0.5, 0.5), MAT_METAL, pal::BRONZE);
    }

    vol.compile(colors)
}

/// World positions of the plaza lanterns (for point lights), given the island centre.
pub fn lantern_positions(center: Vec3) -> Vec<Vec3> {
    (0..LANTERN_POSTS)
        .map(|i| {
            let a = (i as f32 + 0.5) * TAU / LANTERN_POSTS as f32;
            center + Vec3::new(a.cos() * LANTERN_RING_R, 2.95, a.sin() * LANTERN_RING_R)
        })
        .collect()
}

/// One armillary ring (torus in XZ with notches every 15°) centred at the origin.
pub fn build_ring(radius: f32, col: u32, notch_col: u32, colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.12;
    let half = ((radius + 0.8) / s).ceil() as i32;
    let mut vol = MicroVoxelVolume::new(half * 2, 14, half * 2, s, -half, -7, -half);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| torus(p, radius, 0.24), MAT_METAL, col);
    vol.fill_sdf_solid(lo, hi, |p| {
        let q = repeat_polar(p, 24);
        box3(q - Vec3::new(radius, 0.0, 0.0), Vec3::new(0.38, 0.34, 0.07))
    }, MAT_METAL, notch_col);
    vol.compile(colors)
}

/// Glowing core orb of the armillary sphere.
pub fn build_orb(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.12;
    let mut vol = MicroVoxelVolume::new(24, 24, 24, s, -12, -12, -12);
    let (lo, hi) = vol.local_bounds();
    vol.fill_sdf_solid(lo, hi, |p| sphere(p, 1.25), MAT_LAMP, pal::ORB);
    vol.compile(colors)
}
