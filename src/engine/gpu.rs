//! Device / surface ownership, per-scene GPU buffers and the frame loop.

use std::{sync::Arc, time::Instant};

use glam::{
    camera::rh::{
        proj::directx::{orthographic, perspective},
        view::look_at_mat4,
    },
    Vec3,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

use super::camera::OrbitCam;
use super::culling::{build_draw_list, frustum_planes, visible, DrawList};
use super::math::Rng;
use super::pipelines::{
    cube_mesh, make_depth, PetalCam, PetalGpu, Pipelines, SimParams, Uniforms, SHADOW_RES,
};
use super::scene::{DrawBatch, ParticleConfig, Scene};
use super::voxel::ModelInstance;

fn make_shadow_tex(device: &wgpu::Device, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: SHADOW_RES, height: SHADOW_RES, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

fn make_particles(cfg: &ParticleConfig) -> Vec<PetalGpu> {
    let mut rng = Rng(cfg.seed);
    let ext = cfg.bmax - cfg.bmin;
    (0..cfg.count)
        .map(|i| {
            let x = cfg.bmin.x + rng.next_f32() * ext.x;
            let y = cfg.bmin.y + 2.0 + rng.next_f32() * (ext.y - 1.0);
            let z = cfg.bmin.z + rng.next_f32() * ext.z;
            let spin = rng.next_f32() * std::f32::consts::TAU;
            PetalGpu {
                pos_spin: [x, y, z, spin],
                vel_col: [
                    cfg.fall.0 + rng.next_f32() * (cfg.fall.1 - cfg.fall.0),
                    cfg.drift.0 + rng.next_f32() * (cfg.drift.1 - cfg.drift.0),
                    rng.next_f32() * 10.0,
                    (i % 4) as f32,
                ],
            }
        })
        .collect()
}

/// GPU resources that belong to one loaded scene.
struct SceneGpu {
    scene: Box<dyn Scene>,
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    petal_buf: wgpu::Buffer,
    petal_count: u32,
    petal_comp_bg: wgpu::BindGroup,
    dyn_instances: Vec<ModelInstance>,
    /// Cullable main-pass draw lists (static batches split into spatial clusters).
    opaque: DrawList,
    translucent: DrawList,
    /// Shadow casters: static ones go to the cached map, dynamic ones every frame.
    static_shadow: Vec<DrawBatch>,
    dynamic_shadow: Vec<DrawBatch>,
}

pub struct GpuState {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,
    shadow_tex: wgpu::Texture,
    static_shadow_tex: wgpu::Texture,
    static_shadow_view: wgpu::TextureView,
    /// Time-of-day mode the static shadow cache was rendered for (None = stale).
    shadow_key: Option<usize>,
    refresh_shadow: bool,
    vis_opaque: Vec<bool>,
    vis_trans: Vec<bool>,
    pipes: Pipelines,

    main_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    petal_rend_bg: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
    sim_buf: wgpu::Buffer,
    petal_cam_buf: wgpu::Buffer,
    cube_vb: wgpu::Buffer,
    cube_ib: wgpu::Buffer,

    sg: SceneGpu,
    pub cam: OrbitCam,
    pub time_mode: usize,
    pub paused: bool,
    pub sim_time: f32,

    pub left_drag: bool,
    pub right_drag: bool,
    pub last_cursor: Option<(f64, f64)>,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
}

impl GpuState {
    pub async fn new(window: Arc<Window>, scene: Box<dyn Scene>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await
            .unwrap();

        let info = adapter.get_info();
        println!("GPU: {} ({:?}, {:?})", info.name, info.device_type, info.backend);
        let mut limits = wgpu::Limits::default();
        limits.max_buffer_size = adapter.limits().max_buffer_size;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let fmt = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: fmt,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let depth_view = make_depth(&device, config.width, config.height, "depth");
        let (shadow_tex, shadow_view) = make_shadow_tex(&device, "shadow");
        let (static_shadow_tex, static_shadow_view) = make_shadow_tex(&device, "static_shadow");
        let shadow_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_samp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let pipes = Pipelines::new(&device, fmt);

        let ubuf = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let uniform_buf = ubuf("uniforms", std::mem::size_of::<Uniforms>());
        let sim_buf = ubuf("sim_params", std::mem::size_of::<SimParams>());
        let petal_cam_buf = ubuf("petal_cam", std::mem::size_of::<PetalCam>());

        let main_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("main_bg"),
            layout: &pipes.main_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_samp) },
            ],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"),
            layout: &pipes.shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let petal_rend_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_rend_bg"),
            layout: &pipes.petal_rend_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: petal_cam_buf.as_entire_binding() }],
        });

        let (cverts, cindices) = cube_mesh();
        let cube_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_vb"),
            contents: bytemuck::cast_slice(&cverts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_ib"),
            contents: bytemuck::cast_slice(&cindices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let sg = Self::upload_scene(&device, &pipes, &sim_buf, scene);
        let cam = OrbitCam::new(sg.scene.data());

        let s = Self {
            window,
            surface,
            device,
            queue,
            config,
            depth_view,
            shadow_view,
            shadow_tex,
            static_shadow_tex,
            static_shadow_view,
            shadow_key: None,
            refresh_shadow: true,
            vis_opaque: Vec::new(),
            vis_trans: Vec::new(),
            pipes,
            main_bg,
            shadow_bg,
            petal_rend_bg,
            uniform_buf,
            sim_buf,
            petal_cam_buf,
            cube_vb,
            cube_ib,
            sg,
            cam,
            time_mode: 0,
            paused: false,
            sim_time: 0.0,
            left_drag: false,
            right_drag: false,
            last_cursor: None,
            last_frame: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
        };
        s.update_title(0);
        s
    }

    fn upload_scene(device: &wgpu::Device, pipes: &Pipelines, sim_buf: &wgpu::Buffer, scene: Box<dyn Scene>) -> SceneGpu {
        let d = scene.data();
        println!(
            "Built {}: {} micro-voxels, {} tris, {} batches ({} verts, {} indices)",
            d.title,
            d.total_voxels,
            d.total_tris,
            d.total_draws(),
            d.vertices.len(),
            d.indices.len()
        );
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world_vb"),
            contents: bytemuck::cast_slice(&d.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("world_ib"),
            contents: bytemuck::cast_slice(&d.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut all_instances = d.static_instances.clone();
        // Culling data (reorders static instances within each batch's range).
        let cell = 80.0;
        let opaque = build_draw_list(&d.opaque_batches, &d.vertices, &d.indices, &mut all_instances, d.dynamic_inst_start, cell);
        let translucent = build_draw_list(&d.translucent_batches, &d.vertices, &d.indices, &mut all_instances, d.dynamic_inst_start, cell);
        let (dynamic_shadow, static_shadow): (Vec<DrawBatch>, Vec<DrawBatch>) =
            d.opaque_batches.iter().partition(|b| b.inst_start >= d.dynamic_inst_start);
        let mut dyn_instances = Vec::with_capacity(d.dynamic_inst_count as usize);
        scene.update_dynamic_instances(0.0, &mut dyn_instances);
        all_instances.extend_from_slice(&dyn_instances);
        if all_instances.is_empty() {
            all_instances.push(ModelInstance::identity());
        }
        let instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("instance_buf"),
            contents: bytemuck::cast_slice(&all_instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let mut petals = make_particles(&d.particles);
        let petal_count = petals.len() as u32;
        if petals.is_empty() {
            petals.push(PetalGpu { pos_spin: [0.0, -1e4, 0.0, 0.0], vel_col: [0.0; 4] });
        }
        let petal_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("petal_buf"),
            contents: bytemuck::cast_slice(&petals),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        });
        let petal_comp_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("petal_comp_bg"),
            layout: &pipes.petal_comp_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: petal_buf.as_entire_binding() },
            ],
        });

        println!(
            "Culling: {} opaque / {} translucent draw clusters; {} static + {} dynamic shadow casters",
            opaque.batches.len(),
            translucent.batches.len(),
            static_shadow.len(),
            dynamic_shadow.len()
        );
        SceneGpu {
            scene,
            vb,
            ib,
            instance_buf,
            petal_buf,
            petal_count,
            petal_comp_bg,
            dyn_instances,
            opaque,
            translucent,
            static_shadow,
            dynamic_shadow,
        }
    }

    /// Replace the current scene (buffers are rebuilt; camera resets to preset 1).
    pub fn load_scene(&mut self, scene: Box<dyn Scene>) {
        self.sg = Self::upload_scene(&self.device, &self.pipes, &self.sim_buf, scene);
        self.shadow_key = None;
        self.cam = OrbitCam::new(self.sg.scene.data());
        self.time_mode = 0;
        self.sim_time = 0.0;
        self.update_title(0);
    }

    /// Jump straight to a preset (screenshots / benchmarks).
    pub fn set_preset(&mut self, idx: usize) {
        let d = self.sg.scene.data();
        self.cam.set_preset(d, idx, true);
    }

    /// Glide to a preset (keyboard).
    pub fn fly_to_preset(&mut self, idx: usize) {
        let d = self.sg.scene.data();
        self.cam.set_preset(d, idx, false);
    }

    pub fn preset_count(&self) -> usize {
        self.sg.scene.data().presets.len()
    }

    pub fn cycle_time(&mut self) {
        self.time_mode = (self.time_mode + 1) % 3;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = make_depth(&self.device, w, h, "depth");
        }
    }

    fn update_title(&self, fps: u32) {
        let d = self.sg.scene.data();
        let mode = d.time_modes[self.time_mode].name;
        let view = if self.cam.tour { "Tour" } else { self.cam.preset_name };
        let pause = if self.paused { " | PAUSED" } else { "" };
        self.window.set_title(&format!(
            "{} | {mode} | {view}{pause} | {} tris · {} draws · {fps} FPS | [Tab] Scene [1-9] [T] Tour [N] Time [P] Pause",
            d.title,
            d.total_tris,
            d.total_draws()
        ));
    }

    fn draw_batch(rpass: &mut wgpu::RenderPass<'_>, b: &DrawBatch) {
        if b.idx_count > 0 && b.inst_count > 0 {
            rpass.draw_indexed(
                b.idx_start..(b.idx_start + b.idx_count),
                b.base_vertex,
                b.inst_start..(b.inst_start + b.inst_count),
            );
        }
    }

    /// Advance simulation clocks and upload all per-frame data.
    fn prepare(&mut self, dt: f32) {
        if !self.paused {
            self.sim_time += dt;
        }
        let t = self.sim_time;
        let d = self.sg.scene.data();
        self.cam.advance_tour(d, dt);
        self.cam.advance(dt);

        if !self.paused && d.dynamic_inst_count > 0 {
            self.sg.scene.update_dynamic_instances(t, &mut self.sg.dyn_instances);
            let off = (d.dynamic_inst_start as usize * std::mem::size_of::<ModelInstance>()) as u64;
            self.queue.write_buffer(&self.sg.instance_buf, off, bytemuck::cast_slice(&self.sg.dyn_instances));
        }

        let eye = self.cam.eye();
        let tgt = self.cam.look_target();
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let proj = perspective(44.0f32.to_radians(), aspect, 0.8, 1300.0);
        let vp = proj * look_at_mat4(eye, tgt, Vec3::Y);

        // Frustum culling.
        let planes = frustum_planes(vp);
        self.vis_opaque = self.sg.opaque.bounds.iter().map(|b| visible(&planes, b)).collect();
        self.vis_trans = self.sg.translucent.bounds.iter().map(|b| visible(&planes, b)).collect();
        self.refresh_shadow = self.shadow_key != Some(self.time_mode);
        self.shadow_key = Some(self.time_mode);

        let tod = &d.time_modes[self.time_mode];
        let sh = &d.shadow;
        let light_proj = orthographic(-sh.half_extent, sh.half_extent, -sh.half_extent, sh.half_extent, sh.near, sh.far);
        let light_vp = light_proj * look_at_mat4(tod.sun_pos, sh.target, Vec3::Y);
        let sun_dir = (tod.sun_pos - sh.target).normalize();
        let pc = d.point_light_col;

        let uniforms = Uniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, t],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, tod.fog_d],
            sky_top: [tod.sky_top[0], tod.sky_top[1], tod.sky_top[2], tod.exposure],
            sky_mid: [tod.sky_mid[0], tod.sky_mid[1], tod.sky_mid[2], tod.sun_int],
            sky_bot: [tod.sky_bot[0], tod.sky_bot[1], tod.sky_bot[2], tod.hemi_int],
            fog_col: [tod.fog_col[0], tod.fog_col[1], tod.fog_col[2], tod.point_int],
            sun_col: [tod.sun_col[0], tod.sun_col[1], tod.sun_col[2], tod.win_emissive],
            hemi_sky: [tod.hemi_sky[0], tod.hemi_sky[1], tod.hemi_sky[2], tod.lamp_emissive],
            hemi_gnd: [tod.hemi_gnd[0], tod.hemi_gnd[1], tod.hemi_gnd[2], 0.0],
            extra: [pc[0], pc[1], pc[2], tod.stars],
            point_lights: d.point_lights_padded(),
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let pcfg = &d.particles;
        self.queue.write_buffer(
            &self.sim_buf,
            0,
            bytemuck::bytes_of(&SimParams {
                dt,
                time: t,
                count: self.sg.petal_count,
                paused: u32::from(self.paused),
                bmin: [pcfg.bmin.x, pcfg.bmin.y, pcfg.bmin.z, 0.0],
                bmax: [pcfg.bmax.x, pcfg.bmax.y, pcfg.bmax.z, 0.0],
            }),
        );
        let col = |hex: u32| {
            let c = [((hex >> 16) & 0xff) as f32 / 255.0, ((hex >> 8) & 0xff) as f32 / 255.0, (hex & 0xff) as f32 / 255.0];
            [c[0], c[1], c[2], 1.0]
        };
        self.queue.write_buffer(
            &self.petal_cam_buf,
            0,
            bytemuck::bytes_of(&PetalCam {
                view_proj: vp.to_cols_array_2d(),
                cam_pos: [eye.x, eye.y, eye.z, tod.fog_d],
                fog_col: [tod.fog_col[0], tod.fog_col[1], tod.fog_col[2], tod.exposure],
                size: [pcfg.size.x, pcfg.size.y, pcfg.size.z, pcfg.glow],
                colors: pcfg.colors.map(col),
            }),
        );
    }

    fn encode(&self, color_view: &wgpu::TextureView, depth_view: &wgpu::TextureView) -> wgpu::CommandBuffer {
        let mut enc = self.device.create_command_encoder(&Default::default());

        if self.sg.petal_count > 0 {
            let mut cpass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("petal_cs"), timestamp_writes: None });
            cpass.set_pipeline(&self.pipes.petal_compute);
            cpass.set_bind_group(0, &self.sg.petal_comp_bg, &[]);
            cpass.dispatch_workgroups(self.sg.petal_count.div_ceil(64), 1, 1);
        }

        // Shadows: static casters are rendered once per lighting setup into a
        // cached map; each frame that map is copied and only moving casters drawn.
        let shadow_pass = |enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, clear: bool, list: &[DrawBatch]| {
            let load = if clear { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load };
            let mut spass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations { load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            spass.set_pipeline(&self.pipes.shadow);
            spass.set_bind_group(0, &self.shadow_bg, &[]);
            spass.set_vertex_buffer(0, self.sg.vb.slice(..));
            spass.set_vertex_buffer(1, self.sg.instance_buf.slice(..));
            spass.set_index_buffer(self.sg.ib.slice(..), wgpu::IndexFormat::Uint32);
            for b in list {
                Self::draw_batch(&mut spass, b);
            }
        };
        if self.refresh_shadow {
            shadow_pass(&mut enc, &self.static_shadow_view, true, &self.sg.static_shadow);
        }
        let full = wgpu::Extent3d { width: SHADOW_RES, height: SHADOW_RES, depth_or_array_layers: 1 };
        let tex_info = |t| wgpu::TexelCopyTextureInfo { texture: t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All };
        enc.copy_texture_to_texture(tex_info(&self.static_shadow_tex), tex_info(&self.shadow_tex), full);
        if !self.sg.dynamic_shadow.is_empty() {
            shadow_pass(&mut enc, &self.shadow_view, false, &self.sg.dynamic_shadow);
        }

        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            rpass.set_pipeline(&self.pipes.sky);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.draw(0..3, 0..1);

            rpass.set_pipeline(&self.pipes.opaque);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.sg.vb.slice(..));
            rpass.set_vertex_buffer(1, self.sg.instance_buf.slice(..));
            rpass.set_index_buffer(self.sg.ib.slice(..), wgpu::IndexFormat::Uint32);
            for (b, &v) in self.sg.opaque.batches.iter().zip(&self.vis_opaque) {
                if v {
                    Self::draw_batch(&mut rpass, b);
                }
            }

            if self.sg.petal_count > 0 {
                rpass.set_pipeline(&self.pipes.petal_render);
                rpass.set_bind_group(0, &self.petal_rend_bg, &[]);
                rpass.set_vertex_buffer(0, self.cube_vb.slice(..));
                rpass.set_index_buffer(self.cube_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.set_vertex_buffer(1, self.sg.petal_buf.slice(..));
                rpass.draw_indexed(0..36, 0, 0..self.sg.petal_count);
            }

            rpass.set_pipeline(&self.pipes.translucent);
            rpass.set_bind_group(0, &self.main_bg, &[]);
            rpass.set_vertex_buffer(0, self.sg.vb.slice(..));
            rpass.set_vertex_buffer(1, self.sg.instance_buf.slice(..));
            rpass.set_index_buffer(self.sg.ib.slice(..), wgpu::IndexFormat::Uint32);
            for (b, &v) in self.sg.translucent.batches.iter().zip(&self.vis_trans) {
                if v {
                    Self::draw_batch(&mut rpass, b);
                }
            }
        }
        enc.finish()
    }

    pub fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        self.fps_frames += 1;
        self.fps_accum += dt;
        if self.fps_accum >= 0.5 {
            let fps = (self.fps_frames as f32 / self.fps_accum).round() as u32;
            self.update_title(fps);
            self.fps_frames = 0;
            self.fps_accum = 0.0;
        }

        self.prepare(dt);

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                drop(f);
                self.resize(self.config.width, self.config.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.resize(self.config.width, self.config.height);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let cmd = self.encode(&view, &self.depth_view);
        self.queue.submit(Some(cmd));
        self.queue.present(frame);
    }

    /// Render `frames` frames offscreen (no vsync), waiting for the GPU after
    /// each one, and return (mean ms, 95th-percentile ms).
    pub fn bench(&mut self, frames: usize, w: u32, h: u32) -> (f32, f32) {
        let (ow, oh) = (self.config.width, self.config.height);
        self.config.width = w;
        self.config.height = h;
        let depth = make_depth(&self.device, w, h, "bench_depth");
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut times = Vec::with_capacity(frames);
        for i in 0..frames + 10 {
            let start = Instant::now();
            self.prepare(1.0 / 60.0);
            let cmd = self.encode(&view, &depth);
            self.queue.submit(Some(cmd));
            self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            if i >= 10 {
                times.push(start.elapsed().as_secs_f32() * 1000.0);
            }
        }
        self.config.width = ow;
        self.config.height = oh;
        let mean = times.iter().sum::<f32>() / times.len() as f32;
        times.sort_by(f32::total_cmp);
        (mean, times[(times.len() as f32 * 0.95) as usize])
    }

    /// Render one frame at sim time `t` offscreen and write it to `path` as PNG.
    pub fn capture(&mut self, t: f32, w: u32, h: u32, path: &str) {
        // Pre-roll particles a few seconds so they are spread out like a live run.
        let was_paused = self.paused;
        self.paused = false;
        self.sim_time = (t - 3.0).max(0.0);
        let (ow, oh) = (self.config.width, self.config.height);
        self.config.width = w;
        self.config.height = h;
        let depth = make_depth(&self.device, w, h, "cap_depth");
        let fmt = self.config.format;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: fmt,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let steps = ((t - self.sim_time) / 0.05).ceil().max(1.0) as usize;
        let step_dt = (t - self.sim_time) / steps as f32;
        for _ in 0..steps {
            self.prepare(step_dt.max(0.0));
            let cmd = self.encode(&view, &depth);
            self.queue.submit(Some(cmd));
        }
        self.paused = was_paused;
        self.config.width = ow;
        self.config.height = oh;

        let bpr = (w * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cap_buf"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = slice.get_mapped_range().expect("map capture buffer");
        let bgra = matches!(fmt, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let r = &data[(row * bpr) as usize..(row * bpr + w * 4) as usize];
            for px in r.chunks_exact(4) {
                if bgra {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        drop(data);
        buf.unmap();
        let file = std::fs::File::create(path).expect("create screenshot file");
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
        println!("Saved {path}");
    }
}
