//! Voxel Dioramas: a multi-scene micro-voxel engine (desktop + WebGPU).

pub mod engine;
pub mod scenes;

/// Browser entry point: runs when the WebAssembly module is instantiated.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn web_start() {
    console_error_panic_hook::set_once();
    // Scene can be chosen with ?scene=sakura in the page URL.
    let query = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    let wanted = query
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix("scene="))
        .unwrap_or("");
    let scene_idx = scenes::REGISTRY.iter().position(|e| e.id == wanted).unwrap_or(0);
    let opts = engine::app::RunOptions { scene_idx, ..Default::default() };
    engine::app::run(scenes::REGISTRY, opts);
}

/// Called by the HTML control bar: "next_scene", "prev_scene", "time", "tour",
/// "pause", "preset:N" (1-based).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn voxel_command(cmd: &str) {
    if let Some(c) = engine::app::Command::parse(cmd) {
        engine::app::push_command(c);
    }
}

/// Scene ids, for the web UI.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn voxel_scene_ids() -> String {
    scenes::REGISTRY.iter().map(|e| e.id).collect::<Vec<_>>().join(",")
}
