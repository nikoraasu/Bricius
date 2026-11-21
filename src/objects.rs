use serde::{Deserialize, Serialize};
use crate::terrain::TerrainConfig;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Sphere,
    Cube,
    Cylinder,
    Cone,
    Pyramid,
    Torus,
    Plane,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsgOp {
    Positive = 0,    // Solid union
    Negative = 1,    // Subtractive cutout
    Intersecting = 2,// Intersection only
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneObject {
    pub name: String,
    pub primitive: PrimitiveType,
    pub csg_op: CsgOp,
    pub position: [f32; 3],
    pub rotation: [f32; 3], // Pitch, Yaw, Roll in degrees
    pub scale: [f32; 3],
    pub material_id: u32,
    pub visible: bool,
}

impl SceneObject {
    pub fn sphere(name: &str, pos: [f32; 3], radius: f32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Sphere,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: [radius, radius, radius],
            material_id,
            visible: true,
        }
    }

    pub fn cube(name: &str, pos: [f32; 3], half_extents: [f32; 3], material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Cube,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: half_extents,
            material_id,
            visible: true,
        }
    }

    pub fn torus(name: &str, pos: [f32; 3], major_rad: f32, tube_rad: f32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Torus,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: [major_rad, tube_rad, major_rad],
            material_id,
            visible: true,
        }
    }

    pub fn cylinder(name: &str, pos: [f32; 3], radius: f32, height: f32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Cylinder,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: [radius, height, radius],
            material_id,
            visible: true,
        }
    }
}

/// Vegetation & Rock Scatter System
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScatterSystem {
    pub enabled: bool,
    pub rock_density: f32,
    pub tree_density: f32,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub max_slope_deg: f32,
    pub random_seed: u32,
}

impl Default for ScatterSystem {
    fn default() -> Self {
        Self {
            enabled: true,
            rock_density: 0.60,
            tree_density: 0.45,
            min_elevation: 0.5,
            max_elevation: 12.0,
            max_slope_deg: 40.0,
            random_seed: 42,
        }
    }
}

impl ScatterSystem {
    pub fn scatter_objects(&self, terrain_config: &TerrainConfig) -> Vec<SceneObject> {
        let mut objects = Vec::new();
        if !self.enabled {
            return objects;
        }

        let num_samples = 32;
        let mut seed = self.random_seed;

        let mut pseudo_rand = || -> f32 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed as f32) / (u32::MAX as f32)
        };

        for i in 0..num_samples {
            let rx = (pseudo_rand() - 0.5) * 70.0;
            let rz = (pseudo_rand() - 0.5) * 70.0;

            let h = crate::terrain::evaluate_procedural_height(rx, rz, terrain_config);
            let h_eps = 0.5;
            let h_x = crate::terrain::evaluate_procedural_height(rx + h_eps, rz, terrain_config);
            let h_z = crate::terrain::evaluate_procedural_height(rx, rz + h_eps, terrain_config);
            let slope = ((h_x - h).powi(2) + (h_z - h).powi(2)).sqrt() / h_eps;
            let slope_deg = slope.atan().to_degrees();

            if h >= self.min_elevation && h <= self.max_elevation && slope_deg <= self.max_slope_deg {
                let r_val = pseudo_rand();
                if r_val < self.rock_density * 0.5 {
                    let scale = 0.6 + pseudo_rand() * 1.4;
                    let rock_mat = if pseudo_rand() > 0.5 { 0 } else { 5 }; // Rock or Basalt
                    let mut rock = SceneObject::sphere(
                        &format!("Rock Boulder #{}", i + 1),
                        [rx, h + scale * 0.4, rz],
                        scale,
                        rock_mat,
                    );
                    rock.rotation = [
                        pseudo_rand() * 360.0,
                        pseudo_rand() * 360.0,
                        pseudo_rand() * 360.0,
                    ];
                    objects.push(rock);
                } else if r_val < (self.rock_density + self.tree_density) * 0.5 {
                    let h_tree = 2.0 + pseudo_rand() * 3.5;
                    let rad = 0.5 + pseudo_rand() * 0.5;
                    let tree = SceneObject::cylinder(
                        &format!("Pine Spire #{}", i + 1),
                        [rx, h + h_tree * 0.5, rz],
                        rad,
                        h_tree,
                        11, // Wood
                    );
                    objects.push(tree);
                }
            }
        }

        objects
    }
}
