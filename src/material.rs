use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct GpuMaterial {
    pub base_color: [f32; 4],
    pub secondary_color: [f32; 4],
    // x: roughness, y: metallic, z: reflectivity, w: transparency
    pub params1: [f32; 4],
    // x: ior, y: bump_strength, z: noise_scale, w: noise_type
    // noise_type: 0 = Solid/Flat, 1 = fBm Granite, 2 = Ridged Multifractal, 3 = Marble Swirl, 4 = Worley Cellular, 5 = Wood Grain
    pub params2: [f32; 4],
    // x: distortion/warp, y: octaves, z: blend_sharpness, w: emission
    pub params3: [f32; 4],
}

impl GpuMaterial {
    pub const NUM_PRESETS: usize = 16;

    pub fn rock() -> Self {
        Self {
            base_color: [0.45, 0.40, 0.35, 1.0],
            secondary_color: [0.22, 0.20, 0.18, 1.0],
            params1: [0.85, 0.05, 0.05, 0.0],
            params2: [1.5, 0.45, 3.5, 1.0],
            params3: [0.3, 5.0, 1.5, 0.0],
        }
    }

    pub fn blue_marble() -> Self {
        Self {
            base_color: [0.92, 0.94, 0.97, 1.0],
            secondary_color: [0.10, 0.35, 0.60, 1.0],
            params1: [0.15, 0.0, 0.45, 0.0],
            params2: [1.55, 0.15, 2.0, 3.0],
            params3: [2.5, 4.0, 3.0, 0.0],
        }
    }

    pub fn chrome() -> Self {
        Self {
            base_color: [0.95, 0.95, 0.98, 1.0],
            secondary_color: [0.85, 0.88, 0.92, 1.0],
            params1: [0.02, 1.0, 0.98, 0.0],
            params2: [2.5, 0.0, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }
    }

    pub fn emerald_glass() -> Self {
        Self {
            base_color: [0.05, 0.85, 0.45, 1.0],
            secondary_color: [0.02, 0.40, 0.20, 1.0],
            params1: [0.05, 0.0, 0.15, 0.85],
            params2: [1.52, 0.0, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }
    }

    pub fn gold() -> Self {
        Self {
            base_color: [1.00, 0.82, 0.25, 1.0],
            secondary_color: [0.80, 0.55, 0.10, 1.0],
            params1: [0.25, 0.95, 0.80, 0.0],
            params2: [1.3, 0.25, 6.0, 1.0],
            params3: [0.1, 3.0, 1.0, 0.0],
        }
    }

    pub fn basalt_obsidian() -> Self {
        Self {
            base_color: [0.06, 0.06, 0.08, 1.0],
            secondary_color: [0.15, 0.12, 0.10, 1.0],
            params1: [0.20, 0.10, 0.35, 0.0],
            params2: [1.6, 0.70, 2.5, 2.0],
            params3: [0.5, 4.0, 2.0, 0.0],
        }
    }

    pub fn ocean_water() -> Self {
        Self {
            base_color: [0.05, 0.25, 0.40, 1.0],
            secondary_color: [0.02, 0.10, 0.22, 1.0],
            params1: [0.05, 0.0, 0.85, 0.70],
            params2: [1.333, 0.50, 4.0, 1.0],
            params3: [0.2, 4.0, 1.0, 0.0],
        }
    }

    pub fn alien_bioluminescent() -> Self {
        Self {
            base_color: [0.10, 0.05, 0.20, 1.0],
            secondary_color: [0.20, 0.95, 0.85, 1.0],
            params1: [0.30, 0.20, 0.25, 0.0],
            params2: [1.5, 0.40, 2.0, 4.0],
            params3: [1.2, 3.0, 4.0, 1.8],
        }
    }

    pub fn retro_plastic() -> Self {
        Self {
            base_color: [0.90, 0.15, 0.25, 1.0],
            secondary_color: [1.0, 0.4, 0.5, 1.0],
            params1: [0.20, 0.0, 0.35, 0.0], // High specular gloss, synthetic
            params2: [1.45, 0.05, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }
    }

    pub fn sandstone() -> Self {
        Self {
            base_color: [0.85, 0.55, 0.30, 1.0],
            secondary_color: [0.65, 0.35, 0.15, 1.0],
            params1: [0.90, 0.0, 0.02, 0.0],
            params2: [1.5, 0.60, 4.0, 2.0], // Ridged multifractal strata
            params3: [0.4, 4.0, 2.0, 0.0],
        }
    }

    pub fn glacial_ice() -> Self {
        Self {
            base_color: [0.75, 0.90, 0.98, 1.0],
            secondary_color: [0.25, 0.60, 0.85, 1.0],
            params1: [0.10, 0.0, 0.50, 0.60],
            params2: [1.31, 0.30, 3.0, 1.0], // Ice IOR = 1.31
            params3: [0.5, 3.0, 1.5, 0.1],
        }
    }

    pub fn wood_grain() -> Self {
        Self {
            base_color: [0.55, 0.32, 0.15, 1.0],
            secondary_color: [0.30, 0.15, 0.06, 1.0],
            params1: [0.65, 0.0, 0.08, 0.0],
            params2: [1.5, 0.35, 5.0, 3.0],
            params3: [1.8, 3.0, 2.5, 0.0],
        }
    }

    pub fn rusted_iron() -> Self {
        Self {
            base_color: [0.45, 0.20, 0.10, 1.0],
            secondary_color: [0.25, 0.26, 0.28, 1.0],
            params1: [0.75, 0.60, 0.15, 0.0],
            params2: [1.6, 0.55, 6.0, 4.0],
            params3: [0.8, 4.0, 1.8, 0.0],
        }
    }

    pub fn amethyst_crystal() -> Self {
        Self {
            base_color: [0.60, 0.15, 0.85, 1.0],
            secondary_color: [0.30, 0.05, 0.50, 1.0],
            params1: [0.08, 0.10, 0.30, 0.75],
            params2: [1.54, 0.10, 2.0, 2.0],
            params3: [0.2, 2.0, 1.0, 0.3],
        }
    }

    pub fn cyber_grid() -> Self {
        Self {
            base_color: [0.02, 0.03, 0.06, 1.0],
            secondary_color: [0.00, 0.95, 1.00, 1.0],
            params1: [0.10, 0.50, 0.60, 0.0],
            params2: [1.5, 0.20, 4.0, 4.0],
            params3: [0.0, 2.0, 4.0, 2.2], // High emissive grid lines
        }
    }

    pub fn red_desert_rock() -> Self {
        Self {
            base_color: [0.75, 0.28, 0.15, 1.0],
            secondary_color: [0.45, 0.15, 0.08, 1.0],
            params1: [0.85, 0.02, 0.04, 0.0],
            params2: [1.55, 0.65, 3.0, 2.0],
            params3: [0.6, 5.0, 1.8, 0.0],
        }
    }

    pub fn default_preset_list() -> [Self; Self::NUM_PRESETS] {
        [
            Self::rock(),
            Self::blue_marble(),
            Self::chrome(),
            Self::emerald_glass(),
            Self::gold(),
            Self::basalt_obsidian(),
            Self::ocean_water(),
            Self::alien_bioluminescent(),
            Self::retro_plastic(),
            Self::sandstone(),
            Self::glacial_ice(),
            Self::wood_grain(),
            Self::rusted_iron(),
            Self::amethyst_crystal(),
            Self::cyber_grid(),
            Self::red_desert_rock(),
        ]
    }
}
