use std::collections::HashMap;

#[inline]
fn srgb_chan_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn hex_to_linear_rgb(hex: u32) -> [f32; 3] {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    [
        srgb_chan_to_linear(r),
        srgb_chan_to_linear(g),
        srgb_chan_to_linear(b),
    ]
}

pub struct ColorCache {
    map: HashMap<u32, [f32; 3]>,
}

impl ColorCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::with_capacity(64),
        }
    }

    #[inline]
    pub fn get(&mut self, hex: u32) -> [f32; 3] {
        *self.map.entry(hex).or_insert_with(|| hex_to_linear_rgb(hex))
    }
}
