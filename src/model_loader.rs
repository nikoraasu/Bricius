use serde::{Deserialize, Serialize};

/// Representation of an imported or generated 3D mesh model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoadedMesh {
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>, // Triangle index triplets: 0, 1, 2, ...
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

impl LoadedMesh {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            aabb_min: [-1.0; 3],
            aabb_max: [1.0; 3],
        }
    }

    /// Compute axis-aligned bounding box (AABB) and center the mesh around [0, 0, 0]
    pub fn compute_bounds_and_normalize(&mut self, target_extent: f32) {
        if self.vertices.is_empty() {
            return;
        }

        let mut min = [f32::MAX, f32::MAX, f32::MAX];
        let mut max = [f32::MIN, f32::MIN, f32::MIN];

        for v in &self.vertices {
            min[0] = min[0].min(v[0]);
            min[1] = min[1].min(v[1]);
            min[2] = min[2].min(v[2]);

            max[0] = max[0].max(v[0]);
            max[1] = max[1].max(v[1]);
            max[2] = max[2].max(v[2]);
        }

        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];

        let extents = [
            (max[0] - min[0]).max(0.001),
            (max[1] - min[1]).max(0.001),
            (max[2] - min[2]).max(0.001),
        ];

        let max_dim = extents[0].max(extents[1]).max(extents[2]);
        let scale_factor = (target_extent * 2.0) / max_dim;

        for v in &mut self.vertices {
            v[0] = (v[0] - center[0]) * scale_factor;
            v[1] = (v[1] - center[1]) * scale_factor;
            v[2] = (v[2] - center[2]) * scale_factor;
        }

        self.aabb_min = [-target_extent, -target_extent, -target_extent];
        self.aabb_max = [target_extent, target_extent, target_extent];
    }

    /// Automatically simplify / decimate triangle count if it exceeds safe GPU raytracer budget
    pub fn decimate_if_needed(&mut self, max_triangles: usize) {
        let tri_count = self.indices.len() / 3;
        if tri_count <= max_triangles {
            return;
        }

        let stride = ((tri_count as f32) / (max_triangles as f32)).ceil() as usize;
        let mut new_indices = Vec::with_capacity(max_triangles * 3);
        for t in (0..tri_count).step_by(stride) {
            new_indices.push(self.indices[t * 3]);
            new_indices.push(self.indices[t * 3 + 1]);
            new_indices.push(self.indices[t * 3 + 2]);
        }
        self.indices = new_indices;
    }

    /// Generate missing flat normals from triangle winding
    pub fn generate_normals_if_missing(&mut self) {
        if !self.normals.is_empty() && self.normals.len() == self.vertices.len() {
            return;
        }

        self.normals = vec![[0.0, 1.0, 0.0]; self.vertices.len()];
        let num_triangles = self.indices.len() / 3;

        for t in 0..num_triangles {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;

            if i0 < self.vertices.len() && i1 < self.vertices.len() && i2 < self.vertices.len() {
                let v0 = glam::Vec3::from(self.vertices[i0]);
                let v1 = glam::Vec3::from(self.vertices[i1]);
                let v2 = glam::Vec3::from(self.vertices[i2]);

                let n = (v1 - v0).cross(v2 - v0).normalize_or_zero();
                self.normals[i0] = n.into();
                self.normals[i1] = n.into();
                self.normals[i2] = n.into();
            }
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

pub struct ModelLoader;

impl ModelLoader {
    /// Universal model loader: automatically detects format by extension (.obj, .glb, .gltf, .fbx)
    pub fn load_from_file(path: &str) -> Result<LoadedMesh, String> {
        let path_lower = path.to_lowercase();
        let file_stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported_Model");

        if path_lower.ends_with(".obj") {
            Self::load_obj_file(path, file_stem)
        } else if path_lower.ends_with(".glb") || path_lower.ends_with(".gltf") {
            Self::load_gltf_file(path, file_stem)
        } else if path_lower.ends_with(".fbx") {
            Self::load_fbx_file(path, file_stem)
        } else {
            Err(format!(
                "Unsupported 3D model format in '{}'. Supported: .obj, .glb, .gltf, .fbx",
                path
            ))
        }
    }

    /// Load Wavefront .obj file using robust `tobj` parser
    pub fn load_obj_file(path: &str, name: &str) -> Result<LoadedMesh, String> {
        let load_options = tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        };

        let (models, _materials) = tobj::load_obj(path, &load_options)
            .map_err(|e| format!("Failed to parse OBJ file '{}': {}", path, e))?;

        if models.is_empty() {
            return Err("OBJ file contains no mesh models".to_string());
        }

        let mut combined_mesh = LoadedMesh::new(name);

        for model in &models {
            let base_vertex = combined_mesh.vertices.len() as u32;
            let m = &model.mesh;

            // Vertices
            for chunk in m.positions.chunks_exact(3) {
                combined_mesh.vertices.push([chunk[0], chunk[1], chunk[2]]);
            }

            // Normals
            if !m.normals.is_empty() {
                for chunk in m.normals.chunks_exact(3) {
                    combined_mesh.normals.push([chunk[0], chunk[1], chunk[2]]);
                }
            }

            // Indices
            for idx in &m.indices {
                combined_mesh.indices.push(base_vertex + idx);
            }
        }

        if combined_mesh.vertices.is_empty() || combined_mesh.indices.is_empty() {
            return Err("OBJ file has no valid geometry".to_string());
        }

        combined_mesh.generate_normals_if_missing();
        combined_mesh.compute_bounds_and_normalize(1.0);
        // Protect GPU real-time performance by capping at 3500 triangles
        combined_mesh.decimate_if_needed(3500);

        Ok(combined_mesh)
    }

    /// Fallback / In-Memory Wavefront .obj parser
    #[allow(dead_code)]
    pub fn load_obj(content: &str, name: &str) -> Result<LoadedMesh, String> {
        let mut cursor = std::io::Cursor::new(content.as_bytes());
        let load_options = tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        };

        let (models, _) = tobj::load_obj_buf(&mut cursor, &load_options, |_| {
            Err(tobj::LoadError::OpenFileFailed)
        })
        .map_err(|e| format!("Failed to parse OBJ buffer: {}", e))?;

        if models.is_empty() {
            return Err("OBJ contains no mesh geometry".to_string());
        }

        let mut combined_mesh = LoadedMesh::new(name);
        for model in &models {
            let base_vertex = combined_mesh.vertices.len() as u32;
            let m = &model.mesh;
            for chunk in m.positions.chunks_exact(3) {
                combined_mesh.vertices.push([chunk[0], chunk[1], chunk[2]]);
            }
            if !m.normals.is_empty() {
                for chunk in m.normals.chunks_exact(3) {
                    combined_mesh.normals.push([chunk[0], chunk[1], chunk[2]]);
                }
            }
            for idx in &m.indices {
                combined_mesh.indices.push(base_vertex + idx);
            }
        }

        combined_mesh.generate_normals_if_missing();
        combined_mesh.compute_bounds_and_normalize(1.0);
        combined_mesh.decimate_if_needed(3500);

        Ok(combined_mesh)
    }

    /// Load glTF / GLB 2.0 file with complete node transforms, indices, and normals
    pub fn load_gltf_file(path: &str, name: &str) -> Result<LoadedMesh, String> {
        let (document, buffers, _images) = gltf::import(path)
            .map_err(|e| format!("Failed to import glTF/GLB: {}", e))?;

        let mut combined_mesh = LoadedMesh::new(name);

        for node in document.nodes() {
            let transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
            let normal_mat = glam::Mat3::from_mat4(transform).inverse().transpose();

            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|b| Some(&buffers[b.index()]));

                    if let Some(pos_iter) = reader.read_positions() {
                        let base_vertex = combined_mesh.vertices.len() as u32;

                        for pos in pos_iter {
                            let p_transformed = transform.transform_point3(glam::Vec3::from(pos));
                            combined_mesh.vertices.push(p_transformed.into());
                        }

                        if let Some(norm_iter) = reader.read_normals() {
                            for norm in norm_iter {
                                let n_transformed = (normal_mat * glam::Vec3::from(norm)).normalize_or_zero();
                                combined_mesh.normals.push(n_transformed.into());
                            }
                        }

                        if let Some(indices_iter) = reader.read_indices() {
                            for idx in indices_iter.into_u32() {
                                combined_mesh.indices.push(base_vertex + idx);
                            }
                        } else {
                            // Unindexed primitive: sequential triplets
                            let count = (combined_mesh.vertices.len() as u32) - base_vertex;
                            for i in (0..count).step_by(3) {
                                if i + 2 < count {
                                    combined_mesh.indices.push(base_vertex + i);
                                    combined_mesh.indices.push(base_vertex + i + 1);
                                    combined_mesh.indices.push(base_vertex + i + 2);
                                }
                            }
                        }
                    }
                }
            }
        }

        if combined_mesh.vertices.is_empty() || combined_mesh.indices.is_empty() {
            return Err("glTF/GLB file contained no valid meshes".to_string());
        }

        combined_mesh.generate_normals_if_missing();
        combined_mesh.compute_bounds_and_normalize(1.0);
        combined_mesh.decimate_if_needed(3500);

        Ok(combined_mesh)
    }

    /// Load GLB from in-memory byte slice
    #[allow(dead_code)]
    pub fn load_glb(bytes: &[u8], name: &str) -> Result<LoadedMesh, String> {
        let (document, buffers, _) = gltf::import_slice(bytes)
            .map_err(|e| format!("Failed to parse GLB bytes: {}", e))?;

        let mut combined_mesh = LoadedMesh::new(name);
        for node in document.nodes() {
            let transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
            let normal_mat = glam::Mat3::from_mat4(transform).inverse().transpose();

            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|b| Some(&buffers[b.index()]));
                    if let Some(pos_iter) = reader.read_positions() {
                        let base_vertex = combined_mesh.vertices.len() as u32;
                        for pos in pos_iter {
                            let p_transformed = transform.transform_point3(glam::Vec3::from(pos));
                            combined_mesh.vertices.push(p_transformed.into());
                        }
                        if let Some(norm_iter) = reader.read_normals() {
                            for norm in norm_iter {
                                let n_transformed = (normal_mat * glam::Vec3::from(norm)).normalize_or_zero();
                                combined_mesh.normals.push(n_transformed.into());
                            }
                        }
                        if let Some(indices_iter) = reader.read_indices() {
                            for idx in indices_iter.into_u32() {
                                combined_mesh.indices.push(base_vertex + idx);
                            }
                        }
                    }
                }
            }
        }

        if combined_mesh.vertices.is_empty() {
            return Err("GLB bytes contained no valid vertex geometry".to_string());
        }

        combined_mesh.generate_normals_if_missing();
        combined_mesh.compute_bounds_and_normalize(1.0);
        combined_mesh.decimate_if_needed(3500);
        Ok(combined_mesh)
    }

    /// Load FBX file (supports ASCII FBX, provides diagnostic help for Binary FBX)
    pub fn load_fbx_file(path: &str, name: &str) -> Result<LoadedMesh, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read FBX file: {}", e))?;

        if bytes.starts_with(b"Kaydara FBX Binary") {
            return Err(
                "Binary FBX format detected. Most 3D DCC tools (Blender, Maya) can export directly as glTF/GLB or Wavefront OBJ for seamless real-time GPU raytracing."
                    .to_string(),
            );
        }

        let content = std::str::from_utf8(&bytes)
            .map_err(|_| "FBX file is not valid ASCII".to_string())?;
        Self::load_fbx_ascii(content, name)
    }

    /// ASCII FBX parser (extracts Vertices and PolygonVertexIndex)
    pub fn load_fbx_ascii(content: &str, name: &str) -> Result<LoadedMesh, String> {
        let mut mesh = LoadedMesh::new(name);
        let mut in_vertices = false;
        let mut in_indices = false;
        let mut raw_coords = Vec::new();
        let mut raw_indices = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Vertices:") {
                in_vertices = true;
                let data = trimmed.trim_start_matches("Vertices:").trim();
                for num in data.split(',') {
                    if let Ok(val) = num.trim().parse::<f32>() {
                        raw_coords.push(val);
                    }
                }
                continue;
            }
            if in_vertices {
                if trimmed.contains('}') {
                    in_vertices = false;
                } else {
                    for num in trimmed.split(',') {
                        if let Ok(val) = num.trim().parse::<f32>() {
                            raw_coords.push(val);
                        }
                    }
                }
            }

            if trimmed.starts_with("PolygonVertexIndex:") {
                in_indices = true;
                let data = trimmed.trim_start_matches("PolygonVertexIndex:").trim();
                for num in data.split(',') {
                    if let Ok(val) = num.trim().parse::<i32>() {
                        raw_indices.push(val);
                    }
                }
                continue;
            }
            if in_indices {
                if trimmed.contains('}') {
                    in_indices = false;
                } else {
                    for num in trimmed.split(',') {
                        if let Ok(val) = num.trim().parse::<i32>() {
                            raw_indices.push(val);
                        }
                    }
                }
            }
        }

        for chunk in raw_coords.chunks_exact(3) {
            mesh.vertices.push([chunk[0], chunk[1], chunk[2]]);
        }

        // Triangulate FBX polygon winding
        let mut poly = Vec::new();
        for &idx in &raw_indices {
            if idx >= 0 {
                poly.push(idx as u32);
            } else {
                // Negative index marks end of polygon: actual_index = (-idx - 1)
                poly.push((-idx - 1) as u32);
                for i in 1..poly.len() - 1 {
                    mesh.indices.push(poly[0]);
                    mesh.indices.push(poly[i]);
                    mesh.indices.push(poly[i + 1]);
                }
                poly.clear();
            }
        }

        if mesh.vertices.is_empty() {
            return Err("No vertex data found in ASCII FBX file".to_string());
        }

        combined_normalize_and_prep(&mut mesh, 1.0);
        Ok(mesh)
    }

    // =========================================================================
    // BUILT-IN PROCEDURAL PRESET MODELS
    // =========================================================================

    /// Classic Utah / Bryce CGI Teapot (iconic 1990s 3D creation studio motif)
    pub fn bryce_teapot() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Bryce Utah Teapot");
        let rings = 10;
        let sectors = 16;
        for r in 0..=rings {
            let v = r as f32 / rings as f32;
            let phi = (v - 0.5) * std::f32::consts::PI * 0.8;
            let rad = (phi.cos() * 0.95 + 0.15) * (1.0 - (v - 0.5).powi(2) * 0.4);
            let y = (v - 0.5) * 1.4;

            for s in 0..sectors {
                let u = s as f32 / sectors as f32;
                let theta = u * std::f32::consts::TAU;
                let x = rad * theta.cos();
                let z = rad * theta.sin();
                mesh.vertices.push([x, y, z]);
            }
        }

        for r in 0..rings {
            for s in 0..sectors {
                let next_s = (s + 1) % sectors;
                let i0 = (r * sectors + s) as u32;
                let i1 = (r * sectors + next_s) as u32;
                let i2 = ((r + 1) * sectors + s) as u32;
                let i3 = ((r + 1) * sectors + next_s) as u32;

                mesh.indices.extend_from_slice(&[i0, i1, i2]);
                mesh.indices.extend_from_slice(&[i1, i3, i2]);
            }
        }

        // Add Spout
        let spout_base = mesh.vertices.len() as u32;
        mesh.vertices.push([1.1, 0.1, 0.0]);
        mesh.vertices.push([1.6, 0.6, 0.0]);
        mesh.vertices.push([1.5, 0.8, 0.15]);
        mesh.vertices.push([1.5, 0.8, -0.15]);
        mesh.indices.extend_from_slice(&[spout_base, spout_base + 1, spout_base + 2]);
        mesh.indices.extend_from_slice(&[spout_base, spout_base + 3, spout_base + 1]);

        combined_normalize_and_prep(&mut mesh, 1.2);
        mesh
    }

    /// Surreal 2001 / Bryce Desert Monolith (Sharp bevelled obelisk)
    pub fn obelisk_monolith() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Desert Monolith Obelisk");
        let w = 0.5;
        let d = 0.25;
        let h_base = 2.4;
        let h_top = 2.8;

        mesh.vertices = vec![
            [-w, -h_base, -d],
            [w, -h_base, -d],
            [w, -h_base, d],
            [-w, -h_base, d],
            [-w * 0.8, h_base, -d * 0.8],
            [w * 0.8, h_base, -d * 0.8],
            [w * 0.8, h_base, d * 0.8],
            [-w * 0.8, h_base, d * 0.8],
            [0.0, h_top, 0.0],
        ];

        mesh.indices = vec![
            0, 1, 5, 0, 5, 4,
            1, 2, 6, 1, 6, 5,
            2, 3, 7, 2, 7, 6,
            3, 0, 4, 3, 4, 7,
            4, 5, 8,
            5, 6, 8,
            6, 7, 8,
            7, 4, 8,
        ];

        combined_normalize_and_prep(&mut mesh, 1.5);
        mesh
    }

    /// Faceted Crystal Geode Spire
    pub fn crystal_geode() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Crystal Geode Spire");
        let sectors = 8;
        let r_mid = 0.85;
        let r_waist = 0.55;

        mesh.vertices.push([0.0, 1.8, 0.0]);
        mesh.vertices.push([0.0, -1.8, 0.0]);

        for s in 0..sectors {
            let u = s as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU;
            let perturb = if s % 2 == 0 { 1.15 } else { 0.85 };
            let x = theta.cos() * r_mid * perturb;
            let z = theta.sin() * r_mid * perturb;
            mesh.vertices.push([x, 0.3, z]);
        }

        for s in 0..sectors {
            let u = s as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU + 0.35;
            let x = theta.cos() * r_waist;
            let z = theta.sin() * r_waist;
            mesh.vertices.push([x, -0.6, z]);
        }

        for s in 0..sectors {
            let next_s = (s + 1) % sectors;
            let u_curr = (2 + s) as u32;
            let u_next = (2 + next_s) as u32;
            let l_curr = (2 + sectors + s) as u32;
            let l_next = (2 + sectors + next_s) as u32;

            mesh.indices.extend_from_slice(&[0, u_next, u_curr]);
            mesh.indices.extend_from_slice(&[u_curr, u_next, l_curr]);
            mesh.indices.extend_from_slice(&[u_next, l_next, l_curr]);
            mesh.indices.extend_from_slice(&[1, l_curr, l_next]);
        }

        combined_normalize_and_prep(&mut mesh, 1.3);
        mesh
    }

    /// Retro 1990s Sci-Fi Spaceship
    pub fn starfighter_ship() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Starfighter Interceptor");
        mesh.vertices = vec![
            [0.0, 0.0, 2.2],
            [-0.5, 0.25, 0.5],
            [0.5, 0.25, 0.5],
            [0.0, 0.6, 0.2],
            [-2.4, -0.1, -1.2],
            [2.4, -0.1, -1.2],
            [-0.6, -0.2, -1.6],
            [0.6, -0.2, -1.6],
            [0.0, 1.1, -1.4],
        ];

        mesh.indices = vec![
            0, 3, 1,
            0, 2, 3,
            1, 3, 6,
            2, 7, 3,
            1, 4, 6,
            2, 7, 5,
            0, 1, 4,
            0, 5, 2,
            3, 8, 6,
            3, 7, 8,
            0, 6, 7,
            0, 4, 6,
            0, 7, 5,
        ];

        combined_normalize_and_prep(&mut mesh, 1.4);
        mesh
    }
}

fn combined_normalize_and_prep(mesh: &mut LoadedMesh, extent: f32) {
    mesh.compute_bounds_and_normalize(extent);
    mesh.generate_normals_if_missing();
}
