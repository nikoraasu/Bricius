use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderQualityPreset {
    InteractivePreview,
    Draft,
    Standard,
    HighQuality,
    MaximumProduction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub quality: RenderQualityPreset,
    pub retro_90s_cgi: bool,     // Classic Bryce 1994 vibrant synthetic aesthetic
    pub bloom_intensity: f32,
    pub vignette_strength: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub chromatic_aberration: f32,
    pub max_reflection_bounces: u32,
    pub supersampling_scale: f32,
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
            chromatic_aberration: 0.002,
            max_reflection_bounces: 2,
            supersampling_scale: 1.0,
        }
    }
}

impl PostProcessConfig {
    pub fn retro_bryce_preset() -> Self {
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
        }
    }
}
