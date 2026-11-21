//! Offscreen preview render service (material balls, thumbnails).
//!
//! The UI never touches the GPU: it calls `EditorState::request_preview(id, scene, size)` every
//! frame it wants to show a preview and gets back the `egui::TextureId` of the most recent
//! result (if any). After the UI pass, `State` hands the queued requests to `PreviewService::service`,
//! which renders a few path-traced samples per frame per entry (up to a sample target) into a
//! private accumulation pair, then post-processes into an `Rgba8Unorm` texture registered with egui.
//! Entries whose inputs (byte-compared) did not change are not re-rendered; entries that were not
//! requested recently are not rendered at all (and are freed after a while).
use std::collections::HashMap;
use wgpu::util::DeviceExt;

use crate::material::GpuMaterial;
use crate::scene::{CameraUniform, GpuPrimitive, SceneController, SceneUniform};

const PREVIEW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Samples traced per entry per frame.
const SAMPLES_PER_FRAME: u32 = 4;
/// Entries advanced per frame.
const ENTRIES_PER_FRAME: usize = 2;
/// Frames an entry may go unrequested before its GPU resources are freed.
const EVICT_AFTER_FRAMES: u64 = 1800;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PreviewShape {
    Sphere,
    Cube,
    Plane,
}

/// Everything a preview render depends on: camera, scene uniform, primitives and materials.
#[derive(Clone)]
pub struct PreviewScene {
    pub camera: CameraUniform,
    pub scene: SceneUniform,
    pub prims: [GpuPrimitive; 32],
    pub materials: [GpuMaterial; GpuMaterial::NUM_PRESETS],
    pub num_prims: usize,
    /// Optional terrain heightmap (normalized, row-major `TERRAIN_PREVIEW_RES`^2) bound at binding 6.
    pub terrain_heights: Option<Vec<f32>>,
}

/// Resolution of the heightmap copy a terrain preview carries.
pub const TERRAIN_PREVIEW_RES: usize = 128;

fn empty_prims() -> [GpuPrimitive; 32] {
    [GpuPrimitive { pos_and_type: [0.0; 4], scale_and_mat: [0.0; 4], rot_and_csg: [0.0; 4], extra: [0.0; 4] }; 32]
}

fn look_camera(pos: glam::Vec3, target: glam::Vec3, tan_half_fov: f32) -> CameraUniform {
    let fwd = (target - pos).normalize();
    let right = fwd.cross(glam::Vec3::Y).normalize();
    let up = right.cross(fwd);
    CameraUniform {
        pos: [pos.x, pos.y, pos.z, 1.0],
        forward: [fwd.x, fwd.y, fwd.z, 0.0],
        right: [right.x, right.y, right.z, 0.0],
        up: [up.x, up.y, up.z, 0.0],
        params: [tan_half_fov, 1.0, 0.1, 1000.0],
    }
}

/// Scene controller with every post effect that makes previews slow or noisy switched off.
fn clean_ctrl() -> SceneController {
    let mut ctrl = SceneController::new();
    ctrl.water.enabled = false;
    ctrl.terrain.backdrop_enabled = false;
    ctrl.ground.enabled = false;
    ctrl.lights.clear();
    ctrl.postprocess.vignette_strength = 0.0;
    ctrl.postprocess.bloom_intensity = 0.0;
    ctrl.postprocess.chromatic_aberration = 0.0;
    ctrl.postprocess.max_reflection_bounces = 4;
    ctrl
}

impl PreviewScene {
    /// A chrome sphere on the water at eye height, looking toward the sun's side of the horizon.
    /// Used for sky presets, the live sky preview and water preset thumbnails.
    pub fn sky_thumb(sky: &crate::sky::SkyConfig, water: &crate::water::WaterConfig) -> Self {
        Self::sky_thumb_view(sky, water, None)
    }

    /// Same as `sky_thumb` with an explicit camera pitch in radians (negative looks down at the water).
    pub fn sky_thumb_view(sky: &crate::sky::SkyConfig, water: &crate::water::WaterConfig, pitch: Option<f32>) -> Self {
        let mut ctrl = clean_ctrl();
        ctrl.sky = sky.clone();
        ctrl.water = water.clone();
        ctrl.water.enabled = true;
        ctrl.water.animate = false;
        ctrl.water.water_height = 0.0;
        let mut materials = [GpuMaterial::default(); GpuMaterial::NUM_PRESETS];
        materials[0] = GpuMaterial::default_preset_list()[2];
        let mut prims = empty_prims();
        // Sphere 3.2 units ahead (slightly right of centre), resting just above the water
        let az = sky.sun_azimuth - 0.3;
        let dir = glam::Vec3::new(az.cos(), 0.0, az.sin());
        let right = dir.cross(glam::Vec3::Y).normalize();
        let sp = dir * 3.4 + right * 0.55 + glam::Vec3::new(0.0, 0.45, 0.0);
        prims[0] = GpuPrimitive {
            pos_and_type: [sp.x, sp.y, sp.z, 0.0],
            scale_and_mat: [0.6, 0.6, 0.6, 0.0],
            rot_and_csg: [0.0; 4],
            extra: [1.0, 0.0, 0.0, 0.0],
        };
        let pos = glam::Vec3::new(0.0, 1.3, 0.0);
        let pitch = pitch.unwrap_or_else(|| (sky.sun_elevation * 0.5).clamp(0.08, 0.38));
        let target = pos + glam::Vec3::new(dir.x * pitch.cos(), pitch.sin(), dir.z * pitch.cos()) * 10.0;
        let camera = look_camera(pos, target, 0.55);
        let scene = crate::state::make_scene_uniform(&ctrl, 0, 0, 0.0, 0.0, 1, false);
        Self { camera, scene, prims, materials, num_prims: 1, terrain_heights: None }
    }

    /// A terrain alone under a sun, seen from an orbit camera (`yaw`, `pitch` in radians).
    /// `height_ratio` = object scale.y / scale.x so the silhouette matches the scene object.
    pub fn terrain(grid: &crate::terrain::HeightmapGrid, material: &GpuMaterial, height_ratio: f32, yaw: f32, pitch: f32) -> Self {
        let mut ctrl = clean_ctrl();
        ctrl.sky = crate::sky::SkyConfig::clear_blue();
        ctrl.sky.sun_azimuth = 0.9;
        ctrl.sky.sun_elevation = 0.65;
        ctrl.sky.fog_density = 0.0015;
        ctrl.postprocess.max_reflection_bounces = 2;
        let mut materials = [GpuMaterial::default(); GpuMaterial::NUM_PRESETS];
        materials[0] = *material;
        let mut prims = empty_prims();
        let n = TERRAIN_PREVIEW_RES;
        let mut heights = Vec::with_capacity(n * n);
        for z in 0..n {
            for x in 0..n {
                heights.push(grid.sample_bilinear(x as f32 / (n - 1) as f32, z as f32 / (n - 1) as f32));
            }
        }
        let half = 2.0;
        let sy = (half * height_ratio.clamp(0.05, 2.0)).max(0.1);
        prims[0] = GpuPrimitive {
            pos_and_type: [0.0, 0.0, 0.0, 7.0],
            scale_and_mat: [half, sy, half, 0.0],
            rot_and_csg: [0.0; 4],
            extra: [1.0, 0.0, n as f32, 0.0],
        };
        let target = glam::Vec3::new(0.0, sy * 0.3, 0.0);
        let pitch = pitch.clamp(0.08, 1.45);
        let dist = 6.2;
        let pos = target + glam::Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), yaw.sin() * pitch.cos()) * dist;
        let camera = look_camera(pos, target, 0.42);
        let scene = crate::state::make_scene_uniform(&ctrl, 0, 0, 0.0, 0.0, 1, false);
        Self { camera, scene, prims, materials, num_prims: 1, terrain_heights: Some(heights) }
    }

    /// The material (slot 0) on a shape floating over a checkered ground (slot 1) under a sky + sun.
    pub fn material_ball(mat: &GpuMaterial, shape: PreviewShape) -> Self {
        let mut ctrl = SceneController::new();
        ctrl.sky.sun_azimuth = 0.9;
        ctrl.sky.sun_elevation = 0.75;
        ctrl.water.enabled = false;
        ctrl.terrain.backdrop_enabled = false;
        ctrl.ground.enabled = true;
        ctrl.ground.height = -1.0;
        ctrl.ground.material_id = 1;
        ctrl.lights.clear();
        ctrl.postprocess.vignette_strength = 0.0;
        ctrl.postprocess.bloom_intensity = 0.0;
        ctrl.postprocess.chromatic_aberration = 0.0;
        ctrl.postprocess.max_reflection_bounces = 4;

        let mut materials = [GpuMaterial::default(); GpuMaterial::NUM_PRESETS];
        materials[0] = *mat;
        let mut checker = GpuMaterial::default();
        checker.set_noise(0, 7, 0.9, 1.0, 0.0, 1.0);
        checker.set_tex_color(0, 0, [0.16, 0.16, 0.17]);
        checker.set_tex_color(0, 1, [0.5, 0.5, 0.5]);
        checker.set_tex_color(0, 2, [0.78, 0.78, 0.8]);
        checker.set_src(0, 1);
        checker.values1 = [0.15, 0.7, 0.08, 0.0];
        materials[1] = checker;

        let mut prims = [GpuPrimitive { pos_and_type: [0.0; 4], scale_and_mat: [0.0; 4], rot_and_csg: [0.0; 4], extra: [0.0; 4] }; 32];
        let (ty, scale, rot) = match shape {
            PreviewShape::Sphere => (0.0, [1.0, 1.0, 1.0], [0.0; 3]),
            PreviewShape::Cube => (1.0, [0.8, 0.8, 0.8], [0.0, 30.0, 0.0]),
            PreviewShape::Plane => (1.0, [1.25, 1.25, 0.04], [0.0; 3]),
        };
        prims[0] = GpuPrimitive {
            pos_and_type: [0.0, 0.0, 0.0, ty],
            scale_and_mat: [scale[0], scale[1], scale[2], 0.0],
            rot_and_csg: [rot[0], rot[1], rot[2], 0.0],
            extra: [1.0, 0.0, 0.0, 0.0],
        };

        let pos = glam::Vec3::new(0.0, 0.55, 4.4);
        let target = glam::Vec3::new(0.0, -0.1, 0.0);
        let fwd = (target - pos).normalize();
        let right = fwd.cross(glam::Vec3::Y).normalize();
        let up = right.cross(fwd);
        let camera = CameraUniform {
            pos: [pos.x, pos.y, pos.z, 1.0],
            forward: [fwd.x, fwd.y, fwd.z, 0.0],
            right: [right.x, right.y, right.z, 0.0],
            up: [up.x, up.y, up.z, 0.0],
            params: [0.42, 1.0, 0.1, 1000.0],
        };
        let scene = crate::state::make_scene_uniform(&ctrl, 0, 0, 0.0, 0.0, 1, false);
        Self { camera, scene, prims, materials, num_prims: 1, terrain_heights: None }
    }

    fn key(&self) -> Vec<u8> {
        let mut k = Vec::new();
        k.extend_from_slice(bytemuck::bytes_of(&self.camera));
        k.extend_from_slice(bytemuck::bytes_of(&self.scene));
        k.extend_from_slice(bytemuck::cast_slice(&self.prims));
        k.extend_from_slice(bytemuck::cast_slice(&self.materials));
        if let Some(h) = &self.terrain_heights {
            k.extend_from_slice(bytemuck::cast_slice(h));
        }
        k
    }
}

pub struct PreviewRequest {
    pub id: u64,
    pub scene: PreviewScene,
    pub size: u32,
}

struct Entry {
    size: u32,
    key: Vec<u8>,
    samples_done: u32,
    samples_target: u32,
    cur: usize,
    last_requested: u64,
    egui_id: egui::TextureId,
    out_view: wgpu::TextureView,
    accum_views: [wgpu::TextureView; 2],
    accum_bgs: [wgpu::BindGroup; 2],
    bind_group: wgpu::BindGroup,
    scene_buf: wgpu::Buffer,
    camera_buf: wgpu::Buffer,
    prims_buf: wgpu::Buffer,
    materials_buf: wgpu::Buffer,
    terrain_buf: Option<wgpu::Buffer>,
}

/// Borrowed GPU objects the service needs from `State`.
pub struct PreviewGpu<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub egui_renderer: &'a mut egui_wgpu::Renderer,
    pub layout: &'a wgpu::BindGroupLayout,
    pub accum_layout: &'a wgpu::BindGroupLayout,
    pub trace_pipeline: &'a wgpu::RenderPipeline,
    pub display_pipeline: &'a wgpu::RenderPipeline,
}

pub struct PreviewService {
    entries: HashMap<u64, Entry>,
    frame: u64,
    lights_buf: wgpu::Buffer,
    mesh_buf: wgpu::Buffer,
    terrain_buf: wgpu::Buffer,
    bvh_buf: wgpu::Buffer,
}

impl PreviewService {
    pub fn new(device: &wgpu::Device) -> Self {
        let zeros = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &vec![0u8; size],
                usage,
            })
        };
        Self {
            entries: HashMap::new(),
            frame: 0,
            lights_buf: zeros("Preview Lights", 8 * 64, wgpu::BufferUsages::UNIFORM),
            mesh_buf: zeros("Preview Mesh", 64, wgpu::BufferUsages::STORAGE),
            terrain_buf: zeros("Preview Terrain", 16, wgpu::BufferUsages::STORAGE),
            bvh_buf: zeros("Preview BVH", 64, wgpu::BufferUsages::STORAGE),
        }
    }

    /// Texture ids of all live entries (for the UI to look up).
    pub fn texture_ids(&self) -> HashMap<u64, egui::TextureId> {
        self.entries.iter().map(|(k, e)| (*k, e.egui_id)).collect()
    }

    /// Handles this frame's requests and advances pending renders. Cheap when `requests` is empty.
    pub fn service(&mut self, gpu: &mut PreviewGpu, requests: Vec<PreviewRequest>) {
        self.frame += 1;
        for r in requests {
            let key = r.scene.key();
            let size = r.size.clamp(16, 512);
            let recreate = self.entries.get(&r.id).map_or(true, |e| e.size != size);
            if recreate {
                let entry = self.create_entry(gpu, &r, size, self.entries.get(&r.id).map(|e| e.egui_id));
                self.entries.insert(r.id, entry);
            }
            let e = self.entries.get_mut(&r.id).unwrap();
            e.last_requested = self.frame;
            if recreate || e.key != key {
                let mut sc = r.scene.scene;
                sc.resolution = [size as f32, size as f32, 0.0, r.scene.num_prims as f32];
                gpu.queue.write_buffer(&e.scene_buf, 0, bytemuck::bytes_of(&sc));
                gpu.queue.write_buffer(&e.camera_buf, 0, bytemuck::bytes_of(&r.scene.camera));
                gpu.queue.write_buffer(&e.prims_buf, 0, bytemuck::cast_slice(&r.scene.prims));
                gpu.queue.write_buffer(&e.materials_buf, 0, bytemuck::cast_slice(&r.scene.materials));
                if let (Some(buf), Some(h)) = (&e.terrain_buf, &r.scene.terrain_heights) {
                    gpu.queue.write_buffer(buf, 0, bytemuck::cast_slice(h));
                }
                e.key = key;
                e.samples_done = 0;
                e.samples_target = if size >= 128 { 32 } else { 12 };
            }
        }

        // Advance entries that were requested this frame and still need samples (round-robin by
        // fewest samples first so thumbnails fill in evenly).
        let frame = self.frame;
        let mut pending: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, e)| e.last_requested + 1 >= frame && e.samples_done < e.samples_target)
            .map(|(k, _)| *k)
            .collect();
        pending.sort_by_key(|k| (self.entries[k].samples_done, *k));
        for id in pending.into_iter().take(ENTRIES_PER_FRAME) {
            let e = self.entries.get_mut(&id).unwrap();
            let n = SAMPLES_PER_FRAME.min(e.samples_target - e.samples_done);
            for _ in 0..n {
                gpu.queue.write_buffer(
                    &e.scene_buf,
                    crate::state::sample_index_offset(),
                    bytemuck::bytes_of(&(e.samples_done as f32)),
                );
                let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Preview Trace") });
                crate::state::encode_fullscreen_pass(
                    &mut enc,
                    "Preview Trace Pass",
                    &e.accum_views[1 - e.cur],
                    gpu.trace_pipeline,
                    &e.bind_group,
                    &e.accum_bgs[e.cur],
                );
                gpu.queue.submit(Some(enc.finish()));
                e.cur = 1 - e.cur;
                e.samples_done += 1;
            }
            let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Preview Display") });
            crate::state::encode_fullscreen_pass(
                &mut enc,
                "Preview Display Pass",
                &e.out_view,
                gpu.display_pipeline,
                &e.bind_group,
                &e.accum_bgs[e.cur],
            );
            gpu.queue.submit(Some(enc.finish()));
        }

        // Free long-unused entries
        let stale: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, e)| frame.saturating_sub(e.last_requested) > EVICT_AFTER_FRAMES)
            .map(|(k, _)| *k)
            .collect();
        for k in stale {
            if let Some(e) = self.entries.remove(&k) {
                gpu.egui_renderer.free_texture(&e.egui_id);
            }
        }
    }

    fn create_entry(&self, gpu: &mut PreviewGpu, r: &PreviewRequest, size: u32, reuse_id: Option<egui::TextureId>) -> Entry {
        let device = gpu.device;
        let out = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Preview Output"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PREVIEW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let out_view = out.create_view(&wgpu::TextureViewDescriptor::default());
        let egui_id = match reuse_id {
            Some(id) => {
                gpu.egui_renderer.update_egui_texture_from_wgpu_texture(device, &out_view, wgpu::FilterMode::Linear, id);
                id
            }
            None => gpu.egui_renderer.register_native_texture(device, &out_view, wgpu::FilterMode::Linear),
        };
        let (accum_views, accum_bgs) = crate::state::create_accum_targets(device, gpu.accum_layout, size, size);
        let mk = |label: &str, bytes: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        let camera_buf = mk("Preview Camera", bytemuck::bytes_of(&r.scene.camera));
        let scene_buf = mk("Preview Scene", bytemuck::bytes_of(&r.scene.scene));
        let prims_buf = mk("Preview Prims", bytemuck::cast_slice(&r.scene.prims));
        let materials_buf = mk("Preview Materials", bytemuck::cast_slice(&r.scene.materials));
        let terrain_buf = r.scene.terrain_heights.as_ref().map(|h| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Preview Terrain Heights"),
                contents: bytemuck::cast_slice(h),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            })
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Preview Bind Group"),
            layout: gpu.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: camera_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: scene_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: materials_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: prims_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: self.mesh_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: self.lights_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: terrain_buf.as_ref().unwrap_or(&self.terrain_buf).as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: self.bvh_buf.as_entire_binding() },
            ],
        });
        Entry {
            size,
            key: Vec::new(),
            samples_done: 0,
            samples_target: 0,
            cur: 0,
            last_requested: self.frame,
            egui_id,
            out_view,
            accum_views,
            accum_bgs,
            bind_group,
            scene_buf,
            camera_buf,
            prims_buf,
            materials_buf,
            terrain_buf,
        }
    }
}
