use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderQualityPreset {
    InteractivePreview,
    Draft,
    Standard,
    HighQuality,
    MaximumProduction,
}

fn default_render_width() -> u32 { 1024 }
fn default_render_height() -> u32 { 768 }
fn default_still_samples() -> u32 { 64 }
fn default_sun_radius() -> f32 { 0.8 }
fn default_max_bounces() -> u32 { 4 }

impl RenderQualityPreset {
    /// Number of jittered samples accumulated in the interactive viewport.
    pub fn target_samples(self) -> u32 {
        match self {
            RenderQualityPreset::InteractivePreview => 1,
            RenderQualityPreset::Draft => 4,
            RenderQualityPreset::Standard => 16,
            RenderQualityPreset::HighQuality => 64,
            RenderQualityPreset::MaximumProduction => 256,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub quality: RenderQualityPreset,
    pub retro_90s_cgi: bool,     // Classic 1994 vibrant synthetic aesthetic
    pub bloom_intensity: f32,
    pub vignette_strength: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub chromatic_aberration: f32,
    #[serde(default = "default_max_bounces")]
    pub max_reflection_bounces: u32,
    pub supersampling_scale: f32,
    /// Ordered 4x4 Bayer dither to 5-6-5 levels (16-bit framebuffer look)
    #[serde(default)]
    pub dither_16bit: bool,
    /// F9 still render resolution
    #[serde(default = "default_render_width")]
    pub render_width: u32,
    #[serde(default = "default_render_height")]
    pub render_height: u32,
    /// F9 still render sample count
    #[serde(default = "default_still_samples")]
    pub still_samples: u32,
    /// Jitter the sun direction within its angular radius (soft shadows)
    #[serde(default)]
    pub soft_shadows: bool,
    #[serde(default = "default_sun_radius")]
    pub sun_angular_radius_deg: f32,
    /// Jitter reflection directions by material roughness
    #[serde(default)]
    pub blurry_reflections: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            quality: RenderQualityPreset::Standard,
            retro_90s_cgi: false,
            bloom_intensity: 0.35,
            vignette_strength: 0.25,
            contrast: 1.05,
            saturation: 1.10,
            gamma: 2.2,
            chromatic_aberration: 0.0,
            max_reflection_bounces: 4,
            supersampling_scale: 1.0,
            dither_16bit: false,
            render_width: default_render_width(),
            render_height: default_render_height(),
            still_samples: default_still_samples(),
            soft_shadows: false,
            sun_angular_radius_deg: default_sun_radius(),
            blurry_reflections: false,
        }
    }
}

impl PostProcessConfig {
    /// Replaces the visual look but keeps the output settings (render size and still samples).
    pub fn apply_look_preset(&mut self, look: PostProcessConfig) {
        let (width, height, samples) = (self.render_width, self.render_height, self.still_samples);
        *self = look;
        self.render_width = width;
        self.render_height = height;
        self.still_samples = samples;
    }

    /// Bryce 4 style: retro grade, hard shadows, no bloom / aberration
    pub fn classic_90s_preset() -> Self {
        Self {
            quality: RenderQualityPreset::Standard,
            retro_90s_cgi: true,
            bloom_intensity: 0.0,
            vignette_strength: 0.15,
            contrast: 1.15,
            saturation: 1.25,
            gamma: 2.2,
            chromatic_aberration: 0.0,
            max_reflection_bounces: 4,
            supersampling_scale: 1.0,
            dither_16bit: false,
            soft_shadows: false,
            blurry_reflections: false,
            ..Self::default()
        }
    }

    /// Bryce 7 style: filmic grade, soft shadows, blurry reflections, bloom
    pub fn modern_2000s_preset() -> Self {
        Self {
            quality: RenderQualityPreset::HighQuality,
            retro_90s_cgi: false,
            bloom_intensity: 0.25,
            vignette_strength: 0.2,
            contrast: 1.03,
            saturation: 1.05,
            gamma: 2.2,
            chromatic_aberration: 0.0015,
            max_reflection_bounces: 6,
            supersampling_scale: 1.0,
            dither_16bit: false,
            soft_shadows: true,
            sun_angular_radius_deg: 1.5,
            blurry_reflections: true,
            ..Self::default()
        }
    }

    pub fn retro_90s_preset() -> Self {
        Self {
            quality: RenderQualityPreset::Standard,
            retro_90s_cgi: true,
            bloom_intensity: 0.55,
            vignette_strength: 0.35,
            contrast: 1.25,
            saturation: 1.45,
            gamma: 2.0,
            chromatic_aberration: 0.006,
            max_reflection_bounces: 3,
            supersampling_scale: 1.0,
            dither_16bit: false,
            render_width: default_render_width(),
            render_height: default_render_height(),
            still_samples: default_still_samples(),
            soft_shadows: false,
            sun_angular_radius_deg: default_sun_radius(),
            blurry_reflections: false,
        }
    }

    pub fn photorealistic_preset() -> Self {
        Self {
            quality: RenderQualityPreset::HighQuality,
            retro_90s_cgi: false,
            bloom_intensity: 0.15,
            vignette_strength: 0.15,
            contrast: 1.02,
            saturation: 1.00,
            gamma: 2.2,
            chromatic_aberration: 0.001,
            max_reflection_bounces: 3,
            supersampling_scale: 1.0,
            dither_16bit: false,
            render_width: default_render_width(),
            render_height: default_render_height(),
            still_samples: default_still_samples(),
            soft_shadows: false,
            sun_angular_radius_deg: default_sun_radius(),
            blurry_reflections: false,
        }
    }
}
