//! Sky Islands — a floating archipelago above a sea of clouds.
//!
//! Every shape is generated from math: islands are height-field functions with
//! paraboloid undersides, buildings are composite SDFs, bridges are catenaries,
//! trees grow recursively, waterfalls follow ballistic arcs.

mod bridges;
mod effects;
mod flora;
mod flowers;
mod island;
mod layout;
mod observatory;
mod pal;

use glam::Vec3;

use crate::engine::prelude::*;
use effects::{Bird, Islet, Puff, Spray};

pub struct SkyIslands {
    data: SceneData,
    armillary: Vec3,
    islets: [Vec<Islet>; 3],
    birds: Vec<Bird>,
    clouds: Vec<Puff>,
    spray: Vec<Spray>,
}

pub fn build() -> Box<dyn Scene> {
    Box::new(SkyIslands::build())
}

impl Scene for SkyIslands {
    fn data(&self) -> &SceneData {
        &self.data
    }

    fn update_dynamic_instances(&self, t: f32, out: &mut Vec<ModelInstance>) {
        out.clear();
        out.reserve(self.data.dynamic_inst_count as usize);
        // Order must match the add_dynamic calls in layout.rs.
        effects::push_armillary(self.armillary, t, out);
        for group in &self.islets {
            effects::push_islets(group, t, out);
        }
        effects::push_birds(&self.birds, t, out);
        effects::push_clouds(&self.clouds, t, out);
        effects::push_spray(&self.spray, t, out);
    }
}

pub fn time_modes() -> [TimeModeParams; 3] {
    [
        TimeModeParams {
            name: "Day",
            sky_top: hex_to_linear_rgb(0x5d9ee8),
            sky_mid: hex_to_linear_rgb(0xa9d2f5),
            sky_bot: hex_to_linear_rgb(0xe4f1fb),
            fog_col: hex_to_linear_rgb(0xd9ebf8),
            fog_d: 0.0011,
            sun_col: hex_to_linear_rgb(0xfff4e0),
            sun_int: 2.4,
            sun_pos: Vec3::new(260.0, 320.0, 180.0),
            hemi_sky: hex_to_linear_rgb(0xdcefff),
            hemi_gnd: hex_to_linear_rgb(0xb4c6d6),
            hemi_int: 0.95,
            win_emissive: 0.4,
            lamp_emissive: 0.9,
            point_int: 6.0,
            exposure: 1.05,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Golden Hour",
            sky_top: hex_to_linear_rgb(0x3e5a9a),
            sky_mid: hex_to_linear_rgb(0xf0a070),
            sky_bot: hex_to_linear_rgb(0xffd2a0),
            fog_col: hex_to_linear_rgb(0xf2c49a),
            fog_d: 0.0017,
            sun_col: hex_to_linear_rgb(0xffa860),
            sun_int: 2.2,
            sun_pos: Vec3::new(-330.0, 120.0, 210.0),
            hemi_sky: hex_to_linear_rgb(0xffc8a8),
            hemi_gnd: hex_to_linear_rgb(0x9a7f8f),
            hemi_int: 0.8,
            win_emissive: 1.1,
            lamp_emissive: 1.6,
            point_int: 8.0,
            exposure: 1.03,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Starry Night",
            sky_top: hex_to_linear_rgb(0x050814),
            sky_mid: hex_to_linear_rgb(0x0f1a36),
            sky_bot: hex_to_linear_rgb(0x1d2b52),
            fog_col: hex_to_linear_rgb(0x14203d),
            fog_d: 0.0017,
            sun_col: hex_to_linear_rgb(0x9bb8ff),
            sun_int: 0.75,
            sun_pos: Vec3::new(200.0, 300.0, -180.0),
            hemi_sky: hex_to_linear_rgb(0x3a4f80),
            hemi_gnd: hex_to_linear_rgb(0x2a3550),
            hemi_int: 0.6,
            win_emissive: 1.8,
            lamp_emissive: 2.6,
            point_int: 16.0,
            exposure: 1.2,
            stars: 1.2,
        },
    ]
}

/// Leaves and blossom petals drifting down off the islands.
pub fn leaves() -> ParticleConfig {
    ParticleConfig {
        count: 1600,
        seed: 4242,
        bmin: Vec3::new(-200.0, 12.0, -200.0),
        bmax: Vec3::new(200.0, 125.0, 200.0),
        fall: (0.8, 1.8),
        drift: (0.4, 1.2),
        size: Vec3::new(0.3, 0.06, 0.3),
        colors: [0xf7a8c8, 0xf2c55a, 0x8fd16a, 0xfff4f8],
        glow: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout::{plan_bridges, plan_islands};

    #[test]
    fn islands_do_not_overlap() {
        let isl = plan_islands();
        for i in 0..isl.len() {
            for j in i + 1..isl.len() {
                let d = (isl[i].center - isl[j].center).with_y(0.0).length();
                let min = (isl[i].radius + isl[j].radius) * 1.3 + 4.0;
                assert!(d > min, "{} and {} overlap: {d} < {min}", isl[i].name, isl[j].name);
            }
        }
    }

    #[test]
    fn islands_hang_clear_of_the_cloud_puffs() {
        let mut rng = Rng(1);
        for spec in plan_islands() {
            let st = spec.plan_stalactites(&mut rng);
            let (vol, _) = spec.voxelize(&st);
            let lowest = (vol.oy..vol.oy + vol.ny)
                .find(|&y| (vol.oz..vol.oz + vol.nz).any(|z| (vol.ox..vol.ox + vol.nx).any(|x| vol.get(x, y, z) != 0)))
                .unwrap();
            let world_y = spec.center.y + lowest as f32 * vol.scale;
            assert!(world_y > effects::PUFF_MAX_Y, "{} hangs down to {world_y}", spec.name);
        }
    }

    #[test]
    fn every_island_is_reachable_by_bridge() {
        let isl = plan_islands();
        let br = plan_bridges(&isl);
        assert_eq!(br.len(), isl.len() - 1, "some bridges were rejected: {}", br.len());
    }

    #[test]
    fn sky_islands_scene_is_consistent() {
        let scene = SkyIslands::build();
        let d = scene.data();
        assert!(d.total_voxels > 1_000_000, "{}", d.total_voxels);
        crate::engine::scene::assert_scene_consistent(&scene);
    }
}
