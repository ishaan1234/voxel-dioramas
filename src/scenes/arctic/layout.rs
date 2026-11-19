//! World plan for Aurora Village. Everything is placed from math: the village
//! and camp sit where rays from the lake centre leave the shore, cabins ring a
//! square facing inward, igloos form a jittered arc around the fire pit
//! (entrances toward it), icebergs are Poisson-disc samples in the shallows,
//! and the forest is a Poisson disc thinned by an fbm density field.

use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

use super::aurora::{AuroraInst, Curtain};
use super::cabin::{self, CabinStyle};
use super::effects::{Flame, Puff};
use super::pine;
use super::terrain::{smoothstep, Pad, Terrain, ICE_LEVEL, WORLD_HALF};
use crate::engine::prelude::*;
use crate::engine::scene::MAX_POINT_LIGHTS;

#[derive(Clone, Copy)]
pub struct Placed {
    pub pos: Vec3,
    pub rot: f32,
    pub scale: f32,
}

impl Placed {
    pub fn inst(&self) -> ModelInstance {
        ModelInstance::from_trs(self.pos, self.rot, Vec3::splat(self.scale))
    }
    /// World position of a point given in the object's local frame.
    pub fn world(&self, local: Vec3) -> Vec3 {
        let (s, c) = self.rot.sin_cos();
        let l = local * self.scale;
        self.pos + Vec3::new(c * l.x + s * l.z, l.y, -s * l.x + c * l.z)
    }
}

pub struct Building {
    pub style: usize,
    pub at: Placed,
}

pub const STYLES: [CabinStyle; 5] =
    [cabin::LODGE, cabin::COTTAGE_LOG, cabin::COTTAGE_RED, cabin::COTTAGE_OCHRE, cabin::FISHING_HUT];
pub const STYLE_LODGE: usize = 0;
pub const STYLE_HUT: usize = 4;
pub const N_BERG_KINDS: usize = 4;

pub struct Plan {
    pub terrain: Terrain,
    pub village: Vec3,
    pub camp: Vec3,
    pub shore: Vec3,
    pub buildings: Vec<Building>,
    pub igloos: Vec<Placed>,
    pub lamps: Vec<Placed>,
    pub bergs: Vec<Vec<Placed>>,
    pub pines: Vec<Vec<Placed>>,
    pub curtains: Vec<Curtain>,
    pub aurora: Vec<AuroraInst>,
}

fn dir(a: f32) -> Vec2 {
    Vec2::new(a.cos(), a.sin())
}

/// Heading that turns local +X toward world direction `d`.
fn face(d: Vec2) -> f32 {
    (-d.y).atan2(d.x)
}

/// Walk from the lake centre along `a` until the shore is reached.
fn shore_point(t: &Terrain, a: f32) -> Vec2 {
    let d = dir(a);
    let mut r = 5.0;
    while r < 140.0 && t.lake_d(t.lake_c.x + d.x * r, t.lake_c.y + d.y * r) < 0.0 {
        r += 0.5;
    }
    t.lake_c + d * r
}

/// Walk inward from the shore until the lake is `depth` metres deep (signed distance).
fn lake_point(t: &Terrain, a: f32, inside: f32) -> Vec2 {
    let d = dir(a);
    let mut r = 140.0;
    while r > 2.0 && t.lake_d(t.lake_c.x + d.x * r, t.lake_c.y + d.y * r) > -inside {
        r -= 0.5;
    }
    t.lake_c + d * r
}

pub fn plan(params: &WorldParams) -> Plan {
    let canon = params.seed == 0;
    let mut rng = Rng(params.seed.wrapping_mul(0x2C1B_3C6D) ^ 0x297A_2D39);
    let mut jr = Rng(params.seed.wrapping_mul(0x9E37_79B9) ^ 0x6A09_E667);
    let mut jit = |r: f32| if canon { 0.0 } else { (jr.next_f32() - 0.5) * 2.0 * r };
    let mut t = Terrain::new(params.seed, params.season);

    // ---- Sites -----------------------------------------------------------
    let v_ang = PI * 0.5 + 0.18 + jit(0.55);
    let c_ang = v_ang + 1.0 + jit(0.25);
    let shore_v = shore_point(&t, v_ang);
    let village2 = shore_v + dir(v_ang) * (31.0 + jit(4.0));
    let camp2 = shore_point(&t, c_ang) + dir(c_ang) * (21.0 + jit(3.0));
    let pad_h = |t: &Terrain, p: Vec2| t.height(p.x, p.y).max(1.5);
    let h = pad_h(&t, village2);
    t.pads.push(Pad { c: village2, r: 11.0, h });
    let h = pad_h(&t, camp2);
    t.pads.push(Pad { c: camp2, r: 9.0, h });

    // ---- Lodge + cottages around the square, doors facing it ------------
    let away = v_ang; // direction from the lake through the village
    let mut buildings = Vec::new();
    let order = [0i32, -1, 1, -2, 2];
    let cottage_styles = [1usize, 2, 3, 2];
    for (i, &k) in order.iter().enumerate() {
        let a = away + k as f32 * 0.66 + jit(0.12);
        let r = if k == 0 { 21.0 } else { 17.5 + jit(2.5) + (k.abs() as f32) * 0.8 };
        let p2 = village2 + dir(a) * r;
        let style = if k == 0 { STYLE_LODGE } else { cottage_styles[(i - 1 + if canon { 0 } else { params.seed as usize }) % 4] };
        let st = &STYLES[style];
        let h = pad_h(&t, p2);
        t.pads.push(Pad { c: p2, r: st.len.max(st.wid) * 0.5 + 1.8, h });
        let rot = face(village2 - p2) + jit(0.15);
        buildings.push(Building { style, at: Placed { pos: Vec3::new(p2.x, h, p2.y), rot, scale: 1.0 } });
    }

    // ---- Igloo camp: jittered arc open toward the lake ------------------------
    let n_igloo = if canon { 6 } else { 5 + (params.seed % 3) as usize };
    let to_lake = c_ang + PI;
    let mut igloos = Vec::new();
    for k in 0..n_igloo {
        let span = TAU - 1.7;
        let a = to_lake + 0.85 + span * k as f32 / (n_igloo as f32 - 1.0) + jit(0.16) + if canon { [0.0, 0.1, -0.08, 0.06, -0.1, 0.0][k % 6] } else { 0.0 };
        let r = 13.0 + if canon { [0.4, -0.8, 1.2, -0.3, 0.9, -0.6][k % 6] } else { jit(1.6) };
        let p2 = camp2 + dir(a) * r;
        let scale = if canon { [1.0, 0.84, 1.08, 0.9, 1.02, 0.86][k % 6] } else { 0.84 + (jit(0.5) + 0.5) * 0.24 };
        let h = pad_h(&t, p2);
        t.pads.push(Pad { c: p2, r: 4.6 * scale, h });
        let rot = face(camp2 - p2) + if canon { [0.1, -0.15, 0.05, 0.18, -0.08, 0.12][k % 6] } else { jit(0.2) };
        igloos.push(Placed { pos: Vec3::new(p2.x, h, p2.y), rot, scale });
    }

    // ---- Fishing hut on the ice -------------------------------------------
    let hut_ang = v_ang - 0.75 + jit(0.3);
    let hut2 = lake_point(&t, hut_ang, 13.0);
    let hut = Placed { pos: Vec3::new(hut2.x, ICE_LEVEL, hut2.y), rot: face(shore_point(&t, hut_ang) - hut2) + 0.4, scale: 1.0 };
    buildings.push(Building { style: STYLE_HUT, at: hut });

    // ---- Footpaths + lamp posts ----------------------------------------------
    let mut paths = Vec::new();
    for b in buildings.iter().filter(|b| b.style != STYLE_HUT) {
        let st = &STYLES[b.style];
        let door = b.at.world(Vec3::new(st.len * 0.5 + 1.4, 0.0, 0.0));
        paths.push((village2, Vec2::new(door.x, door.z)));
    }
    let shore_c = shore_point(&t, c_ang);
    paths.push((village2, camp2));
    paths.push((village2, shore_v + (village2 - shore_v).normalize() * 2.0));
    paths.push((camp2, shore_c + (camp2 - shore_c).normalize() * 2.0));
    t.paths = paths.clone();
    let mut lamps = Vec::new();
    for (i, &(a, b)) in paths.iter().enumerate().skip(buildings.len() - 1) {
        let len = (b - a).length();
        let n = (len / 12.0).floor() as usize;
        let d = (b - a) / len;
        let side = Vec2::new(-d.y, d.x) * if i % 2 == 0 { 1.7 } else { -1.7 };
        for k in 1..=n {
            let p = a + d * (k as f32 * len / (n as f32 + 1.0)) + side * if k % 2 == 0 { 1.0 } else { -1.0 };
            lamps.push(Placed { pos: Vec3::new(p.x, t.ground_top(p.x, p.y) - 0.05, p.y), rot: face(-side), scale: 1.0 });
        }
    }

    // ---- Icebergs in the shallows (Poisson disc) --------------------------
    let reach = t.lake_r.max_element() + t.lake_warp * 1.5;
    let lc = t.lake_c;
    let keep_clear = [hut2, shore_v, lake_point(&t, v_ang, 1.0)];
    let pts = poisson_disc(&mut rng, reach, 10.0, 600, |x, z| {
        let (wx, wz) = (lc.x + x, lc.y + z);
        let d = t.lake_d(wx, wz);
        let p = Vec2::new(wx, wz);
        d < -5.5 && d > -24.0 && keep_clear.iter().all(|k| (*k - p).length() > 16.0)
    });
    let mut bergs: Vec<Vec<Placed>> = (0..N_BERG_KINDS).map(|_| Vec::new()).collect();
    let mut rng2 = Rng(params.seed ^ 0x51ED_27A1);
    for (i, &(x, z)) in pts.iter().take(18).enumerate() {
        let (wx, wz) = (lc.x + x, lc.y + z);
        let room = -t.lake_d(wx, wz) - 3.0;
        let u = rng2.next_f32();
        let scale = (0.45 + u * u * 1.15).min(room / 6.5).max(0.35);
        let kind = (i + (rng2.next_f32() * 4.0) as usize) % N_BERG_KINDS;
        let pl = Placed { pos: Vec3::new(wx, ICE_LEVEL + 0.13, wz), rot: rng2.next_f32() * TAU, scale };
        bergs[kind].push(pl);
    }

    // ---- Pine forest: Poisson disc thinned by an fbm density field ------------
    let o = t.off;
    let blds: Vec<Vec2> = buildings.iter().map(|b| Vec2::new(b.at.pos.x, b.at.pos.z)).collect();
    let igl: Vec<Vec2> = igloos.iter().map(|g| Vec2::new(g.pos.x, g.pos.z)).collect();
    let mut pine_pts = poisson_disc(&mut rng, 212.0, 6.4, 5200, |x, z| {
        if x.abs() > WORLD_HALF - 3.0 || z.abs() > WORLD_HALF - 3.0 {
            return false;
        }
        if t.lake_d(x, z) < 6.0 {
            return false;
        }
        let p = Vec2::new(x, z);
        if (p - village2).length() < 33.0 || (p - camp2).length() < 22.0 {
            return false;
        }
        if blds.iter().any(|b| (*b - p).length() < 9.0) || igl.iter().any(|g| (*g - p).length() < 6.0) {
            return false;
        }
        let h = t.height(x, z);
        if h > 46.0 || t.slope(x, z) > 0.85 {
            return false;
        }
        let dens = fbm2(x * 0.021 + o.x + 50.0, z * 0.021 + o.y, 3);
        let near_v = smoothstep(33.0, 60.0, (p - village2).length());
        dens * (0.6 + 0.4 * near_v) > 0.36
    });
    pine_pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut pines: Vec<Vec<Placed>> = (0..pine::SHAPES.len()).map(|_| Vec::new()).collect();
    for &(x, z) in &pine_pts {
        let hsh = hash2(x * 0.37, z * 0.71);
        let kind = (hsh * 31.0) as usize % pine::SHAPES.len();
        let scale = 0.72 + hash2(z, x) * 0.55;
        let y = t.height(x, z) - 0.25;
        pines[kind].push(Placed { pos: Vec3::new(x, y, z), rot: hsh * TAU * 7.0, scale });
    }

    // ---- Aurora: long curtains arcing across the northern sky -------------
    let base = [
        (Vec3::new(-152.0, 232.0, -938.0), 0.12, Curtain { len: 1050.0, height: 276.0, amp: 76.0, seed: 1.3 }),
        (Vec3::new(359.0, 270.0, -828.0), -0.45, Curtain { len: 773.0, height: 235.0, amp: 62.0, seed: 4.1 }),
        (Vec3::new(-580.0, 207.0, -718.0), 0.62, Curtain { len: 690.0, height: 220.0, amp: 55.0, seed: 7.7 }),
        (Vec3::new(90.0, 320.0, -1050.0), -0.04, Curtain { len: 1000.0, height: 280.0, amp: 80.0, seed: 2.6 }),
    ];
    let mut curtains = Vec::new();
    let mut aurora = Vec::new();
    for (i, (p, yaw, c)) in base.iter().enumerate() {
        let mut c = *c;
        c.seed += jit(3.0);
        c.amp *= 1.0 + jit(0.25);
        let pos = *p + Vec3::new(jit(95.0), jit(28.0), jit(60.0));
        curtains.push(c);
        aurora.push(AuroraInst { pos, yaw: yaw + jit(0.2), phase: i as f32 * 2.1 + jit(1.0), speed: 0.11 + i as f32 * 0.017 });
    }

    // Settle everything on the final voxel surface (pads are all in place now).
    for b in buildings.iter_mut().filter(|b| b.style != STYLE_HUT) {
        b.at.pos.y = t.ground_top(b.at.pos.x, b.at.pos.z);
    }
    for g in igloos.iter_mut() {
        g.pos.y = t.ground_top(g.pos.x, g.pos.z);
    }
    let top = |p: Vec2| Vec3::new(p.x, t.ground_top(p.x, p.y), p.y);
    Plan {
        village: top(village2),
        camp: top(camp2),
        shore: top(shore_v),
        terrain: t,
        buildings,
        igloos,
        lamps,
        bergs,
        pines,
        curtains,
        aurora,
    }
}

/// Up to 12 warm point lights: fire, lodge, cottages, hut, then lamp posts.
pub fn lights(plan: &Plan) -> Vec<[f32; 4]> {
    let mut v: Vec<[f32; 4]> = Vec::new();
    let f = plan.camp;
    v.push([f.x, f.y + 1.4, f.z, 24.0]);
    for b in &plan.buildings {
        let st = &STYLES[b.style];
        let front = b.at.world(Vec3::new(st.len * 0.5 + 1.6, 2.2, 0.0));
        let r = if b.style == STYLE_LODGE { 22.0 } else if b.style == STYLE_HUT { 10.0 } else { 15.0 };
        v.push([front.x, front.y, front.z, r]);
    }
    for l in plan.lamps.iter().step_by(2) {
        if v.len() >= MAX_POINT_LIGHTS {
            break;
        }
        v.push([l.pos.x + 0.4, l.pos.y + 2.6, l.pos.z, 11.0]);
    }
    v.truncate(MAX_POINT_LIGHTS);
    v
}

pub fn landmarks(plan: &Plan) -> Vec<Landmark> {
    let t = &plan.terrain;
    let lodge = plan.buildings.iter().find(|b| b.style == STYLE_LODGE).unwrap();
    let hut = plan.buildings.iter().find(|b| b.style == STYLE_HUT).unwrap();
    let biggest = plan.bergs.iter().flatten().max_by(|a, b| a.scale.total_cmp(&b.scale)).map(|b| b.pos).unwrap_or(plan.shore);
    let all_pines: Vec<Vec3> = plan.pines.iter().flatten().map(|p| p.pos).collect();
    // Densest stand: the pine with the most neighbours within 18 m.
    let grove = all_pines
        .iter()
        .max_by_key(|p| all_pines.iter().filter(|q| (**q - **p).length() < 18.0).count())
        .copied()
        .unwrap_or(plan.village);
    // Highest peak: sample the rim on a coarse grid.
    let mut peak = Vec3::new(0.0, -1e9, 0.0);
    for iz in -14..=14 {
        for ix in -14..=14 {
            let (x, z) = (ix as f32 * 10.0, iz as f32 * 10.0);
            let h = t.height(x, z);
            if h > peak.y {
                peak = Vec3::new(x, h, z);
            }
        }
    }
    let a = &plan.aurora[0];
    vec![
        Landmark { name: "Frozen Lake".into(), pos: Vec3::new(t.lake_c.x, ICE_LEVEL, t.lake_c.y), radius: t.lake_r.max_element(), kind: "water" },
        Landmark { name: "Village Square".into(), pos: plan.village + Vec3::Y * 2.0, radius: 14.0, kind: "village" },
        Landmark { name: "The Lodge".into(), pos: lodge.at.pos + Vec3::Y * 4.0, radius: 9.0, kind: "building" },
        Landmark { name: "Igloo Camp".into(), pos: plan.camp + Vec3::Y * 2.0, radius: 14.0, kind: "village" },
        Landmark { name: "Campfire".into(), pos: plan.camp + Vec3::Y * 0.8, radius: 3.0, kind: "fire" },
        Landmark { name: "Ice Fishing Hut".into(), pos: hut.at.pos + Vec3::Y * 1.5, radius: 3.0, kind: "building" },
        Landmark { name: "Iceberg Shoals".into(), pos: biggest + Vec3::Y * 3.0, radius: 10.0, kind: "ice" },
        Landmark { name: "Pine Forest".into(), pos: grove + Vec3::Y * 5.0, radius: 20.0, kind: "forest" },
        Landmark { name: "Northern Peaks".into(), pos: peak, radius: 30.0, kind: "mountain" },
        Landmark { name: "Aurora Borealis".into(), pos: a.pos + Vec3::Y * 90.0, radius: 300.0, kind: "sky" },
    ]
}

pub fn presets(plan: &Plan, lm: &[Landmark]) -> Vec<CameraPreset> {
    let t = &plan.terrain;
    let lake = Vec3::new(t.lake_c.x, ICE_LEVEL, t.lake_c.y);
    let v = plan.village;
    let c = plan.camp;
    let lodge = plan.buildings.iter().find(|b| b.style == STYLE_LODGE).unwrap();
    let hut = plan.buildings.iter().find(|b| b.style == STYLE_HUT).unwrap();
    let out = |from: Vec3, to: Vec3| (to - from).with_y(0.0).normalize_or(Vec3::Z);
    let vd = out(lake, v); // lake -> village
    let side = vd.cross(Vec3::Y);
    let cd = out(lake, c);
    let csd = cd.cross(Vec3::Y);
    let ld = out(lodge.at.pos, v); // lodge door direction
    let berg = lm[6].pos;
    let bd = out(lake, berg);
    let up = |p: Vec3, dy: f32| p + Vec3::Y * dy;
    vec![
        CameraPreset { name: "Aurora Village", pos: up(lake + vd * 168.0 - side * 22.0, 40.0), target: up(lake - vd * 22.0, 31.0) },
        CameraPreset { name: "Village Square", pos: up(v - vd * 16.0 + side * 7.0, 8.0), target: up(v + vd * 9.0, 3.0) },
        CameraPreset { name: "The Lodge", pos: up(lodge.at.pos + ld * 17.0 + ld.cross(Vec3::Y) * 6.0, 6.5), target: up(lodge.at.pos, 3.8) },
        CameraPreset { name: "Igloo Camp", pos: up(c + cd * 22.0 + csd * 9.0, 12.0), target: up(c - cd * 2.0, 1.5) },
        CameraPreset { name: "Frozen Lake", pos: up(plan.shore + vd * 6.0 + side * 8.0, 7.0), target: up(lake - vd * 20.0, 4.0) },
        CameraPreset { name: "Iceberg Shoals", pos: up(berg - bd * 26.0 + bd.cross(Vec3::Y) * 10.0, 9.0), target: up(berg, 2.0) },
        CameraPreset { name: "Ice Fishing", pos: up(hut.at.pos + out(lake, hut.at.pos) * 9.0 + Vec3::new(5.0, 0.0, 3.0), 4.0), target: up(hut.at.pos, 1.4) },
        CameraPreset { name: "Pine Forest", pos: up(lm[7].pos + out(lake, lm[7].pos) * -30.0 + out(lake, lm[7].pos).cross(Vec3::Y) * 14.0, 16.0), target: up(lm[7].pos, -2.0) },
        CameraPreset { name: "Northern Lights", pos: up(plan.shore + vd * 30.0 + side * 10.0, 24.0), target: up(plan.shore - vd * 60.0, 40.0) },
    ]
}

pub fn tour(views: &[CameraPreset], center: Vec3) -> (CatmullRomCurve3, CatmullRomCurve3) {
    let mut pos = Vec::new();
    let mut look = Vec::new();
    for (i, v) in views.iter().enumerate() {
        pos.push(v.pos);
        look.push(v.target);
        let a = i as f32 / views.len() as f32 * TAU + 0.4;
        pos.push(center + Vec3::new(a.cos() * 120.0, 55.0 + (a * 2.0).sin() * 15.0, a.sin() * 120.0));
        look.push(center + Vec3::Y * 8.0);
    }
    (CatmullRomCurve3::new(pos, true, 0.4), CatmullRomCurve3::new(look, true, 0.4))
}

pub fn smoke_and_flames(plan: &Plan, smoke_local: &[Vec3]) -> (Vec<Puff>, Vec<Flame>) {
    let srcs: Vec<Vec3> = plan.buildings.iter().map(|b| b.at.world(smoke_local[b.style])).collect();
    (super::effects::make_smoke(&srcs), super::effects::make_flames(plan.camp))
}
