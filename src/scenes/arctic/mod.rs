//! Aurora Village — a snowy valley ringed by mountains around an organic
//! frozen lake: an igloo camp around a campfire, timber cabins with warm
//! windows and smoking chimneys, a spruce forest, icebergs locked in the ice,
//! and the northern lights rippling across the night sky.

mod aurora;
mod cabin;
mod effects;
mod ice;
mod igloo;
mod layout;
mod pal;
mod pine;
mod terrain;

use glam::Vec3;

use crate::engine::prelude::*;
use aurora::AuroraInst;
use effects::{Flame, Puff};
use layout::{N_BERG_KINDS, STYLES};
use terrain::Terrain;

pub struct Arctic {
    data: SceneData,
    pub params: WorldParams,
    pub terrain: Terrain,
    aurora: Vec<AuroraInst>,
    flames: Vec<Flame>,
    smoke: Vec<Puff>,
}

pub fn build(params: &WorldParams) -> Box<dyn Scene> {
    Box::new(Arctic::build(params))
}

impl Scene for Arctic {
    fn data(&self) -> &SceneData {
        &self.data
    }

    fn surface_y(&self, x: f32, z: f32) -> Option<f32> {
        let h = terrain::WORLD_HALF;
        (x.abs() < h && z.abs() < h).then(|| self.terrain.surface_y(x, z))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn update_dynamic_instances(&self, t: f32, out: &mut Vec<ModelInstance>) {
        out.clear();
        out.reserve(self.data.dynamic_inst_count as usize);
        // Order matches the add_dynamic calls in `Arctic::build`.
        effects::push_flames(&self.flames, t, out);
        effects::push_smoke(&self.smoke, t, out);
        aurora::push_aurora(&self.aurora, t, out);
    }
}

/// Snow amount on trees / roofs for a season (0 = none).
fn snow_amount(season: Season) -> f32 {
    match season {
        Season::Winter => 0.75,
        Season::Spring => 0.5,
        Season::Autumn => 0.3,
        Season::Summer => 0.0,
    }
}

impl Arctic {
    pub fn build(params: &WorldParams) -> Self {
        let plan = layout::plan(params);
        let season = params.season;
        let snow = snow_amount(season);
        let snowy = snow > 0.2;
        let t = &plan.terrain;

        enum Job {
            Terrain,
            Lake,
            Cabin(usize),
            Igloo,
            FirePit,
            Flame,
            Lamp,
            Berg(usize),
            Pine(usize, f32),
            Curtain(usize),
        }
        let mut jobs = vec![Job::Terrain, Job::Lake, Job::Igloo, Job::FirePit, Job::Flame, Job::Lamp];
        jobs.extend((0..STYLES.len()).map(Job::Cabin));
        jobs.extend((0..N_BERG_KINDS).map(Job::Berg));
        for k in 0..pine::SHAPES.len() {
            jobs.push(Job::Pine(k, 0.4));
            jobs.push(Job::Pine(k, 0.75));
        }
        jobs.extend((0..plan.curtains.len()).map(Job::Curtain));
        let outs = par_map(&jobs, |job| {
            let mut cc = ColorCache::new();
            match job {
                Job::Terrain => (t.build_mesh(&mut cc), Vec3::ZERO),
                Job::Lake => (t.build_lake(&mut cc), Vec3::ZERO),
                Job::Cabin(i) => {
                    let b = cabin::build_cabin(&STYLES[*i], snowy, 31 + *i as u32, &mut cc);
                    ((b.mesh, b.voxels), b.smoke)
                }
                Job::Igloo => (igloo::build_igloo(5, &mut cc), Vec3::ZERO),
                Job::FirePit => (igloo::build_fire_pit(9, &mut cc), Vec3::ZERO),
                Job::Flame => (igloo::build_flame(&mut cc), Vec3::ZERO),
                Job::Lamp => (cabin::build_lamp_post(snowy, &mut cc), Vec3::ZERO),
                Job::Berg(k) => (ice::build_berg(300 + *k as u32 * 17, *k % 2 == 0, &mut cc), Vec3::ZERO),
                Job::Pine(k, s) => (pine::build_pine(&pine::SHAPES[*k], 800 + *k as u32, *s, snow, &mut cc), Vec3::ZERO),
                Job::Curtain(k) => (aurora::build_curtain(&plan.curtains[*k], &mut cc), Vec3::ZERO),
            }
        });
        let mut it = outs.into_iter();
        let mut next = || it.next().expect("job order");

        let mut sb = SceneBuilder::new();
        let ((m, v), _) = next();
        sb.add_mesh(&m, v, &[ModelInstance::identity()], Pass::Opaque);
        let lake = next();
        let ((m, v), _) = next();
        let igloo_inst: Vec<ModelInstance> = plan.igloos.iter().map(|p| p.inst()).collect();
        sb.add_mesh(&m, v, &igloo_inst, Pass::Opaque);
        let ((m, v), _) = next();
        sb.add_mesh(&m, v, &[ModelInstance::from_trs(plan.camp, 0.3, Vec3::ONE)], Pass::Opaque);
        let flame = next();
        let ((m, v), _) = next();
        let lamp_inst: Vec<ModelInstance> = plan.lamps.iter().map(|p| p.inst()).collect();
        sb.add_mesh(&m, v, &lamp_inst, Pass::Opaque);
        let mut smoke_local = Vec::new();
        for style in 0..STYLES.len() {
            let ((m, v), smoke) = next();
            smoke_local.push(smoke);
            let list: Vec<ModelInstance> = plan.buildings.iter().filter(|b| b.style == style).map(|b| b.at.inst()).collect();
            sb.add_mesh(&m, v, &list, Pass::Opaque);
        }
        for k in 0..N_BERG_KINDS {
            let ((m, v), _) = next();
            let list: Vec<ModelInstance> = plan.bergs[k].iter().map(|p| p.inst()).collect();
            sb.add_mesh(&m, v, &list, Pass::Opaque);
        }
        for k in 0..pine::SHAPES.len() {
            let ((fine, fv), _) = next();
            let ((coarse, _), _) = next();
            sb.total_voxels += fv;
            let lods = vec![(sb.push_mesh(&fine), 120.0), (sb.push_mesh(&coarse), f32::MAX)];
            let list: Vec<ModelInstance> = plan.pines[k].iter().map(|p| p.inst()).collect();
            sb.add_detail(lods, list, 8.0);
        }
        // Translucent lake surface after the opaque world.
        sb.add_mesh(&lake.0 .0, lake.0 .1, &[ModelInstance::identity()], Pass::Translucent);

        // Dynamic batches: order must match update_dynamic_instances.
        let (smoke, flames) = layout::smoke_and_flames(&plan, &smoke_local);
        sb.total_voxels += flame.0 .1;
        let span = sb.push_mesh(&flame.0 .0);
        sb.add_dynamic(span, flames.len(), Pass::Opaque);
        let (puff, pv) = effects::build_puff(&mut ColorCache::new());
        sb.total_voxels += pv;
        let span = sb.push_mesh(&puff);
        sb.add_dynamic(span, smoke.len(), Pass::Translucent);
        for _ in 0..plan.curtains.len() {
            let ((m, v), _) = next();
            sb.total_voxels += v;
            let span = sb.push_mesh(&m);
            sb.add_dynamic(span, 1, Pass::Translucent);
        }

        let landmarks = layout::landmarks(&plan);
        let presets = layout::presets(&plan, &landmarks);
        let center = Vec3::new(t.lake_c.x, 0.0, t.lake_c.y + 20.0);
        let (tour_pos, tour_look) = layout::tour(&presets, center);
        let lights = layout::lights(&plan);
        let geo = sb.finish();
        let data = SceneData {
            title: "Aurora Village".to_string(),
            vertices: geo.vertices,
            indices: geo.indices,
            static_instances: geo.static_instances,
            opaque_batches: geo.opaque_batches,
            translucent_batches: geo.translucent_batches,
            dynamic_inst_start: geo.dynamic_inst_start,
            dynamic_inst_count: geo.dynamic_inst_count,
            total_voxels: geo.total_voxels,
            total_tris: geo.total_tris,
            detail: geo.detail,
            presets,
            tour_pos,
            tour_look,
            tour_secs: 100.0,
            max_cam_dist: 300.0,
            min_pitch: -1.2,
            point_lights: lights,
            point_light_col: [1.0, 0.6, 0.28],
            time_modes: time_modes(),
            shadow: ShadowConfig { target: Vec3::new(0.0, 10.0, 0.0), half_extent: 165.0, near: 20.0, far: 900.0 },
            particles: particles(season),
            landmarks,
        };
        Arctic { data, params: *params, terrain: plan.terrain, aurora: plan.aurora, flames, smoke }
    }
}

pub fn time_modes() -> [TimeModeParams; 3] {
    [
        TimeModeParams {
            name: "Polar Day",
            sky_top: hex_to_linear_rgb(0x4f8fd4),
            sky_mid: hex_to_linear_rgb(0xa8cbea),
            sky_bot: hex_to_linear_rgb(0xdce9f5),
            // Fog matches the mid sky so the distant aurora curtains vanish by day.
            fog_col: hex_to_linear_rgb(0xaccdeb),
            fog_d: 0.0022,
            sun_col: hex_to_linear_rgb(0xfff3e2),
            sun_int: 2.1,
            sun_pos: Vec3::new(220.0, 200.0, 260.0),
            hemi_sky: hex_to_linear_rgb(0xd2e6fa),
            hemi_gnd: hex_to_linear_rgb(0xb7c4d4),
            hemi_int: 0.85,
            win_emissive: 0.35,
            lamp_emissive: 0.4,
            point_int: 3.0,
            exposure: 0.94,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Alpenglow",
            sky_top: hex_to_linear_rgb(0x34497e),
            sky_mid: hex_to_linear_rgb(0xd88aa0),
            sky_bot: hex_to_linear_rgb(0xffc59a),
            fog_col: hex_to_linear_rgb(0xcf8ea4),
            fog_d: 0.0026,
            sun_col: hex_to_linear_rgb(0xff9f6a),
            sun_int: 2.0,
            sun_pos: Vec3::new(-320.0, 70.0, 160.0),
            hemi_sky: hex_to_linear_rgb(0xb8a8d8),
            hemi_gnd: hex_to_linear_rgb(0x8a7890),
            hemi_int: 0.8,
            win_emissive: 1.2,
            lamp_emissive: 1.4,
            point_int: 8.0,
            exposure: 1.02,
            stars: 0.0,
        },
        TimeModeParams {
            name: "Aurora Night",
            sky_top: hex_to_linear_rgb(0x02050e),
            sky_mid: hex_to_linear_rgb(0x08142a),
            sky_bot: hex_to_linear_rgb(0x123044),
            fog_col: hex_to_linear_rgb(0x0b1a2c),
            fog_d: 0.0004,
            sun_col: hex_to_linear_rgb(0x9ab8f0),
            sun_int: 0.55,
            sun_pos: Vec3::new(-150.0, 260.0, 220.0),
            hemi_sky: hex_to_linear_rgb(0x3a5a7a),
            hemi_gnd: hex_to_linear_rgb(0x1e3040),
            hemi_int: 0.6,
            win_emissive: 2.0,
            lamp_emissive: 2.6,
            point_int: 14.0,
            exposure: 1.2,
            stars: 1.5,
        },
    ]
}

/// Falling snow (summer: a light drift of cotton-grass fluff instead).
fn particles(season: Season) -> ParticleConfig {
    let h = terrain::WORLD_HALF;
    let summer = season == Season::Summer;
    ParticleConfig {
        count: if summer { 500 } else { 2600 },
        seed: 7_2222,
        bmin: Vec3::new(-h, 2.0, -h),
        bmax: Vec3::new(h, 80.0, h),
        fall: if summer { (0.3, 0.6) } else { (1.6, 3.0) },
        drift: (0.4, 1.4),
        size: Vec3::splat(if summer { 0.1 } else { 0.16 }),
        colors: [0xffffff, 0xeef4fa, 0xdce8f2, 0xf8fcff],
        glow: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arctic_scene_is_consistent_across_seeds() {
        for seed in [0, 3, 77] {
            let scene = Arctic::build(&WorldParams { seed, season: Season::Winter });
            let d = scene.data();
            assert!(d.total_tris < 3_000_000, "seed {seed}: {} tris", d.total_tris);
            assert!((6..=12).contains(&d.landmarks.len()), "seed {seed}: {} landmarks", d.landmarks.len());
            assert!((4..=9).contains(&d.presets.len()));
            crate::engine::scene::assert_scene_consistent(&scene);
        }
    }

    #[test]
    fn non_zero_seed_varies_the_layout() {
        let a = Arctic::build(&WorldParams { seed: 0, season: Season::Winter });
        let b = Arctic::build(&WorldParams { seed: 5, season: Season::Winter });
        assert_ne!(a.data().landmarks[1].pos, b.data().landmarks[1].pos);
        assert_ne!(a.terrain.lake_c, b.terrain.lake_c);
    }

    #[test]
    fn summer_changes_snow_cover() {
        let winter = Terrain::new(0, Season::Winter);
        let summer = Terrain::new(0, Season::Summer);
        let frac = |t: &Terrain| {
            let mut n = 0;
            for iz in -20..20 {
                for ix in -20..20 {
                    let (x, z) = (ix as f32 * 7.0, iz as f32 * 7.0);
                    if t.lake_d(x, z) > 0.0 && t.snowy(x, z, t.height(x, z), t.slope(x, z)) {
                        n += 1;
                    }
                }
            }
            n
        };
        assert!(frac(&winter) > frac(&summer) * 2, "{} vs {}", frac(&winter), frac(&summer));
        let w = Arctic::build(&WorldParams { seed: 0, season: Season::Winter });
        let s = Arctic::build(&WorldParams { seed: 0, season: Season::Summer });
        assert_ne!(w.data().vertices.len(), s.data().vertices.len());
    }

    #[test]
    fn buildings_and_camp_sit_on_dry_land() {
        for seed in [0, 3, 77] {
            let scene = Arctic::build(&WorldParams { seed, season: Season::Winter });
            let lm = &scene.data().landmarks;
            for name in ["Village Square", "The Lodge", "Igloo Camp"] {
                let p = lm.iter().find(|l| l.name == name).unwrap().pos;
                assert!(scene.terrain.lake_d(p.x, p.z) > 8.0, "seed {seed}: {name} too close to the lake");
            }
        }
    }
}
