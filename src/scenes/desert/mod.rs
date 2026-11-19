//! Desert Canyon & Ruins: a round diorama of sandstone mesas, a natural arch,
//! a ruined sun temple, a dry riverbed ending in a palm oasis, and dunes.
//!
//! Everything is generated from math: the terrain is one terraced, domain-
//! warped height-field with strata colouring; the arch is an eroded SDF fin;
//! the temple is a composite SDF with polar-repeated columns; plants are
//! swept capsules; placement is Poisson-disc sampling around a seeded plan.

mod arch;
mod effects;
mod flora;
mod layout;
mod pal;
mod plan;
mod props;
mod ruins;
mod terrain;

use glam::Vec3;

use crate::engine::prelude::*;

pub struct DesertCanyon {
    data: SceneData,
    /// The generated height-field (for picking and experiences).
    pub terrain: terrain::Terrain,
    pub params: WorldParams,
    vultures: Vec<effects::Vulture>,
    flames: Vec<(Vec3, f32)>,
}

pub fn build(params: &WorldParams) -> Box<dyn Scene> {
    Box::new(DesertCanyon::build(params))
}

impl Scene for DesertCanyon {
    fn data(&self) -> &SceneData {
        &self.data
    }

    fn surface_y(&self, x: f32, z: f32) -> Option<f32> {
        self.terrain.surface(x, z)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn update_dynamic_instances(&self, t: f32, out: &mut Vec<ModelInstance>) {
        out.clear();
        out.reserve(self.data.dynamic_inst_count as usize);
        // Order must match the add_dynamic calls in layout.rs.
        effects::push_vultures(&self.vultures, t, out);
        effects::push_flames(&self.flames, t, out);
    }
}

pub fn time_modes(winter: bool) -> [TimeModeParams; 3] {
    let c = |day: u32, snow: u32| hex_to_linear_rgb(if winter { snow } else { day });
    [
        TimeModeParams {
            name: "Day",
            sky_top: c(0x3d7fd6, 0x6f95c8),
            sky_mid: c(0x92c3ee, 0xb4cbe4),
            sky_bot: c(0xd6e6f2, 0xe6ebf0),
            fog_col: c(0xe6e2d6, 0xe2e8ee),
            fog_d: 0.0013,
            sun_col: c(0xfff1d8, 0xfff6ea),
            sun_int: 2.5,
            sun_pos: Vec3::new(230.0, 300.0, 150.0),
            hemi_sky: c(0xd2e4f6, 0xdce8f6),
            hemi_gnd: c(0xd9a476, 0xc8ccd2),
            hemi_int: 0.95,
            win_emissive: 0.4,
            lamp_emissive: 0.9,
            point_int: 4.0,
            exposure: 1.0,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Golden Hour",
            sky_top: c(0x34508f, 0x3c5288),
            sky_mid: c(0xee9a5c, 0xe0a07a),
            sky_bot: c(0xffcf8e, 0xffd8b0),
            fog_col: c(0xf0b27a, 0xe8bc98),
            fog_d: 0.0017,
            sun_col: c(0xffa052, 0xffb070),
            sun_int: 2.4,
            sun_pos: Vec3::new(-320.0, 105.0, 190.0),
            hemi_sky: c(0xffc09a, 0xf2c8b0),
            hemi_gnd: c(0x9a6a58, 0x8a7a80),
            hemi_int: 0.78,
            win_emissive: 1.1,
            lamp_emissive: 1.7,
            point_int: 8.0,
            exposure: 1.03,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Desert Night",
            sky_top: c(0x040714, 0x050a18),
            sky_mid: c(0x0d1734, 0x111d3c),
            sky_bot: c(0x24305a, 0x2a3862),
            fog_col: c(0x161f3e, 0x1a2444),
            fog_d: 0.0016,
            sun_col: c(0x9fb6ff, 0xb0c4ff),
            sun_int: 0.7,
            sun_pos: Vec3::new(180.0, 290.0, -170.0),
            hemi_sky: c(0x3a4c80, 0x46588a),
            hemi_gnd: c(0x3a3040, 0x30384a),
            hemi_int: 0.58,
            win_emissive: 1.8,
            lamp_emissive: 2.8,
            point_int: 17.0,
            exposure: 1.2,
            stars: 1.5,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(params: WorldParams) -> DesertCanyon {
        let scene = DesertCanyon::build(&params);
        let d = scene.data();
        assert!(d.total_voxels > 1_000_000, "{}", d.total_voxels);
        assert!(d.total_tris < 3_200_000, "too many triangles: {}", d.total_tris);
        assert!((6..=12).contains(&d.landmarks.len()), "{} landmarks", d.landmarks.len());
        assert!((4..=9).contains(&d.presets.len()));
        crate::engine::scene::assert_scene_consistent(&scene);
        // Every landmark and camera sits over the plinth and above the ground.
        for l in &d.landmarks {
            let g = scene.surface_y(l.pos.x, l.pos.z).unwrap_or(f32::MIN);
            assert!(l.pos.y >= g - 0.5, "{} is underground", l.name);
        }
        for p in &d.presets {
            if let Some(g) = scene.surface_y(p.pos.x, p.pos.z) {
                assert!(p.pos.y > g + 1.0, "camera {} is underground", p.name);
            }
        }
        scene
    }

    #[test]
    fn desert_seed0_is_consistent() {
        check(WorldParams::default());
    }

    #[test]
    fn desert_other_seeds_are_consistent_and_differ() {
        let a = check(WorldParams { seed: 7, season: Season::Summer });
        let b = check(WorldParams { seed: 1234, season: Season::Winter });
        let c = DesertCanyon::build(&WorldParams::default());
        assert_ne!(a.data().total_tris, c.data().total_tris);
        assert_ne!(b.data().total_tris, c.data().total_tris);
    }

    #[test]
    fn desert_features_sit_on_open_floor() {
        for seed in [0u32, 3, 7, 99] {
            let plan = plan::Plan::new(seed, false);
            let t = terrain::Terrain::new(plan);
            let p = &t.plan;
            for (name, at) in [("ruins", p.ruins), ("camp", p.camp), ("arch", p.arch)] {
                let c = t.column(at.x, at.y);
                assert!(c.kind != terrain::Kind::Out && c.kind != terrain::Kind::Mesa, "seed {seed}: {name} not on the floor");
                assert!(c.top.abs() < 1.5, "seed {seed}: {name} floor at {}", c.top);
            }
            assert_eq!(t.column(p.oasis.x, p.oasis.y).kind, terrain::Kind::Lakebed, "seed {seed}: oasis has no water");
        }
    }
}
