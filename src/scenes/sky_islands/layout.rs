//! World plan for the Sky Islands: island placement on a golden-angle spiral,
//! bridges as a nearest-inward-neighbour tree, flora by Poisson-disc sampling.

use glam::Vec3;
use std::f32::consts::{PI, TAU};

use super::bridges::BridgeSpec;
use super::effects::{self, Islet};
use super::flora::build_flora;
use super::island::{islet_spec, IslandSpec, Lake};
use super::observatory::{self, ARMILLARY_Y};
use super::pal;
use super::*;

pub const HERO: usize = 0;
pub const SPIRIT: usize = 1;

#[derive(Clone, Copy, PartialEq)]
enum Grove {
    Observatory,
    Spirit,
    Oaks,
    Blossoms,
    Golds,
    Pines,
    Mixed,
}

struct Role {
    name: &'static str,
    grove: Grove,
    grass: [u32; 3],
    crystal: u32,
    waterfall: bool,
    hilliness: f32,
}

const ROLES: [Role; 6] = [
    Role { name: "Spirit Tree Isle", grove: Grove::Spirit, grass: pal::GRASS_MINT, crystal: pal::CRYSTAL_VIOLET, waterfall: false, hilliness: 1.6 },
    Role { name: "Waterfall Meadow", grove: Grove::Oaks, grass: pal::GRASS_MEADOW, crystal: pal::CRYSTAL_CYAN, waterfall: true, hilliness: 1.8 },
    Role { name: "Blossom Isle", grove: Grove::Blossoms, grass: pal::GRASS_SPRING, crystal: pal::CRYSTAL_VIOLET, waterfall: true, hilliness: 1.4 },
    Role { name: "Amber Isle", grove: Grove::Golds, grass: pal::GRASS_AUTUMN, crystal: pal::CRYSTAL_AMBER, waterfall: false, hilliness: 2.2 },
    Role { name: "Pine Crag", grove: Grove::Pines, grass: pal::GRASS_MEADOW, crystal: pal::CRYSTAL_CYAN, waterfall: false, hilliness: 3.6 },
    Role { name: "Twin Falls", grove: Grove::Mixed, grass: pal::GRASS_MINT, crystal: pal::CRYSTAL_AMBER, waterfall: true, hilliness: 2.0 },
];

/// Pure-math placement: the hero at the origin, the rest on a golden-angle spiral.
pub fn plan_islands() -> Vec<IslandSpec> {
    let golden = PI * (3.0 - 5f32.sqrt());
    let mut v = vec![IslandSpec {
        name: "Observatory Isle",
        center: Vec3::new(0.0, 72.0, 0.0),
        radius: 36.0,
        depth: 32.0,
        seed: 0.37,
        plaza: observatory::PLAZA_R + 1.5,
        hilliness: 1.5,
        lake: None,
        grass: pal::GRASS_SPRING,
        crystal: pal::CRYSTAL_CYAN,
        paths: Vec::new(),
    }];
    for (i, role) in ROLES.iter().enumerate() {
        let fi = i as f32;
        let a = 0.6 + fi * golden;
        let d = 96.0 + 13.0 * fi;
        let y = 76.0 + 22.0 * (fi * 1.7 + 0.4).sin();
        let radius = 26.0 - 1.4 * fi;
        v.push(IslandSpec {
            name: role.name,
            center: Vec3::new(a.cos() * d, y, a.sin() * d),
            radius,
            depth: radius * 1.05,
            seed: 1.13 + fi * 0.71,
            plaza: 0.0,
            hilliness: role.hilliness,
            // Lakes sit on the outward side so their falls pour away from the hub.
            lake: role.waterfall.then_some(Lake { angle: a + 0.35, dist: 0.42, rx: 6.5, rz: 5.0, depth: 1.6, waterfall: true }),
            grass: role.grass,
            crystal: role.crystal,
            paths: Vec::new(),
        });
    }
    v
}

/// Each outer island links to its nearest neighbour that is closer to the hub.
pub fn plan_bridges(islands: &[IslandSpec]) -> Vec<(usize, usize, BridgeSpec)> {
    let hub_dist = |s: &IslandSpec| Vec3::new(s.center.x, 0.0, s.center.z).length();
    let mut out = Vec::new();
    for i in 1..islands.len() {
        let ci = islands[i].center;
        let j = (0..islands.len())
            .filter(|&j| j != i && hub_dist(&islands[j]) < hub_dist(&islands[i]))
            .min_by(|&a, &b| {
                let da = (islands[a].center - ci).with_y(0.0).length();
                let db = (islands[b].center - ci).with_y(0.0).length();
                da.total_cmp(&db)
            })
            .unwrap();
        let a = islands[i].bridge_anchor(islands[j].center);
        let b = islands[j].bridge_anchor(ci);
        let l = (b - a).with_y(0.0).length();
        if l < 8.0 || l > 85.0 || (b.y - a.y).abs() / l > 0.55 {
            continue;
        }
        out.push((i, j, BridgeSpec { a, b, sag: 1.5 + l * 0.06 }));
    }
    out
}

fn heading(from: Vec3, to: Vec3) -> f32 {
    (to.z - from.z).atan2(to.x - from.x)
}

impl SkyIslands {
    pub fn build() -> Self {
        let mut rng = Rng(7_2026);
        let mut sb = SceneBuilder::new();

        // ---- Plan (pure math) ------------------------------------------------
        let mut islands = plan_islands();
        let bridges = plan_bridges(&islands);
        for (i, j, _) in &bridges {
            let (ci, cj) = (islands[*i].center, islands[*j].center);
            islands[*i].paths.push(heading(ci, cj));
            islands[*j].paths.push(heading(cj, ci));
        }

        // ---- Random planning (sequential: keeps the exact same RNG stream) -----
        let stalactites: Vec<_> = islands.iter().map(|spec| spec.plan_stalactites(&mut rng)).collect();
        let mut lips = Vec::new();
        for spec in &islands {
            if let Some((lip, dir)) = spec.waterfall_lip() {
                lips.push((spec.center + lip, dir));
            }
        }
        // Tree placement (indices into the flora mesh list built below).
        let (oak0, blossom0, gold0, pine0, n_trees) = (0usize, 3usize, 5usize, 7usize, 9usize);
        let mut buckets: Vec<Vec<ModelInstance>> = vec![Vec::new(); n_trees];
        for (k, spec) in islands.iter().enumerate() {
            let grove = if k == HERO { Grove::Observatory } else { ROLES[k - 1].grove };
            let (min_d, keep_out, pool): (f32, f32, Vec<usize>) = match grove {
                Grove::Observatory => (8.0, observatory::PLAZA_R + 3.0, vec![oak0, oak0 + 1, blossom0]),
                Grove::Spirit => (9.0, 13.0, vec![blossom0, blossom0 + 1]),
                Grove::Oaks => (6.5, 0.0, vec![oak0, oak0 + 1, oak0 + 2]),
                Grove::Blossoms => (6.0, 0.0, vec![blossom0, blossom0 + 1, oak0 + 2]),
                Grove::Golds => (5.5, 0.0, vec![gold0, gold0 + 1, oak0]),
                Grove::Pines => (5.0, 0.0, vec![pine0, pine0 + 1]),
                Grove::Mixed => (6.0, 0.0, vec![oak0 + 1, pine0, gold0]),
            };
            for p in spec.scatter(&mut rng, min_d, 260, 0.82, keep_out) {
                let pick = pool[(rng.next_f32() * pool.len() as f32) as usize % pool.len()];
                let sc = 0.8 + rng.next_f32() * 0.45;
                buckets[pick].push(ModelInstance::from_trs(p, rng.next_f32() * TAU, Vec3::splat(sc)));
            }
        }
        // Bobbing islets: golden spiral, rejecting spots near real islands.
        let mut islets: [Vec<Islet>; 3] = Default::default();
        let n = 60;
        for i in 0..n {
            let (x, z) = golden_spiral(i, n, 230.0);
            let clear = islands.iter().all(|s| (Vec3::new(x, 0.0, z) - s.center.with_y(0.0)).length() > s.radius * 1.4 + 10.0);
            if !clear || (x * x + z * z).sqrt() < 50.0 {
                continue;
            }
            islets[i % 3].push(Islet {
                base: Vec3::new(x, 34.0 + hash2(x, z) * 90.0, z),
                phase: rng.next_f32() * TAU,
                spin: (rng.next_f32() - 0.5) * 0.08,
                scale: 0.8 + rng.next_f32() * 0.5,
            });
        }
        let islet_specs: Vec<IslandSpec> =
            [3.0f32, 4.2, 5.6].iter().enumerate().map(|(k, r)| islet_spec(10.0 + k as f32 * 3.3, *r)).collect();
        let islet_stal: Vec<_> = islet_specs.iter().map(|s| s.plan_stalactites(&mut rng)).collect();
        let birds = effects::make_birds(islands[HERO].center, 18, &mut rng);
        let clouds = effects::make_clouds(&mut rng);
        let spray = effects::make_spray(&lips, 90, &mut rng);

        // ---- Heavy geometry, generated in parallel ------------------------------
        let compile = |v: MicroVoxelVolume| -> (MeshData, u64) { v.compile(&mut ColorCache::new()) };
        let hero = &islands[HERO];
        let (island_meshes, sea, obs, bridge_meshes, kit, rings, orb, islet_meshes, bird) = std::thread::scope(|sc| {
            let isl = sc.spawn(|| {
                let jobs: Vec<usize> = (0..islands.len()).collect();
                par_map(&jobs, |&i| {
                    let (vol, water) = islands[i].voxelize(&stalactites[i]);
                    (compile(vol), compile(water))
                })
            });
            let sea = sc.spawn(|| effects::build_cloud_sea(&mut ColorCache::new()));
            let obs = sc.spawn(|| observatory::build_observatory(&hero.paths, &mut ColorCache::new()));
            let br = sc.spawn(|| par_map(&bridges, |(_, _, b)| b.build(&mut ColorCache::new())));
            let kit = sc.spawn(build_flora);
            let rings = sc.spawn(|| {
                let specs = [(5.6, pal::GOLD, pal::BRONZE), (4.7, pal::BRASS, pal::GOLD), (3.8, pal::BRONZE, pal::GOLD)];
                par_map(&specs, |&(r, col, notch)| observatory::build_ring(r, col, notch, &mut ColorCache::new()))
            });
            let orb = sc.spawn(|| observatory::build_orb(&mut ColorCache::new()));
            let islet_m = sc.spawn(|| {
                let jobs: Vec<usize> = (0..islet_specs.len()).collect();
                par_map(&jobs, |&k| compile(islet_specs[k].voxelize(&islet_stal[k]).0))
            });
            let bird = sc.spawn(|| effects::build_bird(&mut ColorCache::new()));
            (
                isl.join().unwrap(),
                sea.join().unwrap(),
                obs.join().unwrap(),
                br.join().unwrap(),
                kit.join().unwrap(),
                rings.join().unwrap(),
                orb.join().unwrap(),
                islet_m.join().unwrap(),
                bird.join().unwrap(),
            )
        });

        // ---- Assemble (same batch order as before) ------------------------------
        for (spec, (opaque, water)) in islands.iter().zip(&island_meshes) {
            let at = [ModelInstance::from_trs(spec.center, 0.0, Vec3::ONE)];
            sb.add_mesh(&opaque.0, opaque.1, &at, Pass::Opaque);
            sb.add_mesh(&water.0, water.1, &at, Pass::Translucent);
        }
        sb.add_mesh(&sea.0, sea.1, &[ModelInstance::identity()], Pass::Opaque);
        sb.add_mesh(&obs.0, obs.1, &[ModelInstance::from_trs(hero.center, 0.0, Vec3::ONE)], Pass::Opaque);
        for ((_, _, b), (m, c)) in bridges.iter().zip(&bridge_meshes) {
            sb.add_mesh(m, *c, &[b.transform()], Pass::Opaque);
        }

        sb.total_voxels += kit.voxels;
        let meshes: Vec<&MeshData> = kit.oaks.iter().chain(&kit.blossoms).chain(&kit.golds).chain(&kit.pines).collect();
        assert_eq!(meshes.len(), n_trees);
        for (mesh, list) in meshes.iter().zip(buckets.iter()) {
            let span = sb.push_mesh(mesh);
            sb.add_static(span, list, Pass::Opaque);
        }
        let spirit = &islands[SPIRIT];
        let spirit_base = spirit.center + Vec3::Y * (spirit.top_y(0.0, 0.0) - 0.3);
        let span = sb.push_mesh(&kit.spirit_tree);
        sb.add_static(span, &[ModelInstance::from_trs(spirit_base, 0.4, Vec3::ONE)], Pass::Opaque);

        // Dynamic batches: order must match update_dynamic_instances.
        for (m, c) in rings.iter().chain(std::iter::once(&orb)) {
            sb.total_voxels += c;
            let span = sb.push_mesh(m);
            sb.add_dynamic(span, 1, Pass::Opaque);
        }
        for (k, (m, c)) in islet_meshes.iter().enumerate() {
            sb.total_voxels += c;
            let span = sb.push_mesh(m);
            sb.add_dynamic(span, islets[k].len(), Pass::Opaque);
        }
        sb.total_voxels += bird.1;
        let span = sb.push_mesh(&bird.0);
        sb.add_dynamic(span, birds.len(), Pass::Opaque);

        let mut colors = ColorCache::new();
        let cloud_mesh = build_unit_box_mesh(9.5, 3.4, 9.5, MAT_CLOUD as u32, pal::CLOUD, &mut colors);
        let span = sb.push_mesh(&cloud_mesh);
        sb.add_dynamic(span, clouds.len(), Pass::Translucent);
        let mist_mesh = build_unit_box_mesh(0.55, 0.55, 0.55, MAT_MIST as u32, pal::MIST, &mut colors);
        let span = sb.push_mesh(&mist_mesh);
        sb.add_dynamic(span, spray.len(), Pass::Translucent);

        // ---- Lights, cameras --------------------------------------------------
        let mut lights: Vec<[f32; 4]> = observatory::lantern_positions(hero.center)
            .iter()
            .step_by(2)
            .map(|p| [p.x, p.y, p.z, 26.0])
            .collect();
        lights.push([spirit_base.x, spirit_base.y + 9.0, spirit_base.z, 30.0]);
        let armillary = hero.center + Vec3::Y * ARMILLARY_Y;
        lights.push([armillary.x, armillary.y, armillary.z, 22.0]);
        for (_, _, b) in &bridges {
            for s in [0.02f32, 0.98] {
                if lights.len() < crate::engine::scene::MAX_POINT_LIGHTS {
                    let p = b.world_point(s);
                    lights.push([p.x, p.y + 2.0, p.z, 14.0]);
                }
            }
        }

        let views = camera_views(&islands, &bridges, &lips);
        let (tour_pos, tour_look) = tour_curves(&views);
        let geo = sb.finish();
        let data = SceneData {
            title: "Sky Islands".to_string(),
            vertices: geo.vertices,
            indices: geo.indices,
            static_instances: geo.static_instances,
            opaque_batches: geo.opaque_batches,
            translucent_batches: geo.translucent_batches,
            dynamic_inst_start: geo.dynamic_inst_start,
            dynamic_inst_count: geo.dynamic_inst_count,
            total_voxels: geo.total_voxels,
            total_tris: geo.total_tris,
            presets: views,
            tour_pos,
            tour_look,
            tour_secs: 90.0,
            max_cam_dist: 520.0,
            min_pitch: -1.25,
            point_lights: lights,
            point_light_col: [1.0, 0.55, 0.2],
            time_modes: time_modes(),
            shadow: ShadowConfig { target: Vec3::new(0.0, 60.0, 0.0), half_extent: 235.0, near: 20.0, far: 950.0 },
            particles: leaves(),
        };

        Self { data, armillary, islets, birds, clouds, spray }
    }
}

/// Camera presets derived from the generated geometry (so they stay valid if
/// the plan changes).
fn camera_views(islands: &[IslandSpec], bridges: &[(usize, usize, BridgeSpec)], lips: &[(Vec3, Vec3)]) -> Vec<CameraPreset> {
    let hero = islands[HERO].center;
    let spirit = islands[SPIRIT].center;
    let out_dir = |c: Vec3| c.with_y(0.0).normalize_or(Vec3::X);
    let mut v = vec![
        CameraPreset { name: "Overview", pos: Vec3::new(205.0, 150.0, 230.0), target: Vec3::new(0.0, 58.0, 0.0) },
        CameraPreset { name: "Observatory", pos: hero + Vec3::new(30.0, 20.0, 38.0), target: hero + Vec3::Y * 9.0 },
        CameraPreset { name: "Armillary", pos: hero + Vec3::new(11.0, ARMILLARY_Y + 4.0, 15.0), target: hero + Vec3::Y * (ARMILLARY_Y - 1.0) },
        CameraPreset { name: "Spirit Tree", pos: spirit + out_dir(spirit) * 46.0 + Vec3::new(0.0, 16.0, 0.0) + out_dir(spirit).cross(Vec3::Y) * 18.0, target: spirit + Vec3::Y * 13.0 },
    ];
    if let Some((_, _, b)) = bridges.first() {
        let mid = b.world_point(0.5);
        let d = (b.b - b.a).with_y(0.0).normalize();
        let side = d.cross(Vec3::Y);
        v.push(CameraPreset { name: "Rope Bridge", pos: mid - d * 12.0 + side * 30.0 + Vec3::Y * 9.0, target: mid + d * 4.0 - Vec3::Y * 2.0 });
    }
    if let Some(&(lip, dir)) = lips.first() {
        let side = dir.cross(Vec3::Y);
        v.push(CameraPreset { name: "Waterfall", pos: lip + dir * 58.0 + side * 30.0 - Vec3::Y * 10.0, target: lip + dir * 4.0 - Vec3::Y * 24.0 });
    }
    v.push(CameraPreset { name: "Beneath the Isles", pos: hero + Vec3::new(44.0, -44.0, 58.0), target: hero + Vec3::new(0.0, -12.0, 0.0) });
    v.push(CameraPreset { name: "Cloud Sea", pos: Vec3::new(140.0, 16.0, 180.0), target: Vec3::new(10.0, 48.0, 0.0) });
    let far = &islands[5];
    v.push(CameraPreset { name: far.name, pos: far.center + out_dir(far.center) * 44.0 + Vec3::new(0.0, 22.0, 0.0) + out_dir(far.center).cross(Vec3::Y) * -20.0, target: far.center + Vec3::Y * 4.0 });
    v
}

fn tour_curves(views: &[CameraPreset]) -> (CatmullRomCurve3, CatmullRomCurve3) {
    // Visit every view, inserting a wide orbit point between them so the path
    // swings around the archipelago instead of cutting straight through rock.
    let mut pos = Vec::new();
    let mut look = Vec::new();
    for (i, v) in views.iter().enumerate() {
        pos.push(v.pos);
        look.push(v.target);
        let a = i as f32 / views.len() as f32 * TAU;
        pos.push(Vec3::new(a.cos() * 230.0, 120.0 + (a * 2.0).sin() * 30.0, a.sin() * 230.0));
        look.push(Vec3::new(0.0, 55.0, 0.0));
    }
    (CatmullRomCurve3::new(pos, true, 0.4), CatmullRomCurve3::new(look, true, 0.4))
}
