//! Experience registry. Add a mode by creating `src/experiences/<name>.rs`
//! with a type implementing `engine::experience::Experience`, then list it
//! below. The first entry is the default.

use crate::engine::experience::ExperienceEntry;

pub mod explore;
pub mod photo;

pub static REGISTRY: &[ExperienceEntry] = &[
    ExperienceEntry { id: "explore", title: "Explore", blurb: "Fly around freely.", make: explore::make },
    ExperienceEntry {
        id: "photo",
        title: "Photo Hunt",
        blurb: "Frame landmarks in the right light, weather, and distance.",
        make: photo::make,
    },
];
