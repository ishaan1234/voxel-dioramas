//! Desert Canyon palette (sRGB hex) and small colour helpers.

/// Sandstone strata, bottom to top of one repeating sequence: (thickness m, colour).
pub const STRATA: [(f32, u32); 11] = [
    (2.6, 0xa24e2e), // deep red
    (1.1, 0xe2c08e), // cream
    (2.2, 0xc8693a), // rust
    (0.7, 0x87503f), // purple-brown
    (1.8, 0xd98c52), // orange
    (1.4, 0xebcd9c), // pale buff
    (2.4, 0xb85a34), // brick
    (0.6, 0xf2e0bc), // white band
    (1.9, 0xd47c46), // salmon
    (1.2, 0x9a4a30), // dark rust
    (1.5, 0xe0a466), // ochre
];

pub const CAPROCK: [u32; 3] = [0x9c5f3c, 0xab6c44, 0x8b5236];

pub const SAND: u32 = 0xebc58e;
pub const SAND_LT: u32 = 0xf4d6a0;
pub const SAND_DK: u32 = 0xdcae74;
pub const SAND_LEE: u32 = 0xd3a26a;

pub const MUD: u32 = 0xc39062;
pub const MUD_CRACK: u32 = 0x8e5f3e;
pub const PEBBLE: u32 = 0xa88d74;
pub const WET_SAND: u32 = 0xa47b52;
pub const LAKEBED: u32 = 0x6f6a48;
pub const GRASS: [u32; 3] = [0x6d9a3a, 0x5b8731, 0x88ad4a];

pub const WATER: u32 = 0x3fb5bf;
pub const WATER_DEEP: u32 = 0x23879e;

pub const FROST: u32 = 0xf1f5fa;
pub const FROST_DK: u32 = 0xdbe4ee;

// Ruins
pub const TEMPLE: [u32; 3] = [0xeedcbf, 0xe0caa8, 0xf6e8d0];
pub const TEMPLE_DK: u32 = 0xc4a882;
pub const JOINT: u32 = 0x9c8a70;
pub const BRONZE: u32 = 0x8a5a2a;
pub const EMBER: u32 = 0xff9a3a;
pub const FLAME: u32 = 0xffc05a;
pub const POLE: u32 = 0x5a3a22;

// Flora
pub const PALM_BARK: u32 = 0x7a5a3a;
pub const PALM_BARK_DK: u32 = 0x5e4229;
pub const FROND: [u32; 3] = [0x4f8f34, 0x3e7a2a, 0x6aa443];
pub const FROND_DEAD: u32 = 0xb08a4a;
pub const COCONUT: u32 = 0x5a3a1e;
pub const CACTUS: u32 = 0x4f8a4a;
pub const CACTUS_DK: u32 = 0x3b6f3c;
pub const CACTUS_LT: u32 = 0x6aa45c;
pub const BLOOM: u32 = 0xfff2c8;
pub const PEAR_FRUIT: u32 = 0xc2306a;
pub const BARREL_FLOWER: u32 = 0xf2b230;
pub const SHRUB: [u32; 3] = [0x8a9a5a, 0x7a8a4e, 0xa0ac6a];
pub const REED: [u32; 3] = [0x6f9a40, 0x86a84a, 0xb0a860];

pub const VULTURE: u32 = 0x3a2e28;
pub const VULTURE_HEAD: u32 = 0xc9a090;
pub const CAMP_STONE: u32 = 0x6f665c;
pub const LOG: u32 = 0x5a3d26;
pub const CANVAS: u32 = 0xe8dcc0;
pub const CANVAS_DK: u32 = 0xb8442e;

#[inline]
fn rgb(c: u32) -> (f32, f32, f32) {
    (((c >> 16) & 0xff) as f32, ((c >> 8) & 0xff) as f32, (c & 0xff) as f32)
}

#[inline]
fn pack(r: f32, g: f32, b: f32) -> u32 {
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u32;
    (q(r) << 16) | (q(g) << 8) | q(b)
}

/// Multiply brightness.
pub fn shade(c: u32, f: f32) -> u32 {
    let (r, g, b) = rgb(c);
    pack(r * f, g * f, b * f)
}

/// Winter palette shift: desaturate toward a cool grey-blue.
pub fn cool(c: u32, winter: bool) -> u32 {
    if !winter {
        return c;
    }
    let (r, g, b) = rgb(c);
    let l = r * 0.3 + g * 0.55 + b * 0.15;
    pack(r * 0.55 + l * 0.33 + 0.0, g * 0.55 + l * 0.37 + 6.0, b * 0.55 + l * 0.38 + 22.0)
}
