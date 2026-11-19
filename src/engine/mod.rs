pub mod app;
pub mod atmosphere;
pub mod camera;
pub mod color;
pub mod culling;
pub mod gpu;
// Toolkit modules expose a general API for future scenes; not every
// primitive is used by every build.
#[allow(dead_code)]
pub mod math;
pub mod experience;
pub mod pipelines;
pub mod platform;
pub mod props;
pub mod scene;
pub mod storage;
pub mod ui;
#[allow(dead_code)]
pub mod sdf;
#[allow(dead_code)]
pub mod voxel;

/// Run `f` over `items` on scoped threads (one per item, results in order).
/// Scene generation is a handful of large independent jobs, so this simple
/// fan-out is enough and keeps output deterministic.
/// In the browser (no threads without extra setup) it simply runs in order.
pub fn par_map<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(items: &[T], f: F) -> Vec<R> {
    #[cfg(target_arch = "wasm32")]
    {
        items.iter().map(f).collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|s| {
        let handles: Vec<_> = items.iter().map(|it| s.spawn(|| f(it))).collect();
        handles.into_iter().map(|h| h.join().expect("generation thread panicked")).collect()
    })
}

/// Everything a scene module typically needs.
#[allow(unused_imports)]
pub mod prelude {
    pub use super::color::{hex_to_linear_rgb, ColorCache};
    pub use super::par_map;
    pub use super::sdf;
    pub use super::math::{
        fbm2, fbm3, golden_spiral, hash2, hash3, noise2, noise3, poisson_disc, ridged2,
        CatmullRomCurve3, Rng,
    };
    pub use super::scene::{
        raycast_surface, CameraPreset, DrawBatch, Landmark, MeshSpan, ParticleConfig, Pass, Scene,
        SceneBuilder, SceneData, Season, ShadowConfig, TimeModeParams, WorldParams,
    };
    pub use super::voxel::{
        build_unit_box_mesh, MeshData, MicroVoxelVolume, ModelInstance, VoxelVertex, MAT_AIR, MAT_CLOUD, MAT_FOLIAGE,
        MAT_LAMP, MAT_MATTE, MAT_METAL, MAT_MIST, MAT_WATER, MAT_WIN, MAT_WOOD,
    };
}
