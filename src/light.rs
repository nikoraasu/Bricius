use serde::{Deserialize, Serialize};

pub const MAX_LIGHTS: usize = 8;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightKind {
    Point,
    Spot,
    Parallel,
}

impl LightKind {
    pub fn label(self) -> &'static str {
        match self {
            LightKind::Point => "Point",
            LightKind::Spot => "Spot",
            LightKind::Parallel => "Parallel",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightConfig {
    pub name: String,
    pub kind: LightKind,
    pub enabled: bool,
    pub position: [f32; 3],
    /// Direction the light points towards (normalized on upload). Used by Spot and Parallel.
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    /// Falloff distance for point / spot lights (inverse-square with smooth cutoff at `range`)
    pub range: f32,
    /// Spot cone half-angle in degrees
    pub spot_angle_deg: f32,
    /// 0..1 penumbra width relative to the cone angle
    pub spot_softness: f32,
    pub cast_shadows: bool,
}

impl LightConfig {
    pub fn new(kind: LightKind, position: [f32; 3]) -> Self {
        let (direction, intensity, range) = match kind {
            LightKind::Point => ([0.0, -1.0, 0.0], 40.0, 25.0),
            LightKind::Spot => ([0.0, -1.0, 0.0], 80.0, 40.0),
            LightKind::Parallel => ([-0.4, -1.0, -0.3], 1.0, 0.0),
        };
        Self {
            name: format!("{} Light", kind.label()),
            kind,
            enabled: true,
            position,
            direction,
            color: [1.0, 0.95, 0.85],
            intensity,
            range,
            spot_angle_deg: 35.0,
            spot_softness: 0.3,
            cast_shadows: true,
        }
    }
}

/// GPU layout: 4 vec4s per light.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
    // xyz: position, w: kind (0: Point, 1: Spot, 2: Parallel)
    pub pos_and_kind: [f32; 4],
    // xyz: normalized direction, w: range
    pub dir_and_range: [f32; 4],
    // rgb: color, w: intensity
    pub color_intensity: [f32; 4],
    // x: cos(outer angle), y: cos(inner angle), z: cast_shadows (0/1), w: padding
    pub spot_params: [f32; 4],
}

impl GpuLight {
    pub fn from_config(l: &LightConfig) -> Self {
        let d = glam::Vec3::from(l.direction);
        let d = if d.length_squared() > 1e-8 { d.normalize() } else { glam::Vec3::NEG_Y };
        let outer = l.spot_angle_deg.clamp(1.0, 89.0);
        let inner = outer * (1.0 - l.spot_softness.clamp(0.0, 1.0));
        let kind = match l.kind {
            LightKind::Point => 0.0,
            LightKind::Spot => 1.0,
            LightKind::Parallel => 2.0,
        };
        Self {
            pos_and_kind: [l.position[0], l.position[1], l.position[2], kind],
            dir_and_range: [d.x, d.y, d.z, l.range.max(0.01)],
            color_intensity: [l.color[0], l.color[1], l.color[2], l.intensity.max(0.0)],
            spot_params: [
                outer.to_radians().cos(),
                inner.to_radians().cos(),
                if l.cast_shadows { 1.0 } else { 0.0 },
                0.0,
            ],
        }
    }

    /// Packs the enabled lights (max 8) into the fixed GPU array; returns it with the active count.
    pub fn pack(lights: &[LightConfig]) -> ([GpuLight; MAX_LIGHTS], usize) {
        let mut out = [GpuLight::zeroed_light(); MAX_LIGHTS];
        let mut n = 0;
        for l in lights.iter().filter(|l| l.enabled).take(MAX_LIGHTS) {
            out[n] = GpuLight::from_config(l);
            n += 1;
        }
        (out, n)
    }

    fn zeroed_light() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}
