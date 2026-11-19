struct SimParams {
    dt: f32,
    time: f32,
    count: u32,
    paused: u32,
    bmin: vec4<f32>,
    bmax: vec4<f32>,
};

struct Petal {
    pos_spin: vec4<f32>,   // xyz = pos, w = spin angle
    vel_col: vec4<f32>,    // x = vy, y = vx, z = phase, w = color_idx
};

@group(0) @binding(0) var<uniform> sim: SimParams;
@group(0) @binding(1) var<storage, read_write> petals: array<Petal>;

@compute @workgroup_size(64)
fn cs_petals(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= sim.count || sim.paused != 0u) { return; }

    var p = petals[idx];
    let phase = p.vel_col.z;
    let wind_x = sin(sim.time * 0.42) * 0.55 + 0.75;
    let wind_z = cos(sim.time * 0.35) * 0.42;

    // bmin.w = sway: 1 = drifting petals / flakes, 0 = straight rain.
    let sway = sim.bmin.w;
    p.pos_spin.y -= p.vel_col.x * sim.dt;
    p.pos_spin.x += (p.vel_col.y * wind_x + sin(sim.time * 2.0 + phase) * 0.65 * sway) * sim.dt;
    p.pos_spin.z += (wind_z + cos(sim.time * 1.7 + phase) * 0.55 * sway) * sim.dt;
    p.pos_spin.w += (1.2 + sin(phase) * 1.6) * sim.dt * sway;

    let ext = sim.bmax.xyz - sim.bmin.xyz;
    if (p.pos_spin.y < sim.bmin.y) { p.pos_spin.y = sim.bmax.y + f32(idx % 9u); }
    if (p.pos_spin.x > sim.bmax.x) { p.pos_spin.x -= ext.x; }
    if (p.pos_spin.x < sim.bmin.x) { p.pos_spin.x += ext.x; }
    if (p.pos_spin.z > sim.bmax.z) { p.pos_spin.z -= ext.z; }
    if (p.pos_spin.z < sim.bmin.z) { p.pos_spin.z += ext.z; }

    petals[idx] = p;
}

// Petal Render Shader
struct CamUniform {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,   // xyz = cam, w = fog_density
    fog_col: vec4<f32>,   // rgb = linear fog, w = exposure
    size: vec4<f32>,      // xyz = particle half-size, w = glow multiplier
    colors: array<vec4<f32>, 4>, // sRGB palette
};
@group(0) @binding(0) var<uniform> cam: CamUniform;

struct PIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) p_pos_spin: vec4<f32>,
    @location(2) p_vel_col: vec4<f32>,
};

struct POut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
};

@vertex
fn vs_petal(in: PIn) -> POut {
    let s = sin(in.p_pos_spin.w);
    let c = cos(in.p_pos_spin.w);
    let p = in.v_pos * cam.size.xyz;
    let rx = vec3<f32>(p.x * c + p.z * s, p.y * c - p.z * s * 0.4, -p.x * s + p.z * c);
    let wpos = rx + in.p_pos_spin.xyz;

    let ci = u32(in.p_vel_col.w);
    let col = cam.colors[min(ci, 3u)].rgb;

    var out: POut;
    out.clip = cam.view_proj * vec4<f32>(wpos, 1.0);
    out.color = pow(col, vec3<f32>(2.2)) * cam.size.w;
    out.world_pos = wpos;
    return out;
}

@fragment
fn fs_petal(in: POut) -> @location(0) vec4<f32> {
    let d = length(cam.cam_pos.xyz - in.world_pos);
    let fog_f = 1.0 - exp(-pow(d * cam.cam_pos.w, 2.0));
    let fogged = mix(in.color, cam.fog_col.rgb, clamp(fog_f, 0.0, 1.0));
    let x = fogged * cam.fog_col.w;
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return vec4<f32>(mapped, 1.0);
}
