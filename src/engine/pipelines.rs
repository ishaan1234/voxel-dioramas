//! GPU-side structs, bind group layouts and render/compute pipelines.

use bytemuck::{Pod, Zeroable};

use super::scene::MAX_POINT_LIGHTS;
use super::voxel::{ModelInstance, VoxelVertex};

pub const SHADOW_RES: u32 = 2048;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CubeVertex {
    pub pos: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PetalGpu {
    pub pos_spin: [f32; 4],
    pub vel_col: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub light_vp: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sky_top: [f32; 4],
    pub sky_mid: [f32; 4],
    pub sky_bot: [f32; 4],
    pub fog_col: [f32; 4],
    pub sun_col: [f32; 4],
    pub hemi_sky: [f32; 4],
    pub hemi_gnd: [f32; 4],
    /// rgb = point light colour, w = star-field intensity
    pub extra: [f32; 4],
    pub point_lights: [[f32; 4]; MAX_POINT_LIGHTS],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SimParams {
    pub dt: f32,
    pub time: f32,
    pub count: u32,
    pub paused: u32,
    pub bmin: [f32; 4],
    pub bmax: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PetalCam {
    pub view_proj: [[f32; 4]; 4],
    pub cam_pos: [f32; 4],
    pub fog_col: [f32; 4],
    /// xyz = half-size, w = glow multiplier
    pub size: [f32; 4],
    pub colors: [[f32; 4]; 4],
}

pub fn cube_mesh() -> ([CubeVertex; 24], [u16; 36]) {
    let f = |p: [[f32; 3]; 4]| p.map(|pos| CubeVertex { pos });
    let mut v = Vec::with_capacity(24);
    v.extend_from_slice(&f([[-0.5,-0.5, 0.5],[ 0.5,-0.5, 0.5],[ 0.5, 0.5, 0.5],[-0.5, 0.5, 0.5]]));
    v.extend_from_slice(&f([[ 0.5,-0.5,-0.5],[-0.5,-0.5,-0.5],[-0.5, 0.5,-0.5],[ 0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[ 0.5,-0.5, 0.5],[ 0.5,-0.5,-0.5],[ 0.5, 0.5,-0.5],[ 0.5, 0.5, 0.5]]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[-0.5,-0.5, 0.5],[-0.5, 0.5, 0.5],[-0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[-0.5, 0.5, 0.5],[ 0.5, 0.5, 0.5],[ 0.5, 0.5,-0.5],[-0.5, 0.5,-0.5]]));
    v.extend_from_slice(&f([[-0.5,-0.5,-0.5],[ 0.5,-0.5,-0.5],[ 0.5,-0.5, 0.5],[-0.5,-0.5, 0.5]]));
    let mut idx = [0u16; 36];
    for face in 0..6u16 {
        let b = face * 4;
        let o = face as usize * 6;
        idx[o..o + 6].copy_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    (v.try_into().unwrap(), idx)
}

pub fn make_depth(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}

pub struct Pipelines {
    pub sky: wgpu::RenderPipeline,
    pub shadow: wgpu::RenderPipeline,
    pub opaque: wgpu::RenderPipeline,
    pub translucent: wgpu::RenderPipeline,
    pub petal_compute: wgpu::ComputePipeline,
    pub petal_render: wgpu::RenderPipeline,
    pub main_bgl: wgpu::BindGroupLayout,
    pub shadow_bgl: wgpu::BindGroupLayout,
    pub petal_comp_bgl: wgpu::BindGroupLayout,
    pub petal_rend_bgl: wgpu::BindGroupLayout,
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction, bias: wgpu::DepthBiasState) -> Option<wgpu::DepthStencilState> {
    Some(wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias,
    })
}

fn color_target(fmt: wgpu::TextureFormat, blend: wgpu::BlendState) -> [Option<wgpu::ColorTargetState>; 1] {
    [Some(wgpu::ColorTargetState { format: fmt, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })]
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, fmt: wgpu::TextureFormat) -> Self {
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let main_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("main_bgl"),
            entries: &[
                uniform_entry(0, vf),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bgl"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let petal_comp_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_comp_bgl"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let petal_rend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("petal_rend_bgl"),
            entries: &[uniform_entry(0, vf)],
        });

        let layout = |label: &str, bgl: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            })
        };
        let shadow_pl = layout("shadow_pl", &shadow_bgl);
        let main_pl = layout("main_pl", &main_bgl);
        let petal_comp_pl = layout("petal_comp_pl", &petal_comp_bgl);
        let petal_rend_pl = layout("petal_rend_pl", &petal_rend_bgl);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/shader.wgsl"));
        let p_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/particles.wgsl"));

        let v_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VoxelVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Unorm16x4 },
                wgpu::VertexAttribute { offset: 20, shader_location: 2, format: wgpu::VertexFormat::Uint32 },
            ],
        };
        let i_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 32, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 48, shader_location: 7, format: wgpu::VertexFormat::Float32x4 },
            ],
        };
        let back_cull = wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() };

        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&main_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &color_target(fmt, wgpu::BlendState::REPLACE),
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_state(false, wgpu::CompareFunction::LessEqual, Default::default()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                buffers: &[Some(v_layout.clone()), Some(i_layout.clone())],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: back_cull,
            depth_stencil: depth_state(
                true,
                wgpu::CompareFunction::Less,
                wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            ),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let lit = |label: &str, fs: &str, blend: wgpu::BlendState, depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&main_pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(v_layout.clone()), Some(i_layout.clone())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &color_target(fmt, blend),
                    compilation_options: Default::default(),
                }),
                primitive: back_cull,
                depth_stencil: depth_state(depth_write, wgpu::CompareFunction::Less, Default::default()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let opaque = lit("opaque", "fs_opaque", wgpu::BlendState::REPLACE, true);
        let translucent = lit("translucent", "fs_translucent", wgpu::BlendState::ALPHA_BLENDING, false);

        let petal_compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("petal_compute"),
            layout: Some(&petal_comp_pl),
            module: &p_shader,
            entry_point: Some("cs_petals"),
            compilation_options: Default::default(),
            cache: None,
        });

        let pv_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 }],
        };
        let pi_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PetalGpu>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            ],
        };
        let petal_render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("petal_render"),
            layout: Some(&petal_rend_pl),
            vertex: wgpu::VertexState {
                module: &p_shader,
                entry_point: Some("vs_petal"),
                buffers: &[Some(pv_layout), Some(pi_layout)],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &p_shader,
                entry_point: Some("fs_petal"),
                targets: &color_target(fmt, wgpu::BlendState::REPLACE),
                compilation_options: Default::default(),
            }),
            primitive: back_cull,
            depth_stencil: depth_state(true, wgpu::CompareFunction::Less, Default::default()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            sky,
            shadow,
            opaque,
            translucent,
            petal_compute,
            petal_render,
            main_bgl,
            shadow_bgl,
            petal_comp_bgl,
            petal_rend_bgl,
        }
    }
}
