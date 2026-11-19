//! Runtime props: meshes that experiences add after a scene is built (a
//! glider, parcels, planted flowers, sculpted shapes, glass jars...).
//!
//! Usage from an experience:
//! ```ignore
//! let glider = ctx.props.add_mesh(&mesh, Pass::Opaque);   // once
//! ctx.props.push(glider, ModelInstance::from_trs(pos, yaw, Vec3::ONE)); // every frame
//! ```
//! Instances are cleared automatically at the start of every frame. Opaque
//! props cast shadows. Meshes persist until `clear_meshes` (called when the
//! scene changes) or `replace_mesh`.

use super::scene::Pass;
use super::voxel::{MeshData, ModelInstance, VoxelVertex};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropMesh(pub u32);

pub(crate) struct PropMeshInfo {
    pub mesh: MeshData,
    pub pass: Pass,
}

#[derive(Default)]
pub struct Props {
    pub(crate) meshes: Vec<PropMeshInfo>,
    pub(crate) frame: Vec<(PropMesh, ModelInstance)>,
    /// Geometry changed since the last upload.
    pub(crate) dirty: bool,
}

impl Props {
    /// Register a mesh; returns a handle for `push`.
    pub fn add_mesh(&mut self, mesh: &MeshData, pass: Pass) -> PropMesh {
        self.meshes.push(PropMeshInfo { mesh: mesh.clone(), pass });
        self.dirty = true;
        PropMesh(self.meshes.len() as u32 - 1)
    }

    /// Swap a mesh's geometry (e.g. after re-voxelising a sculpture).
    pub fn replace_mesh(&mut self, id: PropMesh, mesh: &MeshData) {
        if let Some(m) = self.meshes.get_mut(id.0 as usize) {
            m.mesh = mesh.clone();
            self.dirty = true;
        }
    }

    /// Draw `id` with this transform this frame.
    pub fn push(&mut self, id: PropMesh, inst: ModelInstance) {
        self.frame.push((id, inst));
    }

    /// Forget all meshes (handles become invalid).
    pub fn clear_meshes(&mut self) {
        self.meshes.clear();
        self.frame.clear();
        self.dirty = true;
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame.clear();
    }

    /// Concatenated geometry and per-mesh (index_start, index_count, base_vertex).
    pub(crate) fn flatten(&self) -> (Vec<VoxelVertex>, Vec<u32>, Vec<(u32, u32, i32)>) {
        let mut v = Vec::new();
        let mut i = Vec::new();
        let mut spans = Vec::with_capacity(self.meshes.len());
        for m in &self.meshes {
            spans.push((i.len() as u32, m.mesh.indices.len() as u32, v.len() as i32));
            v.extend_from_slice(&m.mesh.vertices);
            i.extend_from_slice(&m.mesh.indices);
        }
        (v, i, spans)
    }
}
