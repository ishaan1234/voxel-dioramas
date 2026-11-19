pub mod app;
pub mod camera;
pub mod color;
pub mod gpu;
// Toolkit modules expose a general API for future scenes; not every
// primitive is used by every build.
#[allow(dead_code)]
pub mod math;
pub mod pipelines;
pub mod scene;
#[allow(dead_code)]
pub mod sdf;
#[allow(dead_code)]
pub mod voxel;

/// Everything a scene module typically needs.
#[allow(unused_imports)]
pub mod prelude {
    pub use super::color::{hex_to_linear_rgb, ColorCache};
    pub use super::sdf;
    pub use super::math::{
        fbm2, fbm3, golden_spiral, hash2, hash3, noise2, noise3, poisson_disc, ridged2,
        CatmullRomCurve3, Rng,
    };
    pub use super::scene::{
        CameraPreset, DrawBatch, MeshSpan, ParticleConfig, Pass, Scene, SceneBuilder, SceneData,
        ShadowConfig, TimeModeParams,
    };
    pub use super::voxel::{
        build_unit_box_mesh, MeshData, MicroVoxelVolume, ModelInstance, VoxelVertex, MAT_AIR, MAT_CLOUD, MAT_FOLIAGE,
        MAT_LAMP, MAT_MATTE, MAT_METAL, MAT_MIST, MAT_WATER, MAT_WIN, MAT_WOOD,
    };
}
