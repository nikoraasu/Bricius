use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainAlgorithm {
    Fbm = 0,
    RidgedMultifractal = 1,
    Billow = 2,
    WorleyCellular = 3,
    DomainWarped = 4,
    HybridMultifractal = 5,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainConfig {
    pub seed: u32,
    pub algorithm: TerrainAlgorithm,
    pub height_scale: f32,
    pub frequency: f32,
    pub octaves: u32,
    pub mountain_sharpness: f32,
    pub ridge_intensity: f32,
    pub valley_depth: f32,
    pub erosion_hydraulic: f32,
    pub erosion_thermal: f32,
    pub domain_warp: f32,
    pub lagoon_radius: f32,
    pub snow_line: f32,
    /// Global procedural "Backdrop mountains" (raymarched in the shader, independent of terrain objects)
    #[serde(default = "default_true")]
    pub backdrop_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            seed: 1337,
            algorithm: TerrainAlgorithm::RidgedMultifractal,
            height_scale: 1.0,
            frequency: 0.03,
            octaves: 5,
            mountain_sharpness: 1.8,
            ridge_intensity: 1.5,
            valley_depth: 1.2,
            erosion_hydraulic: 0.35,
            erosion_thermal: 0.25,
            domain_warp: 0.4,
            lagoon_radius: 18.0,
            snow_line: 14.0,
            backdrop_enabled: true,
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BrushMode {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Noise,
    ErodeHydraulic,
    ErodeThermal,
}

/// World footprint (full width, units) sampled when generating a terrain object's heightmap
pub const TERRAIN_GEN_EXTENT: f32 = 60.0;
/// Height range (world units) that maps to the normalized [0,1] heightmap range at height_scale 1.0
pub const TERRAIN_GEN_HEIGHT_RANGE: f32 = 14.0;

#[derive(Clone, Debug)]
pub struct HeightmapGrid {
    pub resolution: usize,
    pub data: Vec<f32>,
    /// Multiplier applied to brush deltas and erosion thresholds (1.0 = height-in-world-units grids,
    /// 1/12 for normalized [0,1] terrain-object heightmaps)
    pub unit: f32,
}

impl HeightmapGrid {
    pub fn new(resolution: usize) -> Self {
        Self {
            resolution,
            data: vec![0.0; resolution * resolution],
            unit: 1.0,
        }
    }

    /// Generate a normalized [0,1] heightmap covering the footprint [-extent/2, extent/2]^2
    pub fn generate_normalized(&mut self, config: &TerrainConfig) {
        let res = self.resolution;
        let denom = (res as f32 - 1.0).max(1.0);
        let range = TERRAIN_GEN_HEIGHT_RANGE * config.height_scale.max(0.05);
        for y in 0..res {
            for x in 0..res {
                let wx = (x as f32 / denom - 0.5) * TERRAIN_GEN_EXTENT;
                let wz = (y as f32 / denom - 0.5) * TERRAIN_GEN_EXTENT;
                let h = evaluate_procedural_height(wx, wz, config);
                self.data[y * res + x] = ((h + 2.0) / range).clamp(0.0, 1.0);
            }
        }
        self.unit = 1.0 / 12.0;
    }

    /// Bilinear height at normalized footprint coords u,v in [0,1]
    pub fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let res = self.resolution;
        if res < 2 {
            return self.data.first().copied().unwrap_or(0.0);
        }
        let fx = u.clamp(0.0, 1.0) * (res as f32 - 1.0);
        let fy = v.clamp(0.0, 1.0) * (res as f32 - 1.0);
        let x0 = (fx.floor() as usize).min(res - 2);
        let y0 = (fy.floor() as usize).min(res - 2);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let h = |x: usize, y: usize| self.data[y * res + x];
        let a = h(x0, y0) + (h(x0 + 1, y0) - h(x0, y0)) * tx;
        let b = h(x0, y0 + 1) + (h(x0 + 1, y0 + 1) - h(x0, y0 + 1)) * tx;
        a + (b - a) * ty
    }

    /// Resample to a new resolution (bilinear)
    pub fn resample(&mut self, new_res: usize) {
        if new_res == self.resolution || new_res < 2 {
            return;
        }
        let mut data = vec![0.0; new_res * new_res];
        let denom = (new_res as f32 - 1.0).max(1.0);
        for y in 0..new_res {
            for x in 0..new_res {
                data[y * new_res + x] = self.sample_bilinear(x as f32 / denom, y as f32 / denom);
            }
        }
        self.resolution = new_res;
        self.data = data;
    }

    #[allow(dead_code)]
    pub fn generate_from_config(&mut self, config: &TerrainConfig) {
        let res = self.resolution;
        let center = (res as f32) * 0.5;

        for y in 0..res {
            for x in 0..res {
                let world_x = (x as f32 - center) * 1.5;
                let world_z = (y as f32 - center) * 1.5;

                let h = evaluate_procedural_height(world_x, world_z, config);
                self.data[y * res + x] = h;
            }
        }
    }

    #[allow(dead_code)]
    pub fn apply_brush(
        &mut self,
        center_x: usize,
        center_y: usize,
        radius: usize,
        strength: f32,
        mode: BrushMode,
        target_height: f32,
    ) {
        let res = self.resolution;
        let r_f32 = radius as f32;

        let x_min = center_x.saturating_sub(radius);
        let x_max = (center_x + radius).min(res - 1);
        let y_min = center_y.saturating_sub(radius);
        let y_max = (center_y + radius).min(res - 1);

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as f32 - center_x as f32;
                let dy = y as f32 - center_y as f32;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= r_f32 {
                    let falloff = (1.0 - (dist / r_f32)).powf(2.0);
                    let effect = strength * falloff;
                    let idx = y * res + x;
                    let current = self.data[idx];

                    match mode {
                        BrushMode::Raise => {
                            self.data[idx] += effect * 2.0 * self.unit;
                        }
                        BrushMode::Lower => {
                            self.data[idx] -= effect * 2.0 * self.unit;
                        }
                        BrushMode::Smooth => {
                            let mut sum = 0.0;
                            let mut count = 0.0;
                            for ny in (y.saturating_sub(1))..=(y + 1).min(res - 1) {
                                for nx in (x.saturating_sub(1))..=(x + 1).min(res - 1) {
                                    sum += self.data[ny * res + nx];
                                    count += 1.0;
                                }
                            }
                            let avg = sum / count;
                            self.data[idx] = current + (avg - current) * effect;
                        }
                        BrushMode::Flatten => {
                            self.data[idx] = current + (target_height - current) * effect;
                        }
                        BrushMode::Noise => {
                            let n = (pseudo_hash(x as u32, y as u32) - 0.5) * 4.0 * self.unit;
                            self.data[idx] += n * effect;
                        }
                        BrushMode::ErodeHydraulic => {
                            let mut min_neighbor = current;
                            for ny in (y.saturating_sub(1))..=(y + 1).min(res - 1) {
                                for nx in (x.saturating_sub(1))..=(x + 1).min(res - 1) {
                                    min_neighbor = min_neighbor.min(self.data[ny * res + nx]);
                                }
                            }
                            let slope = current - min_neighbor;
                            self.data[idx] -= slope * effect * 0.5;
                        }
                        BrushMode::ErodeThermal => {
                            let talus_threshold = 0.4 * self.unit;
                            let mut max_diff: f32 = 0.0;
                            for ny in (y.saturating_sub(1))..=(y + 1).min(res - 1) {
                                for nx in (x.saturating_sub(1))..=(x + 1).min(res - 1) {
                                    let diff = current - self.data[ny * res + nx];
                                    if diff > talus_threshold {
                                        max_diff = max_diff.max(diff);
                                    }
                                }
                            }
                            self.data[idx] -= max_diff * effect * 0.3;
                        }
                    }
                }
            }
        }
    }

    /// Full hydraulic erosion simulation pass
    pub fn simulate_hydraulic_erosion(&mut self, iterations: usize) {
        let res = self.resolution;
        let mut sediment = vec![0.0f32; res * res];

        for _ in 0..iterations {
            for y in 1..res - 1 {
                for x in 1..res - 1 {
                    let idx = y * res + x;
                    let h = self.data[idx];

                    // Find lowest neighbor
                    let neighbors = [
                        (x - 1, y),
                        (x + 1, y),
                        (x, y - 1),
                        (x, y + 1),
                    ];

                    let mut lowest_idx = idx;
                    let mut lowest_h = h;

                    for (nx, ny) in neighbors {
                        let n_idx = ny * res + nx;
                        if self.data[n_idx] < lowest_h {
                            lowest_h = self.data[n_idx];
                            lowest_idx = n_idx;
                        }
                    }

                    if lowest_idx != idx {
                        let slope = h - lowest_h;
                        let erode_amt = slope * 0.08;
                        self.data[idx] -= erode_amt;
                        sediment[lowest_idx] += erode_amt * 0.85; // deposit downstream
                    }
                }
            }

            // Apply sediment deposition
            for i in 0..self.data.len() {
                self.data[i] += sediment[i];
                sediment[i] = 0.0;
            }
        }
    }

    /// Full thermal weathering simulation pass (talus slopes)
    pub fn simulate_thermal_erosion(&mut self, iterations: usize) {
        let res = self.resolution;
        let talus_threshold = 0.5 * self.unit;

        for _ in 0..iterations {
            for y in 1..res - 1 {
                for x in 1..res - 1 {
                    let idx = y * res + x;
                    let h = self.data[idx];

                    let neighbors = [
                        (x - 1, y),
                        (x + 1, y),
                        (x, y - 1),
                        (x, y + 1),
                    ];

                    for (nx, ny) in neighbors {
                        let n_idx = ny * res + nx;
                        let diff = h - self.data[n_idx];
                        if diff > talus_threshold {
                            let transfer = (diff - talus_threshold) * 0.2;
                            self.data[idx] -= transfer;
                            self.data[n_idx] += transfer;
                        }
                    }
                }
            }
        }
    }

    /// Export heightmap as PNG image file
    pub fn export_png(&self, path: &str) -> Result<(), String> {
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        for &h in &self.data {
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
        let range = (max_h - min_h).max(0.001);

        let mut img = image::GrayImage::new(self.resolution as u32, self.resolution as u32);
        for y in 0..self.resolution {
            for x in 0..self.resolution {
                let h = self.data[y * self.resolution + x];
                let norm = ((h - min_h) / range).clamp(0.0, 1.0);
                let byte = (norm * 255.0) as u8;
                img.put_pixel(x as u32, y as u32, image::Luma([byte]));
            }
        }

        img.save(path).map_err(|e| format!("Failed to save PNG: {}", e))
    }

    /// Export a normalized terrain-object heightmap as OBJ, scaled to the object's world extents
    /// (footprint [-sx,sx] x [-sz,sz], height [0,sy]).
    pub fn export_obj_scaled(&self, path: &str, scale: [f32; 3]) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|e| format!("Failed to create OBJ file: {}", e))?;
        writeln!(file, "# Bricius - Terrain Object OBJ Export").unwrap();
        let res = self.resolution;
        let denom = (res as f32 - 1.0).max(1.0);
        for y in 0..res {
            for x in 0..res {
                let wx = (x as f32 / denom * 2.0 - 1.0) * scale[0];
                let wz = (y as f32 / denom * 2.0 - 1.0) * scale[2];
                let wy = self.data[y * res + x] * scale[1];
                writeln!(file, "v {:.3} {:.3} {:.3}", wx, wy, wz).unwrap();
            }
        }
        for y in 0..res {
            for x in 0..res {
                writeln!(file, "vt {:.4} {:.4}", x as f32 / denom, y as f32 / denom).unwrap();
            }
        }
        for y in 0..res - 1 {
            for x in 0..res - 1 {
                let i0 = (y * res + x) + 1;
                let i1 = (y * res + (x + 1)) + 1;
                let i2 = ((y + 1) * res + (x + 1)) + 1;
                let i3 = ((y + 1) * res + x) + 1;
                writeln!(file, "f {}/{} {}/{} {}/{}", i0, i0, i2, i2, i1, i1).unwrap();
                writeln!(file, "f {}/{} {}/{} {}/{}", i0, i0, i3, i3, i2, i2).unwrap();
            }
        }
        Ok(())
    }

    /// Export heightmap as 3D OBJ mesh file
    #[allow(dead_code)]
    pub fn export_obj(&self, path: &str) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|e| format!("Failed to create OBJ file: {}", e))?;

        writeln!(file, "# Bricius - Procedural Terrain OBJ Export").unwrap();
        let res = self.resolution;
        let center = (res as f32) * 0.5;

        // Write vertices
        for y in 0..res {
            for x in 0..res {
                let wx = (x as f32 - center) * 1.5;
                let wz = (y as f32 - center) * 1.5;
                let wy = self.data[y * res + x];
                writeln!(file, "v {:.3} {:.3} {:.3}", wx, wy, wz).unwrap();
            }
        }

        // Write UVs
        for y in 0..res {
            for x in 0..res {
                let u = x as f32 / (res as f32 - 1.0);
                let v = y as f32 / (res as f32 - 1.0);
                writeln!(file, "vt {:.4} {:.4}", u, v).unwrap();
            }
        }

        // Write quad faces (two triangles each)
        for y in 0..res - 1 {
            for x in 0..res - 1 {
                let i0 = (y * res + x) + 1;
                let i1 = (y * res + (x + 1)) + 1;
                let i2 = ((y + 1) * res + (x + 1)) + 1;
                let i3 = ((y + 1) * res + x) + 1;

                writeln!(file, "f {}/{} {}/{} {}/{}", i0, i0, i1, i1, i2, i2).unwrap();
                writeln!(file, "f {}/{} {}/{} {}/{}", i0, i0, i2, i2, i3, i3).unwrap();
            }
        }

        Ok(())
    }
}

/// A terrain object's heightmap (normalized [0,1]) plus the generator config that produced it
#[derive(Clone, Debug)]
pub struct TerrainObjectData {
    pub heightmap: HeightmapGrid,
    pub config: TerrainConfig,
}

/// Serializable form (heights quantized to 16 bits)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainObjectSave {
    pub resolution: usize,
    pub config: TerrainConfig,
    pub heights: Vec<u16>,
}

impl TerrainObjectData {
    pub fn new(resolution: usize, config: &TerrainConfig) -> Self {
        let mut heightmap = HeightmapGrid::new(resolution);
        heightmap.generate_normalized(config);
        Self { heightmap, config: config.clone() }
    }

    pub fn clamp_heights(&mut self) {
        for v in &mut self.heightmap.data {
            *v = v.clamp(0.0, 1.0);
        }
    }

    pub fn to_save(&self) -> TerrainObjectSave {
        TerrainObjectSave {
            resolution: self.heightmap.resolution,
            config: self.config.clone(),
            heights: self.heightmap.data.iter().map(|h| (h.clamp(0.0, 1.0) * 65535.0).round() as u16).collect(),
        }
    }

    pub fn from_save(save: &TerrainObjectSave) -> Self {
        let res = save.resolution.max(2);
        let mut heightmap = HeightmapGrid::new(res);
        for (i, v) in heightmap.data.iter_mut().enumerate() {
            *v = save.heights.get(i).copied().unwrap_or(0) as f32 / 65535.0;
        }
        heightmap.unit = 1.0 / 12.0;
        Self { heightmap, config: save.config.clone() }
    }
}

// Procedural math helpers on CPU
pub fn evaluate_procedural_height(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let freq = config.frequency;
    let seed_offset = (config.seed % 1000) as f32 * 12.34;
    let px = (x + seed_offset) * freq;
    let pz = (z + seed_offset) * freq;

    let base = match config.algorithm {
        TerrainAlgorithm::Fbm => cpu_fbm(px, pz, config.octaves),
        TerrainAlgorithm::RidgedMultifractal => cpu_ridged(px, pz, config.octaves, config.ridge_intensity),
        TerrainAlgorithm::Billow => cpu_billow(px, pz, config.octaves),
        TerrainAlgorithm::WorleyCellular => cpu_worley(px, pz),
        TerrainAlgorithm::DomainWarped => cpu_domain_warp(px, pz, config.domain_warp, config.octaves),
        TerrainAlgorithm::HybridMultifractal => cpu_hybrid(px, pz, config.octaves),
    };

    let mountain = base * (12.0 * config.height_scale);
    let dist = (x * x + (z + 18.0) * (z + 18.0)).sqrt();
    let mask = ((dist - 8.0) / (config.lagoon_radius * 2.0)).clamp(0.0, 1.0);

    (mountain - 2.0) * mask
}

fn cpu_noise(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;

    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);

    let n00 = pseudo_hash(ix as u32, iz as u32);
    let n10 = pseudo_hash((ix + 1.0) as u32, iz as u32);
    let n01 = pseudo_hash(ix as u32, (iz + 1.0) as u32);
    let n11 = pseudo_hash((ix + 1.0) as u32, (iz + 1.0) as u32);

    let nx0 = n00 + (n10 - n00) * ux;
    let nx1 = n01 + (n11 - n01) * ux;

    nx0 + (nx1 - nx0) * uz
}

fn cpu_fbm(x: f32, z: f32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut fx = x;
    let mut fz = z;

    for _ in 0..octaves {
        sum += amp * cpu_noise(fx, fz);
        fx = fx * 2.02 + 1.23;
        fz = fz * 2.02 + 3.45;
        amp *= 0.5;
    }
    sum
}

fn cpu_ridged(x: f32, z: f32, octaves: u32, ridge_intensity: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut weight = 1.0;

    for _ in 0..octaves {
        let mut n = cpu_noise(x * freq, z * freq);
        n = 1.0 - (n * 2.0 - 1.0).abs();
        n = n * n * ridge_intensity;
        n *= weight;
        weight = (n * 2.0).clamp(0.0, 1.0);

        sum += n * amp;
        freq *= 2.05;
        amp *= 0.5;
    }
    sum
}

fn cpu_billow(x: f32, z: f32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut fx = x;
    let mut fz = z;

    for _ in 0..octaves {
        let n = (cpu_noise(fx, fz) * 2.0 - 1.0).abs();
        sum += amp * n;
        fx = fx * 2.02 + 1.2;
        fz = fz * 2.02 + 2.3;
        amp *= 0.5;
    }
    sum
}

fn cpu_worley(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;
    let mut min_dist = 1.0f32;

    for oy in -1..=1 {
        for ox in -1..=1 {
            let px = pseudo_hash((ix as i32 + ox) as u32, (iz as i32 + oy) as u32);
            let pz = pseudo_hash((ix as i32 + ox + 31) as u32, (iz as i32 + oy + 47) as u32);
            let dx = ox as f32 + px - fx;
            let dz = oy as f32 + pz - fz;
            let d = (dx * dx + dz * dz).sqrt();
            min_dist = min_dist.min(d);
        }
    }
    1.0 - min_dist.clamp(0.0, 1.0)
}

fn cpu_domain_warp(x: f32, z: f32, warp: f32, octaves: u32) -> f32 {
    let qx = cpu_fbm(x, z, octaves);
    let qz = cpu_fbm(x + 5.2, z + 1.3, octaves);
    cpu_fbm(x + qx * warp, z + qz * warp, octaves)
}

fn cpu_hybrid(x: f32, z: f32, octaves: u32) -> f32 {
    let r = cpu_ridged(x, z, octaves, 1.2);
    let f = cpu_fbm(x * 1.5, z * 1.5, octaves);
    r * 0.7 + f * 0.3
}

fn pseudo_hash(x: u32, z: u32) -> f32 {
    let mut h = x.wrapping_mul(374761393).wrapping_add(z.wrapping_mul(668265263));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h as f32) / (u32::MAX as f32)
}
