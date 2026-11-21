use crate::objects::SceneObject;
use crate::postprocess::PostProcessConfig;
use crate::sky::SkyConfig;
use crate::terrain::{TerrainAlgorithm, TerrainConfig};
use crate::water::WaterConfig;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SceneTheme {
    ClassicBryce1994,
    AlpineFjord,
    AlienPandora,
    SaharaOasis,
    VolcanicCaldera,
    SurrealistChrome,
}

pub struct SceneGenerator;

impl SceneGenerator {
    pub fn generate_world(
        theme: SceneTheme,
        seed: u32,
        terrain: &mut TerrainConfig,
        sky: &mut SkyConfig,
        water: &mut WaterConfig,
        objects: &mut Vec<SceneObject>,
        postprocess: &mut PostProcessConfig,
    ) {
        terrain.seed = seed;

        match theme {
            SceneTheme::ClassicBryce1994 => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::RidgedMultifractal,
                    height_scale: 1.25,
                    frequency: 0.035,
                    octaves: 5,
                    mountain_sharpness: 2.2,
                    ridge_intensity: 1.8,
                    valley_depth: 1.4,
                    erosion_hydraulic: 0.45,
                    erosion_thermal: 0.30,
                    domain_warp: 0.6,
                    lagoon_radius: 18.0,
                    snow_line: 18.0,
                };
                *sky = SkyConfig::bryce_sunset();
                *water = WaterConfig::ocean();
                *postprocess = PostProcessConfig::retro_bryce_preset();

                objects.clear();
                // Iconic central floating chrome sphere
                objects.push(SceneObject::sphere("Chrome Orb", [0.0, 3.2, -6.0], 1.8, 2));
                // Stone pedestal
                objects.push(SceneObject::cube("Sandstone Pedestal", [-5.0, 1.5, -8.0], [1.2, 2.5, 1.2], 9));
                // Floating glass crystal
                objects.push(SceneObject::sphere("Emerald Relic", [5.5, 3.0, -8.0], 1.4, 3));
            }
            SceneTheme::AlpineFjord => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::HybridMultifractal,
                    height_scale: 1.60,
                    frequency: 0.025,
                    octaves: 6,
                    mountain_sharpness: 1.6,
                    ridge_intensity: 1.4,
                    valley_depth: 1.8,
                    erosion_hydraulic: 0.50,
                    erosion_thermal: 0.40,
                    domain_warp: 0.2,
                    lagoon_radius: 24.0,
                    snow_line: 10.0,
                };
                *sky = SkyConfig::clear_blue();
                *water = WaterConfig::lake();
                *postprocess = PostProcessConfig::photorealistic_preset();

                objects.clear();
                objects.push(SceneObject::sphere("Glacial Core", [0.0, 2.8, -7.0], 1.5, 10));
                objects.push(SceneObject::cube("Granite Monolith", [6.0, 2.0, -9.0], [1.0, 3.0, 1.0], 0));
            }
            SceneTheme::AlienPandora => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::DomainWarped,
                    height_scale: 1.40,
                    frequency: 0.040,
                    octaves: 5,
                    mountain_sharpness: 2.5,
                    ridge_intensity: 2.0,
                    valley_depth: 1.5,
                    erosion_hydraulic: 0.20,
                    erosion_thermal: 0.20,
                    domain_warp: 1.4,
                    lagoon_radius: 16.0,
                    snow_line: 25.0,
                };
                *sky = SkyConfig::alien_violet();
                *water = WaterConfig::alien_liquid();
                *postprocess = PostProcessConfig {
                    retro_90s_cgi: false,
                    bloom_intensity: 0.85,
                    vignette_strength: 0.30,
                    contrast: 1.15,
                    saturation: 1.40,
                    ..Default::default()
                };

                objects.clear();
                // Bioluminescent alien spires & crystal
                objects.push(SceneObject::sphere("Alien Biosphere", [0.0, 3.5, -6.5], 1.9, 7));
                objects.push(SceneObject::torus("Floating Void Ring", [0.0, 6.0, -6.5], 3.0, 0.4, 14));
                objects.push(SceneObject::cylinder("Amethyst Spire", [-5.5, 3.0, -8.0], 0.8, 4.0, 13));
                objects.push(SceneObject::sphere("Plasma Gem", [5.5, 2.5, -7.5], 1.2, 3));
            }
            SceneTheme::SaharaOasis => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::Billow,
                    height_scale: 0.85,
                    frequency: 0.020,
                    octaves: 4,
                    mountain_sharpness: 0.8,
                    ridge_intensity: 0.6,
                    valley_depth: 0.8,
                    erosion_hydraulic: 0.05,
                    erosion_thermal: 0.70,
                    domain_warp: 0.8,
                    lagoon_radius: 14.0,
                    snow_line: 50.0,
                };
                let mut s = SkyConfig::clear_blue();
                s.sun_elevation = 40.0_f32.to_radians();
                s.horizon_color = [0.95, 0.75, 0.45, 1.0];
                s.fog_density = 0.012;
                s.fog_color = [0.90, 0.75, 0.55, 1.0]; // Desert haze
                *sky = s;
                let mut w = WaterConfig::lake();
                w.shallow_color = [0.10, 0.85, 0.80, 1.0]; // Bright oasis turquoise
                *water = w;
                *postprocess = PostProcessConfig::default();

                objects.clear();
                objects.push(SceneObject::cube("Ancient Temple Altar", [0.0, 1.5, -7.0], [2.0, 1.5, 2.0], 9));
                objects.push(SceneObject::sphere("Golden Sun Orb", [0.0, 4.2, -7.0], 1.2, 4));
            }
            SceneTheme::VolcanicCaldera => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::WorleyCellular,
                    height_scale: 1.50,
                    frequency: 0.035,
                    octaves: 5,
                    mountain_sharpness: 2.2,
                    ridge_intensity: 1.6,
                    valley_depth: 2.0,
                    erosion_hydraulic: 0.25,
                    erosion_thermal: 0.35,
                    domain_warp: 0.5,
                    lagoon_radius: 18.0,
                    snow_line: 30.0,
                };
                *sky = SkyConfig::dramatic_storm();
                let mut w = WaterConfig::ocean();
                w.deep_color = [0.02, 0.03, 0.05, 1.0]; // Dark volcanic lake
                *water = w;
                *postprocess = PostProcessConfig::default();

                objects.clear();
                objects.push(SceneObject::cube("Obsidian Monolith", [0.0, 2.5, -6.5], [1.2, 3.5, 1.2], 5));
                objects.push(SceneObject::sphere("Magma Core", [4.5, 2.0, -8.0], 1.4, 8));
            }
            SceneTheme::SurrealistChrome => {
                *terrain = TerrainConfig {
                    seed,
                    algorithm: TerrainAlgorithm::RidgedMultifractal,
                    height_scale: 0.50, // Flat reflective ground
                    frequency: 0.015,
                    octaves: 3,
                    mountain_sharpness: 1.0,
                    ridge_intensity: 0.5,
                    valley_depth: 0.5,
                    erosion_hydraulic: 0.0,
                    erosion_thermal: 0.0,
                    domain_warp: 0.0,
                    lagoon_radius: 50.0,
                    snow_line: 50.0,
                };
                *sky = SkyConfig::cyberpunk_neon();
                *water = WaterConfig::calm_mirror();
                *postprocess = PostProcessConfig::retro_bryce_preset();

                objects.clear();
                // Floating surreal geometric sculptures
                objects.push(SceneObject::sphere("Center Chrome Sphere", [0.0, 2.5, -6.0], 1.8, 2));
                objects.push(SceneObject::torus("Floating Gold Torus", [0.0, 5.2, -6.0], 2.8, 0.35, 4));
                objects.push(SceneObject::cube("Marble Pedestal Left", [-5.0, 1.5, -7.5], [1.2, 2.0, 1.2], 1));
                objects.push(SceneObject::sphere("Emerald Orb Right", [5.0, 2.2, -7.5], 1.4, 3));
            }
        }
    }

    pub fn reroll_sky(seed: u32, sky: &mut SkyConfig) {
        let types = [
            SkyConfig::clear_blue(),
            SkyConfig::bryce_sunset(),
            SkyConfig::dramatic_storm(),
            SkyConfig::twilight_glow(),
            SkyConfig::starry_night(),
            SkyConfig::alien_violet(),
            SkyConfig::cyberpunk_neon(),
        ];
        let idx = (seed as usize) % types.len();
        *sky = types[idx].clone();
    }

    pub fn reroll_terrain(seed: u32, terrain: &mut TerrainConfig) {
        terrain.seed = seed;
        let algos = [
            TerrainAlgorithm::Fbm,
            TerrainAlgorithm::RidgedMultifractal,
            TerrainAlgorithm::Billow,
            TerrainAlgorithm::WorleyCellular,
            TerrainAlgorithm::DomainWarped,
            TerrainAlgorithm::HybridMultifractal,
        ];
        terrain.algorithm = algos[(seed as usize) % algos.len()];
        terrain.height_scale = 0.8 + ((seed % 100) as f32 / 100.0) * 1.2;
        terrain.frequency = 0.02 + ((seed % 50) as f32 / 50.0) * 0.03;
    }
}
