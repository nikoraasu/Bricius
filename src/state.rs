use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::Camera;
use crate::editor::EditorState;
use crate::material::GpuMaterial;
use crate::objects::{PrimitiveType, SceneObject};
use crate::scene::{CameraUniform, GpuPrimitive, SceneController, SceneUniform};

pub struct State {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,

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

    pub bind_group: wgpu::BindGroup,

    pub start_time: Instant,
    pub last_frame_time: Instant,
    pub frame_count: u32,

    // Egui Bryce 7 Studio UI System
    pub egui_ctx: egui::Context,
    pub egui_winit_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
    pub editor: EditorState,

    // Input tracking
    pub keys_pressed: std::collections::HashSet<winit::keyboard::KeyCode>,
    pub mouse_pressed: bool,
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
                label: Some("Bryce Modern Device"),
                ..Default::default()
            })
            .await
            .expect("Failed to create wgpu device");

        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = surface
            .get_default_config(&adapter, width, height)
            .expect("Surface incompatible with adapter");
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
        let scene_uniform = SceneUniform {
            sun_direction: scene_ctrl.sun_dir(),
            sun_color_intensity: [scene_ctrl.sky.sun_color[0], scene_ctrl.sky.sun_color[1], scene_ctrl.sky.sun_color[2], scene_ctrl.sky.sun_intensity],
            zenith_color: scene_ctrl.sky.zenith_color,
            horizon_color: scene_ctrl.sky.horizon_color,
            ground_color: scene_ctrl.sky.ground_color,
            fog_params: [scene_ctrl.sky.fog_density, scene_ctrl.sky.fog_height_falloff, scene_ctrl.sky.mie_coeff, scene_ctrl.sky.cloud_coverage],
            fog_color: scene_ctrl.sky.fog_color,
            water_params1: [scene_ctrl.water.water_height, scene_ctrl.water.wave_amplitude, scene_ctrl.water.wave_frequency, scene_ctrl.water.wave_speed],
            water_params2: [scene_ctrl.water.transparency, scene_ctrl.water.ior, scene_ctrl.water.roughness, scene_ctrl.water.reflection_strength],
            water_deep_color: scene_ctrl.water.deep_color,
            water_shallow_color: scene_ctrl.water.shallow_color,
            terrain_params1: [scene_ctrl.terrain.height_scale, scene_ctrl.terrain.frequency, scene_ctrl.terrain.octaves as f32, scene_ctrl.terrain.algorithm as u32 as f32],
            terrain_params2: [scene_ctrl.terrain.mountain_sharpness, scene_ctrl.terrain.ridge_intensity, scene_ctrl.terrain.valley_depth, scene_ctrl.terrain.domain_warp],
            postprocess_params: [if scene_ctrl.postprocess.retro_90s_cgi { 1.0 } else { 0.0 }, scene_ctrl.postprocess.bloom_intensity, scene_ctrl.postprocess.contrast, scene_ctrl.postprocess.saturation],
            time_and_flags: [0.0, scene_ctrl.selected_material as f32, scene_ctrl.terrain.seed as f32, scene_ctrl.render_mode as f32],
            resolution: [width as f32, height as f32, 0.0, 3.0],
        };

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
        }; 8];

        for (i, obj) in objects.iter().enumerate().take(8) {
            let prim_type = match obj.primitive {
                PrimitiveType::Sphere => 0.0,
                PrimitiveType::Cube => 1.0,
                PrimitiveType::Cylinder => 2.0,
                PrimitiveType::Torus => 3.0,
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

        // Create Bind Group Layout (4 bindings)
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
            ],
        });

        // Load WGSL Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bryce Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bryce_render.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bryce Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
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
        });

        // Initialize Egui & Bryce 7 Studio UI
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
            render_pipeline,
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
            last_mouse_pos: None,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
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
        let tan_half_fov = (self.camera.fov_y * 0.5).tan();
        let aspect_ratio = self.config.width as f32 / self.config.height as f32;
        self.camera_uniform = CameraUniform {
            pos: [self.camera.position.x, self.camera.position.y, self.camera.position.z, 1.0],
            forward: [self.camera.forward().x, self.camera.forward().y, self.camera.forward().z, 0.0],
            right: [self.camera.right().x, self.camera.right().y, self.camera.right().z, 0.0],
            up: [self.camera.up().x, self.camera.up().y, self.camera.up().z, 0.0],
            params: [tan_half_fov, aspect_ratio, 0.1, 1000.0],
        };
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

        // Update Dynamic Scene Primitives Buffer
        let mut gpu_prims = [GpuPrimitive {
            pos_and_type: [0.0; 4],
            scale_and_mat: [0.0; 4],
            rot_and_csg: [0.0; 4],
            extra: [0.0; 4],
        }; 8];

        let count = self.objects.len().min(8);
        for i in 0..count {
            let obj = &self.objects[i];
            let prim_type = match obj.primitive {
                PrimitiveType::Sphere => 0.0,
                PrimitiveType::Cube => 1.0,
                PrimitiveType::Cylinder => 2.0,
                PrimitiveType::Torus => 3.0,
                _ => 0.0,
            };
            gpu_prims[i] = GpuPrimitive {
                pos_and_type: [obj.position[0], obj.position[1], obj.position[2], prim_type],
                scale_and_mat: [obj.scale[0], obj.scale[1], obj.scale[2], obj.material_id as f32],
                rot_and_csg: [obj.rotation[0], obj.rotation[1], obj.rotation[2], obj.csg_op as u32 as f32],
                extra: [if obj.visible { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
            };
        }
        self.queue.write_buffer(&self.primitives_buffer, 0, bytemuck::cast_slice(&gpu_prims));

        // Update Scene Uniform
        let ctrl = &self.scene_ctrl;
        self.scene_uniform = SceneUniform {
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
            time_and_flags: [elapsed, ctrl.selected_material as f32, ctrl.terrain.seed as f32, ctrl.render_mode as f32],
            resolution: [self.config.width as f32, self.config.height as f32, self.frame_count as f32, count as f32],
        };
        self.queue.write_buffer(&self.scene_buffer, 0, bytemuck::cast_slice(&[self.scene_uniform]));
    }

    pub fn render(&mut self) -> Result<(), &'static str> {
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

        // 1. Run Egui Bryce 7 Studio UI Pass
        let raw_input = self.egui_winit_state.take_egui_input(&self.window);
        self.egui_ctx.begin_pass(raw_input);
        let mut mat_dirty = self.material_dirty;
        self.editor.draw(
            &self.egui_ctx,
            &mut self.scene_ctrl,
            &mut self.camera,
            &mut self.materials,
            &mut self.objects,
            &mut mat_dirty,
        );
        self.material_dirty = mat_dirty;
        let full_output = self.egui_ctx.end_pass();

        // If user changed materials in UI, write to GPU uniform buffer immediately
        if self.material_dirty {
            self.queue.write_buffer(
                &self.materials_buffer,
                0,
                bytemuck::cast_slice(&self.materials),
            );
            self.material_dirty = false;
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

        // 2. Render Pass: Draw Procedural 3D Environment + Egui Bryce Studio Overlay
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bryce Viewport Pass"),
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

            // Draw 3D Procedural Raytracer
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);

            // Draw Bryce Studio UI
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
}
