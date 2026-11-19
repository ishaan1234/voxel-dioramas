//! World layout for the Sakura Empire: terrain, landmarks, town, flora, dynamics.

use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::archetypes::{
    build_boat_archetypes, build_villager_archetypes, compile_building_archetypes,
    compile_tree_archetypes,
};
use super::kit::*;
use super::landmarks::{
    build_bell_tower, build_ceremonial_staircase, build_cliff_stilt_temple, build_covered_bridge,
    build_golden_hall, build_grand_taiko_bridge, build_harbor_pier, build_island_deck,
    build_nandaimon_gate, build_seven_tier_pagoda, build_watermill_house,
};
use super::terrain::{is_water_body, make_senbon_curve, river_z, TerrainGrid, GRID_N};
use super::*;

#[derive(Copy, Clone, PartialEq, Eq)]
enum BuildingKey {
    RyokanBlue,
    RyokanRed,
    MachiyaCream,
    MachiyaDark,
    Teahouse,
    StallOrange,
    StallBlue,
    Watchtower,
    Inari,
}

struct TownPlacement {
    key: BuildingKey,
    x: f32,
    y: f32,
    z: f32,
    rot_y: f32,
}

/// Gate centre: its front plinth steps (6.25 m ahead of centre) end exactly at
/// the citadel wall face (z = -24), where the ceremonial stairs arrive.
const GATE_Z: f32 = -30.25;

#[derive(Copy, Clone)]
struct MeshSpan {
    idx_start: u32,
    idx_count: u32,
    base_vertex: i32,
    tri_count: usize,
}

impl SakuraScene {
    pub fn build(params: &WorldParams) -> Self {
        let mut rng = Rng(20261113);
        let mut colors = ColorCache::new();
        let mut total_voxels = (GRID_N * GRID_N) as u64;

        let mut grid = TerrainGrid::new();
        let mut town_placements: Vec<TownPlacement> = Vec::new();

        let mut plan_building = |grid: &mut TerrainGrid, key: BuildingKey, x: f32, z: f32, rot_y: f32, hw: f32, hd: f32, pad_y: f32| {
            grid.register_pad(x, z, hw, hd, pad_y, true);
            town_placements.push(TownPlacement { key, x, y: pad_y, z, rot_y });
        };

        // Plan world layout
        grid.register_pad(-56.0, -82.0, 14.0, 14.0, 28.0, true);
        grid.register_pad(36.0, -82.0, 16.0, 12.0, 28.0, true);
        grid.register_pad(-14.0, -76.0, 6.0, 6.0, 28.0, true);
        grid.register_pad(10.0, -76.0, 6.0, 6.0, 28.0, true);
        grid.register_pad(0.0, GATE_Z, 12.0, 6.0, 22.0, true);
        grid.register_pad(-146.0, -88.0, 9.0, 9.0, 42.0, true);
        grid.register_pad(-116.0, 10.0, 6.0, 6.0, 10.0, true);

        plan_building(&mut grid, BuildingKey::RyokanBlue, -16.0, 94.0, FRAC_PI_2, 7.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::RyokanRed, 16.0, 94.0, -FRAC_PI_2, 7.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::RyokanRed, -16.0, 146.0, FRAC_PI_2, 7.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::RyokanBlue, 16.0, 146.0, -FRAC_PI_2, 7.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::Teahouse, -24.0, 118.0, FRAC_PI_2, 6.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::Teahouse, 24.0, 118.0, -FRAC_PI_2, 6.0, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::Watchtower, -12.0, 178.0, 0.0, 5.0, 5.0, 10.0);
        plan_building(&mut grid, BuildingKey::Watchtower, 12.0, 178.0, 0.0, 5.0, 5.0, 10.0);

        for (sx, sz, rot, k) in [
            (-11.0, 110.0, FRAC_PI_2, BuildingKey::StallOrange),
            (-11.0, 124.0, FRAC_PI_2, BuildingKey::StallBlue),
            (11.0, 110.0, -FRAC_PI_2, BuildingKey::StallBlue),
            (11.0, 124.0, -FRAC_PI_2, BuildingKey::StallOrange),
            (-28.0, 102.0, 0.0, BuildingKey::StallOrange),
            (28.0, 102.0, 0.0, BuildingKey::StallBlue),
            (106.0, 66.0, FRAC_PI_2, BuildingKey::StallBlue),
            (116.0, 74.0, PI, BuildingKey::StallOrange),
        ] {
            plan_building(&mut grid, k, sx, sz, rot, 4.0, 4.0, 10.0);
        }

        let machiya_keys = [
            BuildingKey::MachiyaCream,
            BuildingKey::MachiyaDark,
            BuildingKey::Teahouse,
            BuildingKey::RyokanBlue,
            BuildingKey::RyokanRed,
        ];
        let mut add_street_row = |grid: &mut TerrainGrid, xs: &[f32], z: f32, rot: f32| {
            for &bx in xs {
                if is_water_body(bx, z) || bx.abs() < 22.0 {
                    continue;
                }
                let idx = (rng.next_f32() * 3.6).floor() as usize;
                let k = machiya_keys[idx.min(machiya_keys.len() - 1)];
                plan_building(grid, k, bx, z, rot, 5.5, 5.5, 10.0);
            }
        };

        add_street_row(&mut grid, &[-150.0, -138.0, -118.0, -104.0, -84.0, -70.0, -50.0, -38.0, 38.0, 50.0, 72.0, 86.0, 114.0], 70.0, 0.0);
        add_street_row(&mut grid, &[-150.0, -138.0, -118.0, -104.0, -84.0, -70.0, -50.0, -38.0, 38.0, 50.0, 72.0, 86.0], 86.0, PI);
        add_street_row(&mut grid, &[-148.0, -120.0, -106.0, -84.0, -70.0, -48.0, -38.0, 38.0, 48.0, 70.0], 98.0, 0.0);
        add_street_row(&mut grid, &[-148.0, -120.0, -106.0, -84.0, -70.0, -48.0, -38.0, 38.0, 48.0, 70.0], 114.0, PI);
        add_street_row(&mut grid, &[-148.0, -120.0, -106.0, -84.0, -70.0, -48.0, -38.0, 38.0, 48.0, 70.0], 132.0, 0.0);
        add_street_row(&mut grid, &[-148.0, -120.0, -106.0, -84.0, -70.0, -48.0, -38.0, 38.0, 48.0, 70.0], 148.0, PI);
        add_street_row(&mut grid, &[-122.0, -106.0, -84.0, -70.0, -48.0, -36.0, 36.0, 48.0, 70.0], 166.0, 0.0);
        add_street_row(&mut grid, &[-106.0, -84.0, -48.0, -36.0, 36.0, 48.0], 184.0, PI);

        plan_building(&mut grid, BuildingKey::Teahouse, -96.0, 28.0, 0.0, 5.5, 5.5, 10.0);
        plan_building(&mut grid, BuildingKey::RyokanRed, -54.0, -4.0, 0.0, 6.5, 6.0, 10.0);
        plan_building(&mut grid, BuildingKey::Teahouse, 42.0, -4.0, 0.0, 5.5, 5.5, 10.0);
        plan_building(&mut grid, BuildingKey::MachiyaCream, 72.0, -4.0, 0.0, 5.5, 5.5, 10.0);

        plan_building(&mut grid, BuildingKey::MachiyaDark, 114.0, 48.0, FRAC_PI_2, 5.5, 5.5, 10.0);
        plan_building(&mut grid, BuildingKey::Watchtower, 120.0, 88.0, PI, 5.0, 5.0, 10.0);

        town_placements.push(TownPlacement {
            key: BuildingKey::Inari,
            x: -146.0,
            y: 42.0,
            z: -88.0,
            rot_y: PI * 0.15,
        });

        let senbon_curve = make_senbon_curve();
        grid.generate(&senbon_curve);
        grid.apply_pads_and_roads();
        let (terrain_opaque, terrain_water) = grid.build_chunks(&mut colors);

        let mut vertices: Vec<VoxelVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut static_instances: Vec<ModelInstance> = Vec::new();
        let mut opaque_shadow_batches: Vec<DrawBatch> = Vec::new();
        let mut total_tris = 0usize;

        let push_mesh = |vertices: &mut Vec<VoxelVertex>, indices: &mut Vec<u32>, mesh: &MeshData| -> MeshSpan {
            let base_vertex = vertices.len() as i32;
            let idx_start = indices.len() as u32;
            let idx_count = mesh.indices.len() as u32;
            vertices.extend_from_slice(&mesh.vertices);
            indices.extend_from_slice(&mesh.indices);
            MeshSpan {
                idx_start,
                idx_count,
                base_vertex,
                tri_count: mesh.tri_count(),
            }
        };

        let mut add_static_batch = |span: MeshSpan, transforms: &[ModelInstance]| {
            if span.idx_count == 0 || transforms.is_empty() {
                return;
            }
            let inst_start = static_instances.len() as u32;
            let inst_count = transforms.len() as u32;
            static_instances.extend_from_slice(transforms);
            opaque_shadow_batches.push(DrawBatch {
                idx_start: span.idx_start,
                idx_count: span.idx_count,
                base_vertex: span.base_vertex,
                inst_start,
                inst_count,
            });
            total_tris += span.tri_count * transforms.len();
        };

        // 1. Terrain Opaque
        let t_span = push_mesh(&mut vertices, &mut indices, &terrain_opaque);
        add_static_batch(t_span, &[ModelInstance::identity()]);

        // 2. Hero Landmarks
        let (pagoda_m, c) = build_seven_tier_pagoda(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &pagoda_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(-56.0, 28.0, -82.0), 0.0, Vec3::ONE)]);

        let (hall_m, c) = build_golden_hall(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &hall_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(36.0, 28.0, -82.0), 0.0, Vec3::ONE)]);

        let (bell_m, c) = build_bell_tower(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &bell_m);
        add_static_batch(
            sp,
            &[
                ModelInstance::from_trs(Vec3::new(-14.0, 28.0, -76.0), 0.0, Vec3::ONE),
                ModelInstance::from_trs(Vec3::new(10.0, 28.0, -76.0), 0.0, Vec3::ONE),
            ],
        );

        let (gate_m, c) = build_nandaimon_gate(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &gate_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(0.0, 22.0, GATE_Z), 0.0, Vec3::ONE)]);

        let (cliff_m, c) = build_cliff_stilt_temple(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &cliff_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(110.0, 34.0, -44.0), FRAC_PI_2, Vec3::ONE)]);

        let (st_central, c) = build_ceremonial_staircase(20.0, 12.0, 6.0, &mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &st_central);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(0.0, 10.0, -14.0), 0.0, Vec3::ONE)]);

        let (st_side, c) = build_ceremonial_staircase(16.0, 12.0, 3.0, &mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &st_side);
        add_static_batch(
            sp,
            &[
                ModelInstance::from_trs(Vec3::new(-68.0, 10.0, -16.0), 0.0, Vec3::ONE),
                ModelInstance::from_trs(Vec3::new(68.0, 10.0, -16.0), 0.0, Vec3::ONE),
            ],
        );

        let (st_upper, c) = build_ceremonial_staircase(10.0, 6.0, 5.0, &mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &st_upper);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(0.0, 22.0, -53.0), 0.0, Vec3::ONE)]);

        let (taiko_m, c) = build_grand_taiko_bridge(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &taiko_m);
        add_static_batch(
            sp,
            &[
                ModelInstance::from_trs(Vec3::new(0.0, 10.0, 44.0), 0.0, Vec3::ONE),
                ModelInstance::from_trs(Vec3::new(86.0, 10.0, river_z(86.0)), 0.0, Vec3::new(0.65, 0.65, 0.72)),
            ],
        );

        let (cov_m, c) = build_covered_bridge(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &cov_m);
        add_static_batch(
            sp,
            &[
                ModelInstance::from_trs(Vec3::new(-58.0, 10.0, river_z(-58.0)), 0.0, Vec3::ONE),
                ModelInstance::from_trs(Vec3::new(-116.0, 10.0, 11.0), 0.2, Vec3::new(0.75, 0.75, 0.62)),
                ModelInstance::from_trs(Vec3::new(132.0, 10.0, 24.0), FRAC_PI_2, Vec3::new(0.68, 0.68, 0.58)),
            ],
        );

        let (mill_m, wheel_m, c) = build_watermill_house(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &mill_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(-116.0, 10.0, 4.5), FRAC_PI_2, Vec3::ONE)]);
        let wheel_span = push_mesh(&mut vertices, &mut indices, &wheel_m);

        let (pier_m, c) = build_harbor_pier(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &pier_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(119.0, 9.6, 56.0), 0.0, Vec3::ONE)]);

        let (deck_m, c) = build_island_deck(&mut colors);
        total_voxels += c;
        let sp = push_mesh(&mut vertices, &mut indices, &deck_m);
        add_static_batch(sp, &[ModelInstance::from_trs(Vec3::new(-96.0, 9.8, 28.0), 0.0, Vec3::ONE)]);

        // 3. Instanced Village Buildings
        let b_arch = compile_building_archetypes(&mut colors);
        total_voxels += b_arch.total_voxels;
        let group_by_key = |k: BuildingKey| -> Vec<ModelInstance> {
            town_placements
                .iter()
                .filter(|p| p.key == k)
                .map(|p| ModelInstance::from_trs(Vec3::new(p.x, p.y, p.z), p.rot_y, Vec3::ONE))
                .collect()
        };
        for (mesh, key) in [
            (&b_arch.ryokan_blue, BuildingKey::RyokanBlue),
            (&b_arch.ryokan_red, BuildingKey::RyokanRed),
            (&b_arch.machiya_cream, BuildingKey::MachiyaCream),
            (&b_arch.machiya_dark, BuildingKey::MachiyaDark),
            (&b_arch.teahouse, BuildingKey::Teahouse),
            (&b_arch.stall_orange, BuildingKey::StallOrange),
            (&b_arch.stall_blue, BuildingKey::StallBlue),
            (&b_arch.watchtower, BuildingKey::Watchtower),
            (&b_arch.inari, BuildingKey::Inari),
        ] {
            let list = group_by_key(key);
            if !list.is_empty() {
                let sp = push_mesh(&mut vertices, &mut indices, mesh);
                add_static_batch(sp, &list);
            }
        }

        // Torii Gates
        let grand_torii = vec![
            ModelInstance::from_trs(Vec3::new(0.0, 10.0, 174.0), 0.0, Vec3::splat(1.15)),
            ModelInstance::from_trs(Vec3::new(0.0, 10.0, 82.0), 0.0, Vec3::splat(1.05)),
            ModelInstance::from_trs(Vec3::new(0.0, 10.0, 14.0), 0.0, Vec3::splat(1.05)),
            ModelInstance::from_trs(Vec3::new(0.0, 22.0, -38.0), 0.0, Vec3::splat(0.95)),
            ModelInstance::from_trs(Vec3::new(-38.0, 8.0, river_z(-38.0)), 0.0, Vec3::splat(1.2)),
            ModelInstance::from_trs(Vec3::new(132.0, 8.0, -6.0), FRAC_PI_2, Vec3::splat(1.1)),
        ];
        let sp = push_mesh(&mut vertices, &mut indices, &b_arch.torii_grand);
        add_static_batch(sp, &grand_torii);

        let mut small_torii = Vec::with_capacity(42);
        for i in 0..42 {
            let t = 0.02 + (i as f32 / 41.0) * 0.94;
            let pt = senbon_curve.get_point_at(t);
            let tan = senbon_curve.get_tangent_at(t);
            let sy = grid.get_surface_world_y(pt.x, pt.z);
            small_torii.push(ModelInstance::from_trs(
                Vec3::new(pt.x, sy, pt.z),
                tan.x.atan2(tan.z),
                Vec3::splat(0.92),
            ));
        }
        let sp = push_mesh(&mut vertices, &mut indices, &b_arch.torii_small);
        add_static_batch(sp, &small_torii);

        // Stone Lanterns
        let mut lanterns = Vec::new();
        let mut z = 62.0f32;
        while z <= 188.0 {
            if (z - 118.0).abs() >= 12.0 {
                lanterns.push(ModelInstance::from_trs(Vec3::new(-8.0, 10.0, z), 0.0, Vec3::ONE));
                lanterns.push(ModelInstance::from_trs(Vec3::new(8.0, 10.0, z), 0.0, Vec3::ONE));
            }
            z += 18.0;
        }
        for x in [-90.0, -56.0, -24.0, 24.0, 56.0, 90.0] {
            lanterns.push(ModelInstance::from_trs(Vec3::new(x, 10.0, 74.0), 0.0, Vec3::ONE));
            lanterns.push(ModelInstance::from_trs(Vec3::new(x, 10.0, 136.0), 0.0, Vec3::ONE));
        }
        for (lx, ly, lz) in [
            (-8.0, 10.0, 26.0), (8.0, 10.0, 26.0), (-8.0, 10.0, 0.0), (8.0, 10.0, 0.0),
            (-28.0, 22.0, -32.0), (20.0, 22.0, -32.0), (78.0, 22.0, -32.0),
            (-32.0, 28.0, -70.0), (-18.0, 28.0, -62.0), (18.0, 28.0, -62.0), (58.0, 28.0, -70.0),
            (-74.0, 22.0, -38.0), (-38.0, 22.0, -38.0), (16.0, 22.0, -38.0),
        ] {
            lanterns.push(ModelInstance::from_trs(Vec3::new(lx, ly, lz), 0.0, Vec3::ONE));
        }
        let sp = push_mesh(&mut vertices, &mut indices, &b_arch.lantern);
        add_static_batch(sp, &lanterns);

        // 4. Flora
        let tree_arch = compile_tree_archetypes(&mut rng, &mut colors);
        total_voxels += tree_arch.total_voxels;
        let mut sakura_buckets: [Vec<ModelInstance>; 4] = Default::default();
        let mut pine_buckets: [Vec<ModelInstance>; 3] = Default::default();
        let mut bamboo_list: Vec<ModelInstance> = Vec::new();
        let mut rock_list = vec![
            ModelInstance::from_trs(Vec3::new(38.0, 22.0, -41.0), 0.4, Vec3::splat(1.55)),
            ModelInstance::from_trs(Vec3::new(60.0, 22.0, -38.0), 2.1, Vec3::splat(1.35)),
            ModelInstance::from_trs(Vec3::new(48.0, 22.0, -46.0), 1.2, Vec3::splat(0.9)),
        ];

        let hero_sakuras: &[(f32, Option<f32>, f32, f32)] = &[
            (-12.0, None, 162.0, 1.25), (12.0, None, 162.0, 1.25),
            (-12.0, None, 130.0, 1.15), (12.0, None, 130.0, 1.15),
            (-12.0, None, 74.0, 1.2), (12.0, None, 74.0, 1.2),
            (-14.0, None, 24.0, 1.25), (14.0, None, 24.0, 1.25),
            (-42.0, None, 42.0, 1.2), (42.0, None, 42.0, 1.2),
            (-80.0, None, 44.0, 1.15), (96.0, None, 24.0, 1.25),
            (158.0, None, 14.0, 1.3), (160.0, None, -14.0, 1.2), (108.0, None, 12.0, 1.15),
            (-86.0, Some(28.0), -76.0, 1.15), (-32.0, Some(28.0), -94.0, 1.1),
            (62.0, Some(28.0), -78.0, 1.2), (-84.0, Some(22.0), -48.0, 1.15),
            (88.0, Some(22.0), -48.0, 1.1),
        ];
        for (i, &(sx, opt_y, sz, sc)) in hero_sakuras.iter().enumerate() {
            let sy = match opt_y {
                Some(y) => y,
                None => {
                    if is_water_body(sx, sz) {
                        continue;
                    }
                    grid.get_surface_world_y(sx, sz)
                }
            };
            sakura_buckets[i % 4].push(ModelInstance::from_trs(
                Vec3::new(sx, sy, sz),
                rng.next_f32() * TAU,
                Vec3::splat(sc),
            ));
        }

        let near_building = |x: f32, z: f32| -> bool {
            town_placements.iter().any(|b| (x - b.x).hypot(z - b.z) < 10.5)
        };

        for i in 0..155 {
            let x = (rng.next_f32() - 0.5) * 395.0;
            let z = -95.0 + rng.next_f32() * 305.0;
            if is_water_body(x, z) || near_building(x, z) {
                continue;
            }
            if x.abs() < 15.0 && z > -106.0 {
                continue;
            }
            if x.abs() <= 104.0 && (-106.0..=-22.0).contains(&z) {
                continue;
            }
            if x >= 82.0 && z >= 88.0 {
                continue;
            }
            if (z - 78.0).abs() < 9.0 || (z - 106.0).abs() < 9.0 || (z - 140.0).abs() < 9.0 || (z - 174.0).abs() < 9.0 {
                continue;
            }
            let y = grid.get_surface_world_y(x, z);
            if y > 38.0 {
                continue;
            }
            if x < -118.0 && z < 16.0 {
                let sc = 0.95 + rng.next_f32() * 0.45;
                pine_buckets[i % 3].push(ModelInstance::from_trs(Vec3::new(x, y, z), rng.next_f32() * TAU, Vec3::splat(sc)));
            } else {
                let sc = 0.9 + rng.next_f32() * 0.45;
                sakura_buckets[i % 4].push(ModelInstance::from_trs(Vec3::new(x, y, z), rng.next_f32() * TAU, Vec3::splat(sc)));
            }
        }

        for i in 0..140 {
            let side = if rng.next_f32() < 0.5 { -1.0 } else { 1.0 };
            let x = side * (112.0 + rng.next_f32() * 98.0);
            let z = -135.0 + rng.next_f32() * 155.0;
            if is_water_body(x, z) || near_building(x, z) || (x > 105.0 && x < 154.0 && z < -22.0) {
                continue;
            }
            let y = grid.get_surface_world_y(x, z);
            if (10.0..=46.0).contains(&y) {
                let sc = 0.95 + rng.next_f32() * 0.5;
                pine_buckets[i % 3].push(ModelInstance::from_trs(Vec3::new(x, y, z), rng.next_f32() * TAU, Vec3::splat(sc)));
            }
        }

        for _ in 0..160 {
            let x = -195.0 + rng.next_f32() * 82.0;
            let z = -98.0 + rng.next_f32() * 116.0;
            if is_water_body(x, z) || near_building(x, z) {
                continue;
            }
            let y = grid.get_surface_world_y(x, z);
            if y <= 40.0 {
                let sc = 0.85 + rng.next_f32() * 0.45;
                bamboo_list.push(ModelInstance::from_trs(Vec3::new(x, y, z), rng.next_f32() * TAU, Vec3::splat(sc)));
            }
        }

        for _ in 0..55 {
            let ang = rng.next_f32() * TAU;
            let r = 23.0 + rng.next_f32() * 8.0;
            let x = 132.0 + ang.cos() * r;
            let z = -6.0 + ang.sin() * r;
            if !is_water_body(x, z) {
                let y = grid.get_surface_world_y(x, z);
                let sc = 0.8 + rng.next_f32() * 0.8;
                rock_list.push(ModelInstance::from_trs(Vec3::new(x, y, z), rng.next_f32() * TAU, Vec3::splat(sc)));
            }
        }

        for (i, list) in sakura_buckets.iter().enumerate() {
            let sp = push_mesh(&mut vertices, &mut indices, &tree_arch.sakuras[i]);
            add_static_batch(sp, list);
        }
        for (i, list) in pine_buckets.iter().enumerate() {
            let sp = push_mesh(&mut vertices, &mut indices, &tree_arch.pines[i]);
            add_static_batch(sp, list);
        }
        let sp = push_mesh(&mut vertices, &mut indices, &tree_arch.bamboo);
        add_static_batch(sp, &bamboo_list);
        let sp = push_mesh(&mut vertices, &mut indices, &tree_arch.rock);
        add_static_batch(sp, &rock_list);

        // Static translucent Water batch
        let w_span = push_mesh(&mut vertices, &mut indices, &terrain_water);
        let water_inst_start = static_instances.len() as u32;
        static_instances.push(ModelInstance::identity());
        let water_batch = DrawBatch {
            idx_start: w_span.idx_start,
            idx_count: w_span.idx_count,
            base_vertex: w_span.base_vertex,
            inst_start: water_inst_start,
            inst_count: 1,
        };
        total_tris += w_span.tri_count;

        // 5. Dynamic Entities Setup (append dynamic meshes, reserve contiguous dynamic instance range)
        let boat_arch = build_boat_archetypes(&mut colors);
        total_voxels += boat_arch.total_voxels;
        let row_span = push_mesh(&mut vertices, &mut indices, &boat_arch.rowboat);
        let yaka_span = push_mesh(&mut vertices, &mut indices, &boat_arch.yakatabune);

        let (villager_meshes, vc) = build_villager_archetypes(&mut colors);
        total_voxels += vc;
        let villager_spans = [
            push_mesh(&mut vertices, &mut indices, &villager_meshes[0]),
            push_mesh(&mut vertices, &mut indices, &villager_meshes[1]),
            push_mesh(&mut vertices, &mut indices, &villager_meshes[2]),
            push_mesh(&mut vertices, &mut indices, &villager_meshes[3]),
        ];

        let fl_mesh = build_unit_box_mesh(0.45, 0.45, 0.45, MAT_LAMP as u32, pal::LAMP_GLOW, &mut colors);
        let fl_span = push_mesh(&mut vertices, &mut indices, &fl_mesh);

        let cloud_mesh = build_unit_box_mesh(5.5, 3.2, 5.5, MAT_CLOUD as u32, 0xf4f8fc, &mut colors);
        let cloud_span = push_mesh(&mut vertices, &mut indices, &cloud_mesh);

        let mist_mesh = build_unit_box_mesh(0.45, 0.45, 0.45, MAT_MIST as u32, 0xeafafc, &mut colors);
        let mist_span = push_mesh(&mut vertices, &mut indices, &mist_mesh);

        // Boats
        let mut boats_row = Vec::new();
        let mut boats_yaka = Vec::new();
        let mut spawn_boat = |is_yaka: bool, x: f32, z: f32, rot_y: f32, glide_range: f32| {
            let b = BoatAnim {
                base_x: x,
                base_y: 8.0,
                base_z: z,
                base_rot: rot_y,
                glide_range,
                phase: rng.next_f32() * TAU,
                speed: 0.16 + rng.next_f32() * 0.1,
            };
            if is_yaka {
                boats_yaka.push(b);
            } else {
                boats_row.push(b);
            }
        };
        spawn_boat(true, 140.0, 52.0, -0.35, 0.0);
        spawn_boat(true, 150.0, 66.0, 0.28, 0.0);
        spawn_boat(false, 136.0, 61.0, 0.4, 0.0);
        spawn_boat(false, 146.0, 46.0, -0.5, 0.0);
        spawn_boat(false, 156.0, 56.0, 1.1, 0.0);
        spawn_boat(false, 124.0, -10.0, 0.55, 0.0);
        spawn_boat(true, 142.0, -2.0, -0.8, 0.0);
        spawn_boat(false, -100.0, 14.0, -0.25, 0.0);
        spawn_boat(true, -28.0, river_z(-28.0), 0.0, 14.0);
        spawn_boat(false, 32.0, river_z(32.0), 0.0, 22.0);
        spawn_boat(false, -84.0, river_z(-84.0), 0.0, 18.0);
        spawn_boat(false, 64.0, river_z(64.0), 0.0, 12.0);

        // Villagers
        let mut villagers: [Vec<VillagerAnim>; 4] = Default::default();
        let mut spawn_villager = |x: f32, y: f32, z: f32, arch: usize, axis: WalkAxis, range: f32, rot_y: f32, phase: f32, speed: f32| {
            let a = arch % 4;
            villagers[a].push(VillagerAnim {
                base_x: x,
                base_y: y,
                base_z: z,
                axis,
                range,
                rot_y,
                phase,
                speed,
            });
        };
        let p0 = rng.next_f32() * TAU; let s0 = 0.48 + rng.next_f32() * 0.36;
        spawn_villager(-1.5, 14.5, 44.0, 1, WalkAxis::Z, 5.5, 0.0, p0, s0);
        let p0 = rng.next_f32() * TAU; let s0 = 0.48 + rng.next_f32() * 0.36;
        spawn_villager(1.8, 14.5, 42.0, 0, WalkAxis::Z, 4.5, PI, p0, s0);
        let p0 = rng.next_f32() * TAU; let s0 = 0.48 + rng.next_f32() * 0.36;
        spawn_villager(0.2, 14.5, 46.0, 2, WalkAxis::None, 0.0, 0.4, p0, s0);
        let p0 = rng.next_f32() * TAU; let s0 = 0.48 + rng.next_f32() * 0.36;
        spawn_villager(-58.0, 12.6, river_z(-58.0), 3, WalkAxis::Z, 4.5, 0.0, p0, s0);

        let town_spots = [
            (9.5, 95.0, 1, WalkAxis::Z, 5.0), (11.5, 102.0, 2, WalkAxis::Z, 4.0), (-9.5, 96.0, 0, WalkAxis::Z, 5.5),
            (-3.5, 118.0, 3, WalkAxis::X, 6.5), (4.5, 114.0, 1, WalkAxis::X, 5.5), (-16.0, 118.0, 2, WalkAxis::None, 0.0), (16.0, 120.0, 0, WalkAxis::None, 0.0),
            (-2.0, 152.0, 0, WalkAxis::Z, 8.0), (2.5, 168.0, 3, WalkAxis::Z, 7.0), (0.5, 74.0, 1, WalkAxis::Z, 6.0),
            (-45.0, 78.0, 2, WalkAxis::X, 12.0), (-78.0, 78.0, 0, WalkAxis::X, 10.0), (-112.0, 78.0, 3, WalkAxis::X, 9.0),
            (44.0, 78.0, 1, WalkAxis::X, 12.0), (78.0, 78.0, 2, WalkAxis::X, 9.0), (-44.0, 106.0, 0, WalkAxis::X, 10.0),
            (44.0, 106.0, 3, WalkAxis::X, 10.0), (-62.0, 140.0, 1, WalkAxis::X, 11.0), (54.0, 140.0, 2, WalkAxis::X, 11.0),
            (-106.0, 140.0, 0, WalkAxis::X, 9.0), (108.0, 62.0, 3, WalkAxis::Z, 5.0), (122.0, 56.0, 0, WalkAxis::X, 4.0),
        ];
        for (vx, vz, arch, ax, rg) in town_spots {
            let vy = grid.get_surface_world_y(vx, vz);
            let rot = rng.next_f32() * TAU;
            let ph = rng.next_f32() * TAU;
            let sp = 0.48 + rng.next_f32() * 0.36;
            spawn_villager(vx, vy, vz, arch, ax, rg, rot, ph, sp);
        }
        for (vx, vy, vz, arch, ax, rg) in [
            (-12.0, 22.0, -40.0, 0, WalkAxis::X, 8.0), (14.0, 22.0, -40.0, 2, WalkAxis::X, 8.0),
            (34.0, 22.0, -32.0, 3, WalkAxis::None, 0.0), (56.0, 22.0, -48.0, 0, WalkAxis::None, 0.0),
            (-44.0, 28.0, -80.0, 1, WalkAxis::X, 6.0), (-24.0, 28.0, -80.0, 2, WalkAxis::X, 8.0),
            (24.0, 28.0, -68.0, 0, WalkAxis::X, 7.0), (36.0, 28.0, -64.0, 3, WalkAxis::None, 0.0),
            (110.0, 34.0, -36.0, 1, WalkAxis::None, 0.0), (-146.0, 42.0, -80.0, 2, WalkAxis::None, 0.0),
            (-96.0, 10.1, 31.0, 1, WalkAxis::None, 0.0),
        ] {
            let rot = rng.next_f32() * TAU;
            let ph = rng.next_f32() * TAU;
            let sp = 0.48 + rng.next_f32() * 0.36;
            spawn_villager(vx, vy, vz, arch, ax, rg, rot, ph, sp);
        }

        // Clouds
        let mut clouds = Vec::with_capacity(26);
        let mut total_cloud_blocks = 0usize;
        for _ in 0..26 {
            let lx = 3 + (rng.next_f32() * 5.0).floor() as i32;
            let lz = 2 + (rng.next_f32() * 4.0).floor() as i32;
            let mut offsets = Vec::new();
            for dx in -lx..=lx {
                for dz in -lz..=lz {
                    let d = (dx * dx) as f32 / (lx * lx) as f32 + 0.0001;
                    let d2 = (dx * dx) as f32 / ((lx * lx) as f32 + 0.5) + (dz * dz) as f32 / ((lz * lz) as f32 + 0.5);
                    let _ = d;
                    if d2 <= 1.0 {
                        offsets.push([dx as f32 * 4.8, 0.0, dz as f32 * 4.8]);
                        if d2 < 0.45 && rng.next_f32() < 0.78 {
                            offsets.push([dx as f32 * 4.8, 2.8, dz as f32 * 4.8]);
                        }
                    }
                }
            }
            total_cloud_blocks += offsets.len();
            clouds.push(CloudCluster {
                base_x: (rng.next_f32() - 0.5) * 560.0,
                y: 128.0 + rng.next_f32() * 34.0,
                z: (rng.next_f32() - 0.5) * 480.0,
                speed: 1.8 + rng.next_f32() * 1.6,
                offsets,
            });
        }

        // Floating River Paper Lanterns (48)
        let mut floating_lanterns = Vec::with_capacity(48);
        for _ in 0..48 {
            let x = -165.0 + rng.next_f32() * 320.0;
            let _z = river_z(x) + (rng.next_f32() - 0.5) * 7.5;
            floating_lanterns.push(FloatingLantern {
                start_x: x,
                speed: 1.1 + rng.next_f32() * 0.7,
                phase: rng.next_f32() * TAU,
            });
        }

        // Waterfall Mist Particles (320)
        let mut wf_particles = Vec::with_capacity(320);
        for i in 0..320 {
            let is_upper = (i as f32) < 320.0 * 0.45;
            wf_particles.push(WfParticle {
                is_upper,
                t0: rng.next_f32(),
                ox: (rng.next_f32() - 0.5) * 8.5,
                oz: (rng.next_f32() - 0.5) * 4.5,
                speed: 0.5 + rng.next_f32() * 0.45,
                scale: 0.6 + rng.next_f32() * 1.1,
            });
        }

        // Register dynamic batch offsets right after static_instances
        let dynamic_inst_start = static_instances.len() as u32;
        let mut cur_inst = dynamic_inst_start;

        let mut push_dyn_batch = |span: MeshSpan, count: usize| -> DrawBatch {
            let b = DrawBatch {
                idx_start: span.idx_start,
                idx_count: span.idx_count,
                base_vertex: span.base_vertex,
                inst_start: cur_inst,
                inst_count: count as u32,
            };
            cur_inst += count as u32;
            total_tris += span.tri_count * count;
            b
        };

        // Dynamic shadow-casting opaque batches:
        let wheel_batch = push_dyn_batch(wheel_span, 1);
        opaque_shadow_batches.push(wheel_batch);

        let row_batch = push_dyn_batch(row_span, boats_row.len());
        opaque_shadow_batches.push(row_batch);

        let yaka_batch = push_dyn_batch(yaka_span, boats_yaka.len());
        opaque_shadow_batches.push(yaka_batch);

        for i in 0..4 {
            let vb = push_dyn_batch(villager_spans[i], villagers[i].len());
            opaque_shadow_batches.push(vb);
        }

        let fl_batch = push_dyn_batch(fl_span, floating_lanterns.len());
        opaque_shadow_batches.push(fl_batch);

        // Dynamic translucent non-shadow batches:
        let cloud_batch = push_dyn_batch(cloud_span, total_cloud_blocks);
        let mist_batch = push_dyn_batch(mist_span, wf_particles.len());

        let dynamic_inst_count = cur_inst - dynamic_inst_start;
        let (tour_pos, tour_look) = make_tour_curves();
        let data = SceneData {
            title: "Sakura Empire".to_string(),
            vertices,
            indices,
            static_instances,
            opaque_batches: opaque_shadow_batches,
            translucent_batches: vec![water_batch, cloud_batch, mist_batch],
            dynamic_inst_start,
            dynamic_inst_count,
            total_voxels,
            total_tris,
            detail: Vec::new(),
            presets: presets(),
            tour_pos,
            tour_look,
            tour_secs: 75.0,
            max_cam_dist: 460.0,
            min_pitch: 0.02,
            point_lights: NIGHT_LIGHTS.to_vec(),
            point_light_col: [1.0, 0.38, 0.04],
            time_modes: time_modes(),
            shadow: ShadowConfig { target: Vec3::new(0.0, 18.0, -10.0), half_extent: 240.0, near: 20.0, far: 650.0 },
            particles: petals(),
            landmarks: landmarks(),
        };
        let heights = grid.top_y.iter().zip(&grid.water_y).map(|(&t, &w)| t.max(w)).collect();

        Self {
            data,
            heights,
            params: *params,
            boats_row,
            boats_yaka,
            villagers,
            clouds,
            floating_lanterns,
            wf_particles,
        }
    }
}
