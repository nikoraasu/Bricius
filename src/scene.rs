use crate::postprocess::PostProcessConfig;
use crate::sky::SkyConfig;
use crate::terrain::TerrainConfig;
use crate::water::WaterConfig;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub pos: [f32; 4],
    pub forward: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    // x: tan_half_fov, y: aspect_ratio, z: near, w: far
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneUniform {
    pub sun_direction: [f32; 4],
    pub sun_color_intensity: [f32; 4],
    pub zenith_color: [f32; 4],
    pub horizon_color: [f32; 4],
    pub ground_color: [f32; 4],
    // x: fog_density, y: fog_height_falloff, z: mie_haze, w: cloud_density
    pub fog_params: [f32; 4],
    pub fog_color: [f32; 4],
    // x: water_height, y: wave_amp, z: wave_freq, w: wave_speed
    pub water_params1: [f32; 4],
    // x: transparency, y: ior, z: roughness, w: reflection_strength
    pub water_params2: [f32; 4],
    pub water_deep_color: [f32; 4],
    pub water_shallow_color: [f32; 4],
    // x: height_scale, y: frequency, z: octaves, w: algorithm
    pub terrain_params1: [f32; 4],
    // x: sharpness, y: ridge_intensity, z: valley_depth, w: domain_warp
    pub terrain_params2: [f32; 4],
    // x: retro_90s_cgi (1.0 or 0.0), y: bloom, z: contrast, w: saturation
    pub postprocess_params: [f32; 4],
    // x: time, y: selected_material, z: seed, w: render_mode
    pub time_and_flags: [f32; 4],
    // x: width, y: height, z: frame_count, w: num_active_primitives
    pub resolution: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPrimitive {
    // x, y, z: position, w: primitive_type (0: Sphere, 1: Cube, 2: Cylinder, 3: Torus)
    pub pos_and_type: [f32; 4],
    // x, y, z: scale/dimensions, w: material_id
    pub scale_and_mat: [f32; 4],
    // x, y, z: rotation (deg), w: csg_op (0: Positive, 1: Negative, 2: Intersecting)
    pub rot_and_csg: [f32; 4],
    // x: visible (1.0 or 0.0), y, z, w: padding
    pub extra: [f32; 4],
}

pub struct SceneController {
    pub terrain: TerrainConfig,
    pub sky: SkyConfig,
    pub water: WaterConfig,
    pub postprocess: PostProcessConfig,
    pub selected_material: u32,
    pub render_mode: u32,
}

impl SceneController {
    pub fn new() -> Self {
        Self {
            terrain: TerrainConfig::default(),
            sky: SkyConfig::default(),
            water: WaterConfig::default(),
            postprocess: PostProcessConfig::default(),
            selected_material: 1,
            render_mode: 0,
        }
    }

    pub fn sun_dir(&self) -> [f32; 4] {
        let x = self.sky.sun_azimuth.cos() * self.sky.sun_elevation.cos();
        let y = self.sky.sun_elevation.sin();
        let z = self.sky.sun_azimuth.sin() * self.sky.sun_elevation.cos();
        let len = (x * x + y * y + z * z).sqrt().max(0.001);
        [x / len, y / len, z / len, 0.0]
    }
}
