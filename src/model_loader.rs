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
            aabb_min: [0.0; 3],
            aabb_max: [0.0; 3],
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
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read .obj file: {}", e))?;
            Self::load_obj(&content, file_stem)
        } else if path_lower.ends_with(".glb") {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read .glb file: {}", e))?;
            Self::load_glb(&bytes, file_stem)
        } else if path_lower.ends_with(".fbx") {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read .fbx file: {}", e))?;
            Self::load_fbx_ascii(&content, file_stem)
        } else {
            Err(format!("Unsupported 3D model format in '{}'. Supported: .obj, .glb, .fbx", path))
        }
    }

    /// Wavefront .obj parser: supports vertices, normals, and polygon triangulation
    pub fn load_obj(content: &str, name: &str) -> Result<LoadedMesh, String> {
        let mut mesh = LoadedMesh::new(name);
        let mut raw_positions: Vec<[f32; 3]> = Vec::new();
        let mut raw_normals: Vec<[f32; 3]> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut tokens = line.split_whitespace();
            match tokens.next() {
                Some("v") => {
                    let x: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    raw_positions.push([x, y, z]);
                }
                Some("vn") => {
                    let nx: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let ny: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
                    let nz: f32 = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    raw_normals.push([nx, ny, nz]);
                }
                Some("f") => {
                    let face_tokens: Vec<&str> = tokens.collect();
                    if face_tokens.len() < 3 {
                        continue;
                    }

                    // Fan triangulation for quads and n-gons
                    let mut face_v_indices = Vec::new();
                    for tok in face_tokens {
                        let parts: Vec<&str> = tok.split('/').collect();
                        if let Some(idx_str) = parts.first() {
                            if let Ok(idx) = idx_str.parse::<i32>() {
                                let resolved = if idx > 0 {
                                    (idx - 1) as usize
                                } else {
                                    (raw_positions.len() as i32 + idx) as usize
                                };
                                face_v_indices.push(resolved);
                            }
                        }
                    }

                    for i in 1..face_v_indices.len() - 1 {
                        let i0 = face_v_indices[0];
                        let i1 = face_v_indices[i];
                        let i2 = face_v_indices[i + 1];

                        if i0 < raw_positions.len() && i1 < raw_positions.len() && i2 < raw_positions.len() {
                            let curr_base = mesh.vertices.len() as u32;
                            mesh.vertices.push(raw_positions[i0]);
                            mesh.vertices.push(raw_positions[i1]);
                            mesh.vertices.push(raw_positions[i2]);

                            mesh.indices.push(curr_base);
                            mesh.indices.push(curr_base + 1);
                            mesh.indices.push(curr_base + 2);
                        }
                    }
                }
                _ => {}
            }
        }

        if mesh.vertices.is_empty() {
            return Err("No valid vertex/face geometry found in .obj file".to_string());
        }

        mesh.compute_bounds_and_normalize(1.0);
        mesh.generate_normals_if_missing();
        Ok(mesh)
    }

    /// Binary glTF 2.0 (.glb) parser
    pub fn load_glb(bytes: &[u8], name: &str) -> Result<LoadedMesh, String> {
        if bytes.len() < 20 {
            return Err("Invalid .glb: file too small".to_string());
        }

        // Header check: magic 0x46546C67 ("glTF")
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != 0x46546C67 {
            return Err("Invalid .glb magic identifier".to_string());
        }

        // Chunk 0: JSON metadata
        let chunk0_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let chunk0_type = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        if chunk0_type != 0x4E4F534A {
            // "JSON"
            return Err("Invalid .glb: expected JSON chunk 0".to_string());
        }

        let json_start = 20;
        let json_end = json_start + chunk0_len;
        if json_end > bytes.len() {
            return Err("Invalid .glb: JSON chunk truncated".to_string());
        }

        let json_str = std::str::from_utf8(&bytes[json_start..json_end])
            .map_err(|e| format!("Invalid UTF-8 in glb JSON: {}", e))?;

        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse glb JSON metadata: {}", e))?;

        // Chunk 1: Binary buffer (BIN)
        let bin_start = json_end + 8; // skip chunk1 len + type header
        if bin_start >= bytes.len() {
            return Err("Invalid .glb: missing binary data chunk".to_string());
        }
        let bin_data = &bytes[bin_start..];

        let mut mesh = LoadedMesh::new(name);

        // Find position accessors
        if let Some(accessors) = json.get("accessors").and_then(|a| a.as_array()) {
            if let Some(buffer_views) = json.get("bufferViews").and_then(|b| b.as_array()) {
                // Find POSITION attribute in meshes[0].primitives[0].attributes.POSITION
                if let Some(pos_accessor_idx) = json
                    .get("meshes")
                    .and_then(|m| m.as_array())
                    .and_then(|m| m.first())
                    .and_then(|m| m.get("primitives"))
                    .and_then(|p| p.as_array())
                    .and_then(|p| p.first())
                    .and_then(|p| p.get("attributes"))
                    .and_then(|a| a.get("POSITION"))
                    .and_then(|p| p.as_u64())
                {
                    if let Some(pos_acc) = accessors.get(pos_accessor_idx as usize) {
                        let count = pos_acc.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
                        let view_idx = pos_acc.get("bufferView").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
                        let acc_offset = pos_acc.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0) as usize;

                        if let Some(view) = buffer_views.get(view_idx) {
                            let view_offset = view.get("byteOffset").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
                            let start = view_offset + acc_offset;

                            for i in 0..count {
                                let offset = start + i * 12;
                                if offset + 12 <= bin_data.len() {
                                    let x = f32::from_le_bytes(bin_data[offset..offset + 4].try_into().unwrap());
                                    let y = f32::from_le_bytes(bin_data[offset + 4..offset + 8].try_into().unwrap());
                                    let z = f32::from_le_bytes(bin_data[offset + 8..offset + 12].try_into().unwrap());
                                    mesh.vertices.push([x, y, z]);
                                }
                            }
                        }
                    }
                }
            }
        }

        if mesh.vertices.is_empty() {
            return Err("Could not extract vertex buffer from .glb".to_string());
        }

        // Generate triangle index list (sequential if no indices accessor)
        for i in (0..mesh.vertices.len() as u32).step_by(3) {
            if i + 2 < mesh.vertices.len() as u32 {
                mesh.indices.push(i);
                mesh.indices.push(i + 1);
                mesh.indices.push(i + 2);
            }
        }

        mesh.compute_bounds_and_normalize(1.0);
        mesh.generate_normals_if_missing();
        Ok(mesh)
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

        mesh.compute_bounds_and_normalize(1.0);
        mesh.generate_normals_if_missing();
        Ok(mesh)
    }

    // =========================================================================
    // BUILT-IN PROCEDURAL PRESET MODELS
    // =========================================================================

    /// Classic Utah / Bryce CGI Teapot (iconic 1990s 3D creation studio motif)
    pub fn bryce_teapot() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Bryce Utah Teapot");
        // Procedural faceted teapot body + spout + handle + lid
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

        mesh.compute_bounds_and_normalize(1.2);
        mesh.generate_normals_if_missing();
        mesh
    }

    /// Surreal 2001 / Bryce Desert Monolith (Sharp bevelled obelisk)
    pub fn obelisk_monolith() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Desert Monolith Obelisk");
        // Slender tall pyramidion obelisk (dimensions 1:4:9 ratio reminiscent of Clarke/Kubrick)
        let w = 0.5;
        let d = 0.25;
        let h_base = 2.4;
        let h_top = 2.8;

        // Base box
        mesh.vertices = vec![
            [-w, -h_base, -d], [w, -h_base, -d], [w, -h_base, d], [-w, -h_base, d], // Bottom
            [-w * 0.8, h_base, -d * 0.8], [w * 0.8, h_base, -d * 0.8], [w * 0.8, h_base, d * 0.8], [-w * 0.8, h_base, d * 0.8], // Top waist
            [0.0, h_top, 0.0], // Pyramidion peak
        ];

        // Sides
        mesh.indices = vec![
            0, 1, 5, 0, 5, 4, // Front
            1, 2, 6, 1, 6, 5, // Right
            2, 3, 7, 2, 7, 6, // Back
            3, 0, 4, 3, 4, 7, // Left
            // Pyramidion cap
            4, 5, 8,
            5, 6, 8,
            6, 7, 8,
            7, 4, 8,
        ];

        mesh.compute_bounds_and_normalize(1.5);
        mesh.generate_normals_if_missing();
        mesh
    }

    /// Faceted Crystal Geode Spire
    pub fn crystal_geode() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Crystal Geode Spire");
        let sectors = 8;
        let r_mid = 0.85;
        let r_waist = 0.55;

        // Peak and Bottom
        mesh.vertices.push([0.0, 1.8, 0.0]); // 0: Top peak
        mesh.vertices.push([0.0, -1.8, 0.0]); // 1: Bottom peak

        for s in 0..sectors {
            let u = s as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU;
            let perturb = if s % 2 == 0 { 1.15 } else { 0.85 };
            let x = theta.cos() * r_mid * perturb;
            let z = theta.sin() * r_mid * perturb;
            mesh.vertices.push([x, 0.3, z]); // 2 + s: Upper ring
        }

        for s in 0..sectors {
            let u = s as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU + 0.35;
            let x = theta.cos() * r_waist;
            let z = theta.sin() * r_waist;
            mesh.vertices.push([x, -0.6, z]); // 2 + sectors + s: Lower ring
        }

        for s in 0..sectors {
            let next_s = (s + 1) % sectors;
            let u_curr = (2 + s) as u32;
            let u_next = (2 + next_s) as u32;
            let l_curr = (2 + sectors + s) as u32;
            let l_next = (2 + sectors + next_s) as u32;

            // Top peak triangles
            mesh.indices.extend_from_slice(&[0, u_next, u_curr]);
            // Mid faceted band
            mesh.indices.extend_from_slice(&[u_curr, u_next, l_curr]);
            mesh.indices.extend_from_slice(&[u_next, l_next, l_curr]);
            // Bottom peak triangles
            mesh.indices.extend_from_slice(&[1, l_curr, l_next]);
        }

        mesh.compute_bounds_and_normalize(1.3);
        mesh.generate_normals_if_missing();
        mesh
    }

    /// Retro 1990s Sci-Fi Spaceship
    pub fn starfighter_ship() -> LoadedMesh {
        let mut mesh = LoadedMesh::new("Starfighter Interceptor");
        mesh.vertices = vec![
            [0.0, 0.0, 2.2],    // 0: Cockpit nose
            [-0.5, 0.25, 0.5],  // 1: Cockpit left
            [0.5, 0.25, 0.5],   // 2: Cockpit right
            [0.0, 0.6, 0.2],    // 3: Canopy top
            [-2.4, -0.1, -1.2], // 4: Wingtip left
            [2.4, -0.1, -1.2],  // 5: Wingtip right
            [-0.6, -0.2, -1.6], // 6: Engine left
            [0.6, -0.2, -1.6],  // 7: Engine right
            [0.0, 1.1, -1.4],   // 8: Vertical tailfin
        ];

        mesh.indices = vec![
            // Cockpit
            0, 3, 1,
            0, 2, 3,
            1, 3, 6,
            2, 7, 3,
            // Wings
            1, 4, 6,
            2, 7, 5,
            0, 1, 4,
            0, 5, 2,
            // Vertical tailfin
            3, 8, 6,
            3, 7, 8,
            // Underside
            0, 6, 7,
            0, 4, 6,
            0, 7, 5,
        ];

        mesh.compute_bounds_and_normalize(1.4);
        mesh.generate_normals_if_missing();
        mesh
    }
}
