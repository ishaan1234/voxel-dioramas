pub mod app;
pub mod camera;
pub mod color;
pub mod gpu;
pub mod math;
pub mod pipelines;
pub mod scene;
pub mod sdf;
pub mod voxel;

/// Everything a scene module typically needs.
pub mod prelude {
    pub use super::color::{hex_to_linear_rgb, ColorCache};
    pub use super::math::{
        fbm2, fbm3, golden_spiral, hash2, hash3, noise2, noise3, poisson_disc, ridged2,
        CatmullRomCurve3, Rng,
    };
    pub use super::scene::{
        CameraPreset, DrawBatch, MeshSpan, ParticleConfig, Pass, Scene, SceneBuilder, SceneData,
        ShadowConfig, TimeModeParams,
    };
    pub use super::voxel::{
        MeshData, MicroVoxelVolume, ModelInstance, VoxelVertex, MAT_AIR, MAT_CLOUD, MAT_FOLIAGE,
        MAT_LAMP, MAT_MATTE, MAT_METAL, MAT_MIST, MAT_WATER, MAT_WIN, MAT_WOOD,
    };
}
