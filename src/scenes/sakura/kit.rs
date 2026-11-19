//! Sakura-specific palette and architectural helpers.
pub use crate::engine::prelude::*;

pub mod pal {
    pub const GRASS_1: u32 = 0x578e34;
    pub const GRASS_2: u32 = 0x4b7d2c;
    pub const GRASS_3: u32 = 0x679e40;
    pub const GRASS_RICE: u32 = 0x72b444;
    pub const MOSS: u32 = 0x3f6b2c;
    pub const DIRT: u32 = 0x5d4a3a;
    pub const BEDROCK: u32 = 0x262a2f;
    pub const STONE_DARK: u32 = 0x34383e;
    pub const STONE_MID: u32 = 0x62676d;
    pub const STONE_LT: u32 = 0x878d93;
    pub const COURTYARD: u32 = 0x90969a;
    pub const COURTYARD_ALT: u32 = 0x80868b;
    pub const COURTYARD_LT: u32 = 0xa3a8ad;
    pub const ZEN_GRAVEL: u32 = 0xd4d6d2;
    pub const ZEN_GRAVEL_DK: u32 = 0xb7bab4;
    pub const MTN_BASE: u32 = 0x73777c;
    pub const MTN_MID: u32 = 0x8a8e94;
    pub const MTN_DARK: u32 = 0x5a5e64;
    pub const SNOW: u32 = 0xffffff;
    pub const SNOW_SHADE: u32 = 0xe6edf5;
    pub const SAND: u32 = 0xa49b87;
    pub const WATER: u32 = 0x2ba4a0;
    pub const FOAM: u32 = 0xdff7f9;
    pub const ROOF_DARK: u32 = 0x1d2025;
    pub const ROOF_TRIM: u32 = 0x2f343c;
    pub const ROOF_RIDGE: u32 = 0x3b404a;
    pub const CRIMSON: u32 = 0x9f2726;
    pub const VERMILION: u32 = 0xc21e14;
    pub const TORII_BASE: u32 = 0x181a1e;
    pub const PLASTER: u32 = 0xf4f0e7;
    pub const PLASTER_W: u32 = 0xe9e1d2;
    pub const YAKISUGI: u32 = 0x2e2521;
    pub const WOOD_WARM: u32 = 0xb56926;
    pub const WOOD_LT: u32 = 0xcf873f;
    pub const WOOD_DARK: u32 = 0x3d281a;
    pub const WOOD_BEAM: u32 = 0x6b3d1b;
    pub const GOLD: u32 = 0xe6b800;
    pub const BRASS: u32 = 0xd69f22;
    pub const IRON: u32 = 0x24262b;
    pub const WIN_GLOW: u32 = 0xffe89c;
    pub const LAMP_GLOW: u32 = 0xff841c;
    pub const LANTERN_RED: u32 = 0xff5a2c;
    pub const SAKURA_1: u32 = 0xffb7c5;
    pub const SAKURA_2: u32 = 0xff9bb3;
    pub const SAKURA_3: u32 = 0xffd2de;
    pub const SAKURA_DEEP: u32 = 0xe97b97;
    pub const PINE: u32 = 0x28641c;
    pub const PINE_DARK: u32 = 0x1e4d14;
    pub const BAMBOO: u32 = 0x529b32;
    pub const BAMBOO_NODE: u32 = 0x336e22;
    pub const BAMBOO_LEAF: u32 = 0x3d8428;
    pub const NOREN_BLUE: u32 = 0x223b63;
    pub const NOREN_RED: u32 = 0x9a2626;
    pub const NOREN_CREAM: u32 = 0xeee5d2;
}

pub fn add_detailed_entrance(
    vol: &mut MicroVoxelVolume,
    cx: i32,
    y0: i32,
    front_z: i32,
    door_half_w: i32,
    door_h: i32,
    has_noren: bool,
    noren_col: u32,
    has_lantern: bool,
) {
    vol.box_fill(cx - door_half_w - 2, y0 - 1, front_z, cx + door_half_w + 2, y0, front_z + 2, MAT_MATTE, pal::STONE_LT);
    vol.box_fill(cx - door_half_w - 1, y0 + 1, front_z, cx + door_half_w + 1, y0 + 1, front_z, MAT_WOOD, pal::WOOD_DARK);
    vol.box_fill(cx - door_half_w - 1, y0 + 1, front_z, cx - door_half_w - 1, y0 + door_h + 1, front_z + 1, MAT_WOOD, pal::WOOD_BEAM);
    vol.box_fill(cx + door_half_w + 1, y0 + 1, front_z, cx + door_half_w + 1, y0 + door_h + 1, front_z + 1, MAT_WOOD, pal::WOOD_BEAM);
    vol.box_fill(cx - door_half_w - 2, y0 + door_h + 1, front_z, cx + door_half_w + 2, y0 + door_h + 2, front_z + 1, MAT_WOOD, pal::WOOD_DARK);

    let door_z = front_z - 1;
    for dy in 2..=door_h {
        for dx in -door_half_w..=door_half_w {
            let is_center_stile = dx == 0 || (dx == -1 && door_half_w > 3);
            let is_bottom_kick = dy <= 4;
            let is_grid = dy % 3 == 0 || dx.abs() % 2 == 0;
            if is_center_stile || is_bottom_kick || is_grid {
                vol.set(cx + dx, y0 + dy, door_z, MAT_WOOD, if is_center_stile { pal::WOOD_DARK } else { pal::WOOD_WARM });
            } else {
                vol.set(cx + dx, y0 + dy, door_z, MAT_WIN, pal::WIN_GLOW);
            }
        }
    }

    let h_y = y0 + (door_h as f32 * 0.48).round() as i32;
    let handle_x = cx + 1;
    vol.box_fill(handle_x, h_y - 1, door_z, handle_x, h_y + 1, door_z, MAT_METAL, pal::IRON);
    vol.box_fill(handle_x, h_y, door_z + 1, handle_x, h_y + 1, door_z + 1, MAT_METAL, pal::BRASS);
    vol.set(cx - 1, h_y, door_z, MAT_METAL, pal::IRON);
    vol.set(cx - 1, h_y, door_z + 1, MAT_METAL, pal::BRASS);

    if has_noren {
        let rod_y = y0 + door_h + 3;
        vol.box_fill(cx - door_half_w - 2, rod_y, front_z + 2, cx + door_half_w + 2, rod_y, front_z + 2, MAT_WOOD, pal::WOOD_LT);
        let panels = 3;
        let span = door_half_w * 2 + 1;
        let slit = (span / panels).max(2);
        for dx in -door_half_w..=door_half_w {
            let rel = dx + door_half_w;
            if rel > 0 && rel % slit == 0 {
                continue;
            }
            vol.box_fill(cx + dx, rod_y - 4, front_z + 2, cx + dx, rod_y - 1, front_z + 2, MAT_MATTE, noren_col);
            if dx == 0 && rod_y - 2 >= 0 {
                vol.set(cx + dx, rod_y - 2, front_z + 2, MAT_MATTE, pal::PLASTER);
            }
        }
    }

    if has_lantern {
        let lx = cx - door_half_w - 3;
        let ly = y0 + door_h;
        let lz = front_z + 2;
        vol.box_fill(lx, ly + 1, front_z + 1, lx, ly + 1, lz, MAT_METAL, pal::IRON);
        vol.set(lx, ly, lz, MAT_WOOD, pal::WOOD_DARK);
        vol.box_fill(lx, ly - 3, lz, lx, ly - 1, lz, MAT_LAMP, pal::LANTERN_RED);
        vol.set(lx, ly - 4, lz, MAT_WOOD, pal::WOOD_DARK);
    }
}

pub fn add_tiled_roof(
    vol: &mut MicroVoxelVolume,
    cx: i32,
    base_y: i32,
    cz: i32,
    hw: i32,
    hd: i32,
    ridge_height: i32,
    eave_flare: i32,
    axis: char,
    gable_wall_col: u32,
) {
    for layer in 0..=ridge_height {
        let t = layer as f32 / ridge_height.max(1) as f32;
        let flare = (eave_flare as f32 * (1.0 - t) * (1.0 - t)).round() as i32;
        let rx = if axis == 'X' {
            hw + flare - (layer as f32 * 0.35).floor() as i32
        } else {
            hw + flare - layer
        };
        let rz = if axis == 'Z' {
            hd + flare - (layer as f32 * 0.35).floor() as i32
        } else {
            hd + flare - layer
        };
        if rx < 1 || rz < 1 {
            break;
        }
        for dz in -rz..=rz {
            for dx in -rx..=rx {
                let on_edge = dx.abs() == rx || dz.abs() == rz;
                let is_corner = dx.abs() >= rx - 1 && dz.abs() >= rz - 1;
                let corner_lift = if layer <= 1 && is_corner { 1 } else { 0 };
                if axis == 'X' && layer >= 2 && dx.abs() == rx && dz.abs() < rz - 1 {
                    vol.set(cx + dx, base_y + layer, cz + dz, MAT_MATTE, gable_wall_col);
                } else if axis == 'Z' && layer >= 2 && dz.abs() == rz && dx.abs() < rx - 1 {
                    vol.set(cx + dx, base_y + layer, cz + dz, MAT_MATTE, gable_wall_col);
                } else if on_edge || layer == ridge_height {
                    let is_rib = (if axis == 'X' { dx } else { dz }).rem_euclid(2) == 0;
                    let col = if is_rib { pal::ROOF_TRIM } else { pal::ROOF_DARK };
                    vol.set(cx + dx, base_y + layer + corner_lift, cz + dz, MAT_WOOD, col);
                    if layer == 0 && is_rib && !is_corner {
                        vol.set(cx + dx, base_y - 1, cz + dz, MAT_WOOD, pal::WOOD_BEAM);
                    }
                }
            }
        }
    }
    let top_y = base_y + ridge_height + 1;
    let r_span = if axis == 'X' {
        (hw - (ridge_height as f32 * 0.35).floor() as i32).max(2)
    } else {
        (hd - (ridge_height as f32 * 0.35).floor() as i32).max(2)
    };
    for i in -r_span..=r_span {
        let wx = if axis == 'X' { cx + i } else { cx };
        let wz = if axis == 'Z' { cz + i } else { cz };
        let is_end = i.abs() >= r_span - 1;
        vol.set(
            wx,
            top_y,
            wz,
            if is_end { MAT_METAL } else { MAT_WOOD },
            if is_end { pal::GOLD } else { pal::ROOF_RIDGE },
        );
        if i.abs() == r_span {
            vol.set(wx, top_y + 1, wz, MAT_METAL, pal::GOLD);
        }
    }
}
