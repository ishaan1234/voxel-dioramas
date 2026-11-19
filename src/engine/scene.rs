//! Scene abstraction: everything the renderer needs to know about a diorama.

use glam::Vec3;

use super::math::CatmullRomCurve3;
use super::voxel::{MeshData, ModelInstance, VoxelVertex};

pub const MAX_POINT_LIGHTS: usize = 12;

/// Season a world is generated for (palettes, foliage, snow cover...).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
}

/// Inputs to world generation. `seed == 0` is each scene's canonical,
/// hand-tuned layout; any other seed produces a new variation of it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WorldParams {
    pub seed: u32,
    pub season: Season,
}

impl Default for WorldParams {
    fn default() -> Self {
        Self { seed: 0, season: Season::Spring }
    }
}

/// A named point of interest in a scene: used as courier stops, photo-hunt
/// subjects, ambient-tour waypoints, etc.
#[derive(Clone, Debug)]
pub struct Landmark {
    pub name: String,
    /// World position of the thing itself.
    pub pos: Vec3,
    /// Rough size (metres), for framing and proximity checks.
    pub radius: f32,
    /// Free-form tag, e.g. "island", "building", "tree", "bridge", "water".
    pub kind: &'static str,
}

#[derive(Copy, Clone, Debug)]
pub struct DrawBatch {
    pub idx_start: u32,
    pub idx_count: u32,
    pub base_vertex: i32,
    pub inst_start: u32,
    pub inst_count: u32,
}

pub struct CameraPreset {
    pub name: &'static str,
    pub pos: Vec3,
    pub target: Vec3,
}

/// Lighting / atmosphere for one time-of-day mode.
#[derive(Clone)]
pub struct TimeModeParams {
    pub name: &'static str,
    pub sky_top: [f32; 3],
    pub sky_mid: [f32; 3],
    pub sky_bot: [f32; 3],
    pub fog_col: [f32; 3],
    pub fog_d: f32,
    pub sun_col: [f32; 3],
    pub sun_int: f32,
    pub sun_pos: Vec3,
    pub hemi_sky: [f32; 3],
    pub hemi_gnd: [f32; 3],
    pub hemi_int: f32,
    pub win_emissive: f32,
    pub lamp_emissive: f32,
    pub point_int: f32,
    pub exposure: f32,
    /// Procedural star-field intensity in the sky shader (0 = none).
    pub stars: f32,
}

/// GPU-simulated falling particles (petals, leaves, sparks...).
#[derive(Clone)]
pub struct ParticleConfig {
    pub count: u32,
    pub seed: u32,
    /// Spawn / wrap volume.
    pub bmin: Vec3,
    pub bmax: Vec3,
    /// Fall speed range (units/s, positive = downward).
    pub fall: (f32, f32),
    /// Horizontal drift speed range.
    pub drift: (f32, f32),
    /// Half-size of each particle cube (x, y, z).
    pub size: Vec3,
    /// Four sRGB colours picked per particle.
    pub colors: [u32; 4],
    /// Multiplier on particle brightness (1.0 = unlit albedo).
    pub glow: f32,
}

pub struct ShadowConfig {
    pub target: Vec3,
    pub half_extent: f32,
    pub near: f32,
    pub far: f32,
}

/// Many small instanced props (flowers, grass) drawn with per-instance level
/// of detail: each instance picks the first LOD whose `max_dist` (metres at
/// 1080p; scaled with screen height) exceeds its camera distance.
pub struct DetailSet {
    pub lods: Vec<(MeshSpan, f32)>,
    pub instances: Vec<ModelInstance>,
    /// Bounding radius of one unscaled instance, for frustum tests.
    pub radius: f32,
}

/// Fully built, renderer-ready scene data.
pub struct SceneData {
    pub title: String,
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
    pub static_instances: Vec<ModelInstance>,
    /// Opaque, shadow-casting batches (static and dynamic).
    pub opaque_batches: Vec<DrawBatch>,
    /// Alpha-blended batches drawn after opaque geometry, in order.
    pub translucent_batches: Vec<DrawBatch>,
    pub dynamic_inst_start: u32,
    pub dynamic_inst_count: u32,
    pub total_voxels: u64,
    pub total_tris: usize,
    /// Per-instance LOD props, drawn with the opaque pass.
    pub detail: Vec<DetailSet>,
    pub presets: Vec<CameraPreset>,
    pub tour_pos: CatmullRomCurve3,
    pub tour_look: CatmullRomCurve3,
    pub tour_secs: f32,
    pub max_cam_dist: f32,
    /// Lowest orbit pitch in radians (negative lets the camera look up from below).
    pub min_pitch: f32,
    pub point_lights: Vec<[f32; 4]>,
    pub point_light_col: [f32; 3],
    pub time_modes: [TimeModeParams; 3],
    pub shadow: ShadowConfig,
    pub particles: ParticleConfig,
    /// Points of interest (may be empty).
    pub landmarks: Vec<Landmark>,
}

impl SceneData {
    pub fn total_draws(&self) -> usize {
        self.opaque_batches.len() + self.translucent_batches.len()
    }

    pub fn point_lights_padded(&self) -> [[f32; 4]; MAX_POINT_LIGHTS] {
        let mut out = [[0.0f32; 4]; MAX_POINT_LIGHTS];
        for (o, l) in out.iter_mut().zip(self.point_lights.iter()) {
            *o = *l;
        }
        out
    }
}

pub trait Scene {
    fn data(&self) -> &SceneData;
    /// Fill `out` with exactly `data().dynamic_inst_count` instances for time `t`.
    fn update_dynamic_instances(&self, t: f32, out: &mut Vec<ModelInstance>);
    /// Walkable ground height at world (x, z), if any. Used for picking
    /// (tap-to-place) and for keeping cameras / gliders above ground.
    fn surface_y(&self, _x: f32, _z: f32) -> Option<f32> {
        None
    }
    /// For experiences that need scene-specific data (downcast with
    /// `scene.as_any().downcast_ref::<MyScene>()`).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// March a ray against `Scene::surface_y`; returns the first ground hit.
pub fn raycast_surface(scene: &dyn Scene, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Vec3> {
    let dir = dir.normalize_or_zero();
    let mut t = 0.0;
    let mut prev: Option<(f32, f32)> = None; // (t, height above ground)
    while t < max_dist {
        let p = origin + dir * t;
        if let Some(g) = scene.surface_y(p.x, p.z) {
            let h = p.y - g;
            if h <= 0.0 {
                // Refine between the previous sample and this one.
                let (t0, h0) = prev.unwrap_or((t, h));
                let f = if (h0 - h).abs() > 1e-5 { h0 / (h0 - h) } else { 0.0 };
                let th = t0 + (t - t0) * f.clamp(0.0, 1.0);
                let q = origin + dir * th;
                return Some(Vec3::new(q.x, scene.surface_y(q.x, q.z).unwrap_or(q.y), q.z));
            }
            prev = Some((t, h));
            t += (h * 0.5).clamp(0.25, 4.0);
        } else {
            prev = None;
            t += 2.0;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// SceneBuilder: accumulates meshes + instances, then resolves dynamic offsets.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub struct MeshSpan {
    pub idx_start: u32,
    pub idx_count: u32,
    pub base_vertex: i32,
    pub tri_count: usize,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Pass {
    Opaque,
    Translucent,
}

struct PendingDyn {
    span: MeshSpan,
    rel_start: u32,
    count: u32,
    pass: Pass,
}

#[derive(Default)]
pub struct SceneBuilder {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
    pub static_instances: Vec<ModelInstance>,
    opaque: Vec<DrawBatch>,
    translucent: Vec<DrawBatch>,
    dynamic: Vec<PendingDyn>,
    dyn_count: u32,
    detail: Vec<DetailSet>,
    pub total_voxels: u64,
    pub total_tris: usize,
}

pub struct BuiltGeometry {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
    pub static_instances: Vec<ModelInstance>,
    pub opaque_batches: Vec<DrawBatch>,
    pub translucent_batches: Vec<DrawBatch>,
    pub dynamic_inst_start: u32,
    pub dynamic_inst_count: u32,
    pub detail: Vec<DetailSet>,
    pub total_voxels: u64,
    pub total_tris: usize,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_mesh(&mut self, mesh: &MeshData) -> MeshSpan {
        let span = MeshSpan {
            idx_start: self.indices.len() as u32,
            idx_count: mesh.indices.len() as u32,
            base_vertex: self.vertices.len() as i32,
            tri_count: mesh.tri_count(),
        };
        self.vertices.extend_from_slice(&mesh.vertices);
        self.indices.extend_from_slice(&mesh.indices);
        span
    }

    /// Add a mesh drawn with fixed transforms.
    pub fn add_static(&mut self, span: MeshSpan, transforms: &[ModelInstance], pass: Pass) {
        if span.idx_count == 0 || transforms.is_empty() {
            return;
        }
        let b = DrawBatch {
            idx_start: span.idx_start,
            idx_count: span.idx_count,
            base_vertex: span.base_vertex,
            inst_start: self.static_instances.len() as u32,
            inst_count: transforms.len() as u32,
        };
        self.static_instances.extend_from_slice(transforms);
        self.total_tris += span.tri_count * transforms.len();
        match pass {
            Pass::Opaque => self.opaque.push(b),
            Pass::Translucent => self.translucent.push(b),
        }
    }

    /// Convenience: compile a mesh and add it statically in one go.
    pub fn add_mesh(&mut self, mesh: &MeshData, voxels: u64, transforms: &[ModelInstance], pass: Pass) {
        self.total_voxels += voxels;
        let span = self.push_mesh(mesh);
        self.add_static(span, transforms, pass);
    }

    /// Register instanced props with several LOD meshes (finest first).
    pub fn add_detail(&mut self, lods: Vec<(MeshSpan, f32)>, instances: Vec<ModelInstance>, radius: f32) {
        if instances.is_empty() || lods.is_empty() {
            return;
        }
        self.total_tris += lods[0].0.tri_count * instances.len();
        self.detail.push(DetailSet { lods, instances, radius });
    }

    /// Reserve `count` per-frame instances. The scene's `update_dynamic_instances`
    /// must emit them in the same order these calls were made.
    pub fn add_dynamic(&mut self, span: MeshSpan, count: usize, pass: Pass) {
        self.dynamic.push(PendingDyn { span, rel_start: self.dyn_count, count: count as u32, pass });
        self.dyn_count += count as u32;
        self.total_tris += span.tri_count * count;
    }

    pub fn finish(mut self) -> BuiltGeometry {
        let base = self.static_instances.len() as u32;
        for d in &self.dynamic {
            if d.count == 0 || d.span.idx_count == 0 {
                continue;
            }
            let b = DrawBatch {
                idx_start: d.span.idx_start,
                idx_count: d.span.idx_count,
                base_vertex: d.span.base_vertex,
                inst_start: base + d.rel_start,
                inst_count: d.count,
            };
            match d.pass {
                Pass::Opaque => self.opaque.push(b),
                Pass::Translucent => self.translucent.push(b),
            }
        }
        BuiltGeometry {
            vertices: self.vertices,
            indices: self.indices,
            static_instances: self.static_instances,
            opaque_batches: self.opaque,
            translucent_batches: self.translucent,
            dynamic_inst_start: base,
            dynamic_inst_count: self.dyn_count,
            detail: self.detail,
            total_voxels: self.total_voxels,
            total_tris: self.total_tris,
        }
    }
}

/// Shared sanity checks used by every scene's unit test.
#[cfg(test)]
pub fn assert_scene_consistent(scene: &dyn Scene) {
    let d = scene.data();
    assert!(!d.vertices.is_empty());
    assert_eq!(d.indices.len() % 3, 0);
    assert_eq!(d.static_instances.len() as u32, d.dynamic_inst_start);
    let total_inst = d.dynamic_inst_start + d.dynamic_inst_count;
    for b in d.opaque_batches.iter().chain(d.translucent_batches.iter()) {
        assert!((b.idx_start + b.idx_count) as usize <= d.indices.len());
        assert!(b.base_vertex >= 0);
        assert!(b.inst_start + b.inst_count <= total_inst);
    }
    let max_v = d.vertices.len() as i64;
    for b in d.opaque_batches.iter().chain(d.translucent_batches.iter()) {
        for &i in &d.indices[b.idx_start as usize..(b.idx_start + b.idx_count) as usize] {
            assert!((i as i64 + b.base_vertex as i64) < max_v);
        }
    }
    let mut buf = Vec::new();
    for t in [0.0f32, 12.5, 333.3] {
        scene.update_dynamic_instances(t, &mut buf);
        assert_eq!(buf.len(), d.dynamic_inst_count as usize);
    }
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        assert!(d.tour_pos.get_point_at(t).is_finite());
        assert!(d.tour_look.get_point_at(t).is_finite());
    }
    assert!(!d.presets.is_empty() && d.presets.len() <= 9);
    assert!(d.point_lights.len() <= MAX_POINT_LIGHTS);
    for m in &d.time_modes {
        assert!(m.sun_int > 0.0 && m.exposure > 0.0);
    }
}
