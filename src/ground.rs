use serde::{Deserialize, Serialize};

fn default_ground_height() -> f32 { -0.5 }

/// Infinite horizontal ground plane (below the water it acts as the seabed).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroundConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ground_height")]
    pub height: f32,
    #[serde(default)]
    pub material_id: u32,
}

impl Default for GroundConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            height: default_ground_height(),
            material_id: 0,
        }
    }
}
