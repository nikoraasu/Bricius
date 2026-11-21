use serde::{Deserialize, Serialize};

/// Names of the procedural noise types, indexed by the value stored in `tex[4*i].x`.
pub const NOISE_NAMES: [&str; 8] = ["None", "fBm", "Ridged", "Domain warp", "Worley", "Billow", "Stripes", "Checker"];
/// Names of the material channels, in channel-index order (see `GpuMaterial::src`).
pub const CHANNEL_NAMES: [&str; 7] = ["Diffuse", "Specular", "Roughness", "Reflection", "Transparency", "Bump", "Emission"];

/// Bryce-style material: four procedural textures (A-D) and seven channels, each of which
/// takes either a constant value (source 0) or one of the textures (source 1..=4).
///
/// GPU layout (std140-safe, 21 x vec4 = 336 bytes; mirrored by `GpuMaterial` in the WGSL):
/// - `diffuse`: rgb = diffuse value colour, w = IOR
/// - `srcs1`: channel sources for Diffuse, Specular, Roughness, Reflection
/// - `srcs2`: channel sources for Transparency, Bump, Emission, (unused)
/// - `values1`: Specular, Roughness, Reflection, Transparency amounts
/// - `values2`: Bump height, Emission amount, Metallic, (unused)
/// - `tex[4*i + 0]`: texture i = (noise_type, scale, octaves, warp)
/// - `tex[4*i + 1]`: ramp colour 1 rgb, w = sharpness
/// - `tex[4*i + 2]`: ramp colour 2 rgb
/// - `tex[4*i + 3]`: ramp colour 3 rgb
///
/// A scalar channel with a texture source evaluates to `texture value * amount`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct GpuMaterial {
    pub diffuse: [f32; 4],
    pub srcs1: [f32; 4],
    pub srcs2: [f32; 4],
    pub values1: [f32; 4],
    pub values2: [f32; 4],
    pub tex: [[f32; 4]; 16],
}

/// The pre-Bryce-style material layout (one noise, two colours). Used to define the
/// presets and to migrate old project files / material JSON.
#[derive(Copy, Clone, Debug, Deserialize)]
pub struct LegacyMaterial {
    pub base_color: [f32; 4],
    pub secondary_color: [f32; 4],
    // x: roughness, y: metallic, z: reflectivity, w: transparency
    pub params1: [f32; 4],
    // x: ior, y: bump_strength, z: noise_scale, w: noise_type
    pub params2: [f32; 4],
    // x: warp, y: octaves, z: blend_sharpness, w: emission
    pub params3: [f32; 4],
}

impl From<LegacyMaterial> for GpuMaterial {
    fn from(l: LegacyMaterial) -> Self {
        let mut m = GpuMaterial::default();
        let nt = match l.params2[3] as u32 {
            5 => 6, // old wood grain -> stripes
            n => n.min(7),
        };
        let sharp = l.params3[2].max(0.05);
        let b = l.base_color;
        let s = l.secondary_color;
        m.set_noise(0, nt, l.params2[2], l.params3[1], l.params3[0], sharp);
        m.set_tex_color(0, 0, [b[0], b[1], b[2]]);
        m.set_tex_color(0, 1, [(b[0] + s[0]) * 0.5, (b[1] + s[1]) * 0.5, (b[2] + s[2]) * 0.5]);
        m.set_tex_color(0, 2, [s[0], s[1], s[2]]);
        let textured = nt != 0;
        if textured {
            m.set_src(0, 1);
        } else {
            let t = 0.5f32.powf(sharp);
            m.diffuse = [
                b[0] + (s[0] - b[0]) * t,
                b[1] + (s[1] - b[1]) * t,
                b[2] + (s[2] - b[2]) * t,
                0.0,
            ];
        }
        m.diffuse[3] = l.params2[0];
        *m.value_mut(1) = l.params1[2]; // specular follows old reflectivity
        *m.value_mut(2) = l.params1[0];
        *m.value_mut(3) = l.params1[2];
        *m.value_mut(4) = l.params1[3];
        m.values2[2] = l.params1[1];
        if textured {
            m.set_src(5, 1);
            *m.value_mut(5) = l.params2[1];
            if l.params3[3] > 0.0 {
                m.set_src(6, 1);
            }
        }
        *m.value_mut(6) = l.params3[3];
        m
    }
}

impl Default for GpuMaterial {
    fn default() -> Self {
        let mut m = GpuMaterial {
            diffuse: [0.7, 0.7, 0.7, 1.5],
            srcs1: [0.0; 4],
            srcs2: [0.0; 4],
            values1: [0.3, 0.6, 0.1, 0.0],
            values2: [0.0, 0.0, 0.0, 0.0],
            tex: [[0.0; 4]; 16],
        };
        for i in 0..4 {
            m.set_noise(i, if i == 0 { 1 } else { 0 }, 3.0, 4.0, 0.5, 1.0);
            m.set_tex_color(i, 0, [0.1, 0.1, 0.1]);
            m.set_tex_color(i, 1, [0.5, 0.5, 0.5]);
            m.set_tex_color(i, 2, [0.9, 0.9, 0.9]);
        }
        m
    }
}

impl GpuMaterial {
    pub const NUM_PRESETS: usize = 16;

    /// Source of a channel: 0 = Value, 1..=4 = Texture A..D.
    pub fn src(&self, ch: usize) -> u32 {
        (if ch < 4 { self.srcs1[ch] } else { self.srcs2[ch - 4] }) as u32
    }

    pub fn set_src(&mut self, ch: usize, src: u32) {
        if ch < 4 { self.srcs1[ch] = src as f32 } else { self.srcs2[ch - 4] = src as f32 }
    }

    /// Amount of scalar channels 1..=6 (Specular .. Emission). Channel 0 (Diffuse) uses `diffuse`.
    pub fn value_mut(&mut self, ch: usize) -> &mut f32 {
        match ch {
            1..=4 => &mut self.values1[ch - 1],
            5 => &mut self.values2[0],
            _ => &mut self.values2[1],
        }
    }

    pub fn ior_mut(&mut self) -> &mut f32 { &mut self.diffuse[3] }
    pub fn metallic_mut(&mut self) -> &mut f32 { &mut self.values2[2] }

    pub fn set_noise(&mut self, i: usize, noise_type: u32, scale: f32, octaves: f32, warp: f32, sharpness: f32) {
        self.tex[i * 4] = [noise_type as f32, scale, octaves, warp];
        self.tex[i * 4 + 1][3] = sharpness;
    }

    pub fn tex_color(&self, i: usize, k: usize) -> [f32; 3] {
        let c = self.tex[i * 4 + 1 + k];
        [c[0], c[1], c[2]]
    }

    pub fn set_tex_color(&mut self, i: usize, k: usize, c: [f32; 3]) {
        let t = &mut self.tex[i * 4 + 1 + k];
        t[0] = c[0];
        t[1] = c[1];
        t[2] = c[2];
    }

    /// Parses either the current or the legacy material JSON layout.
    pub fn from_json_value(v: serde_json::Value) -> Option<Self> {
        if v.get("base_color").is_some() {
            serde_json::from_value::<LegacyMaterial>(v).ok().map(Into::into)
        } else {
            serde_json::from_value::<GpuMaterial>(v).ok()
        }
    }

    pub fn from_json(s: &str) -> Option<Self> {
        Self::from_json_value(serde_json::from_str(s).ok()?)
    }

    /// serde helper for project files: accepts current and legacy materials in one list.
    pub fn deserialize_list<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Self>, D::Error> {
        let vals = Vec::<serde_json::Value>::deserialize(d)?;
        vals.into_iter()
            .map(|v| Self::from_json_value(v).ok_or_else(|| serde::de::Error::custom("invalid material")))
            .collect()
    }

    pub fn rock() -> Self {
        LegacyMaterial {
            base_color: [0.45, 0.40, 0.35, 1.0],
            secondary_color: [0.22, 0.20, 0.18, 1.0],
            params1: [0.85, 0.05, 0.05, 0.0],
            params2: [1.5, 0.45, 3.5, 1.0],
            params3: [0.3, 5.0, 1.5, 0.0],
        }.into()
    }

    pub fn blue_marble() -> Self {
        LegacyMaterial {
            base_color: [0.92, 0.94, 0.97, 1.0],
            secondary_color: [0.10, 0.35, 0.60, 1.0],
            params1: [0.15, 0.0, 0.45, 0.0],
            params2: [1.55, 0.15, 2.0, 3.0],
            params3: [2.5, 4.0, 3.0, 0.0],
        }.into()
    }

    pub fn chrome() -> Self {
        LegacyMaterial {
            base_color: [0.95, 0.95, 0.98, 1.0],
            secondary_color: [0.85, 0.88, 0.92, 1.0],
            params1: [0.02, 1.0, 0.98, 0.0],
            params2: [2.5, 0.0, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }.into()
    }

    pub fn emerald_glass() -> Self {
        LegacyMaterial {
            base_color: [0.05, 0.85, 0.45, 1.0],
            secondary_color: [0.02, 0.40, 0.20, 1.0],
            params1: [0.05, 0.0, 0.15, 0.85],
            params2: [1.52, 0.0, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }.into()
    }

    pub fn gold() -> Self {
        LegacyMaterial {
            base_color: [1.00, 0.82, 0.25, 1.0],
            secondary_color: [0.80, 0.55, 0.10, 1.0],
            params1: [0.25, 0.95, 0.80, 0.0],
            params2: [1.3, 0.25, 6.0, 1.0],
            params3: [0.1, 3.0, 1.0, 0.0],
        }.into()
    }

    pub fn basalt_obsidian() -> Self {
        LegacyMaterial {
            base_color: [0.06, 0.06, 0.08, 1.0],
            secondary_color: [0.15, 0.12, 0.10, 1.0],
            params1: [0.20, 0.10, 0.35, 0.0],
            params2: [1.6, 0.70, 2.5, 2.0],
            params3: [0.5, 4.0, 2.0, 0.0],
        }.into()
    }

    pub fn ocean_water() -> Self {
        LegacyMaterial {
            base_color: [0.05, 0.25, 0.40, 1.0],
            secondary_color: [0.02, 0.10, 0.22, 1.0],
            params1: [0.05, 0.0, 0.85, 0.70],
            params2: [1.333, 0.50, 4.0, 1.0],
            params3: [0.2, 4.0, 1.0, 0.0],
        }.into()
    }

    pub fn alien_bioluminescent() -> Self {
        LegacyMaterial {
            base_color: [0.10, 0.05, 0.20, 1.0],
            secondary_color: [0.20, 0.95, 0.85, 1.0],
            params1: [0.30, 0.20, 0.25, 0.0],
            params2: [1.5, 0.40, 2.0, 4.0],
            params3: [1.2, 3.0, 4.0, 1.8],
        }.into()
    }

    pub fn retro_plastic() -> Self {
        LegacyMaterial {
            base_color: [0.90, 0.15, 0.25, 1.0],
            secondary_color: [1.0, 0.4, 0.5, 1.0],
            params1: [0.20, 0.0, 0.35, 0.0], // High specular gloss, synthetic
            params2: [1.45, 0.05, 1.0, 0.0],
            params3: [0.0, 1.0, 1.0, 0.0],
        }.into()
    }

    pub fn sandstone() -> Self {
        LegacyMaterial {
            base_color: [0.85, 0.55, 0.30, 1.0],
            secondary_color: [0.65, 0.35, 0.15, 1.0],
            params1: [0.90, 0.0, 0.02, 0.0],
            params2: [1.5, 0.60, 4.0, 2.0], // Ridged multifractal strata
            params3: [0.4, 4.0, 2.0, 0.0],
        }.into()
    }

    pub fn glacial_ice() -> Self {
        LegacyMaterial {
            base_color: [0.75, 0.90, 0.98, 1.0],
            secondary_color: [0.25, 0.60, 0.85, 1.0],
            params1: [0.10, 0.0, 0.50, 0.60],
            params2: [1.31, 0.30, 3.0, 1.0], // Ice IOR = 1.31
            params3: [0.5, 3.0, 1.5, 0.1],
        }.into()
    }

    pub fn wood_grain() -> Self {
        LegacyMaterial {
            base_color: [0.55, 0.32, 0.15, 1.0],
            secondary_color: [0.30, 0.15, 0.06, 1.0],
            params1: [0.65, 0.0, 0.08, 0.0],
            params2: [1.5, 0.35, 5.0, 3.0],
            params3: [1.8, 3.0, 2.5, 0.0],
        }.into()
    }

    pub fn rusted_iron() -> Self {
        LegacyMaterial {
            base_color: [0.45, 0.20, 0.10, 1.0],
            secondary_color: [0.25, 0.26, 0.28, 1.0],
            params1: [0.75, 0.60, 0.15, 0.0],
            params2: [1.6, 0.55, 6.0, 4.0],
            params3: [0.8, 4.0, 1.8, 0.0],
        }.into()
    }

    pub fn amethyst_crystal() -> Self {
        LegacyMaterial {
            base_color: [0.60, 0.15, 0.85, 1.0],
            secondary_color: [0.30, 0.05, 0.50, 1.0],
            params1: [0.08, 0.10, 0.30, 0.75],
            params2: [1.54, 0.10, 2.0, 2.0],
            params3: [0.2, 2.0, 1.0, 0.3],
        }.into()
    }

    pub fn cyber_grid() -> Self {
        LegacyMaterial {
            base_color: [0.02, 0.03, 0.06, 1.0],
            secondary_color: [0.00, 0.95, 1.00, 1.0],
            params1: [0.10, 0.50, 0.60, 0.0],
            params2: [1.5, 0.20, 4.0, 4.0],
            params3: [0.0, 2.0, 4.0, 2.2], // High emissive grid lines
        }.into()
    }

    pub fn red_desert_rock() -> Self {
        LegacyMaterial {
            base_color: [0.75, 0.28, 0.15, 1.0],
            secondary_color: [0.45, 0.15, 0.08, 1.0],
            params1: [0.85, 0.02, 0.04, 0.0],
            params2: [1.55, 0.65, 3.0, 2.0],
            params3: [0.6, 5.0, 1.8, 0.0],
        }.into()
    }

    pub fn default_preset_list() -> [Self; Self::NUM_PRESETS] {
        [
            Self::rock(),
            Self::blue_marble(),
            Self::chrome(),
            Self::emerald_glass(),
            Self::gold(),
            Self::basalt_obsidian(),
            Self::ocean_water(),
            Self::alien_bioluminescent(),
            Self::retro_plastic(),
            Self::sandstone(),
            Self::glacial_ice(),
            Self::wood_grain(),
            Self::rusted_iron(),
            Self::amethyst_crystal(),
            Self::cyber_grid(),
            Self::red_desert_rock(),
        ]
    }
}
