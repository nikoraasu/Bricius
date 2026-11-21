use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterType {
    Ocean,
    Lake,
    CalmMirror,
    AlienLiquid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterConfig {
    pub water_type: WaterType,
    pub water_height: f32,
    pub wave_amplitude: f32,
    pub wave_frequency: f32,
    pub wave_speed: f32,
    pub wave_steepness: f32,
    pub reflection_strength: f32,
    pub transparency: f32,
    pub ior: f32,
    pub roughness: f32,
    pub deep_color: [f32; 4],
    pub shallow_color: [f32; 4],
    pub foam_intensity: f32,
    pub caustics_strength: f32,
    /// Animate waves over time (disables progressive accumulation)
    #[serde(default)]
    pub animate: bool,
    /// Render the water plane at all
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

impl Default for WaterConfig {
    fn default() -> Self {
        Self::ocean()
    }
}

impl WaterConfig {
    pub fn ocean() -> Self {
        Self {
            water_type: WaterType::Ocean,
            water_height: 0.0,
            wave_amplitude: 0.15,
            wave_frequency: 1.5,
            wave_speed: 1.8,
            wave_steepness: 1.2,
            reflection_strength: 0.90,
            transparency: 0.65,
            ior: 1.333,
            roughness: 0.04,
            deep_color: [0.02, 0.12, 0.22, 1.0], // Deep oceanic blue
            shallow_color: [0.05, 0.45, 0.55, 1.0], // Turquoise shore
            foam_intensity: 0.45,
            caustics_strength: 0.60,
            animate: false,
            enabled: true,
        }
    }

    pub fn lake() -> Self {
        Self {
            water_type: WaterType::Lake,
            water_height: 0.0,
            wave_amplitude: 0.05,
            wave_frequency: 2.2,
            wave_speed: 0.8,
            wave_steepness: 0.5,
            reflection_strength: 0.92,
            transparency: 0.80,
            ior: 1.333,
            roughness: 0.02,
            deep_color: [0.04, 0.18, 0.18, 1.0], // Alpine emerald lake
            shallow_color: [0.10, 0.50, 0.40, 1.0],
            foam_intensity: 0.10,
            caustics_strength: 0.40,
            animate: false,
            enabled: true,
        }
    }

    pub fn calm_mirror() -> Self {
        Self {
            water_type: WaterType::CalmMirror,
            water_height: 0.0,
            wave_amplitude: 0.0,
            wave_frequency: 1.0,
            wave_speed: 0.0,
            wave_steepness: 0.0,
            reflection_strength: 0.98, // Pure mirror reflection
            transparency: 0.40,
            ior: 1.333,
            roughness: 0.005,
            deep_color: [0.01, 0.05, 0.10, 1.0],
            shallow_color: [0.05, 0.20, 0.35, 1.0],
            foam_intensity: 0.0,
            caustics_strength: 0.0,
            animate: false,
            enabled: true,
        }
    }

    pub fn alien_liquid() -> Self {
        Self {
            water_type: WaterType::AlienLiquid,
            water_height: 0.0,
            wave_amplitude: 0.20,
            wave_frequency: 1.0,
            wave_speed: 1.2,
            wave_steepness: 1.5,
            reflection_strength: 0.85,
            transparency: 0.50,
            ior: 1.65, // Dense glowing mercury / alien nectar
            roughness: 0.08,
            deep_color: [0.35, 0.02, 0.45, 1.0], // Glowing magenta/purple
            shallow_color: [0.10, 0.95, 0.85, 1.0], // Glowing cyan edges
            foam_intensity: 0.80,
            caustics_strength: 0.90,
            animate: false,
            enabled: true,
        }
    }
}
