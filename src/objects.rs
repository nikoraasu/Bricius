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
    Terrain,
    Stone,
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
    #[serde(default)]
    pub terrain_id: Option<usize>,
    /// Stone only: noise seed that defines the lump pattern
    #[serde(default)]
    pub stone_seed: u32,
    /// Stone only: lumpiness multiplier (0 = smooth ellipsoid, 1 = default, 2 = very craggy)
    #[serde(default = "default_lumpiness")]
    pub stone_lumpiness: f32,
}

fn default_lumpiness() -> f32 {
    1.0
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
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
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
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
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
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
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
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
        }
    }

    /// Cone with its base at -half_height and its apex at +half_height (local Y)
    pub fn cone(name: &str, pos: [f32; 3], radius: f32, half_height: f32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Cone,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: [radius, half_height, radius],
            material_id,
            visible: true,
            mesh_id: None,
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
        }
    }

    /// Square pyramid: base half-extents (x, z), base at -half_height, apex at +half_height
    pub fn pyramid(name: &str, pos: [f32; 3], half_extents: [f32; 2], half_height: f32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Pyramid,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale: [half_extents[0], half_height, half_extents[1]],
            material_id,
            visible: true,
            mesh_id: None,
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
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
            terrain_id: None,
            stone_seed: 0,
            stone_lumpiness: 1.0,
        }
    }

    /// Terrain object: footprint x,z in [-1,1] * scale.x/scale.z, height [0,1] * scale.y
    pub fn terrain(name: &str, pos: [f32; 3], scale: [f32; 3], material_id: u32, terrain_id: usize) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Terrain,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale,
            material_id,
            visible: true,
            mesh_id: None,
            terrain_id: Some(terrain_id),
            stone_seed: 0,
            stone_lumpiness: 1.0,
        }
    }

    /// Bryce-style rock: a noise-displaced ellipsoid (scale = semi-axes), default material 0 (Mountain Rock)
    pub fn stone(name: &str, pos: [f32; 3], scale: [f32; 3], seed: u32, material_id: u32) -> Self {
        Self {
            name: name.to_string(),
            primitive: PrimitiveType::Stone,
            csg_op: CsgOp::Positive,
            position: pos,
            rotation: [0.0, 0.0, 0.0],
            scale,
            material_id,
            visible: true,
            mesh_id: None,
            terrain_id: None,
            stone_seed: seed,
            stone_lumpiness: 1.0,
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

        // Terrain objects: oriented bounding box (yaw only) in unit-footprint local space
        if self.primitive == PrimitiveType::Terrain {
            let yaw = self.rotation[1].to_radians();
            let inv = glam::Mat3::from_rotation_y(yaw).transpose();
            let o = inv * (ro - center);
            let d = inv * rd;
            let half = glam::Vec3::new(scale.x, scale.y * 0.5, scale.z);
            let o = o - glam::Vec3::new(0.0, half.y, 0.0);
            let safe = glam::Vec3::new(
                if d.x.abs() < 1e-6 { 1e-6 } else { d.x },
                if d.y.abs() < 1e-6 { 1e-6 } else { d.y },
                if d.z.abs() < 1e-6 { 1e-6 } else { d.z },
            );
            let inv_d = glam::Vec3::ONE / safe;
            let t0s = (-half - o) * inv_d;
            let t1s = (half - o) * inv_d;
            let t_min = t0s.min(t1s).max_element();
            let t_max = t0s.max(t1s).min_element();
            if t_max >= t_min.max(0.05) {
                return Some(t_min.max(0.05));
            }
            return None;
        }

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
            PrimitiveType::Cube | PrimitiveType::Cone | PrimitiveType::Pyramid => {
                // Cone / pyramid picking uses their bounding box
                let ext = if self.primitive == PrimitiveType::Cone {
                    glam::Vec3::new(scale.x, scale.y, scale.x)
                } else {
                    scale
                };
                let min = -ext;
                let max = ext;
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
            PrimitiveType::Stone => {
                // Bounding ellipsoid (slightly padded for the lumps): unit-sphere test in scaled space
                let inv = glam::Vec3::ONE / (scale * 1.2).max(glam::Vec3::splat(1e-3));
                let o = ro_local * inv;
                let d = rd_local * inv;
                let a = d.dot(d);
                let b = o.dot(d);
                let c = o.dot(o) - 1.0;
                let disc = b * b - a * c;
                if disc > 0.0 && a > 1e-9 {
                    let t = (-b - disc.sqrt()) / a;
                    if t > 0.05 {
                        return Some(t);
                    }
                }
                None
            }
            PrimitiveType::Terrain | PrimitiveType::Mesh | PrimitiveType::Plane => {
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
