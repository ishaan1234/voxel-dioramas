//! Scene registry. Add a new diorama by creating a module with a
//! `pub fn build() -> Box<dyn Scene>` and listing it here.

use crate::engine::app::SceneEntry;

pub mod sakura;
pub mod sky_islands;

pub static REGISTRY: &[SceneEntry] = &[
    SceneEntry { id: "sky-islands", build: sky_islands::build },
    SceneEntry { id: "sakura", build: sakura::build },
];
