//! CPU frustum culling.
//!
//! At load time every static batch gets a world-space bounding sphere. Instanced
//! batches whose instances are spread over the world (trees, lanterns...) are
//! first split into spatial clusters by sorting their instances into an XZ grid,
//! so a cluster that is off-screen can be skipped without skipping the rest.

use glam::{Mat4, Vec3, Vec4};

use super::scene::DrawBatch;
use super::voxel::{ModelInstance, VoxelVertex};

#[derive(Copy, Clone, Debug)]
pub struct Sphere {
    pub c: Vec3,
    pub r: f32,
}

/// Batches plus optional bounds (`None` = always draw, e.g. animated batches).
#[derive(Default)]
pub struct DrawList {
    pub batches: Vec<DrawBatch>,
    pub bounds: Vec<Option<Sphere>>,
}

/// Local-space AABB of the vertices a batch's indices reference.
fn mesh_aabb(b: &DrawBatch, vertices: &[VoxelVertex], indices: &[u32]) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for &i in &indices[b.idx_start as usize..(b.idx_start + b.idx_count) as usize] {
        let p = Vec3::from(vertices[(i as i64 + b.base_vertex as i64) as usize].pos);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo, hi)
}

fn instance_sphere(m: &ModelInstance, local: Sphere) -> Sphere {
    let mat = Mat4::from_cols_array_2d(&m.model);
    let scale = mat.x_axis.truncate().length().max(mat.y_axis.truncate().length()).max(mat.z_axis.truncate().length());
    Sphere { c: mat.transform_point3(local.c), r: local.r * scale }
}

fn merge(spheres: &[Sphere]) -> Sphere {
    let lo = spheres.iter().fold(Vec3::splat(f32::MAX), |a, s| a.min(s.c - Vec3::splat(s.r)));
    let hi = spheres.iter().fold(Vec3::splat(f32::MIN), |a, s| a.max(s.c + Vec3::splat(s.r)));
    let c = (lo + hi) * 0.5;
    let r = spheres.iter().fold(0.0f32, |a, s| a.max(s.c.distance(c) + s.r));
    Sphere { c, r }
}

/// Build a cullable draw list. `instances` is the full instance array; static
/// batch instance ranges are reordered in place by grid cell (`cell` metres).
/// Batches whose instances start at or after `dynamic_start` are left as-is.
pub fn build_draw_list(
    batches: &[DrawBatch],
    vertices: &[VoxelVertex],
    indices: &[u32],
    instances: &mut [ModelInstance],
    dynamic_start: u32,
    cell: f32,
) -> DrawList {
    let mut out = DrawList::default();
    for b in batches {
        if b.idx_count == 0 || b.inst_count == 0 {
            continue;
        }
        if b.inst_start >= dynamic_start {
            out.batches.push(*b);
            out.bounds.push(None);
            continue;
        }
        let (lo, hi) = mesh_aabb(b, vertices, indices);
        let local = Sphere { c: (lo + hi) * 0.5, r: (hi - lo).length() * 0.5 };

        let range = b.inst_start as usize..(b.inst_start + b.inst_count) as usize;
        let slice = &mut instances[range];
        let key = |m: &ModelInstance| {
            let t = m.model[3];
            ((t[0] / cell).floor() as i32, (t[2] / cell).floor() as i32)
        };
        slice.sort_by_key(|m| key(m));

        let mut start = 0;
        while start < slice.len() {
            let k = key(&slice[start]);
            let mut end = start + 1;
            while end < slice.len() && key(&slice[end]) == k {
                end += 1;
            }
            let spheres: Vec<Sphere> = slice[start..end].iter().map(|m| instance_sphere(m, local)).collect();
            out.batches.push(DrawBatch { inst_start: b.inst_start + start as u32, inst_count: (end - start) as u32, ..*b });
            out.bounds.push(Some(merge(&spheres)));
            start = end;
        }
    }
    out
}

/// The six frustum planes (inward normals) of a view-projection matrix with a
/// 0..1 depth range.
pub fn frustum_planes(vp: Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (vp.row(0), vp.row(1), vp.row(2), vp.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(|p| p / p.truncate().length())
}

pub fn visible(planes: &[Vec4; 6], s: &Option<Sphere>) -> bool {
    match s {
        None => true,
        Some(s) => planes.iter().all(|p| p.truncate().dot(s.c) + p.w >= -s.r),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use glam::camera::rh::{proj::directx::perspective, view::look_at_mat4};

    #[test]
    fn frustum_accepts_front_rejects_behind() {
        let vp = perspective(1.0, 1.5, 0.5, 500.0) * look_at_mat4(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let planes = frustum_planes(vp);
        assert!(visible(&planes, &Some(Sphere { c: Vec3::new(0.0, 0.0, -50.0), r: 1.0 })));
        assert!(!visible(&planes, &Some(Sphere { c: Vec3::new(0.0, 0.0, 50.0), r: 1.0 })));
        assert!(!visible(&planes, &Some(Sphere { c: Vec3::new(400.0, 0.0, -50.0), r: 5.0 })));
        assert!(visible(&planes, &Some(Sphere { c: Vec3::new(0.0, 0.0, 10.0), r: 20.0 })));
    }
}
