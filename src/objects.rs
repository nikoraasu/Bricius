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
    Mesh,
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
    pub mesh_id: Option<usize>,
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
            mesh_id: None,
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
            mesh_id: None,
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
            mesh_id: None,
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
            mesh_id: None,
        }
    }

    pub fn mesh(name: &str, pos: [f32; 3], scale: [f32; 3], material_id: u32, mesh_id: usize) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Mesh,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale,
            material_id,
            visible: true,
            mesh_id: Some(mesh_id),
        }
    }

    /// Intersect an arbitrary world ray against this object for viewport clicking & selection
    pub fn intersect_ray(&self, ro: glam::Vec3, rd: glam::Vec3) -> Option<f32> {
        if !self.visible {
            return None;
        }

        let center = glam::Vec3::from(self.position);
        let scale = glam::Vec3::from(self.scale);

        // Transform ray into local object coordinate space with rotation
        let rad_x = self.rotation[0].to_radians();
        let rad_y = self.rotation[1].to_radians();
        let rad_z = self.rotation[2].to_radians();

        let rot_mat = glam::Mat3::from_rotation_z(rad_z)
            * glam::Mat3::from_rotation_y(rad_y)
            * glam::Mat3::from_rotation_x(rad_x);
        let inv_rot = rot_mat.transpose();

        let ro_local = inv_rot * (ro - center);
        let rd_local = (inv_rot * rd).normalize();

        match self.primitive {
            PrimitiveType::Sphere => {
                let rad = scale.x;
                let b = ro_local.dot(rd_local);
                let c = ro_local.dot(ro_local) - rad * rad;
                let disc = b * b - c;
                if disc > 0.0 {
                    let t = -b - disc.sqrt();
                    if t > 0.05 {
                        return Some(t);
                    }
                }
                None
            }
            PrimitiveType::Cube => {
                let min = -scale;
                let max = scale;
                let inv_d = glam::Vec3::new(
                    1.0 / rd_local.x.abs().max(1e-6) * rd_local.x.signum(),
                    1.0 / rd_local.y.abs().max(1e-6) * rd_local.y.signum(),
                    1.0 / rd_local.z.abs().max(1e-6) * rd_local.z.signum(),
                );
                let t0s = (min - ro_local) * inv_d;
                let t1s = (max - ro_local) * inv_d;
                let t_min = t0s.min(t1s).max_element();
                let t_max = t0s.max(t1s).min_element();
                if t_max >= t_min.max(0.05) {
                    Some(t_min)
                } else {
                    None
                }
            }
            PrimitiveType::Cylinder => {
                let rad = scale.x;
                let half_h = scale.y;
                let a = rd_local.x * rd_local.x + rd_local.z * rd_local.z;
                let b = ro_local.x * rd_local.x + ro_local.z * rd_local.z;
                let c = ro_local.x * ro_local.x + ro_local.z * ro_local.z - rad * rad;
                let disc = b * b - a * c;
                if disc > 0.0 && a > 0.0001 {
                    let t = (-b - disc.sqrt()) / a;
                    let y = ro_local.y + rd_local.y * t;
                    if t > 0.05 && y.abs() <= half_h {
                        return Some(t);
                    }
                }
                None
            }
            PrimitiveType::Torus => {
                let bound_r = scale.x + scale.y;
                let b = ro_local.dot(rd_local);
                let c = ro_local.dot(ro_local) - bound_r * bound_r;
                let disc = b * b - c;
                if disc > 0.0 {
                    let t = -b - disc.sqrt();
                    if t > 0.05 {
                        return Some(t);
                    }
                }
                None
            }
            PrimitiveType::Mesh | PrimitiveType::Cone | PrimitiveType::Pyramid | PrimitiveType::Plane => {
                // Bounding sphere/box hit test for picking
                let max_dim = scale.max_element().max(0.5);
                let b = ro_local.dot(rd_local);
                let c = ro_local.dot(ro_local) - max_dim * max_dim * 1.5;
                let disc = b * b - c;
                if disc > 0.0 {
                    let t = -b - disc.sqrt();
                    if t > 0.05 {
                        return Some(t);
                    }
                }
                None
            }
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
