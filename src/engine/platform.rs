//! The few things that differ between desktop and the browser build.

use std::sync::Arc;
use winit::window::Window;

/// Show the HUD line (scene, time of day, view, fps). Desktop: window title.
/// Web: the page's HUD element (via `globalThis.voxelHud`).
pub fn hud(window: &Arc<Window>, text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = window;
        web::voxel_hud(text);
    }
    #[cfg(not(target_arch = "wasm32"))]
    window.set_title(text);
}

/// Loading / status message ("Generating Sky Islands..."); empty hides it.
pub fn status(text: &str) {
    #[cfg(target_arch = "wasm32")]
    web::voxel_status(text);
    #[cfg(not(target_arch = "wasm32"))]
    if !text.is_empty() {
        println!("{text}");
    }
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = globalThis, js_name = voxelHud)]
        pub fn voxel_hud(text: &str);
        #[wasm_bindgen(js_namespace = globalThis, js_name = voxelStatus)]
        pub fn voxel_status(text: &str);
    }

    /// Resolve after the browser has had a chance to paint (two animation
    /// frames), so a status message is visible before a long synchronous job.
    pub async fn yield_to_browser() {
        for _ in 0..2 {
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                let _ = web_sys::window().unwrap().request_animation_frame(&resolve);
            });
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        }
    }

    /// Find the page's canvas.
    pub fn canvas() -> web_sys::HtmlCanvasElement {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("scene-canvas"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("page needs a <canvas id=\"scene-canvas\">")
    }
}
