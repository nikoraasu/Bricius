use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::Camera;
use crate::editor::EditorState;
use crate::material::GpuMaterial;
use crate::objects::{PrimitiveType, SceneObject};
use crate::scene::{CameraUniform, GpuPrimitive, SceneController, SceneUniform};


/// Number of f32-aligned bytes into SceneUniform where the sample index (resolution.z) lives.
fn sample_index_offset() -> u64 {
    (std::mem::offset_of!(SceneUniform, resolution) + 8) as u64
}

const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

fn make_scene_uniform(
    ctrl: &SceneController,
    width: u32,
    height: u32,
    sample_index: f32,
    time: f32,
    num_prims: usize,
    surface_srgb: bool,
) -> SceneUniform {
    SceneUniform {
        sun_direction: ctrl.sun_dir(),
        sun_color_intensity: [ctrl.sky.sun_color[0], ctrl.sky.sun_color[1], ctrl.sky.sun_color[2], ctrl.sky.sun_intensity],
        zenith_color: ctrl.sky.zenith_color,
        horizon_color: ctrl.sky.horizon_color,
        ground_color: ctrl.sky.ground_color,
        fog_params: [ctrl.sky.fog_density, ctrl.sky.fog_height_falloff, ctrl.sky.mie_coeff, ctrl.sky.cloud_coverage],
        fog_color: ctrl.sky.fog_color,
        water_params1: [ctrl.water.water_height, ctrl.water.wave_amplitude, ctrl.water.wave_frequency, ctrl.water.wave_speed],
        water_params2: [ctrl.water.transparency, ctrl.water.ior, ctrl.water.roughness, ctrl.water.reflection_strength],
        water_deep_color: ctrl.water.deep_color,
        water_shallow_color: ctrl.water.shallow_color,
        terrain_params1: [ctrl.terrain.height_scale, ctrl.terrain.frequency, ctrl.terrain.octaves as f32, ctrl.terrain.algorithm as u32 as f32],
        terrain_params2: [ctrl.terrain.mountain_sharpness, ctrl.terrain.ridge_intensity, ctrl.terrain.valley_depth, ctrl.terrain.domain_warp],
        postprocess_params: [if ctrl.postprocess.retro_90s_cgi { 1.0 } else { 0.0 }, ctrl.postprocess.bloom_intensity, ctrl.postprocess.contrast, ctrl.postprocess.saturation],
        time_and_flags: [time, ctrl.selected_material as f32, ctrl.terrain.seed as f32, ctrl.render_mode as f32],
        resolution: [width as f32, height as f32, sample_index, num_prims as f32],
        post_params2: [
            ctrl.postprocess.vignette_strength,
            ctrl.postprocess.gamma,
            if ctrl.postprocess.dither_16bit { 1.0 } else { 0.0 },
            if surface_srgb { 1.0 } else { 0.0 },
        ],
        render_params: [
            ctrl.postprocess.max_reflection_bounces.clamp(1, 8) as f32,
            if ctrl.postprocess.soft_shadows { 1.0 } else { 0.0 },
            ctrl.postprocess.sun_angular_radius_deg,
            if ctrl.postprocess.blurry_reflections { 1.0 } else { 0.0 },
        ],
        ground_params: [
            if ctrl.ground.enabled { 1.0 } else { 0.0 },
            ctrl.ground.height,
            ctrl.ground.material_id as f32,
            if ctrl.water.enabled { 1.0 } else { 0.0 },
        ],
        light_params: [
            ctrl.lights.iter().filter(|l| l.enabled).take(crate::light::MAX_LIGHTS).count() as f32,
            if ctrl.terrain.backdrop_enabled { 1.0 } else { 0.0 },
            0.0,
            0.0,
        ],
        display_params: [ctrl.postprocess.chromatic_aberration, 0.0, 0.0, 0.0],
    }
}

fn build_camera_uniform(camera: &Camera, aspect_ratio: f32) -> CameraUniform {
    let tan_half_fov = (camera.fov_y * 0.5).tan();
    CameraUniform {
        pos: [camera.position.x, camera.position.y, camera.position.z, 1.0],
        forward: [camera.forward().x, camera.forward().y, camera.forward().z, 0.0],
        right: [camera.right().x, camera.right().y, camera.right().z, 0.0],
        up: [camera.up().x, camera.up().y, camera.up().z, 0.0],
        params: [tan_half_fov, aspect_ratio, 0.1, 1000.0],
    }
}

/// Creates the two ping-pong accumulation targets and a bind group per target
/// (bind group i samples texture i).
fn create_accum_targets(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> ([wgpu::TextureView; 2], [wgpu::BindGroup; 2]) {
    let make = |label: &str| {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ACCUM_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        (view, bg)
    };
    let (v0, b0) = make("Accumulation Texture A");
    let (v1, b1) = make("Accumulation Texture B");
    ([v0, v1], [b0, b1])
}

fn create_fullscreen_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fs_entry: &str,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Rgba32Float is not blendable; the shaders write final values directly
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn encode_fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    scene_bg: &wgpu::BindGroup,
    accum_bg: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, scene_bg, &[]);
    pass.set_bind_group(1, accum_bg, &[]);
    pass.draw(0..3, 0..1);
}

pub struct State {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub trace_pipeline: wgpu::RenderPipeline,
    pub display_pipeline: wgpu::RenderPipeline,
    pub display_pipeline_rgba8: wgpu::RenderPipeline,
    pub accum_layout: wgpu::BindGroupLayout,
    pub accum_views: [wgpu::TextureView; 2],
    pub accum_bind_groups: [wgpu::BindGroup; 2],
    /// Index of the accumulation texture holding the latest result
    pub accum_cur: usize,
    pub samples_done: u32,
    pub accum_reset: bool,
    pub last_state_key: Vec<u8>,
    pub surface_is_srgb: bool,

    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,

    pub scene_ctrl: SceneController,
    pub scene_uniform: SceneUniform,
    pub scene_buffer: wgpu::Buffer,

    pub materials: [GpuMaterial; GpuMaterial::NUM_PRESETS],
    pub materials_buffer: wgpu::Buffer,
    pub material_dirty: bool,

    pub objects: Vec<SceneObject>,
    pub primitives_buffer: wgpu::Buffer,
    pub lights_buffer: wgpu::Buffer,

    pub meshes: Vec<crate::model_loader::LoadedMesh>,
    pub mesh_triangles_buffer: wgpu::Buffer,
    pub mesh_dirty: bool,

    pub terrains: Vec<crate::terrain::TerrainObjectData>,
    pub terrain_buffer: wgpu::Buffer,
    pub terrain_dirty: bool,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,

    pub start_time: Instant,
    pub last_frame_time: Instant,
    pub frame_count: u32,

    // Egui Bricius Studio UI System
    pub egui_ctx: egui::Context,
    pub egui_winit_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
    pub editor: EditorState,

    // Input tracking & 3D Viewport Transform Dragging
    pub keys_pressed: std::collections::HashSet<winit::keyboard::KeyCode>,
    pub mouse_pressed: bool,
    pub dragging_object: bool,
    pub drag_start_obj_pos: glam::Vec3,
    pub drag_start_mouse: glam::Vec2,
    pub last_mouse_pos: Option<(f64, f64)>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create wgpu surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Bricius Device"),
                ..Default::default()
            })
            .await
            .expect("Failed to create wgpu device");

        let width = size.width.max(1);
        let height = size.height.max(1);

        let mut config = surface
            .get_default_config(&adapter, width, height)
            .expect("Surface incompatible with adapter");
        // The shader and egui both output gamma-encoded values, so prefer a non-sRGB
        // surface format (an sRGB one would encode a second time in hardware).
        if let Some(fmt) = surface.get_capabilities(&adapter).formats.iter().copied().find(|f| !f.is_srgb()) {
            config.format = fmt;
        }
        surface.configure(&device, &config);

        // Setup Camera
        let camera = Camera::new(glam::Vec3::new(0.0, 3.5, 6.0), -90.0, -8.0);
        let tan_half_fov = (camera.fov_y * 0.5).tan();
        let aspect_ratio = width as f32 / height as f32;

        let camera_uniform = CameraUniform {
            pos: [camera.position.x, camera.position.y, camera.position.z, 1.0],
            forward: [camera.forward().x, camera.forward().y, camera.forward().z, 0.0],
            right: [camera.right().x, camera.right().y, camera.right().z, 0.0],
            up: [camera.up().x, camera.up().y, camera.up().z, 0.0],
            params: [tan_half_fov, aspect_ratio, 0.1, 1000.0],
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Setup Scene Controller & Uniform
        let scene_ctrl = SceneController::new();
        let scene_uniform = make_scene_uniform(&scene_ctrl, width, height, 0.0, 0.0, 3, config.format.is_srgb());

        let scene_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene Buffer"),
            contents: bytemuck::cast_slice(&[scene_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Setup 16 Material Presets Uniform Buffer
        let materials = GpuMaterial::default_preset_list();
        let materials_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Materials Buffer"),
            contents: bytemuck::cast_slice(&materials),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Setup Initial Scene Objects
        let objects = vec![
            SceneObject::sphere("Chrome Showcase Orb", [0.0, 3.2, -6.0], 1.8, 2),
            SceneObject::cube("Basalt Monolith", [-5.5, 2.0, -8.0], [1.2, 3.0, 1.2], 5),
            SceneObject::sphere("Emerald Crystal", [5.5, 3.0, -8.0], 1.4, 3),
        ];

        let mut gpu_prims = [GpuPrimitive {
            pos_and_type: [0.0; 4],
            scale_and_mat: [0.0; 4],
            rot_and_csg: [0.0; 4],
            extra: [0.0; 4],
        }; 32];

        for (i, obj) in objects.iter().enumerate().take(32) {
            let prim_type = match obj.primitive {
                PrimitiveType::Sphere => 0.0,
                PrimitiveType::Cube => 1.0,
                PrimitiveType::Cylinder => 2.0,
                PrimitiveType::Torus => 3.0,
                PrimitiveType::Cone => 4.0,
                PrimitiveType::Pyramid => 5.0,
                PrimitiveType::Mesh => 6.0,
                PrimitiveType::Terrain => 7.0,
                _ => 0.0,
            };
            gpu_prims[i] = GpuPrimitive {
                pos_and_type: [obj.position[0], obj.position[1], obj.position[2], prim_type],
                scale_and_mat: [obj.scale[0], obj.scale[1], obj.scale[2], obj.material_id as f32],
                rot_and_csg: [obj.rotation[0], obj.rotation[1], obj.rotation[2], obj.csg_op as u32 as f32],
                extra: [if obj.visible { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
            };
        }

        let primitives_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Primitives Buffer"),
            contents: bytemuck::cast_slice(&gpu_prims),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Initialize Built-in Mesh Models
        let meshes = vec![
            crate::model_loader::ModelLoader::utah_teapot(),
            crate::model_loader::ModelLoader::obelisk_monolith(),
            crate::model_loader::ModelLoader::crystal_geode(),
            crate::model_loader::ModelLoader::starfighter_ship(),
        ];

        let mut gpu_triangles = Vec::new();
        for (mesh_idx, mesh) in meshes.iter().enumerate() {
            let num_triangles = mesh.indices.len() / 3;
            for t in 0..num_triangles {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                    let n = if i0 < mesh.normals.len() { mesh.normals[i0] } else { [0.0, 1.0, 0.0] };
                    gpu_triangles.push(crate::scene::GpuTriangle {
                        v0: [mesh.vertices[i0][0], mesh.vertices[i0][1], mesh.vertices[i0][2], mesh_idx as f32],
                        v1: [mesh.vertices[i1][0], mesh.vertices[i1][1], mesh.vertices[i1][2], 0.0],
                        v2: [mesh.vertices[i2][0], mesh.vertices[i2][1], mesh.vertices[i2][2], 0.0],
                        normal: [n[0], n[1], n[2], 0.0],
                    });
                }
            }
        }
        if gpu_triangles.is_empty() {
            gpu_triangles.push(crate::scene::GpuTriangle {
                v0: [0.0; 4],
                v1: [0.0; 4],
                v2: [0.0; 4],
                normal: [0.0, 1.0, 0.0, 0.0],
            });
        }

        let mesh_triangles_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Triangles Buffer"),
            contents: bytemuck::cast_slice(&gpu_triangles),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Terrain heightmaps storage buffer (concatenated f32 grids)
        let terrains: Vec<crate::terrain::TerrainObjectData> = Vec::new();
        let terrain_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Heightmaps Buffer"),
            contents: bytemuck::cast_slice(&Self::pack_terrain_heights(&terrains)),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Lights uniform buffer (8 x GpuLight)
        let (gpu_lights, _) = crate::light::GpuLight::pack(&scene_ctrl.lights);
        let lights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lights Buffer"),
            contents: bytemuck::cast_slice(&gpu_lights),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create Bind Group Layout (7 bindings)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Main Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: materials_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: primitives_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: mesh_triangles_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: terrain_buffer.as_entire_binding(),
                },
            ],
        });

        // Load WGSL Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bricius Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bricius_render.wgsl").into()),
        });

        let accum_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Accumulation Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let (accum_views, accum_bind_groups) = create_accum_targets(&device, &accum_layout, width, height);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout), Some(&accum_layout)],
            immediate_size: 0,
        });

        let trace_pipeline = create_fullscreen_pipeline(&device, &shader, &pipeline_layout, "fs_trace", ACCUM_FORMAT, "Bricius Trace Pipeline");
        let display_pipeline = create_fullscreen_pipeline(&device, &shader, &pipeline_layout, "fs_display", config.format, "Bricius Display Pipeline");
        let display_pipeline_rgba8 = create_fullscreen_pipeline(&device, &shader, &pipeline_layout, "fs_display", wgpu::TextureFormat::Rgba8Unorm, "Bricius Still Display Pipeline");
        let surface_is_srgb = config.format.is_srgb();

        // Initialize Egui & Bricius Studio UI
        let egui_ctx = egui::Context::default();
        let egui_winit_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        let editor = EditorState::new();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            trace_pipeline,
            display_pipeline,
            display_pipeline_rgba8,
            accum_layout,
            accum_views,
            accum_bind_groups,
            accum_cur: 0,
            samples_done: 0,
            accum_reset: true,
            last_state_key: Vec::new(),
            surface_is_srgb,
            camera,
            camera_uniform,
            camera_buffer,
            scene_ctrl,
            scene_uniform,
            scene_buffer,
            materials,
            materials_buffer,
            material_dirty: false,
            objects,
            primitives_buffer,
            lights_buffer,
            meshes,
            mesh_triangles_buffer,
            mesh_dirty: false,
            terrains,
            terrain_buffer,
            terrain_dirty: false,
            bind_group_layout,
            bind_group,
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
            frame_count: 0,
            egui_ctx,
            egui_winit_state,
            egui_renderer,
            editor,
            keys_pressed: std::collections::HashSet::new(),
            mouse_pressed: false,
            dragging_object: false,
            drag_start_obj_pos: glam::Vec3::ZERO,
            drag_start_mouse: glam::Vec2::ZERO,
            last_mouse_pos: None,
        }
    }

    /// Concatenate all terrain heightmaps into one f32 array (never empty: storage buffers need >0 bytes)
    pub fn pack_terrain_heights(terrains: &[crate::terrain::TerrainObjectData]) -> Vec<f32> {
        let mut out: Vec<f32> = Vec::new();
        for t in terrains {
            out.extend_from_slice(&t.heightmap.data);
        }
        if out.is_empty() {
            out.push(0.0);
        }
        out
    }

    pub fn rebuild_terrain_buffer(&mut self) {
        self.terrain_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Heightmaps Buffer"),
            contents: bytemuck::cast_slice(&Self::pack_terrain_heights(&self.terrains)),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        self.rebuild_bind_group();
        self.terrain_dirty = false;
    }

    pub fn rebuild_mesh_buffer(&mut self) {
        let mut gpu_triangles = Vec::new();
        for (mesh_idx, mesh) in self.meshes.iter().enumerate() {
            let num_triangles = mesh.indices.len() / 3;
            for t in 0..num_triangles {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;
                if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                    let n = if i0 < mesh.normals.len() { mesh.normals[i0] } else { [0.0, 1.0, 0.0] };
                    gpu_triangles.push(crate::scene::GpuTriangle {
                        v0: [mesh.vertices[i0][0], mesh.vertices[i0][1], mesh.vertices[i0][2], mesh_idx as f32],
                        v1: [mesh.vertices[i1][0], mesh.vertices[i1][1], mesh.vertices[i1][2], 0.0],
                        v2: [mesh.vertices[i2][0], mesh.vertices[i2][1], mesh.vertices[i2][2], 0.0],
                        normal: [n[0], n[1], n[2], 0.0],
                    });
                }
            }
        }
        if gpu_triangles.is_empty() {
            gpu_triangles.push(crate::scene::GpuTriangle {
                v0: [0.0; 4],
                v1: [0.0; 4],
                v2: [0.0; 4],
                normal: [0.0, 1.0, 0.0, 0.0],
            });
        }

        self.mesh_triangles_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Triangles Buffer"),
            contents: bytemuck::cast_slice(&gpu_triangles),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        self.rebuild_bind_group();
        self.mesh_dirty = false;
    }

    pub fn rebuild_bind_group(&mut self) {
        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Main Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.scene_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.materials_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.primitives_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.mesh_triangles_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.lights_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.terrain_buffer.as_entire_binding(),
                },
            ],
        });
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let (views, bgs) = create_accum_targets(&self.device, &self.accum_layout, new_size.width, new_size.height);
            self.accum_views = views;
            self.accum_bind_groups = bgs;
            self.accum_cur = 0;
            self.accum_reset = true;
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        self.frame_count = self.frame_count.wrapping_add(1);

        if dt > 0.0 {
            self.editor.frame_time_ms = dt * 1000.0;
            self.editor.fps = 1.0 / dt;
        }

        let elapsed = now.duration_since(self.start_time).as_secs_f32();

        // Keyboard navigation
        use winit::keyboard::KeyCode;
        if self.keys_pressed.contains(&KeyCode::KeyW) {
            self.camera.move_forward(dt);
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            self.camera.move_backward(dt);
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            self.camera.move_left(dt);
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            self.camera.move_right(dt);
        }
        if self.keys_pressed.contains(&KeyCode::Space) || self.keys_pressed.contains(&KeyCode::KeyE) {
            self.camera.move_up(dt);
        }
        if self.keys_pressed.contains(&KeyCode::ShiftLeft) || self.keys_pressed.contains(&KeyCode::KeyQ) {
            self.camera.move_down(dt);
        }

        // Sun movement controls
        if self.keys_pressed.contains(&KeyCode::ArrowLeft) {
            self.scene_ctrl.sky.sun_azimuth -= 1.2 * dt;
        }
        if self.keys_pressed.contains(&KeyCode::ArrowRight) {
            self.scene_ctrl.sky.sun_azimuth += 1.2 * dt;
        }
        if self.keys_pressed.contains(&KeyCode::ArrowUp) {
            self.scene_ctrl.sky.sun_elevation = (self.scene_ctrl.sky.sun_elevation + 0.8 * dt).min(1.5);
        }
        if self.keys_pressed.contains(&KeyCode::ArrowDown) {
            self.scene_ctrl.sky.sun_elevation = (self.scene_ctrl.sky.sun_elevation - 0.8 * dt).max(-0.2);
        }

        // Update Camera Uniform
        let aspect_ratio = self.config.width as f32 / self.config.height as f32;
        self.camera_uniform = build_camera_uniform(&self.camera, aspect_ratio);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

        // Update Dynamic Scene Primitives Buffer
        let mut gpu_prims = [GpuPrimitive {
            pos_and_type: [0.0; 4],
            scale_and_mat: [0.0; 4],
            rot_and_csg: [0.0; 4],
            extra: [0.0; 4],
        }; 32];

        let count = self.objects.len().min(32);
        for i in 0..count {
            let obj = &self.objects[i];
            let prim_type = match obj.primitive {
                PrimitiveType::Sphere => 0.0,
                PrimitiveType::Cube => 1.0,
                PrimitiveType::Cylinder => 2.0,
                PrimitiveType::Torus => 3.0,
                PrimitiveType::Cone => 4.0,
                PrimitiveType::Pyramid => 5.0,
                PrimitiveType::Mesh => 6.0,
                PrimitiveType::Terrain => 7.0,
                _ => 0.0,
            };

            let (tri_offset, tri_count) = if let Some(mesh_idx) = obj.mesh_id {
                let mut offset = 0;
                for (m_i, m) in self.meshes.iter().enumerate() {
                    if m_i == mesh_idx {
                        break;
                    }
                    offset += m.indices.len() / 3;
                }
                let c = self.meshes.get(mesh_idx).map(|m| m.indices.len() / 3).unwrap_or(0);
                (offset as f32, c as f32)
            } else {
                (0.0, 0.0)
            };

            // Terrains reuse tri_offset/tri_count: heightmap float offset and grid resolution
            let (tri_offset, tri_count) = match obj.terrain_id {
                Some(tid) if tid < self.terrains.len() => {
                    let off: usize = self.terrains[..tid].iter().map(|t| t.heightmap.data.len()).sum();
                    (off as f32, self.terrains[tid].heightmap.resolution as f32)
                }
                _ => (tri_offset, tri_count),
            };
            // A terrain object with a missing heightmap is hidden
            let visible = obj.visible && (obj.primitive != PrimitiveType::Terrain
                || obj.terrain_id.map_or(false, |t| t < self.terrains.len()));

            gpu_prims[i] = GpuPrimitive {
                pos_and_type: [obj.position[0], obj.position[1], obj.position[2], prim_type],
                scale_and_mat: [obj.scale[0], obj.scale[1], obj.scale[2], obj.material_id as f32],
                rot_and_csg: [obj.rotation[0], obj.rotation[1], obj.rotation[2], obj.csg_op as u32 as f32],
                extra: [if visible { 1.0 } else { 0.0 }, tri_offset, tri_count, 0.0],
            };
        }
        self.queue.write_buffer(&self.primitives_buffer, 0, bytemuck::cast_slice(&gpu_prims));

        let (gpu_lights, _) = crate::light::GpuLight::pack(&self.scene_ctrl.lights);
        self.queue.write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&gpu_lights));

        // Update Scene Uniform (shader time is frozen unless waves are animated, so accumulation converges)
        let shader_time = if self.scene_ctrl.water.animate { elapsed } else { 0.0 };
        self.scene_uniform = make_scene_uniform(&self.scene_ctrl, self.config.width, self.config.height, 0.0, shader_time, count, self.surface_is_srgb);
        self.queue.write_buffer(&self.scene_buffer, 0, bytemuck::cast_slice(&[self.scene_uniform]));

        // Detect anything that changes the traced image and restart accumulation.
        // Excluded: shader time, sample counter and display-only post-processing params.
        let mut scene_key = self.scene_uniform;
        scene_key.time_and_flags[0] = 0.0;
        scene_key.resolution[2] = 0.0;
        scene_key.postprocess_params = [0.0; 4];
        scene_key.post_params2 = [0.0; 4];
        scene_key.display_params = [0.0; 4];
        let mut key = Vec::with_capacity(4096);
        key.extend_from_slice(bytemuck::bytes_of(&self.camera_uniform));
        key.extend_from_slice(bytemuck::bytes_of(&scene_key));
        key.extend_from_slice(bytemuck::cast_slice(&gpu_prims));
        key.extend_from_slice(bytemuck::cast_slice(&gpu_lights));
        if key != self.last_state_key {
            self.last_state_key = key;
            self.accum_reset = true;
        }
        if self.scene_ctrl.water.animate {
            self.accum_reset = true;
        }
    }

    pub fn render(&mut self) -> Result<(), &'static str> {
        // If user requested a still-image render, capture pristine frame to PNG
        if self.editor.request_render {
            self.editor.request_render = false;
            self.capture_still_frame();
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout => return Err("Surface timeout"),
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Validation => return Err("Surface validation error"),
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 1. Run Egui Bricius Studio UI Pass
        let raw_input = self.egui_winit_state.take_egui_input(&self.window);
        self.egui_ctx.begin_pass(raw_input);
        let mut mat_dirty = self.material_dirty;
        let mut mesh_dirty = self.mesh_dirty;
        let mut terrain_dirty = self.terrain_dirty;
        self.editor.draw(
            &self.egui_ctx,
            &mut self.scene_ctrl,
            &mut self.camera,
            &mut self.materials,
            &mut self.objects,
            &mut self.meshes,
            &mut mat_dirty,
            &mut mesh_dirty,
            &mut self.terrains,
            &mut terrain_dirty,
        );
        self.material_dirty = mat_dirty;
        self.mesh_dirty = mesh_dirty;
        self.terrain_dirty = terrain_dirty;
        if self.terrain_dirty {
            self.rebuild_terrain_buffer();
            self.accum_reset = true;
        }

        if self.mesh_dirty {
            self.rebuild_mesh_buffer();
            self.accum_reset = true;
        }
        let full_output = self.egui_ctx.end_pass();

        // If user changed materials in UI, write to GPU uniform buffer immediately
        if self.material_dirty {
            self.queue.write_buffer(
                &self.materials_buffer,
                0,
                bytemuck::cast_slice(&self.materials),
            );
            self.material_dirty = false;
            self.accum_reset = true;
        }

        self.egui_winit_state.handle_platform_output(&self.window, full_output.platform_output);

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Main Render Encoder"),
        });

        let mut cmd_buffers = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // 2a. Progressive accumulation: trace one more jittered sample until the target is reached
        if self.accum_reset {
            self.samples_done = 0;
            self.accum_reset = false;
        }
        let target_samples = if self.scene_ctrl.water.animate {
            1
        } else {
            self.scene_ctrl.postprocess.quality.target_samples()
        };
        if self.samples_done < target_samples {
            self.queue.write_buffer(
                &self.scene_buffer,
                sample_index_offset(),
                bytemuck::bytes_of(&(self.samples_done as f32)),
            );
            let dst = 1 - self.accum_cur;
            encode_fullscreen_pass(
                &mut encoder,
                "Bricius Trace Pass",
                &self.accum_views[dst],
                &self.trace_pipeline,
                &self.bind_group,
                &self.accum_bind_groups[self.accum_cur],
            );
            self.accum_cur = dst;
            self.samples_done += 1;
        }
        self.editor.samples_done = self.samples_done;
        self.editor.sample_target = target_samples;

        // 2b. Render Pass: Display accumulated image (post-processing) + Egui Bricius Studio Overlay
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bricius Viewport Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.10,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            }).forget_lifetime();

            // Draw accumulated 3D image
            render_pass.set_pipeline(&self.display_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_bind_group(1, &self.accum_bind_groups[self.accum_cur], &[]);
            render_pass.draw(0..3, 0..1);

            // Draw Bricius Studio UI
            self.egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        cmd_buffers.push(encoder.finish());
        self.queue.submit(cmd_buffers);
        output.present();

        Ok(())
    }

    pub fn capture_still_frame(&mut self) {
        let pp = self.scene_ctrl.postprocess.clone();
        let width = pp.render_width.clamp(16, 8192);
        let height = pp.render_height.clamp(16, 8192);
        let samples = pp.still_samples.clamp(1, 4096);

        // Dedicated uniforms so the still render has its own resolution / aspect / sample counter
        let elapsed = Instant::now().duration_since(self.start_time).as_secs_f32();
        let shader_time = if self.scene_ctrl.water.animate { elapsed } else { 0.0 };
        let num_prims = self.objects.len().min(32);
        let still_camera = build_camera_uniform(&self.camera, width as f32 / height as f32);
        let still_scene = make_scene_uniform(&self.scene_ctrl, width, height, 0.0, shader_time, num_prims, false);

        let still_camera_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Still Camera Buffer"),
            contents: bytemuck::cast_slice(&[still_camera]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let still_scene_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Still Scene Buffer"),
            contents: bytemuck::cast_slice(&[still_scene]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let still_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Still Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: still_camera_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: still_scene_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.materials_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.primitives_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: self.mesh_triangles_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: self.lights_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: self.terrain_buffer.as_entire_binding() },
            ],
        });
        let (still_views, still_bgs) = create_accum_targets(&self.device, &self.accum_layout, width, height);

        // Accumulate jittered samples (one submission per sample keeps GPU work units short
        // and lets each sample see its own sample-index uniform value)
        let mut cur = 0usize;
        for i in 0..samples {
            self.queue.write_buffer(&still_scene_buffer, sample_index_offset(), bytemuck::bytes_of(&(i as f32)));
            let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Still Trace Encoder"),
            });
            encode_fullscreen_pass(
                &mut enc,
                "Still Trace Pass",
                &still_views[1 - cur],
                &self.trace_pipeline,
                &still_bind_group,
                &still_bgs[cur],
            );
            self.queue.submit(Some(enc.finish()));
            cur = 1 - cur;
            if i % 4 == 3 {
                let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            }
        }

        let capture_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Still Render Capture Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = capture_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Still Render Encoder"),
        });

        // Post-process the accumulated linear image into the 8-bit target (no UI)
        encode_fullscreen_pass(
            &mut encoder,
            "Still Display Pass",
            &view,
            &self.display_pipeline_rgba8,
            &still_bind_group,
            &still_bgs[cur],
        );

        // 2. Setup Staging Buffer to copy pixels back to CPU
        let bytes_per_pixel = 4;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT; // 256
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Still Render Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &capture_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let submission_index = self.queue.submit(Some(encoder.finish()));

        // 3. Map staging buffer synchronously
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index),
            timeout: None,
        });
        if let Ok(Ok(())) = rx.recv() {
            let data = buffer_slice.get_mapped_range();
            let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);

            // Still capture target is always Rgba8Unorm (no channel swap needed)
            let is_bgra = false;

            for y in 0..height {
                let row_start = (y * padded_bytes_per_row) as usize;
                for x in 0..width {
                    let px = row_start + (x * 4) as usize;
                    if is_bgra {
                        let b = data[px];
                        let g = data[px + 1];
                        let r = data[px + 2];
                        let a = data[px + 3];
                        rgba_pixels.extend_from_slice(&[r, g, b, a]);
                    } else {
                        let r = data[px];
                        let g = data[px + 1];
                        let b = data[px + 2];
                        let a = data[px + 3];
                        rgba_pixels.extend_from_slice(&[r, g, b, a]);
                    }
                }
            }
            drop(data);
            staging_buffer.unmap();

            if let Some(img) = image::RgbaImage::from_raw(width, height, rgba_pixels) {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let path = format!("bricius_render_{}.png", timestamp);
                match img.save(&path) {
                    Ok(_) => {
                        let _ = img.save("bricius_artwork.png");
                        self.editor.status_message = format!("📸 Rendered & saved {} ({}x{})", path, width, height);
                        println!("📸 Rendered still frame saved to: {}", path);
                    }
                    Err(e) => {
                        self.editor.status_message = format!("Render save error: {}", e);
                    }
                }
            }
        }
    }
}
