//! Assembly: plan -> terrain -> Poisson-disc scatter of plants, rocks and
//! torches -> parallel voxelisation -> batches, lights, cameras, landmarks.

use glam::{Quat, Vec2, Vec3};
use std::f32::consts::TAU;

use super::arch::Arch;
use super::plan::{Plan, WORLD_R};
use super::ruins::Ruins;
use super::terrain::{Kind, Terrain};
use super::*;

/// (height, lean) of the palm archetypes.
const PALMS: [(f32, f32); 3] = [(8.0, 1.6), (9.6, 2.6), (11.2, 3.4)];

struct Scatter {
    palms: Vec<Vec<ModelInstance>>,
    cacti: Vec<Vec<ModelInstance>>,
    shrubs: Vec<Vec<ModelInstance>>,
    reeds: Vec<Vec<ModelInstance>>,
    rocks: Vec<Vec<ModelInstance>>,
    palm_spots: Vec<Vec3>,
}

fn v3(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}

/// Yaw that turns local +X toward `dir` (from_trs rotates about +Y).
fn yaw_to(dir: Vec2) -> f32 {
    (-dir.y).atan2(dir.x)
}

/// Keep-out test shared by everything scattered on the open floor.
fn open_floor(t: &Terrain, arch: &Arch, p: Vec2, ruins: f32, river: f32, oasis_e: f32) -> bool {
    let plan = &t.plan;
    (p - plan.ruins).length() > ruins
        && (p - plan.camp).length() > 13.0
        && (p - arch.c).length() > 21.0
        && plan.river_dist(p).0 > river
        && plan.oasis_e(p) > oasis_e
}

fn scatter(t: &Terrain, arch: &Arch, rng: &mut Rng) -> Scatter {
    let plan = &t.plan;
    let mut s = Scatter {
        palms: vec![Vec::new(); PALMS.len()],
        cacti: vec![Vec::new(); 5],
        shrubs: vec![Vec::new(); 3],
        reeds: vec![Vec::new(); 2],
        rocks: vec![Vec::new(); 4],
        palm_spots: Vec::new(),
    };
    // Palms ring the oasis (Poisson disc on the shore annulus), leaning in.
    let ro = plan.oasis_r.max_element() * 2.0;
    for (x, z) in poisson_disc(rng, ro, 5.2, 400, |x, z| {
        let p = plan.oasis + Vec2::new(x, z);
        let e = plan.oasis_e(p);
        e > 1.06 && e < 1.7 && (p - plan.camp).length() > 6.5 && t.roughness(p.x, p.y, 1.0) < 1.0
    })
    .into_iter()
    .take(10)
    {
        let p = plan.oasis + Vec2::new(x, z);
        let k = (hash2(x, z) * PALMS.len() as f32) as usize % PALMS.len();
        let to_water = (plan.oasis - p).normalize_or(Vec2::X);
        let yaw = yaw_to(Vec2::from_angle((rng.next_f32() - 0.5) * 0.6).rotate(to_water));
        let base = v3(p, t.seat(p.x, p.y, 0.6) - 0.35);
        s.palm_spots.push(base);
        s.palms[k].push(ModelInstance::from_trs(base, yaw, Vec3::splat(0.9 + rng.next_f32() * 0.25)));
    }
    // Reeds on the wet shore.
    for (x, z) in poisson_disc(rng, ro, 1.9, 900, |x, z| {
        let e = plan.oasis_e(plan.oasis + Vec2::new(x, z));
        e > 0.86 && e < 1.3
    }) {
        let p = plan.oasis + Vec2::new(x, z);
        let k = (hash2(z, x) * 2.0) as usize % 2;
        let base = v3(p, t.seat(p.x, p.y, 0.4) - 0.1);
        s.reeds[k].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(0.8 + rng.next_f32() * 0.5)));
    }
    // Cacti across the open floor.
    let r_all = WORLD_R * 0.97;
    for (x, z) in poisson_disc(rng, r_all, 13.0, 1400, |x, z| {
        let c = t.column(x, z);
        c.kind == Kind::Sand && c.talus < 0.6 && c.dune < 2.6 && t.roughness(x, z, 1.0) < 0.75 && open_floor(t, arch, Vec2::new(x, z), 22.0, 7.0, 1.9)
    }) {
        let h = hash2(x * 0.37, z * 0.71);
        let k = if h < 0.18 { 0 } else if h < 0.36 { 1 } else if h < 0.56 { 2 } else if h < 0.8 { 3 } else { 4 };
        let base = Vec3::new(x, t.seat(x, z, 0.5) - 0.25, z);
        s.cacti[k].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(0.85 + rng.next_f32() * 0.35)));
    }
    // Sagebrush: denser, also along the riverbanks.
    for (x, z) in poisson_disc(rng, r_all, 7.5, 2400, |x, z| {
        let c = t.column(x, z);
        (c.kind == Kind::Sand || c.kind == Kind::Grass) && c.dune < 2.8 && t.roughness(x, z, 0.8) < 0.9 && open_floor(t, arch, Vec2::new(x, z), 20.0, 4.0, 1.25)
    }) {
        let k = (hash2(z * 1.3, x * 0.7) * 3.0) as usize % 3;
        let base = Vec3::new(x, t.seat(x, z, 0.5) - 0.15, z);
        s.shrubs[k].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(0.7 + rng.next_f32() * 0.6)));
    }
    // Boulders: talus at the feet of the cliffs, a few loose on the floor,
    // and pebbles strewn down the riverbed.
    for (x, z) in poisson_disc(rng, r_all, 4.5, 2500, |x, z| {
        let c = t.column(x, z);
        c.kind == Kind::Sand && c.talus > 0.9 && t.roughness(x, z, 1.2) < 2.0 && open_floor(t, arch, Vec2::new(x, z), 16.0, 3.0, 1.6)
    }) {
        let k = (hash2(x, z * 3.1) * 4.0) as usize % 4;
        let sc = 0.6 + rng.next_f32() * 0.9;
        let base = Vec3::new(x, t.seat(x, z, 1.2 * sc) - 0.35 * sc, z);
        s.rocks[k].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(sc)));
    }
    for (x, z) in poisson_disc(rng, r_all, 15.0, 500, |x, z| {
        let c = t.column(x, z);
        c.kind == Kind::Sand && c.dune < 1.5 && open_floor(t, arch, Vec2::new(x, z), 17.0, 6.0, 1.8)
    }) {
        let k = (hash2(x * 2.0, z) * 4.0) as usize % 4;
        let sc = 0.45 + rng.next_f32() * 0.5;
        let base = Vec3::new(x, t.seat(x, z, sc) - 0.3 * sc, z);
        s.rocks[k].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(sc)));
    }
    for i in (4..plan.river.len() - 12).step_by(7) {
        let p = plan.river[i] + Vec2::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5) * 4.0;
        let sc = 0.25 + rng.next_f32() * 0.25;
        let base = v3(p, t.seat(p.x, p.y, sc) - 0.25 * sc);
        s.rocks[i % 4].push(ModelInstance::from_trs(base, rng.next_f32() * TAU, Vec3::splat(sc)));
    }
    s
}

impl DesertCanyon {
    pub fn build(params: &WorldParams) -> Self {
        let winter = params.season == Season::Winter;
        let terrain = Terrain::new(Plan::new(params.seed, winter));
        let plan = &terrain.plan;
        let arch = Arch::new(plan);
        let ruins = Ruins::new(plan, terrain.seat(plan.ruins.x, plan.ruins.y, 12.0).min(0.0));
        let camp_y = terrain.seat(plan.camp.x, plan.camp.y, 5.0);
        let camp_at = ModelInstance::from_trs(v3(plan.camp, camp_y), plan.camp_rot, Vec3::splat(props::CAMP_SCALE));
        let camp_fire = v3(plan.camp, camp_y) + Quat::from_rotation_y(plan.camp_rot) * (props::CAMPFIRE * props::CAMP_SCALE);

        let mut rng = Rng(0xDE5E_0000 ^ params.seed);
        let sc = scatter(&terrain, &arch, &mut rng);

        // Torches: ruins, flanking the arch window, and the overlook brazier.
        let mut torches = ruins.torches();
        let across = plan.arch_across;
        for k in [-1.0f32, 1.0] {
            let p = arch.c + across * 6.5 + arch.dir * (4.5 * k);
            torches.push(v3(p, terrain.seat(p.x, p.y, 0.5)));
        }
        let ov = plan.overlook;
        torches.push(v3(ov, terrain.seat(ov.x, ov.y, 0.6)));
        let vultures = effects::make_vultures(v3(arch.c, 0.0), 7, &mut rng);

        // ---- heavy geometry in parallel ----------------------------------------
        enum Job {
            Terrain,
            Water,
            Ruins,
            Palms,
            Cacti,
            Shrubs,
            Reeds,
            Rocks,
            Small,
        }
        let jobs = [Job::Terrain, Job::Water, Job::Ruins, Job::Palms, Job::Cacti, Job::Shrubs, Job::Reeds, Job::Rocks, Job::Small];
        let mut outs = par_map(&jobs, |job| match job {
            Job::Terrain => terrain.build_meshes(&arch),
            Job::Water => vec![terrain.build_water()],
            Job::Ruins => vec![ruins.build()],
            Job::Palms => par_map(&[0usize, 1, 2], |&k| flora::build_palm(31 + k as u32 * 17, PALMS[k].0, PALMS[k].1, winter)),
            Job::Cacti => par_map(&[0u32, 1, 2, 3, 4], |&k| match k {
                0..=2 => flora::build_saguaro(101 + k * 13, winter),
                3 => flora::build_prickly_pear(211, winter),
                _ => flora::build_barrel(307, winter),
            }),
            Job::Shrubs => par_map(&[0u32, 1, 2], |&k| flora::build_shrub(401 + k * 7, winter)),
            Job::Reeds => par_map(&[0u32, 1], |&k| flora::build_reeds(503 + k * 11, winter)),
            Job::Rocks => par_map(&[0u32, 1, 2, 3], |&k| props::build_rock(601 + k * 19, winter)),
            Job::Small => vec![props::build_torch(), props::build_camp(winter), props::build_vulture(), props::build_flame()],
        })
        .into_iter();
        let mut next = || outs.next().expect("job order");
        let terrain_meshes = next();
        let water = next();
        let ruins_mesh = next();
        let palm_meshes = next();
        let cactus_meshes = next();
        let shrub_meshes = next();
        let reed_meshes = next();
        let rock_meshes = next();
        let small = next();

        // ---- assemble ------------------------------------------------------------
        let mut sb = SceneBuilder::new();
        let id = [ModelInstance::identity()];
        for (m, c) in &terrain_meshes {
            sb.add_mesh(m, *c, &id, Pass::Opaque);
        }
        sb.add_mesh(&ruins_mesh[0].0, ruins_mesh[0].1, &[ruins.transform()], Pass::Opaque);
        sb.add_mesh(&small[1].0, small[1].1, &[camp_at], Pass::Opaque);
        let groups: [(&Vec<(MeshData, u64)>, &Vec<Vec<ModelInstance>>); 5] =
            [(&palm_meshes, &sc.palms), (&cactus_meshes, &sc.cacti), (&shrub_meshes, &sc.shrubs), (&reed_meshes, &sc.reeds), (&rock_meshes, &sc.rocks)];
        for (meshes, lists) in groups {
            for ((m, c), list) in meshes.iter().zip(lists.iter()) {
                if list.is_empty() {
                    continue;
                }
                sb.total_voxels += c;
                let span = sb.push_mesh(m);
                sb.add_static(span, list, Pass::Opaque);
            }
        }
        let torch_at: Vec<ModelInstance> = torches.iter().map(|&p| ModelInstance::from_trs(p, hash2(p.x, p.z) * TAU, Vec3::ONE)).collect();
        sb.total_voxels += small[0].1;
        let span = sb.push_mesh(&small[0].0);
        sb.add_static(span, &torch_at, Pass::Opaque);
        sb.add_mesh(&water[0].0, water[0].1, &id, Pass::Translucent);

        // Dynamic batches: order must match update_dynamic_instances.
        let mut flames: Vec<(Vec3, f32)> = torches.iter().map(|&p| (p + Vec3::Y * props::TORCH_H, 1.0)).collect();
        flames.push((ruins.fire() - Vec3::Y * 0.2, 2.0));
        flames.push((camp_fire + Vec3::Y * 0.1, 1.5));
        sb.total_voxels += small[2].1 + small[3].1;
        let span = sb.push_mesh(&small[2].0);
        sb.add_dynamic(span, vultures.len(), Pass::Opaque);
        let span = sb.push_mesh(&small[3].0);
        sb.add_dynamic(span, flames.len(), Pass::Opaque);

        // ---- lights, cameras, landmarks -----------------------------------------
        let mut lights: Vec<[f32; 4]> = torches.iter().map(|p| [p.x, p.y + props::TORCH_H + 0.6, p.z, 15.0]).collect();
        let f = ruins.fire();
        lights.push([f.x, f.y + 0.8, f.z, 20.0]);
        lights.push([camp_fire.x, camp_fire.y + 1.0, camp_fire.z, 16.0]);
        lights.truncate(crate::engine::scene::MAX_POINT_LIGHTS);

        let views = camera_views(&terrain, &arch, &ruins, across, v3(plan.camp, camp_y));
        let (tour_pos, tour_look) = tour_curves(&views);
        let landmarks = landmarks(&terrain, &arch, &ruins, &sc.palm_spots, v3(plan.camp, camp_y));
        let geo = sb.finish();
        let data = SceneData {
            title: "Desert Canyon".to_string(),
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
            presets: views,
            tour_pos,
            tour_look,
            tour_secs: 95.0,
            max_cam_dist: 400.0,
            min_pitch: -0.6,
            point_lights: lights,
            point_light_col: [1.0, 0.58, 0.24],
            time_modes: time_modes(winter),
            shadow: ShadowConfig { target: Vec3::new(0.0, 8.0, 0.0), half_extent: 135.0, near: 10.0, far: 700.0 },
            particles: effects::sand(),
            landmarks,
        };
        Self { data, terrain, params: *params, vultures, flames }
    }
}

/// Lift a camera so it never sits inside the rock.
fn clear_cam(t: &Terrain, pos: Vec3, min_h: f32) -> Vec3 {
    let mut g: f32 = f32::MIN;
    for (dx, dz) in [(0.0, 0.0), (2.0, 0.0), (-2.0, 0.0), (0.0, 2.0), (0.0, -2.0)] {
        if let Some(h) = t.surface(pos.x + dx, pos.z + dz) {
            g = g.max(h);
        }
    }
    Vec3::new(pos.x, pos.y.max(g + min_h), pos.z)
}

fn camera_views(t: &Terrain, arch: &Arch, ruins: &Ruins, across: Vec2, camp: Vec3) -> Vec<CameraPreset> {
    let plan = &t.plan;
    let ground = |p: Vec2| t.surface(p.x, p.y).unwrap_or(0.0);
    let mut v = Vec::new();
    let mut add = |name: &'static str, pos: Vec3, target: Vec3| v.push(CameraPreset { name, pos: clear_cam(t, pos, 2.5), target });

    add("Overview", plan.xf3(Vec3::new(150.0, 112.0, 168.0)), plan.xf3(Vec3::new(-8.0, 2.0, -8.0)));
    let a = v3(arch.c, 0.0);
    let ad = v3(arch.dir, 0.0);
    let ax = v3(across, 0.0);
    add("Great Arch", a + ax * 50.0 - ad * 12.0 + Vec3::Y * 13.0, a + Vec3::Y * 12.0);
    let front = Quat::from_rotation_y(ruins.rot) * Vec3::Z;
    let side = front.cross(Vec3::Y);
    let tc = ruins.top_center();
    add("Sun Temple", tc + front * 25.0 + side * 13.0 + Vec3::Y * 10.0, tc + Vec3::Y * 2.0);
    let o = v3(plan.oasis, 0.0);
    let to_c = (-o).with_y(0.0).normalize_or(Vec3::X);
    add("Oasis", o - to_c * 30.0 + to_c.cross(Vec3::Y) * 10.0 + Vec3::Y * 13.0, o + to_c * 4.0 + Vec3::Y * 1.5);
    let r0 = plan.river[plan.river.len() * 58 / 100];
    let r1 = plan.river[plan.river.len() * 18 / 100];
    add("Dry Riverbed", v3(r0, ground(r0) + 3.2), v3(r1, ground(r1) + 7.0));
    let ov = plan.overlook;
    let inward = (-ov).normalize_or(Vec2::X);
    add("Canyon Overlook", v3(ov + inward * 6.0, ground(ov) + 13.0), plan.xf3(Vec3::new(14.0, 0.0, 14.0)));
    add("Dune Sea", plan.xf3(Vec3::new(98.0, 9.0, 36.0)), v3(plan.ruins, 4.0));
    add("Through the Arch", a - ax * 34.0 + ad * 2.0 + Vec3::Y * 4.0, a + ax * 20.0 + Vec3::Y * 8.0);
    let cf = Quat::from_rotation_y(plan.camp_rot) * Vec3::Z;
    let cs = cf.cross(Vec3::Y);
    add("Nomad Camp", camp + cf * 13.0 + cs * 8.0 + Vec3::Y * 6.0, camp + cf * 1.5 + Vec3::Y * 1.0);
    v
}

fn landmarks(t: &Terrain, arch: &Arch, ruins: &Ruins, palms: &[Vec3], camp: Vec3) -> Vec<Landmark> {
    let plan = &t.plan;
    let on = |p: Vec2, dy: f32| v3(p, t.surface(p.x, p.y).unwrap_or(0.0) + dy);
    let mut v = vec![
        Landmark { name: "Great Arch".into(), pos: arch.crown(), radius: 18.0, kind: "rock" },
        Landmark { name: "Sun Temple".into(), pos: ruins.top_center() + Vec3::Y * 3.0, radius: 13.0, kind: "building" },
        Landmark { name: "Oasis".into(), pos: on(plan.oasis, 0.5), radius: plan.oasis_r.max_element(), kind: "water" },
        Landmark { name: "Dry Riverbed".into(), pos: on(plan.river[plan.river.len() / 2], 0.5), radius: 10.0, kind: "terrain" },
        Landmark { name: "Canyon Overlook".into(), pos: on(plan.overlook, 1.0), radius: 14.0, kind: "viewpoint" },
        Landmark { name: "Lone Butte".into(), pos: on(plan.butte, 0.5), radius: 12.0, kind: "rock" },
        Landmark { name: "Nomad Camp".into(), pos: camp + Vec3::Y * 1.0, radius: 5.0, kind: "building" },
        Landmark { name: "Dune Sea".into(), pos: on(plan.xf(Vec2::new(70.0, 30.0)), 1.0), radius: 30.0, kind: "terrain" },
    ];
    if !palms.is_empty() {
        let c = palms.iter().copied().sum::<Vec3>() / palms.len() as f32;
        v.push(Landmark { name: "Palm Grove".into(), pos: Vec3::new(c.x, c.y + 6.0, c.z), radius: 12.0, kind: "tree" });
    }
    v
}

fn tour_curves(views: &[CameraPreset]) -> (CatmullRomCurve3, CatmullRomCurve3) {
    let mut pos = Vec::new();
    let mut look = Vec::new();
    for (i, v) in views.iter().enumerate() {
        pos.push(v.pos);
        look.push(v.target);
        let a = (i as f32 + 0.5) / views.len() as f32 * TAU;
        pos.push(Vec3::new(a.cos() * 170.0, 75.0 + (a * 2.0).sin() * 20.0, a.sin() * 170.0));
        look.push(Vec3::new(0.0, 6.0, 0.0));
    }
    (CatmullRomCurve3::new(pos, true, 0.4), CatmullRomCurve3::new(look, true, 0.4))
}
