struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,      // xyz = cam, w = time
    sun_dir: vec4<f32>,      // xyz = sun dir, w = fog_density
    sky_top: vec4<f32>,      // rgb = linear sky top, w = exposure
    sky_mid: vec4<f32>,      // rgb = linear sky mid, w = sun_int
    sky_bot: vec4<f32>,      // rgb = linear sky bot, w = hemi_int
    fog_col: vec4<f32>,      // rgb = linear fog, w = point_int
    sun_col: vec4<f32>,      // rgb = linear sun, w = win_emissive
    hemi_sky: vec4<f32>,     // rgb = linear hemi sky, w = lamp_emissive
    hemi_gnd: vec4<f32>,     // rgb = linear hemi gnd, w = unused
    extra: vec4<f32>,        // rgb = point light colour, w = star intensity
    point_lights: array<vec4<f32>, 12>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

fn aces_tonemap(col: vec3<f32>) -> vec3<f32> {
    let x = col * u.sky_top.w;
    return (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
}

// -------------------- SKY DOME FULLSCREEN PASS --------------------
struct SkyOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vid: u32) -> SkyOut {
    let x = f32(i32(vid & 1u) * 4 - 1);
    let y = f32(i32(vid >> 1u) * 4 - 1);
    var out: SkyOut;
    out.pos = vec4<f32>(x, y, 0.9999, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let wp = u.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(wp.xyz / wp.w - u.cam_pos.xyz);
    let h = dir.y;
    var col = mix(u.sky_bot.rgb, u.sky_mid.rgb, smoothstep(-0.04, 0.18, h));
    col = mix(col, u.sky_top.rgb, smoothstep(0.18, 0.85, h));

    // Subtle sun / moon halo
    let sdot = max(dot(dir, normalize(u.sun_dir.xyz)), 0.0);
    col += u.sun_col.rgb * pow(sdot, 96.0) * 0.45 * u.sky_mid.w;

    // Procedural star field: hash a direction lattice, keep the rare bright cells.
    if (u.extra.w > 0.0 && h > -0.05) {
        let g = dir * 420.0;
        let cell = floor(g);
        let r = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
        let f = fract(g) - vec3<f32>(0.5);
        let star = step(0.9975, r) * smoothstep(0.42, 0.0, length(f));
        let twinkle = 0.65 + 0.35 * sin(u.cam_pos.w * 2.3 + r * 917.0);
        col += vec3<f32>(0.85, 0.9, 1.0) * star * twinkle * u.extra.w * smoothstep(-0.05, 0.25, h);
    }
    return vec4<f32>(aces_tonemap(col), 1.0);
}

// -------------------- MICRO-VOXEL SHADOW & LIT PASS --------------------
struct VIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) v_norm: vec3<f32>,
    @location(2) v_col: vec3<f32>,
    @location(3) v_mat: u32,
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) mat_id: u32,
};

@vertex
fn vs_shadow(in: VIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let wpos = (model * vec4<f32>(in.v_pos, 1.0)).xyz;
    return u.light_vp * vec4<f32>(wpos, 1.0);
}

@vertex
fn vs_main(in: VIn) -> VOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let wpos = (model * vec4<f32>(in.v_pos, 1.0)).xyz;
    let wnorm = normalize((model * vec4<f32>(in.v_norm, 0.0)).xyz);
    var out: VOut;
    out.world_pos = wpos;
    out.normal = wnorm;
    out.clip = u.view_proj * vec4<f32>(wpos, 1.0);
    out.color = in.v_col;
    out.mat_id = in.v_mat;
    return out;
}

fn sample_shadow(wpos: vec3<f32>, n: vec3<f32>) -> f32 {
    let lclip = u.light_vp * vec4<f32>(wpos + n * 0.16, 1.0);
    let ndc = lclip.xyz / lclip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / 2048.0;
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(
                shadow_tex, shadow_samp,
                uv + vec2<f32>(f32(x), f32(y)) * texel,
                ndc.z - 0.0007
            );
        }
    }
    return sum / 9.0;
}

fn shade_common(in: VOut, alpha: f32) -> vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(u.sun_dir.xyz);
    let v = normalize(u.cam_pos.xyz - in.world_pos);
    let shadow = sample_shadow(in.world_pos, n);

    // Hemisphere ambient
    let hemi_t = n.y * 0.5 + 0.5;
    let amb = mix(u.hemi_gnd.rgb, u.hemi_sky.rgb, hemi_t) * u.sky_bot.w * 0.52;
    var lit = in.color * amb;

    // Directional sun / moon
    let sun_int = u.sky_mid.w;
    let ndotl = max(dot(n, l), 0.0);
    lit += in.color * u.sun_col.rgb * (sun_int * 0.65 * ndotl * shadow);

    // Soft sky bounce fill
    let fill_l = normalize(vec3<f32>(-0.55, 0.45, -0.5));
    lit += in.color * u.hemi_sky.rgb * (0.14 * max(dot(n, fill_l), 0.0));

    // Material-specific highlights & subsurface
    let hvec = normalize(l + v);
    if (in.mat_id == 3u) {
        // MAT_METAL (gold, brass, iron)
        let spec = pow(max(dot(n, hvec), 0.0), 48.0) * shadow;
        lit += mix(in.color, vec3<f32>(1.0, 0.9, 0.5), 0.5) * spec * sun_int * 0.9;
    } else if (in.mat_id == 4u) {
        // MAT_FOLIAGE (sakura, pine, bamboo) gentle backlit wrap
        let wrap = max(dot(n, -l) * 0.35 + 0.2, 0.0);
        lit += in.color * u.sun_col.rgb * (wrap * 0.14 * sun_int);
    } else if (in.mat_id == 7u) {
        // MAT_WATER animated ripple specular
        let t = u.cam_pos.w;
        let wave = sin(in.world_pos.x * 0.65 + t * 2.1) * cos(in.world_pos.z * 0.65 - t * 1.6);
        let wn = normalize(n + vec3<f32>(wave * 0.09, 0.0, wave * 0.09));
        let wh = normalize(l + v);
        let spec = pow(max(dot(wn, wh), 0.0), 64.0) * shadow;
        lit += u.sun_col.rgb * spec * (0.65 * sun_int);
        let fres = pow(1.0 - max(dot(wn, v), 0.0), 4.0);
        lit += u.sky_mid.rgb * fres * 0.25;
    } else if (in.mat_id == 8u) {
        // MAT_CLOUD soft scattering
        lit += u.sky_mid.rgb * 0.22;
    } else if (in.mat_id == 9u) {
        // MAT_MIST bright waterfall spray
        lit += u.hemi_sky.rgb * 0.35;
    }

    // 12 Warm Lantern Point Lights
    let point_int = u.fog_col.w;
    for (var i = 0u; i < 12u; i++) {
        let pl = u.point_lights[i];
        let diff = pl.xyz - in.world_pos;
        let d2 = dot(diff, diff);
        let r2 = pl.w * pl.w;
        if (d2 < r2) {
            let dist = sqrt(d2);
            let att = pow(1.0 - dist / pl.w, 2.0);
            let p_ndotl = max(dot(n, diff / max(dist, 0.001)), 0.18);
            lit += in.color * u.extra.rgb * (point_int * 0.14 * att * p_ndotl);
        }
    }

    // Emissive Shoji Windows (MAT_WIN = 5) & Lanterns (MAT_LAMP = 6)
    if (in.mat_id == 5u) {
        lit += in.color * u.sun_col.w;
    } else if (in.mat_id == 6u) {
        lit += in.color * u.hemi_sky.w;
    }

    // FogExp2 in linear space, then ACES tone mapping (matches sky horizon seamlessly)
    let dist_cam = length(u.cam_pos.xyz - in.world_pos);
    let fog_d = u.sun_dir.w;
    let fog_f = 1.0 - exp(-pow(dist_cam * fog_d, 2.0));
    let fogged = mix(lit, u.fog_col.rgb, clamp(fog_f, 0.0, 1.0));
    return vec4<f32>(aces_tonemap(fogged), alpha);
}

@fragment
fn fs_opaque(in: VOut) -> @location(0) vec4<f32> {
    return shade_common(in, 1.0);
}

@fragment
fn fs_translucent(in: VOut) -> @location(0) vec4<f32> {
    var alpha = 0.78;
    if (in.mat_id == 8u) {
        alpha = 0.76;
    } else if (in.mat_id == 9u) {
        alpha = 0.75;
    }
    return shade_common(in, alpha);
}
