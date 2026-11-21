use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};

use crate::ground::GroundConfig;
use crate::light::LightConfig;
use crate::material::GpuMaterial;
use crate::objects::SceneObject;
use crate::postprocess::PostProcessConfig;
use crate::sky::SkyConfig;
use crate::terrain::TerrainConfig;
use crate::water::WaterConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub name: String,
    pub version: String,
    pub terrain: TerrainConfig,
    pub sky: SkyConfig,
    pub water: WaterConfig,
    #[serde(default)]
    pub ground: GroundConfig,
    #[serde(default)]
    pub lights: Vec<LightConfig>,
    pub materials: Vec<GpuMaterial>,
    pub objects: Vec<SceneObject>,
    pub postprocess: PostProcessConfig,
    #[serde(default)]
    pub terrain_objects: Vec<crate::terrain::TerrainObjectSave>,
}

pub struct ProjectIO;

impl ProjectIO {
    pub fn save_project(path: &str, project: &ProjectFile) -> Result<(), String> {
        let json = serde_json::to_string_pretty(project)
            .map_err(|e| format!("Serialization error: {}", e))?;
        let mut file = File::create(path).map_err(|e| format!("File creation error: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }

    pub fn load_project(path: &str) -> Result<ProjectFile, String> {
        let mut file = File::open(path).map_err(|e| format!("File open error: {}", e))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| format!("Read error: {}", e))?;
        let project: ProjectFile = serde_json::from_str(&contents)
            .map_err(|e| format!("JSON parse error: {}", e))?;
        Ok(project)
    }

    #[allow(dead_code)]
    pub fn save_image_png(path: &str, width: u32, height: u32, rgba_pixels: &[u8]) -> Result<(), String> {
        image::save_buffer(
            path,
            rgba_pixels,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to save image: {}", e))
    }
}
