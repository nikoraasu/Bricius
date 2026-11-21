//! Bryce-4-style user interface: theme, procedural icons, panel shell and lab hosting.
pub mod icons;
pub mod labs;
pub mod materials_lab;
pub mod shell;
pub mod sky_lab;
pub mod terrain_lab;
pub mod theme;

use crate::camera::Camera;
use crate::material::GpuMaterial;
use crate::model_loader::LoadedMesh;
use crate::objects::SceneObject;
use crate::scene::SceneController;
use crate::terrain::TerrainObjectData;

/// Mutable borrows of everything the UI can edit, bundled to keep signatures short.
pub struct World<'a> {
    pub scene_ctrl: &'a mut SceneController,
    pub camera: &'a mut Camera,
    pub materials: &'a mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
    pub objects: &'a mut Vec<SceneObject>,
    pub meshes: &'a mut Vec<LoadedMesh>,
    pub material_dirty: &'a mut bool,
    pub mesh_dirty: &'a mut bool,
    pub terrains: &'a mut Vec<TerrainObjectData>,
    pub terrain_dirty: &'a mut bool,
}
