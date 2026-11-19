//! 桜の帝都村 — the original Massive Micro-Voxel Sakura Empire diorama.

mod archetypes;
mod kit;
mod landmarks;
mod layout;
mod terrain;

use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

use crate::engine::prelude::*;
use terrain::river_z;

pub const NIGHT_LIGHTS: [[f32; 4]; 12] = [
    [0.0, 14.0, 174.0, 48.0],
    [0.0, 14.0, 118.0, 48.0],
    [0.0, 14.0, 82.0, 48.0],
    [0.0, 18.0, 44.0, 48.0],
    [0.0, 26.0, -28.0, 48.0],
    [-56.0, 36.0, -82.0, 48.0],
    [36.0, 36.0, -82.0, 48.0],
    [110.0, 38.0, -44.0, 48.0],
    [142.0, 13.0, 56.0, 48.0],
    [-116.0, 13.0, 6.0, 48.0],
    [14.0, 13.0, 96.0, 48.0],
    [-14.0, 13.0, 96.0, 48.0],
];

pub fn presets() -> Vec<CameraPreset> {
    vec![
    CameraPreset { name: "Overview", pos: Vec3::new(172.0, 108.0, 196.0), target: Vec3::new(0.0, 26.0, -12.0) },
    CameraPreset { name: "Town & Doors", pos: Vec3::new(4.2, 13.6, 103.0), target: Vec3::new(16.0, 12.1, 94.0) },
    CameraPreset { name: "Harbor & Boats", pos: Vec3::new(154.0, 14.5, 71.0), target: Vec3::new(137.0, 9.2, 55.0) },
    CameraPreset { name: "Grand Bridge", pos: Vec3::new(42.0, 32.0, 66.0), target: Vec3::new(0.0, 14.0, 44.0) },
    CameraPreset { name: "Watermill", pos: Vec3::new(-132.0, 20.0, 22.0), target: Vec3::new(-116.0, 11.8, 6.5) },
    CameraPreset { name: "Cliff Temple", pos: Vec3::new(148.0, 44.0, -8.0), target: Vec3::new(118.0, 32.0, -46.0) },
    CameraPreset { name: "Senbon Torii", pos: Vec3::new(-96.0, 34.0, -12.0), target: Vec3::new(-136.0, 25.0, -54.0) },
    CameraPreset { name: "Zen Courtyard", pos: Vec3::new(20.0, 38.0, -14.0), target: Vec3::new(46.0, 26.0, -54.0) },
    CameraPreset { name: "7-Tier Pagoda", pos: Vec3::new(-16.0, 62.0, -46.0), target: Vec3::new(-56.0, 52.0, -82.0) },
    ]
}

pub fn time_modes() -> [TimeModeParams; 3] {
    [
        TimeModeParams {
            name: "Day",
            sky_top: hex_to_linear_rgb(0x8bbbe5),
            sky_mid: hex_to_linear_rgb(0xd2e4f2),
            sky_bot: hex_to_linear_rgb(0xdde7f0),
            fog_col: hex_to_linear_rgb(0xdde7f0),
            fog_d: 0.00135,
            sun_col: hex_to_linear_rgb(0xfff6e5),
            sun_int: 2.35,
            sun_pos: Vec3::new(210.0, 260.0, 175.0),
            hemi_sky: hex_to_linear_rgb(0xdff0ff),
            hemi_gnd: hex_to_linear_rgb(0x6f8e58),
            hemi_int: 0.92,
            win_emissive: 0.55,
            lamp_emissive: 1.05,
            point_int: 8.0,
            exposure: 1.05,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Sunset",
            sky_top: hex_to_linear_rgb(0x3b4d7a),
            sky_mid: hex_to_linear_rgb(0xd97d68),
            sky_bot: hex_to_linear_rgb(0xf2b279),
            fog_col: hex_to_linear_rgb(0xe3a87d),
            fog_d: 0.00155,
            sun_col: hex_to_linear_rgb(0xff9a52),
            sun_int: 2.15,
            sun_pos: Vec3::new(-240.0, 85.0, 160.0),
            hemi_sky: hex_to_linear_rgb(0xffbfa0),
            hemi_gnd: hex_to_linear_rgb(0x4c4452),
            hemi_int: 0.75,
            win_emissive: 1.15,
            lamp_emissive: 1.85,
            point_int: 22.0,
            exposure: 1.02,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Lantern Night",
            sky_top: hex_to_linear_rgb(0x070b14),
            sky_mid: hex_to_linear_rgb(0x131d33),
            sky_bot: hex_to_linear_rgb(0x1a263d),
            fog_col: hex_to_linear_rgb(0x131c2e),
            fog_d: 0.00175,
            sun_col: hex_to_linear_rgb(0x8db0ff),
            sun_int: 0.68,
            sun_pos: Vec3::new(160.0, 230.0, -140.0),
            hemi_sky: hex_to_linear_rgb(0x324770),
            hemi_gnd: hex_to_linear_rgb(0x181c26),
            hemi_int: 0.55,
            win_emissive: 1.75,
            lamp_emissive: 2.65,
            point_int: 46.0,
            exposure: 1.18,
            stars: 0.0,
        },
    ]
}

/// The original 2,800 falling sakura petals.
pub fn petals() -> ParticleConfig {
    ParticleConfig {
        count: 2800,
        seed: 991177,
        bmin: Vec3::new(-200.0, 8.0, -200.0),
        bmax: Vec3::new(200.0, 74.0, 200.0),
        fall: (1.4, 2.8),
        drift: (0.7, 1.8),
        size: Vec3::new(0.26, 0.05, 0.26),
        colors: [0xffb7c5, 0xff9eb5, 0xffd1dc, 0xffc0cb],
        glow: 1.0,
    }
}

pub fn make_tour_curves() -> (CatmullRomCurve3, CatmullRomCurve3) {
    let pos_curve = CatmullRomCurve3::new(
        vec![
            Vec3::new(172.0, 108.0, 196.0),
            Vec3::new(24.0, 34.0, 186.0),
            Vec3::new(4.0, 14.2, 104.0),
            Vec3::new(0.0, 17.0, 74.0),
            Vec3::new(18.0, 22.0, 54.0),
            Vec3::new(154.0, 17.0, 72.0),
            Vec3::new(148.0, 32.0, 14.0),
            Vec3::new(132.0, 52.0, -20.0),
            Vec3::new(58.0, 36.0, -34.0),
            Vec3::new(16.0, 46.0, -50.0),
            Vec3::new(-28.0, 56.0, -54.0),
            Vec3::new(-105.0, 36.0, -24.0),
            Vec3::new(-102.0, 20.0, 18.0),
            Vec3::new(90.0, 96.0, 152.0),
        ],
        true,
        0.45,
    );
    let look_curve = CatmullRomCurve3::new(
        vec![
            Vec3::new(0.0, 26.0, -12.0),
            Vec3::new(0.0, 16.0, 132.0),
            Vec3::new(16.0, 12.2, 94.0),
            Vec3::new(0.0, 16.0, 44.0),
            Vec3::new(0.0, 14.0, 44.0),
            Vec3::new(136.0, 9.5, 56.0),
            Vec3::new(132.0, 22.0, -40.0),
            Vec3::new(110.0, 34.0, -44.0),
            Vec3::new(48.0, 22.0, -40.0),
            Vec3::new(36.0, 36.0, -82.0),
            Vec3::new(-56.0, 48.0, -82.0),
            Vec3::new(-138.0, 28.0, -62.0),
            Vec3::new(-116.0, 12.0, 6.0),
            Vec3::new(0.0, 26.0, -12.0),
        ],
        true,
        0.45,
    );
    (pos_curve, look_curve)
}

struct BoatAnim {
    base_x: f32,
    base_y: f32,
    base_z: f32,
    base_rot: f32,
    glide_range: f32,
    phase: f32,
    speed: f32,
}

#[derive(Copy, Clone)]
enum WalkAxis {
    None,
    X,
    Z,
}

struct VillagerAnim {
    base_x: f32,
    base_y: f32,
    base_z: f32,
    axis: WalkAxis,
    range: f32,
    rot_y: f32,
    phase: f32,
    speed: f32,
}

struct CloudCluster {
    base_x: f32,
    y: f32,
    z: f32,
    speed: f32,
    offsets: Vec<[f32; 3]>,
}

struct FloatingLantern {
    start_x: f32,
    speed: f32,
    phase: f32,
}

struct WfParticle {
    is_upper: bool,
    t0: f32,
    ox: f32,
    oz: f32,
    speed: f32,
    scale: f32,
}

pub struct SakuraScene {
    data: SceneData,
    boats_row: Vec<BoatAnim>,
    boats_yaka: Vec<BoatAnim>,
    villagers: [Vec<VillagerAnim>; 4],
    clouds: Vec<CloudCluster>,
    floating_lanterns: Vec<FloatingLantern>,
    wf_particles: Vec<WfParticle>,
}

pub fn build() -> Box<dyn Scene> {
    Box::new(SakuraScene::build())
}

impl Scene for SakuraScene {
    fn data(&self) -> &SceneData {
        &self.data
    }

    fn update_dynamic_instances(&self, t: f32, out: &mut Vec<ModelInstance>) {
        out.clear();
        out.reserve(self.data.dynamic_inst_count as usize);

        // 1. Watermill Wheel: parent (-116, 10, 4.5) rotY = PI/2, child local (-4.0, 1.3, 0) rotX = -t * 0.9
        let mill_mat = Mat4::from_translation(Vec3::new(-116.0, 10.0, 4.5))
            * Mat4::from_rotation_y(FRAC_PI_2)
            * Mat4::from_translation(Vec3::new(-4.0, 1.3, 0.0))
            * Mat4::from_rotation_x(-t * 0.9);
        out.push(ModelInstance::from_mat4(mill_mat));

        // 2. Boats (Rowboats, then Yakatabune)
        let push_boats = |boats: &[BoatAnim], out: &mut Vec<ModelInstance>| {
            for b in boats {
                let y = b.base_y + (t * 1.65 + b.phase).sin() * 0.07;
                let roll = (t * 1.3 + b.phase).sin() * 0.032;
                let (x, z, yaw) = if b.glide_range > 0.0 {
                    let s = (t * b.speed + b.phase).sin();
                    let ds = (t * b.speed + b.phase).cos();
                    let nx = b.base_x + s * b.glide_range;
                    let nz = river_z(nx);
                    let dzdx = river_z(nx + 0.5) - river_z(nx - 0.5);
                    let yaw = (-dzdx).atan2(1.0) + if ds >= 0.0 { 0.0 } else { PI };
                    (nx, nz, yaw)
                } else {
                    (b.base_x, b.base_z, b.base_rot)
                };
                let m = Mat4::from_translation(Vec3::new(x, y, z))
                    * Mat4::from_rotation_y(yaw)
                    * Mat4::from_rotation_z(roll);
                out.push(ModelInstance::from_mat4(m));
            }
        };
        push_boats(&self.boats_row, out);
        push_boats(&self.boats_yaka, out);

        // 3. Villagers (4 archetypes)
        for group in &self.villagers {
            for v in group {
                let (x, y, z, yaw) = match v.axis {
                    WalkAxis::Z if v.range > 0.0 => {
                        let s = (t * v.speed + v.phase).sin();
                        let ds = (t * v.speed + v.phase).cos();
                        let py = v.base_y + (t * 6.5 + v.phase).sin().abs() * 0.06;
                        (v.base_x, py, v.base_z + s * v.range, if ds >= 0.0 { 0.0 } else { PI })
                    }
                    WalkAxis::X if v.range > 0.0 => {
                        let s = (t * v.speed + v.phase).sin();
                        let ds = (t * v.speed + v.phase).cos();
                        let py = v.base_y + (t * 6.5 + v.phase).sin().abs() * 0.06;
                        (v.base_x + s * v.range, py, v.base_z, if ds >= 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 })
                    }
                    _ => {
                        let py = v.base_y + (t * 2.0 + v.phase).sin() * 0.02;
                        (v.base_x, py, v.base_z, v.rot_y)
                    }
                };
                out.push(ModelInstance::from_trs(Vec3::new(x, y, z), yaw, Vec3::ONE));
            }
        }

        // 4. Floating River Lanterns
        for fl in &self.floating_lanterns {
            let fx = (fl.start_x + t * fl.speed + 165.0).rem_euclid(330.0) - 165.0;
            let fz = river_z(fx) + (t * 0.5 + fl.phase).sin() * 2.5;
            let fy = 8.15 + (t * 2.5 + fl.phase).sin() * 0.04;
            out.push(ModelInstance::from_trs(
                Vec3::new(fx, fy, fz),
                t * 0.4 + fl.phase,
                Vec3::ONE,
            ));
        }

        // 5. Clouds
        for c in &self.clouds {
            let cx = (c.base_x + t * c.speed + 290.0).rem_euclid(580.0) - 290.0;
            for &[ox, oy, oz] in &c.offsets {
                out.push(ModelInstance::from_trs(
                    Vec3::new(cx + ox, c.y + oy, c.z + oz),
                    0.0,
                    Vec3::ONE,
                ));
            }
        }

        // 6. Waterfall Mist & Spray Particles
        for w in &self.wf_particles {
            let wt = (w.t0 + t * w.speed).fract();
            let (py, pz) = if w.is_upper {
                let pz = -84.0 + wt * 20.0;
                let py = 64.0 - wt * 28.0 + (wt * PI).sin() * 1.4;
                (py, pz)
            } else {
                let pz = -48.0 + wt * 20.0;
                let py = 36.0 - wt * 27.5 + if wt > 0.85 { (wt - 0.85) * 18.0 } else { 0.0 };
                (py, pz)
            };
            let px = 132.0 + 2.5 * (pz * 0.14).sin() + w.ox * (0.4 + wt * 0.8);
            let puff = w.scale * (0.6 + if wt > 0.82 { (wt - 0.82) * 8.0 } else { 0.0 });
            let z_off = w.oz * if wt > 0.8 { 1.5 } else { 0.4 };
            out.push(ModelInstance::from_trs(
                Vec3::new(px, py, pz + z_off),
                0.0,
                Vec3::splat(puff),
            ));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sakura_scene_is_consistent() {
        let scene = SakuraScene::build();
        let d = scene.data();
        assert!(d.total_voxels > 1_000_000);
        assert!(d.total_tris > 1_000_000);
        assert_eq!(d.presets.len(), 9);
        assert_eq!(d.translucent_batches.len(), 3);
        crate::engine::scene::assert_scene_consistent(&scene);
    }
}
