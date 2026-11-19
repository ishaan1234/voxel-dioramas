//! The default mode: free camera, nothing else.

use crate::engine::experience::Experience;

pub struct Explore;

pub fn make() -> Box<dyn Experience> {
    Box::new(Explore)
}

impl Experience for Explore {
    fn id(&self) -> &'static str {
        "explore"
    }
}
