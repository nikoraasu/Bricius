use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkyType {
    ClearBlue,
    #[serde(alias = "BryceSunset")]
    RetroSunset,
    DramaticStorm,
    TwilightGlow,
    StarryNight,
    AlienViolet,
    CyberpunkNeon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkyConfig {
    pub sky_type: SkyType,
    pub sun_azimuth: f32,   // in radians
    pub sun_elevation: f32, // in radians
    pub sun_intensity: f32,
    pub sun_color: [f32; 4],
    pub sun_size: f32,
    pub zenith_color: [f32; 4],
    pub horizon_color: [f32; 4],
    pub ground_color: [f32; 4],
    pub fog_density: f32,
    pub fog_height_falloff: f32,
    pub fog_color: [f32; 4],
    pub rayleigh_coeff: f32,
    pub mie_coeff: f32,
    pub cloud_coverage: f32,
    pub cloud_density: f32,
    pub cloud_altitude: f32,
    pub cloud_scale: f32,
    pub cloud_softness: f32,
    pub cloud_wind_speed: f32,
    pub cloud_color: [f32; 4],
    pub stars_intensity: f32,
    pub moon_phase: f32,
}

impl Default for SkyConfig {
    fn default() -> Self {
        Self::retro_sunset()
    }
}

impl SkyConfig {
    pub fn clear_blue() -> Self {
        Self {
            sky_type: SkyType::ClearBlue,
            sun_azimuth: 45.0_f32.to_radians(),
            sun_elevation: 60.0_f32.to_radians(),
            sun_intensity: 2.2,
            sun_color: [1.0, 0.98, 0.90, 1.0],
            sun_size: 1.0,
            zenith_color: [0.12, 0.38, 0.85, 1.0],
            horizon_color: [0.70, 0.82, 0.95, 1.0],
            ground_color: [0.15, 0.20, 0.25, 1.0],
            fog_density: 0.005,
            fog_height_falloff: 0.04,
            fog_color: [0.75, 0.85, 0.95, 1.0],
            rayleigh_coeff: 1.0,
            mie_coeff: 0.5,
            cloud_coverage: 0.45,
            cloud_density: 0.8,
            cloud_altitude: 60.0,
            cloud_scale: 0.015,
            cloud_softness: 0.6,
            cloud_wind_speed: 1.0,
            cloud_color: [0.98, 0.98, 1.0, 1.0],
            stars_intensity: 0.0,
            moon_phase: 0.0,
        }
    }

    pub fn retro_sunset() -> Self {
        Self {
            sky_type: SkyType::RetroSunset,
            sun_azimuth: 45.0_f32.to_radians(),
            sun_elevation: 12.0_f32.to_radians(),
            sun_intensity: 2.4,
            sun_color: [1.0, 0.85, 0.55, 1.0],
            sun_size: 1.6,
            zenith_color: [0.15, 0.25, 0.65, 1.0],
            horizon_color: [0.98, 0.55, 0.25, 1.0], // Classic 90s golden-orange
            ground_color: [0.15, 0.12, 0.18, 1.0],
            fog_density: 0.008,
            fog_height_falloff: 0.05,
            fog_color: [0.85, 0.60, 0.45, 1.0],
            rayleigh_coeff: 1.5,
            mie_coeff: 1.2,
            cloud_coverage: 0.65,
            cloud_density: 1.0,
            cloud_altitude: 55.0,
            cloud_scale: 0.018,
            cloud_softness: 0.7,
            cloud_wind_speed: 0.8,
            cloud_color: [1.0, 0.75, 0.60, 1.0],
            stars_intensity: 0.1,
            moon_phase: 0.2,
        }
    }

    pub fn dramatic_storm() -> Self {
        Self {
            sky_type: SkyType::DramaticStorm,
            sun_azimuth: 110.0_f32.to_radians(),
            sun_elevation: 20.0_f32.to_radians(),
            sun_intensity: 1.2,
            sun_color: [0.9, 0.85, 0.75, 1.0],
            sun_size: 0.9,
            zenith_color: [0.10, 0.12, 0.16, 1.0],
            horizon_color: [0.30, 0.35, 0.42, 1.0],
            ground_color: [0.08, 0.09, 0.12, 1.0],
            fog_density: 0.022,
            fog_height_falloff: 0.08,
            fog_color: [0.25, 0.28, 0.32, 1.0],
            rayleigh_coeff: 0.8,
            mie_coeff: 2.5,
            cloud_coverage: 0.95,
            cloud_density: 1.8,
            cloud_altitude: 40.0,
            cloud_scale: 0.025,
            cloud_softness: 0.4,
            cloud_wind_speed: 2.5,
            cloud_color: [0.20, 0.22, 0.28, 1.0],
            stars_intensity: 0.0,
            moon_phase: 0.0,
        }
    }

    pub fn twilight_glow() -> Self {
        Self {
            sky_type: SkyType::TwilightGlow,
            sun_azimuth: 60.0_f32.to_radians(),
            sun_elevation: -4.0_f32.to_radians(),
            sun_intensity: 1.0,
            sun_color: [0.95, 0.40, 0.20, 1.0],
            sun_size: 1.4,
            zenith_color: [0.08, 0.12, 0.35, 1.0],
            horizon_color: [0.85, 0.35, 0.45, 1.0], // Deep violet/rose
            ground_color: [0.05, 0.06, 0.10, 1.0],
            fog_density: 0.012,
            fog_height_falloff: 0.06,
            fog_color: [0.45, 0.25, 0.40, 1.0],
            rayleigh_coeff: 1.8,
            mie_coeff: 1.0,
            cloud_coverage: 0.50,
            cloud_density: 0.7,
            cloud_altitude: 65.0,
            cloud_scale: 0.015,
            cloud_softness: 0.8,
            cloud_wind_speed: 0.5,
            cloud_color: [0.70, 0.40, 0.55, 1.0],
            stars_intensity: 0.4,
            moon_phase: 0.5,
        }
    }

    pub fn starry_night() -> Self {
        Self {
            sky_type: SkyType::StarryNight,
            sun_azimuth: 180.0_f32.to_radians(),
            sun_elevation: -40.0_f32.to_radians(),
            sun_intensity: 0.35,
            sun_color: [0.65, 0.75, 1.0, 1.0], // Moonlight
            sun_size: 1.2,
            zenith_color: [0.02, 0.03, 0.08, 1.0],
            horizon_color: [0.06, 0.09, 0.18, 1.0],
            ground_color: [0.02, 0.02, 0.04, 1.0],
            fog_density: 0.006,
            fog_height_falloff: 0.03,
            fog_color: [0.05, 0.08, 0.15, 1.0],
            rayleigh_coeff: 0.5,
            mie_coeff: 0.3,
            cloud_coverage: 0.25,
            cloud_density: 0.5,
            cloud_altitude: 70.0,
            cloud_scale: 0.012,
            cloud_softness: 0.9,
            cloud_wind_speed: 0.3,
            cloud_color: [0.15, 0.18, 0.25, 1.0],
            stars_intensity: 1.8,
            moon_phase: 0.9,
        }
    }

    pub fn alien_violet() -> Self {
        Self {
            sky_type: SkyType::AlienViolet,
            sun_azimuth: 30.0_f32.to_radians(),
            sun_elevation: 35.0_f32.to_radians(),
            sun_intensity: 2.2,
            sun_color: [0.40, 0.95, 1.00, 1.0], // Cyan sun
            sun_size: 2.2,
            zenith_color: [0.35, 0.05, 0.55, 1.0], // Deep alien violet
            horizon_color: [0.95, 0.20, 0.65, 1.0], // Magenta horizon
            ground_color: [0.15, 0.02, 0.25, 1.0],
            fog_density: 0.014,
            fog_height_falloff: 0.05,
            fog_color: [0.60, 0.20, 0.50, 1.0],
            rayleigh_coeff: 2.0,
            mie_coeff: 1.5,
            cloud_coverage: 0.70,
            cloud_density: 1.2,
            cloud_altitude: 50.0,
            cloud_scale: 0.020,
            cloud_softness: 0.6,
            cloud_wind_speed: 1.5,
            cloud_color: [0.85, 0.35, 0.75, 1.0],
            stars_intensity: 0.6,
            moon_phase: 0.7,
        }
    }

    pub fn cyberpunk_neon() -> Self {
        Self {
            sky_type: SkyType::CyberpunkNeon,
            sun_azimuth: 90.0_f32.to_radians(),
            sun_elevation: 8.0_f32.to_radians(),
            sun_intensity: 3.0,
            sun_color: [1.0, 0.10, 0.55, 1.0], // Hot pink sun
            sun_size: 2.8,
            zenith_color: [0.05, 0.02, 0.18, 1.0],
            horizon_color: [0.00, 0.85, 1.00, 1.0], // Glowing neon cyan
            ground_color: [0.02, 0.01, 0.08, 1.0],
            fog_density: 0.018,
            fog_height_falloff: 0.07,
            fog_color: [0.20, 0.05, 0.35, 1.0],
            rayleigh_coeff: 2.2,
            mie_coeff: 2.0,
            cloud_coverage: 0.40,
            cloud_density: 0.9,
            cloud_altitude: 60.0,
            cloud_scale: 0.020,
            cloud_softness: 0.5,
            cloud_wind_speed: 1.0,
            cloud_color: [0.65, 0.10, 0.55, 1.0],
            stars_intensity: 1.0,
            moon_phase: 0.0,
        }
    }
}
