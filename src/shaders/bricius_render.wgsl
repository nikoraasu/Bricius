// =============================================================================
// BRYCE-INSPIRED 3D ENVIRONMENT CREATION STUDIO (WGSL)
// =============================================================================

struct CameraUniform {
    pos: vec4<f32>,
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // x: tan_half_fov, y: aspect_ratio, z: near, w: far
    params: vec4<f32>,
};

struct SceneUniform {
    sun_direction: vec4<f32>,
    sun_color_intensity: vec4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    ground_color: vec4<f32>,
    // x: fog_density, y: fog_height_falloff, z: mie_haze, w: cloud_density
    fog_params: vec4<f32>,
    fog_color: vec4<f32>,
    // x: water_height, y: wave_amp, z: wave_freq, w: wave_speed
    water_params1: vec4<f32>,
    // x: transparency, y: ior, z: roughness, w: reflection_strength
    water_params2: vec4<f32>,
    water_deep_color: vec4<f32>,
    water_shallow_color: vec4<f32>,
    // x: height_scale, y: frequency, z: octaves, w: algorithm (0=fBm, 1=Ridged, 2=Billow, 3=Worley, 4=Warp, 5=Hybrid)
    terrain_params1: vec4<f32>,
    // x: sharpness, y: ridge_intensity, z: valley_depth, w: domain_warp
    terrain_params2: vec4<f32>,
    // x: retro_90s_cgi (1.0 or 0.0), y: bloom, z: contrast, w: saturation
    postprocess_params: vec4<f32>,
    // x: time, y: selected_material, z: seed, w: render_mode (0=Full, 1=Reflections, 2=Normals)
    time_and_flags: vec4<f32>,
    // x: width, y: height, z: sample_index, w: num_active_primitives
    resolution: vec4<f32>,
    // x: vignette_strength, y: gamma, z: dither_16bit (1.0 or 0.0), w: surface_is_srgb (1.0 or 0.0)
    post_params2: vec4<f32>,
    // x: max_bounces, y: soft_shadows (0/1), z: sun_angular_radius_deg, w: blurry_reflections (0/1)
    render_params: vec4<f32>,
    // x: ground_enabled, y: ground_height, z: ground_material_id, w: water_enabled
    ground_params: vec4<f32>,
    // x: active light count, y: backdrop mountains enabled (1/0)
    light_params: vec4<f32>,
    // x: chromatic_aberration (display only)
    display_params: vec4<f32>,
};

struct GpuLight {
    // xyz: position, w: kind (0: Point, 1: Spot, 2: Parallel)
    pos_and_kind: vec4<f32>,
    // xyz: normalized direction, w: range
    dir_and_range: vec4<f32>,
    // rgb: color, w: intensity
    color_intensity: vec4<f32>,
    // x: cos(outer), y: cos(inner), z: cast_shadows
    spot_params: vec4<f32>,
};

struct GpuMaterial {
    base_color: vec4<f32>,
    secondary_color: vec4<f32>,
    // x: roughness, y: metallic, z: reflectivity, w: transparency
    params1: vec4<f32>,
    // x: ior, y: bump_strength, z: noise_scale, w: noise_type (0=flat, 1=fbm, 2=ridged, 3=marble, 4=worley)
    params2: vec4<f32>,
    // x: distortion/warp, y: octaves, z: blend_sharpness, w: emission
    params3: vec4<f32>,
};

struct GpuPrimitive {
    // x, y, z: position, w: primitive_type (0: Sphere, 1: Cube, 2: Cylinder, 3: Torus, 4: Cone, 5: Pyramid, 6: Mesh)
    pos_and_type: vec4<f32>,
    // x, y, z: scale/dimensions, w: material_id
    scale_and_mat: vec4<f32>,
    // x, y, z: rotation (deg), w: csg_op (0: Positive, 1: Negative, 2: Intersecting)
    rot_and_csg: vec4<f32>,
    // x: visible (1.0 or 0.0), y: tri_offset, z: tri_count, w: padding
    extra: vec4<f32>,
};

struct GpuTriangle {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    normal: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> scene: SceneUniform;
@group(0) @binding(2) var<uniform> materials: array<GpuMaterial, 16>;
@group(0) @binding(3) var<uniform> primitives: array<GpuPrimitive, 32>;
@group(0) @binding(4) var<storage, read> mesh_triangles: array<GpuTriangle>;
@group(0) @binding(5) var<uniform> lights: array<GpuLight, 8>;
// All terrain-object heightmaps concatenated (normalized [0,1], row-major res x res each)
@group(0) @binding(6) var<storage, read> terrain_heights: array<f32>;
// Progressive accumulation buffer (linear HDR), read via textureLoad
@group(1) @binding(0) var accum_tex: texture_2d<f32>;

// Full-screen triangle vertex shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(in_vertex_index == 1u) * 4 - 1);
    let y = f32(i32(in_vertex_index == 2u) * 4 - 1);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// =============================================================================
// PROCEDURAL 3D NOISE & FRACTAL SUITE
// =============================================================================

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var q = vec3<f32>(
        dot(p, vec3<f32>(127.1, 311.7, 74.7)),
        dot(p, vec3<f32>(269.5, 183.3, 246.1)),
        dot(p, vec3<f32>(113.5, 271.9, 124.6))
    );
    return fract(sin(q) * 43758.5453123);
}

fn hash13(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    let d = dot(p3, p3.zyx + vec3<f32>(31.32));
    p3 = p3 + vec3<f32>(d);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Quintic Hermite interpolant for smooth C2 continuous derivatives (eliminates bump noise/creases)
    let u = f * f * f * (f * (f * 6.0 - vec3<f32>(15.0)) + vec3<f32>(10.0));

    let n000 = hash13(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);

    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

// 1. Fractional Brownian Motion (fBm)
fn fbm_3d(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * noise3d(pos);
        pos = pos * 2.02 + vec3<f32>(1.2, 3.4, 5.6);
        amplitude = amplitude * 0.5;
    }
    return value;
}

// 2. Ken Musgrave's Ridged Multifractal
fn ridged_mf_3d(p: vec3<f32>, octaves: i32, ridge_mult: f32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    var weight = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i = i + 1) {
        var signal = noise3d(pos * freq);
        signal = 1.0 - abs(signal * 2.0 - 1.0);
        signal = signal * signal * ridge_mult;
        signal = signal * weight;
        weight = clamp(signal * 2.0, 0.0, 1.0);
        sum = sum + signal * amp;
        freq = freq * 2.05;
        amp = amp * 0.5;
    }
    return sum;
}

// 3. Billow Noise (rounded rolling hills)
fn billow_3d(p: vec3<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        let n = abs(noise3d(pos) * 2.0 - 1.0);
        sum = sum + amp * n;
        pos = pos * 2.02 + vec3<f32>(2.1, 1.4, 3.2);
        amp = amp * 0.5;
    }
    return sum;
}

// 4. Worley / Cellular noise (cracked earth / rock spires)
fn worley_3d(p: vec3<f32>) -> f32 {
    let pi = floor(p);
    let pf = fract(p);
    var min_dist = 1.0;

    for (var z = -1; z <= 1; z = z + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            for (var x = -1; x <= 1; x = x + 1) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let point = hash33(pi + neighbor);
                let diff = neighbor + point - pf;
                let dist = length(diff);
                min_dist = min(min_dist, dist);
            }
        }
    }
    return min_dist;
}

// 5. Domain-Warped Turbulence (Marble & alien terrain)
fn domain_warp_3d(p: vec3<f32>, warp: f32, octaves: i32) -> f32 {
    let q = vec3<f32>(
        fbm_3d(p + vec3<f32>(0.0, 0.0, 0.0), octaves),
        fbm_3d(p + vec3<f32>(5.2, 1.3, 2.8), octaves),
        fbm_3d(p + vec3<f32>(1.8, 9.2, 4.3), octaves)
    );
    let warped_pos = p + q * warp;
    let val = sin(warped_pos.x * 3.0 + warped_pos.y * 2.0 + fbm_3d(warped_pos, 3) * 6.0);
    return val * 0.5 + 0.5;
}

// 6. Hybrid Multifractal
fn hybrid_mf_3d(p: vec3<f32>, octaves: i32) -> f32 {
    let r = ridged_mf_3d(p, octaves, 1.2);
    let f = fbm_3d(p * 1.5, octaves);
    return r * 0.65 + f * 0.35;
}

// =============================================================================
// PROCEDURAL MATERIAL EVALUATOR (DEEP TEXTURE EDITOR)
// =============================================================================

struct EvaluatedSurface {
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    reflectivity: f32,
    transparency: f32,
    ior: f32,
    normal: vec3<f32>,
    emission: vec3<f32>,
};

fn evaluate_material(
    mat: GpuMaterial,
    pos: vec3<f32>,
    geom_normal: vec3<f32>,
    hit_dist: f32
) -> EvaluatedSurface {
    let roughness = mat.params1.x;
    let metallic = mat.params1.y;
    let reflectivity = mat.params1.z;
    let transparency = mat.params1.w;

    let ior = mat.params2.x;
    let bump_strength = mat.params2.y;
    let noise_scale = mat.params2.z;
    let noise_type = u32(mat.params2.w + 0.5);

    let warp = mat.params3.x;
    let octaves = i32(mat.params3.y + 0.5);
    let blend_sharpness = mat.params3.z;
    let emission_val = mat.params3.w;

    let sample_p = pos * noise_scale;

    var pattern_val = 0.5;
    if (noise_type == 1u) {
        pattern_val = fbm_3d(sample_p, octaves);
    } else if (noise_type == 2u) {
        pattern_val = ridged_mf_3d(sample_p, octaves, 1.5);
    } else if (noise_type == 3u) {
        pattern_val = domain_warp_3d(sample_p, warp, octaves);
    } else if (noise_type == 4u) {
        pattern_val = 1.0 - worley_3d(sample_p);
    }

    pattern_val = pow(clamp(pattern_val, 0.0, 1.0), blend_sharpness);
    let mixed_color = mix(mat.base_color.rgb, mat.secondary_color.rgb, pattern_val);

    // Procedural normal perturbation
    var perturbed_normal = geom_normal;
    if (bump_strength > 0.001) {
        let eps = 0.035;
        var p_dx = pattern_val;
        var p_dy = pattern_val;
        var p_dz = pattern_val;

        if (noise_type == 1u) {
            p_dx = fbm_3d(sample_p + vec3<f32>(eps, 0.0, 0.0), octaves);
            p_dy = fbm_3d(sample_p + vec3<f32>(0.0, eps, 0.0), octaves);
            p_dz = fbm_3d(sample_p + vec3<f32>(0.0, 0.0, eps), octaves);
        } else if (noise_type == 2u) {
            p_dx = ridged_mf_3d(sample_p + vec3<f32>(eps, 0.0, 0.0), octaves, 1.5);
            p_dy = ridged_mf_3d(sample_p + vec3<f32>(0.0, eps, 0.0), octaves, 1.5);
            p_dz = ridged_mf_3d(sample_p + vec3<f32>(0.0, 0.0, eps), octaves, 1.5);
        } else if (noise_type == 3u) {
            p_dx = domain_warp_3d(sample_p + vec3<f32>(eps, 0.0, 0.0), warp, octaves);
            p_dy = domain_warp_3d(sample_p + vec3<f32>(0.0, eps, 0.0), warp, octaves);
            p_dz = domain_warp_3d(sample_p + vec3<f32>(0.0, 0.0, eps), warp, octaves);
        } else if (noise_type == 4u) {
            p_dx = 1.0 - worley_3d(sample_p + vec3<f32>(eps, 0.0, 0.0));
            p_dy = 1.0 - worley_3d(sample_p + vec3<f32>(0.0, eps, 0.0));
            p_dz = 1.0 - worley_3d(sample_p + vec3<f32>(0.0, 0.0, eps));
        }

        let grad = vec3<f32>(p_dx - pattern_val, p_dy - pattern_val, p_dz - pattern_val) / eps;
        let bump_factor = bump_strength * 0.12 / (1.0 + hit_dist * 0.04);
        perturbed_normal = normalize(geom_normal - grad * bump_factor);
    }

    var surf: EvaluatedSurface;
    surf.albedo = mixed_color;
    surf.roughness = clamp(roughness, 0.01, 1.0);
    surf.metallic = clamp(metallic, 0.0, 1.0);
    surf.reflectivity = clamp(reflectivity, 0.0, 1.0);
    surf.transparency = clamp(transparency, 0.0, 1.0);
    surf.ior = max(ior, 1.0);
    surf.normal = perturbed_normal;
    surf.emission = mat.secondary_color.rgb * (emission_val * pattern_val);

    return surf;
}

// =============================================================================
// SKY & ATMOSPHERE LAB
// =============================================================================

fn render_sky(ray_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let sun_dot = max(dot(ray_dir, sun_dir), 0.0);
    let horizon_y = ray_dir.y;

    let zenith = scene.zenith_color.rgb;
    let horizon = scene.horizon_color.rgb;
    let ground = scene.ground_color.rgb;

    var sky_base: vec3<f32>;
    if (horizon_y >= 0.0) {
        let t = pow(1.0 - horizon_y, 3.5);
        sky_base = mix(zenith, horizon, t);
    } else {
        sky_base = mix(horizon, ground, clamp(-horizon_y * 4.0, 0.0, 1.0));
    }

    // Solar Mie corona flare & glow
    let mie_glow = pow(sun_dot, 18.0) * 0.55 * scene.fog_params.z;
    let mie_flare = pow(sun_dot, 280.0) * 1.8;

    // Sun disc
    var sun_disc = 0.0;
    if (sun_dot > 0.9991) {
        sun_disc = 4.0;
    }

    let sun_light = scene.sun_color_intensity.rgb * scene.sun_color_intensity.w;
    var final_sky = sky_base + sun_light * (mie_glow + mie_flare + sun_disc);

    // Procedural Stars (when sun drops near or below horizon)
    if (sun_dir.y < 0.2 && ray_dir.y > 0.05) {
        let star_p = ray_dir * 180.0;
        let star_hash = hash13(floor(star_p));
        if (star_hash > 0.994) {
            let star_blink = sin(scene.time_and_flags.x * 2.0 + star_hash * 60.0) * 0.5 + 0.5;
            let star_lum = pow((star_hash - 0.994) / 0.006, 4.0) * star_blink * 2.5;
            let day_fade = clamp((0.2 - sun_dir.y) * 5.0, 0.0, 1.0);
            final_sky = final_sky + vec3<f32>(star_lum * day_fade);
        }
    }

    // Procedural Cloud Plane (Infinite ceiling at Y = 65.0)
    if (ray_dir.y > 0.02 && scene.fog_params.w > 0.05) {
        let cloud_plane_y = 65.0;
        let t_cloud = cloud_plane_y / ray_dir.y;
        let cloud_pos = camera.pos.xyz + ray_dir * t_cloud;

        let wind = vec2<f32>(scene.time_and_flags.x * 0.12, scene.time_and_flags.x * 0.04);
        let cloud_sample = (cloud_pos.xz + wind) * 0.014;
        let cloud_fbm = fbm_3d(vec3<f32>(cloud_sample.x, 1.0, cloud_sample.y), 4);

        let coverage = scene.fog_params.w;
        let cloud_mask = smoothstep(0.40, 0.75, cloud_fbm * coverage);
        let cloud_lit = mix(vec3<f32>(0.92, 0.94, 0.98), horizon, pow(sun_dot, 3.0) * 0.7);
        final_sky = mix(final_sky, cloud_lit, cloud_mask * 0.88);
    }

    return final_sky;
}

// =============================================================================
// TERRAIN & PROCEDURAL HEIGHTFIELD EVALUATION
// =============================================================================

fn terrain_height(xz: vec2<f32>) -> f32 {
    let freq = scene.terrain_params1.y;
    let octaves = i32(scene.terrain_params1.z + 0.5);
    let algo = u32(scene.terrain_params1.w + 0.5);

    let seed_offset = scene.time_and_flags.z * 12.34;
    let p = vec3<f32>((xz.x + seed_offset) * freq, 0.0, (xz.y + seed_offset) * freq);

    var base_val = 0.0;
    if (algo == 0u) {
        base_val = fbm_3d(p, octaves);
    } else if (algo == 1u) {
        base_val = ridged_mf_3d(p, octaves, scene.terrain_params2.y);
    } else if (algo == 2u) {
        base_val = billow_3d(p, octaves);
    } else if (algo == 3u) {
        base_val = 1.0 - worley_3d(p * 1.5);
    } else if (algo == 4u) {
        base_val = domain_warp_3d(p, scene.terrain_params2.w, octaves);
    } else {
        base_val = hybrid_mf_3d(p, octaves);
    }

    let mountain = base_val * (12.0 * scene.terrain_params1.x);

    // Lagoon clearing in front of camera
    let dist_from_lagoon = length(xz - vec2<f32>(0.0, -18.0));
    let mask = smoothstep(8.0, 38.0, dist_from_lagoon);

    // Clearing floor sits well below the water plane so the two never coincide
    return mix(-4.0, mountain - 2.0, mask);
}

fn terrain_normal(xz: vec2<f32>) -> vec3<f32> {
    let eps = 0.10;
    let h_l = terrain_height(xz - vec2<f32>(eps, 0.0));
    let h_r = terrain_height(xz + vec2<f32>(eps, 0.0));
    let h_d = terrain_height(xz - vec2<f32>(0.0, eps));
    let h_u = terrain_height(xz + vec2<f32>(0.0, eps));
    return normalize(vec3<f32>(h_l - h_r, 2.0 * eps, h_d - h_u));
}


// =============================================================================
// TERRAIN OBJECTS: sculpted heightmaps (local space: x,z in [-1,1], y in [0,1])
// =============================================================================
fn tmap_texel(off: u32, res: u32, ix: u32, iz: u32) -> f32 {
    return terrain_heights[off + iz * res + ix];
}

// Bilinear height at normalized footprint coords uv in [0,1]
fn tmap_sample(off: u32, res: u32, uv: vec2<f32>) -> f32 {
    let f = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * f32(res - 1u);
    let x0 = min(u32(floor(f.x)), res - 2u);
    let z0 = min(u32(floor(f.y)), res - 2u);
    let tx = f.x - f32(x0);
    let tz = f.y - f32(z0);
    let h00 = tmap_texel(off, res, x0, z0);
    let h10 = tmap_texel(off, res, x0 + 1u, z0);
    let h01 = tmap_texel(off, res, x0, z0 + 1u);
    let h11 = tmap_texel(off, res, x0 + 1u, z0 + 1u);
    return mix(mix(h00, h10, tx), mix(h01, h11, tx), tz);
}

// Local-space height above the heightfield at local position q (q.y is height, 0..1)
fn tmap_above(off: u32, res: u32, q: vec3<f32>) -> f32 {
    return q.y - tmap_sample(off, res, q.xz * 0.5 + vec2<f32>(0.5));
}

// =============================================================================
// SCENE GEOMETRY: WATER, TERRAIN & PRIMITIVES
// =============================================================================

fn rotate_x(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(v.x, c * v.y - s * v.z, s * v.y + c * v.z);
}

fn rotate_y(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

fn rotate_z(v: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * v.x - s * v.y, s * v.x + c * v.y, v.z);
}

struct HitInfo {
    hit: bool,
    t: f32,
    pos: vec3<f32>,
    normal: vec3<f32>,
    mat_id: u32,
    is_water: bool,
};

fn intersect_scene(ro: vec3<f32>, rd: vec3<f32>) -> HitInfo {
    return intersect_scene_ex(ro, rd, 10.0, true);
}

// terrain_t0: where the terrain raymarch starts (primary rays skip the near field);
// use_water: false for shadow rays, which ignore the water plane.
fn intersect_scene_ex(ro: vec3<f32>, rd: vec3<f32>, terrain_t0: f32, use_water: bool) -> HitInfo {
    var closest: HitInfo;
    closest.hit = false;
    closest.t = 1e6;
    closest.mat_id = 0u;
    closest.is_water = false;

    // 0. Infinite ground plane (seabed when below the water)
    if (scene.ground_params.x > 0.5 && abs(rd.y) > 0.0001) {
        let t_ground = (scene.ground_params.y - ro.y) / rd.y;
        if (t_ground > 0.05 && t_ground < closest.t) {
            closest.hit = true;
            closest.t = t_ground;
            closest.pos = ro + rd * t_ground;
            closest.normal = vec3<f32>(0.0, 1.0, 0.0);
            closest.mat_id = min(u32(scene.ground_params.z + 0.5), 15u);
            closest.is_water = false;
        }
    }

    // 1. Water Plane (Y = water_params1.x)
    let water_y = scene.water_params1.x;
    if (use_water && scene.ground_params.w > 0.5 && abs(rd.y) > 0.0001) {
        let t_water = (water_y - ro.y) / rd.y;
        if (t_water > 0.05 && t_water < closest.t) {
            closest.hit = true;
            closest.t = t_water;
            closest.pos = ro + rd * t_water;

            let wave_time = scene.time_and_flags.x * scene.water_params1.w;
            let wp = closest.pos.xz * scene.water_params1.z;
            let amp = scene.water_params1.y;

            let w1 = sin(wp.x * 1.2 + wave_time) * cos(wp.y * 1.5 + wave_time * 0.8);
            let w2 = sin(wp.x * 3.1 - wave_time * 1.3) * sin(wp.y * 2.8 + wave_time * 1.1) * 0.4;
            let ripple_dx = (w1 + w2) * amp * 0.5;
            let ripple_dz = (cos(wp.x * 1.4 + wave_time) + cos(wp.y * 2.2 - wave_time)) * amp * 0.4;

            // Fade ripple amplitude with distance to suppress far-field sparkle
            let ripple_fade = 1.0 / (1.0 + t_water * 0.04);
            closest.normal = normalize(vec3<f32>(-ripple_dx * ripple_fade, 1.0, -ripple_dz * ripple_fade));
            closest.mat_id = 6u; // Ocean Water
            closest.is_water = true;
        }
    }

    // 2. Dynamic Primitives (Spheres, Cubes, Cylinders, Toruses, Meshes)
    let num_prims = u32(scene.resolution.w + 0.5);
    for (var i = 0u; i < 32u; i = i + 1u) {
        if (i >= num_prims) { break; }
        let prim = primitives[i];
        if (prim.extra.x < 0.5) { continue; } // not visible
        let csg_op = u32(prim.rot_and_csg.w + 0.5);
        if (csg_op == 1u) { continue; } // skip negative cutters in positive pass

        let prim_type = u32(prim.pos_and_type.w + 0.5);
        let center = prim.pos_and_type.xyz;
        let scale = prim.scale_and_mat.xyz;
        let mat_id = u32(prim.scale_and_mat.w + 0.5);

        // Primitive Rotations (Pitch, Yaw, Roll in degrees)
        let rad_rot = prim.rot_and_csg.xyz * 0.0174532925; // to radians
        let ro_local = rotate_z(rotate_y(rotate_x(ro - center, -rad_rot.x), -rad_rot.y), -rad_rot.z);
        let rd_local = rotate_z(rotate_y(rotate_x(rd, -rad_rot.x), -rad_rot.y), -rad_rot.z);

        if (prim_type == 7u) {
            // TERRAIN OBJECT: march the heightmap in local space (x,z in [-1,1], y in [0,1])
            let t_off = u32(prim.extra.y + 0.5);
            let t_res = u32(prim.extra.z + 0.5);
            if (t_res < 2u) { continue; }
            let yaw = prim.rot_and_csg.y * 0.0174532925;
            let inv_s = vec3<f32>(1.0) / max(scale, vec3<f32>(0.0001));
            let o_l = rotate_y(ro - center, -yaw) * inv_s;
            let d_l = rotate_y(rd, -yaw) * inv_s;

            // Clip against local AABB [-1,1] x [0,1] x [-1,1]
            let sd = select(d_l, sign(d_l) * 0.00001, abs(d_l) < vec3<f32>(0.00001));
            let sd2 = select(sd, vec3<f32>(0.00001), sd == vec3<f32>(0.0));
            let inv_d = 1.0 / sd2;
            let b_min = vec3<f32>(-1.0, 0.0, -1.0);
            let b_max = vec3<f32>(1.0, 1.0, 1.0);
            let ta = (b_min - o_l) * inv_d;
            let tb = (b_max - o_l) * inv_d;
            let t_enter = max(max(max(min(ta.x, tb.x), min(ta.y, tb.y)), min(ta.z, tb.z)), 0.05);
            let t_exit = min(min(max(ta.x, tb.x), max(ta.y, tb.y)), max(ta.z, tb.z));
            if (t_exit <= t_enter || t_enter >= closest.t) { continue; }
            let t_end = min(t_exit, closest.t);

            let cell = 2.0 / f32(t_res - 1u);
            let xz_speed = max(length(d_l.xz), 0.0001);
            let min_dt = 0.5 * cell / xz_speed;
            var tt = t_enter;
            var f_prev = tmap_above(t_off, t_res, o_l + d_l * tt);
            var t_prev = tt;
            var found = false;
            var t_hit = 0.0;
            for (var k = 0; k < 400; k = k + 1) {
                if (tt >= t_end) { break; }
                // Adaptive step: bigger while high above the surface
                tt = min(tt + max(min_dt, max(f_prev, 0.0) * 0.35 / xz_speed), t_end);
                let f = tmap_above(t_off, t_res, o_l + d_l * tt);
                if (f < 0.0 && f_prev >= 0.0) {
                    var lo = t_prev;
                    var hi = tt;
                    for (var j = 0; j < 8; j = j + 1) {
                        let mid = 0.5 * (lo + hi);
                        if (tmap_above(t_off, t_res, o_l + d_l * mid) < 0.0) { hi = mid; } else { lo = mid; }
                    }
                    t_hit = hi;
                    found = true;
                    break;
                }
                t_prev = tt;
                f_prev = f;
            }

            if (found && t_hit > 0.05 && t_hit < closest.t) {
                let q = o_l + d_l * t_hit;
                let uv = q.xz * 0.5 + vec2<f32>(0.5);
                let e = 1.5 * cell * 0.5; // 1.5 cells in uv units
                let h_l = tmap_sample(t_off, t_res, uv - vec2<f32>(e, 0.0));
                let h_r = tmap_sample(t_off, t_res, uv + vec2<f32>(e, 0.0));
                let h_d = tmap_sample(t_off, t_res, uv - vec2<f32>(0.0, e));
                let h_u = tmap_sample(t_off, t_res, uv + vec2<f32>(0.0, e));
                // Gradient per local unit (uv spans 0.5 local units per uv unit)
                let dh_dx = (h_r - h_l) / (2.0 * e) * 0.5;
                let dh_dz = (h_u - h_d) / (2.0 * e) * 0.5;
                // Local x = world_x / sx, y_world = sy * h  =>  d(y_world)/d(x_world) = (sy/sx) * dh/dx_l
                var n_obj = normalize(vec3<f32>(-dh_dx * scale.y / scale.x, 1.0, -dh_dz * scale.y / scale.z));
                var n_world = rotate_y(n_obj, yaw);
                if (dot(n_world, rd) > 0.0) { n_world = -n_world; }
                closest.hit = true;
                closest.t = t_hit;
                closest.pos = ro + rd * t_hit;
                closest.normal = n_world;
                closest.mat_id = mat_id;
                closest.is_water = false;
            }
            continue;
        }

        if (prim_type == 0u) {
            // SPHERE
            let oc = ro - center;
            let rad = scale.x;
            let b = dot(oc, rd);
            let c = dot(oc, oc) - rad * rad;
            let disc = b * b - c;
            if (disc > 0.0) {
                let sq = sqrt(disc);
                // Near root, or the far root when the ray starts inside the sphere
                let t_sph = select(-b + sq, -b - sq, -b - sq > 0.05);
                if (t_sph > 0.05 && t_sph < closest.t) {
                    closest.hit = true;
                    closest.t = t_sph;
                    closest.pos = ro + rd * t_sph;
                    closest.normal = normalize(closest.pos - center);
                    closest.mat_id = mat_id;
                    closest.is_water = false;
                }
            }
        } else if (prim_type == 1u) {
            // CUBE / BOX (Rotated)
            let box_min = -scale;
            let box_max = scale;
            let inv_d = 1.0 / rd_local;
            let t0s = (box_min - ro_local) * inv_d;
            let t1s = (box_max - ro_local) * inv_d;
            let t_min = max(max(min(t0s.x, t1s.x), min(t0s.y, t1s.y)), min(t0s.z, t1s.z));
            let t_max = min(min(max(t0s.x, t1s.x), max(t0s.y, t1s.y)), max(t0s.z, t1s.z));

            // Entry hit, or the exit face when the ray starts inside the box
            let t_cube = select(t_max, t_min, t_min > 0.05);
            if (t_max >= t_min && t_cube > 0.05 && t_cube < closest.t) {
                closest.hit = true;
                closest.t = t_cube;
                closest.pos = ro + rd * t_cube;

                let p_local = ro_local + rd_local * t_cube;
                var n_local = vec3<f32>(0.0, 1.0, 0.0);
                if (abs(p_local.x) > scale.x * 0.99) { n_local = vec3<f32>(sign(p_local.x), 0.0, 0.0); }
                else if (abs(p_local.y) > scale.y * 0.99) { n_local = vec3<f32>(0.0, sign(p_local.y), 0.0); }
                else if (abs(p_local.z) > scale.z * 0.99) { n_local = vec3<f32>(0.0, 0.0, sign(p_local.z)); }

                closest.normal = normalize(rotate_x(rotate_y(rotate_z(n_local, rad_rot.z), rad_rot.y), rad_rot.x));
                closest.mat_id = mat_id;
                closest.is_water = false;
            }
        } else if (prim_type == 2u) {
            // CYLINDER (Along Y axis, Rotated)
            let rad = scale.x;
            let half_h = scale.y;
            let a = rd_local.x * rd_local.x + rd_local.z * rd_local.z;
            let b = dot(ro_local.xz, rd_local.xz);
            let c = dot(ro_local.xz, ro_local.xz) - rad * rad;
            let disc = b * b - a * c;

            let inside_cyl = c < 0.0 && abs(ro_local.y) < half_h;
            if (inside_cyl) {
                // Ray starts inside: find the exit through the side wall or a cap
                var t_exit = 1e6;
                var n_local = vec3<f32>(0.0, 1.0, 0.0);
                if (a > 0.0001) {
                    t_exit = (-b + sqrt(max(disc, 0.0))) / a;
                    let p_xz = ro_local.xz + rd_local.xz * t_exit;
                    n_local = normalize(vec3<f32>(p_xz.x, 0.0, p_xz.y));
                }
                if (abs(rd_local.y) > 0.0001) {
                    let cap_y = select(-half_h, half_h, rd_local.y > 0.0);
                    let t_cap = (cap_y - ro_local.y) / rd_local.y;
                    if (t_cap < t_exit) {
                        t_exit = t_cap;
                        n_local = vec3<f32>(0.0, sign(cap_y), 0.0);
                    }
                }
                if (t_exit > 0.05 && t_exit < closest.t) {
                    closest.hit = true;
                    closest.t = t_exit;
                    closest.pos = ro + rd * t_exit;
                    closest.normal = normalize(rotate_x(rotate_y(rotate_z(n_local, rad_rot.z), rad_rot.y), rad_rot.x));
                    closest.mat_id = mat_id;
                    closest.is_water = false;
                }
            } else if (disc > 0.0 && a > 0.0001) {
                let sqrt_disc = sqrt(disc);
                let t_side = (-b - sqrt_disc) / a;
                let y_hit = ro_local.y + rd_local.y * t_side;

                if (t_side > 0.05 && abs(y_hit) <= half_h && t_side < closest.t) {
                    closest.hit = true;
                    closest.t = t_side;
                    closest.pos = ro + rd * t_side;
                    let n_local = normalize(vec3<f32>(ro_local.x + rd_local.x * t_side, 0.0, ro_local.z + rd_local.z * t_side));
                    closest.normal = normalize(rotate_x(rotate_y(rotate_z(n_local, rad_rot.z), rad_rot.y), rad_rot.x));
                    closest.mat_id = mat_id;
                    closest.is_water = false;
                } else if (abs(rd_local.y) > 0.0001) {
                    let cap_y = select(-half_h, half_h, rd_local.y < 0.0);
                    let t_cap = (cap_y - ro_local.y) / rd_local.y;
                    if (t_cap > 0.05 && t_cap < closest.t) {
                        let cap_hit_xz = ro_local.xz + rd_local.xz * t_cap;
                        if (dot(cap_hit_xz, cap_hit_xz) <= rad * rad) {
                            closest.hit = true;
                            closest.t = t_cap;
                            closest.pos = ro + rd * t_cap;
                            let n_local = vec3<f32>(0.0, sign(cap_y), 0.0);
                            closest.normal = normalize(rotate_x(rotate_y(rotate_z(n_local, rad_rot.z), rad_rot.y), rad_rot.x));
                            closest.mat_id = mat_id;
                            closest.is_water = false;
                        }
                    }
                }
            }
        } else if (prim_type == 3u) {
            // TORUS (Raymarching torus bounding approximation, Rotated)
            let major_r = scale.x;
            let tube_r = scale.y;
            let bound_r = major_r + tube_r;

            let b = dot(ro_local, rd_local);
            let c = dot(ro_local, ro_local) - bound_r * bound_r;
            if (b * b - c > 0.0) {
                var t_tor = max(0.05, -b - sqrt(b * b - c));
                for (var step = 0; step < 24; step = step + 1) {
                    if (t_tor >= closest.t) { break; }
                    let p = ro_local + rd_local * t_tor;
                    let q = vec2<f32>(length(p.xz) - major_r, p.y);
                    let dist = length(q) - tube_r;
                    if (dist < 0.04) {
                        closest.hit = true;
                        closest.t = t_tor;
                        closest.pos = ro + rd * t_tor;
                        let n_xz = normalize(p.xz) * major_r;
                        let n_local = normalize(p - vec3<f32>(n_xz.x, 0.0, n_xz.y));
                        closest.normal = normalize(rotate_x(rotate_y(rotate_z(n_local, rad_rot.z), rad_rot.y), rad_rot.x));
                        closest.mat_id = mat_id;
                        closest.is_water = false;
                        break;
                    }
                    t_tor = t_tor + dist * 0.6;
                }
            }
        } else if (prim_type == 6u) {
            // MESH / 3D MODEL (AABB Bounding Box culling + Möller–Trumbore ray-triangle intersection)
            let box_min = -scale;
            let box_max = scale;
            let safe_rd = select(rd_local, sign(rd_local) * 0.00001, abs(rd_local) < vec3<f32>(0.00001));
            let inv_d = 1.0 / safe_rd;
            let t0s = (box_min - ro_local) * inv_d;
            let t1s = (box_max - ro_local) * inv_d;
            let t_min = max(max(min(t0s.x, t1s.x), min(t0s.y, t1s.y)), min(t0s.z, t1s.z));
            let t_max = min(min(max(t0s.x, t1s.x), max(t0s.y, t1s.y)), max(t0s.z, t1s.z));

            if (t_max >= max(0.05, t_min) && t_min < closest.t) {
                let tri_start = u32(prim.extra.y + 0.5);
                let tri_count = min(u32(prim.extra.z + 0.5), 3500u);

                for (var t_idx = 0u; t_idx < tri_count; t_idx = t_idx + 1u) {
                    let tri = mesh_triangles[tri_start + t_idx];
                    let v0 = tri.v0.xyz * scale;
                    let v1 = tri.v1.xyz * scale;
                    let v2 = tri.v2.xyz * scale;

                    let edge1 = v1 - v0;
                    let edge2 = v2 - v0;
                    let pvec = cross(rd_local, edge2);
                    let det = dot(edge1, pvec);

                    if (abs(det) > 0.00001) {
                        let inv_det = 1.0 / det;
                        let tvec = ro_local - v0;
                        let u = dot(tvec, pvec) * inv_det;
                        if (u >= 0.0 && u <= 1.0) {
                            let qvec = cross(tvec, edge1);
                            let v = dot(rd_local, qvec) * inv_det;
                            if (v >= 0.0 && (u + v) <= 1.0) {
                                let t_hit = dot(edge2, qvec) * inv_det;
                                if (t_hit > 0.05 && t_hit < closest.t) {
                                    closest.hit = true;
                                    closest.t = t_hit;
                                    closest.pos = ro + rd * t_hit;
                                    var n_world = normalize(rotate_x(rotate_y(rotate_z(tri.normal.xyz, rad_rot.z), rad_rot.y), rad_rot.x));
                                    if (dot(n_world, rd) > 0.0) {
                                        n_world = -n_world;
                                    }
                                    closest.normal = n_world;
                                    closest.mat_id = mat_id;
                                    closest.is_water = false;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Subtractive CSG Pass (Negative cutouts punch holes into solids)
    if (closest.hit && !closest.is_water) {
        for (var i = 0u; i < 32u; i = i + 1u) {
            if (i >= num_prims) { break; }
            let prim = primitives[i];
            if (prim.extra.x < 0.5) { continue; }
            let csg_op = u32(prim.rot_and_csg.w + 0.5);
            if (csg_op != 1u) { continue; } // Negative cutout only

            let center = prim.pos_and_type.xyz;
            let scale = prim.scale_and_mat.xyz;
            let cut_mat = u32(prim.scale_and_mat.w + 0.5);
            let prim_type = u32(prim.pos_and_type.w + 0.5);

            if (prim_type == 0u) {
                // Negative Sphere Cutout
                let dist_to_center = length(closest.pos - center);
                if (dist_to_center < scale.x) {
                    let oc = ro - center;
                    let b = dot(oc, rd);
                    let c = dot(oc, oc) - scale.x * scale.x;
                    let disc = b * b - c;
                    if (disc > 0.0) {
                        let t_exit = -b + sqrt(disc);
                        if (t_exit > closest.t) {
                            closest.t = t_exit;
                            closest.pos = ro + rd * t_exit;
                            closest.normal = normalize(center - closest.pos);
                            closest.mat_id = cut_mat;
                        }
                    }
                }
            } else if (prim_type == 1u) {
                // Negative Cube Cutout
                let p_rel = abs(closest.pos - center);
                if (p_rel.x < scale.x && p_rel.y < scale.y && p_rel.z < scale.z) {
                    let inv_d = 1.0 / rd;
                    let t0s = (center - scale - ro) * inv_d;
                    let t1s = (center + scale - ro) * inv_d;
                    let t_max = min(min(max(t0s.x, t1s.x), max(t0s.y, t1s.y)), max(t0s.z, t1s.z));
                    if (t_max > closest.t) {
                        closest.t = t_max;
                        closest.pos = ro + rd * t_max;
                        let p_exit = closest.pos - center;
                        var n_cut = vec3<f32>(0.0, -1.0, 0.0);
                        if (abs(p_exit.x) > scale.x * 0.99) { n_cut = vec3<f32>(-sign(p_exit.x), 0.0, 0.0); }
                        else if (abs(p_exit.y) > scale.y * 0.99) { n_cut = vec3<f32>(0.0, -sign(p_exit.y), 0.0); }
                        else if (abs(p_exit.z) > scale.z * 0.99) { n_cut = vec3<f32>(0.0, 0.0, -sign(p_exit.z)); }
                        closest.normal = n_cut;
                        closest.mat_id = cut_mat;
                    }
                }
            }
        }
    }

    // 3. Raymarching Backdrop Fractal Mountain
    var tm = terrain_t0;
    var t_prev = tm;
    let tm_max = select(-1.0, 120.0, scene.light_params.y > 0.5); // y: backdrop mountains enabled
    for (var i = 0; i < 75; i = i + 1) {
        if (tm > tm_max || tm >= closest.t) { break; }
        let p = ro + rd * tm;
        let dist_to_surface = p.y - terrain_height(p.xz);
        if (dist_to_surface < 0.0) {
            // Crossed the surface: bisect between last outside sample and this one
            var lo = t_prev;
            var hi = tm;
            for (var j = 0; j < 8; j = j + 1) {
                let mid = 0.5 * (lo + hi);
                let pm = ro + rd * mid;
                if (pm.y - terrain_height(pm.xz) < 0.0) { hi = mid; } else { lo = mid; }
            }
            if (hi < closest.t) {
                closest.hit = true;
                closest.t = hi;
                closest.pos = ro + rd * hi;
                closest.normal = terrain_normal(closest.pos.xz);
                closest.mat_id = 0u; // Mountain Rock
                closest.is_water = false;
            }
            break;
        }
        t_prev = tm;
        tm = tm + max(dist_to_surface * 0.5, 0.35);
    }

    return closest;
}

// =============================================================================
// PATH TRACING CORE (iterative: WGSL has no recursion)
// =============================================================================

fn rnd(s: ptr<function, u32>) -> f32 {
    *s = pcg_hash(*s);
    return f32(*s >> 8u) / 16777216.0;
}

fn rand_unit_vec(s: ptr<function, u32>) -> vec3<f32> {
    let z = rnd(s) * 2.0 - 1.0;
    let a = rnd(s) * 6.2831853;
    let r = sqrt(max(1.0 - z * z, 0.0));
    return vec3<f32>(r * cos(a), r * sin(a), z);
}

fn schlick_scalar(f0: f32, c: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - clamp(c, 0.0, 1.0), 5.0);
}

// Light transmittance along a shadow ray. Opaque hit -> 0, transparent hits tint
// the light (sqrt(transparency * albedo) per interface, so entering + leaving a
// closed object gives transparency * albedo). The water plane is ignored.
// Reusable for point/spot lights by passing a ray towards the light.
fn shadow_transmittance(p: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    return shadow_transmittance_d(p, dir, 1.0e9);
}

// Same as above, but only occluders closer than max_dist count (point / spot lights)
fn shadow_transmittance_d(p: vec3<f32>, dir: vec3<f32>, max_dist: f32) -> vec3<f32> {
    var trans = vec3<f32>(1.0);
    var origin = p;
    var left = max_dist;
    for (var i = 0; i < 4; i = i + 1) {
        let h = intersect_scene_ex(origin, dir, 0.4, false);
        if (!h.hit || h.t > left) { break; }
        let m = materials[h.mat_id];
        let tr = clamp(m.params1.w, 0.0, 1.0);
        if (tr < 0.01) { return vec3<f32>(0.0); }
        trans = trans * sqrt(max(m.base_color.rgb, vec3<f32>(0.0)) * tr);
        if (max(trans.x, max(trans.y, trans.z)) < 0.01) { return vec3<f32>(0.0); }
        origin = h.pos + dir * 0.06;
        left = left - h.t - 0.06;
    }
    return trans;
}

// Sun direction for one shadow sample (jittered inside the solar disc when soft shadows are on)
fn sun_shadow_dir(sun_dir: vec3<f32>, s: ptr<function, u32>) -> vec3<f32> {
    if (scene.render_params.y < 0.5) { return sun_dir; }
    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(sun_dir.y) > 0.99);
    let t = normalize(cross(up, sun_dir));
    let b = cross(sun_dir, t);
    let r = sqrt(rnd(s));
    let a = rnd(s) * 6.2831853;
    let spread = tan(radians(scene.render_params.z));
    return normalize(sun_dir + (t * cos(a) + b * sin(a)) * (r * spread));
}

fn max3(v: vec3<f32>) -> f32 {
    return max(v.x, max(v.y, v.z));
}

fn trace_pixel(uv: vec2<f32>, seed: u32) -> vec3<f32> {
    var rng = seed;
    let tan_half_fov = camera.params.x;
    let aspect_ratio = camera.params.y;

    var rd = normalize(
        camera.forward.xyz +
        camera.right.xyz * (uv.x * tan_half_fov * aspect_ratio) +
        camera.up.xyz * (uv.y * tan_half_fov)
    );
    var ro = camera.pos.xyz;

    let sun_dir = normalize(scene.sun_direction.xyz);
    let sun_color = scene.sun_color_intensity.rgb * scene.sun_color_intensity.w;
    let render_mode = u32(scene.time_and_flags.w + 0.5);
    let max_bounces = clamp(i32(scene.render_params.x + 0.5), 1, 8);
    let blurry = scene.render_params.w > 0.5;
    let water_on = scene.ground_params.w > 0.5;
    let water_y = scene.water_params1.x;
    let fog_density = scene.fog_params.x;
    let fog_color = scene.fog_color.rgb;

    // Water absorption (Beer-Lambert towards the deep colour) and in-scattered light
    let water_deep = max(scene.water_deep_color.rgb, vec3<f32>(0.001));
    let water_sigma = -log(water_deep) * 0.25 * (1.0 - 0.7 * clamp(scene.water_params2.x, 0.0, 1.0));
    let water_scatter = mix(water_deep, scene.water_shallow_color.rgb, 0.35)
        * (sun_color * max(sun_dir.y, 0.0) * 0.3 + scene.ground_color.rgb * 0.35);

    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    var path_len = 0.0;
    var alive = false;

    for (var bounce = 0; bounce < 8; bounce = bounce + 1) {
        if (bounce >= max_bounces) { break; }
        alive = false;
        if (max3(throughput) < 0.004) { break; }

        let hit = intersect_scene_ex(ro, rd, select(0.1, 10.0, bounce == 0), true);
        let seg = select(200.0, hit.t, hit.hit);

        // Participating media along this segment: water below the surface, fog above it
        let mid_y = ro.y + rd.y * 0.5 * seg;
        if (water_on && mid_y < water_y) {
            let tr = exp(-water_sigma * seg);
            radiance = radiance + throughput * (vec3<f32>(1.0) - tr) * water_scatter;
            throughput = throughput * tr;
        } else if (hit.hit) {
            let tr = exp(-seg * fog_density);
            radiance = radiance + throughput * fog_color * (1.0 - tr);
            throughput = throughput * tr;
        }

        if (!hit.hit) {
            radiance = radiance + throughput * render_sky(rd, sun_dir);
            break;
        }

        if (bounce == 0 && render_mode == 2u) {
            // Normals Debug Mode
            return hit.normal * 0.5 + 0.5;
        }

        path_len = path_len + hit.t;
        let entering = dot(rd, hit.normal) < 0.0;

        var surf: EvaluatedSurface;
        var shading_n = hit.normal;
        var albedo = vec3<f32>(1.0);
        var roughness = scene.water_params2.z;
        var transparency = 1.0;
        var ior = scene.water_params2.y;
        if (!hit.is_water) {
            surf = evaluate_material(materials[hit.mat_id], hit.pos, hit.normal, path_len);
            shading_n = surf.normal;
            albedo = surf.albedo;
            roughness = surf.roughness;
            transparency = surf.transparency;
            ior = surf.ior;
        }
        // Normal facing the incoming ray
        let n = select(-shading_n, shading_n, dot(shading_n, rd) < 0.0);
        let cos_i = clamp(dot(-rd, n), 0.0, 1.0);

        // ---- Transparent interface: stochastic Fresnel reflect / refract ----
        if (hit.is_water || (transparency > 0.01 && rnd(&rng) < transparency)) {
            let eta = select(ior, 1.0 / ior, entering);
            let f0 = (ior - 1.0) / (ior + 1.0);
            let refr = refract(rd, n, eta);
            let tir = dot(refr, refr) < 0.5;
            var fres = 1.0;
            if (!tir) {
                let cos_t = clamp(dot(refr, -n), 0.0, 1.0);
                fres = schlick_scalar(f0 * f0, select(cos_t, cos_i, entering));
                if (hit.is_water) { fres = clamp(fres * scene.water_params2.w, 0.0, 1.0); }
            }
            var new_rd: vec3<f32>;
            if (rnd(&rng) < fres) {
                new_rd = reflect(rd, n);
                if (blurry) {
                    new_rd = normalize(new_rd + rand_unit_vec(&rng) * roughness * 0.6);
                    new_rd = normalize(new_rd - 2.0 * min(dot(new_rd, n), 0.0) * n);
                }
                ro = hit.pos + n * 0.04;
            } else {
                new_rd = refr;
                if (!hit.is_water) { throughput = throughput * sqrt(max(albedo, vec3<f32>(0.0))); }
                ro = hit.pos - n * 0.04;
            }
            rd = new_rd;
            alive = true;
            continue;
        }

        // ---- Opaque shading: sun (hard or soft shadow), ambient, emission ----
        let n_dot_l = dot(n, sun_dir);
        var shadow = vec3<f32>(0.0);
        if (n_dot_l > 0.0) {
            shadow = shadow_transmittance(hit.pos + n * 0.04, sun_shadow_dir(sun_dir, &rng));
        }
        let diffuse = albedo * max(n_dot_l, 0.0) * sun_color;
        let ambient = albedo * scene.ground_color.rgb * 0.35;
        let half_vec = normalize(sun_dir - rd);
        let n_dot_h = max(dot(n, half_vec), 0.0);
        let spec_power = (1.0 - roughness) * 128.0 + 2.0;
        let specular = pow(n_dot_h, spec_power) * (1.0 - roughness) * sun_color * surf.reflectivity * shadow;
        var direct_color = ambient + diffuse * shadow + specular + surf.emission;

        // ---- Extra lights (point / spot / parallel) ----
        let light_count = min(u32(scene.light_params.x + 0.5), 8u);
        let spec_gloss = (1.0 - roughness) * surf.reflectivity;
        for (var li = 0u; li < light_count; li = li + 1u) {
            let lt = lights[li];
            let kind = lt.pos_and_kind.w;
            var l_dir = -lt.dir_and_range.xyz;
            var atten = 1.0;
            var l_dist = 1.0e9;
            if (kind < 1.5) {
                let to_l = lt.pos_and_kind.xyz - hit.pos;
                l_dist = max(length(to_l), 0.001);
                l_dir = to_l / l_dist;
                let range = lt.dir_and_range.w;
                let x = l_dist / range;
                let cutoff = clamp(1.0 - x * x * x * x, 0.0, 1.0);
                atten = cutoff * cutoff / max(l_dist * l_dist, 0.25);
                if (kind > 0.5) {
                    let cd = dot(-l_dir, lt.dir_and_range.xyz);
                    atten = atten * smoothstep(lt.spot_params.x, max(lt.spot_params.y, lt.spot_params.x + 0.0001), cd);
                }
            }
            let l_n_dot = dot(n, l_dir);
            if (l_n_dot <= 0.0 || atten < 0.0001) { continue; }
            var l_shadow = vec3<f32>(1.0);
            if (lt.spot_params.z > 0.5) {
                l_shadow = shadow_transmittance_d(hit.pos + n * 0.04, l_dir, l_dist - 0.1);
            }
            let l_col = lt.color_intensity.rgb * lt.color_intensity.w * atten * l_shadow;
            let l_half = normalize(l_dir - rd);
            let l_spec = pow(max(dot(n, l_half), 0.0), spec_power) * spec_gloss;
            direct_color = direct_color + albedo * l_n_dot * l_col + l_spec * l_col;
        }

        // Fresnel-weighted mirror share
        let f0v = mix(vec3<f32>(0.04), albedo, surf.metallic);
        let fresnel = f0v + (vec3<f32>(1.0) - f0v) * pow(1.0 - cos_i, 5.0);
        let refl = clamp(surf.reflectivity * fresnel, vec3<f32>(0.0), vec3<f32>(1.0));

        if (bounce == 0 && render_mode == 1u) {
            // Specular reflections only
            return specular + direct_color * surf.reflectivity;
        }

        radiance = radiance + throughput * direct_color * (vec3<f32>(1.0) - refl);
        throughput = throughput * refl;
        if (max3(refl) < 0.02) { break; }

        var new_rd = reflect(rd, n);
        if (blurry) {
            new_rd = normalize(new_rd + rand_unit_vec(&rng) * roughness * 0.6);
            new_rd = normalize(new_rd - 2.0 * min(dot(new_rd, n), 0.0) * n);
        }
        rd = new_rd;
        ro = hit.pos + n * 0.04;
        alive = true;
    }

    // Bounce budget exhausted with energy left: approximate the remainder with the sky
    if (alive) {
        radiance = radiance + throughput * render_sky(rd, sun_dir);
    }
    return radiance;
}

// =============================================================================
// PROGRESSIVE ACCUMULATION (sub-pixel jittered samples averaged in linear HDR)
// =============================================================================

fn pcg_hash(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn pixel_random(px: vec2<u32>) -> vec2<f32> {
    let h1 = pcg_hash(px.x + pcg_hash(px.y));
    let h2 = pcg_hash(h1 ^ 0x9e3779b9u);
    return vec2<f32>(f32(h1 >> 8u), f32(h2 >> 8u)) / 16777216.0;
}

@fragment
fn fs_trace(in: VertexOutput) -> @location(0) vec4<f32> {
    let px = vec2<u32>(in.clip_position.xy);
    let sample_index = scene.resolution.z;

    // Per-pixel random offset (Cranley-Patterson rotation) + R2 low-discrepancy sequence
    let r2 = vec2<f32>(0.7548776662, 0.5698402910);
    let jitter = fract(pixel_random(px) + r2 * sample_index) - vec2<f32>(0.5);
    let pixel_ndc = vec2<f32>(2.0) / scene.resolution.xy;
    let uv = in.uv + jitter * pixel_ndc;

    let seed = pcg_hash(px.x + pcg_hash(px.y + pcg_hash(u32(sample_index) * 9781u + 1u)));
    var c = trace_pixel(uv, seed);
    // Guard against NaN/Inf and extreme fireflies poisoning the average
    if (any(c != c)) { c = vec3<f32>(0.0); }
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(64.0));

    var result = c;
    if (sample_index > 0.5) {
        let prev = textureLoad(accum_tex, vec2<i32>(in.clip_position.xy), 0).rgb;
        result = mix(prev, c, 1.0 / (sample_index + 1.0));
    }
    return vec4<f32>(result, 1.0);
}

fn bayer4x4(p: vec2<u32>) -> f32 {
    var m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0
    );
    let i = (p.y % 4u) * 4u + (p.x % 4u);
    return (m[i] + 0.5) / 16.0;
}

fn load_accum(p: vec2<i32>) -> vec3<f32> {
    let dims = vec2<i32>(textureDimensions(accum_tex));
    return textureLoad(accum_tex, clamp(p, vec2<i32>(0), dims - vec2<i32>(1)), 0).rgb;
}

// Soft-knee bright-pass: keeps only the part of the colour above luminance 1.0
fn bloom_bright(c: vec3<f32>) -> vec3<f32> {
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let knee = 0.5;
    let soft = clamp(l - 1.0 + knee, 0.0, 2.0 * knee);
    let soft_c = soft * soft / (4.0 * knee);
    return c * (max(soft_c, l - 1.0) / max(l, 0.0001));
}

@fragment
fn fs_display(in: VertexOutput) -> @location(0) vec4<f32> {
    let pix = vec2<i32>(in.clip_position.xy);
    var final_color = load_accum(pix);

    let render_mode = u32(scene.time_and_flags.w + 0.5);
    if (render_mode != 0u) {
        // Debug outputs are shown raw
        return vec4<f32>(final_color, 1.0);
    }

    // Chromatic aberration: R and B sampled with opposite radial offsets
    let ca = scene.display_params.x;
    if (ca > 0.00001) {
        let dims_f = vec2<f32>(textureDimensions(accum_tex));
        let off = (in.clip_position.xy - dims_f * 0.5) * ca;
        final_color.r = load_accum(vec2<i32>(floor(in.clip_position.xy + off))).r;
        final_color.b = load_accum(vec2<i32>(floor(in.clip_position.xy - off))).b;
    }

    // Bloom: sparse 2-ring blur (8 + 8 taps + centre) of the bright-passed HDR
    let bloom_amount = scene.postprocess_params.y;
    if (bloom_amount > 0.0001) {
        let h = f32(textureDimensions(accum_tex).y);
        let r1 = max(h * 0.006, 1.0);
        let r2 = max(h * 0.015, 2.0);
        var acc = bloom_bright(load_accum(pix)) * 2.0;
        var wsum = 2.0;
        for (var i = 0; i < 8; i = i + 1) {
            let a = f32(i) * 0.7853982;
            let d = vec2<f32>(cos(a), sin(a));
            acc = acc + bloom_bright(load_accum(pix + vec2<i32>(round(d * r1)))) * 1.5;
            acc = acc + bloom_bright(load_accum(pix + vec2<i32>(round(d * r2 * 1.0)))) * 1.0;
            wsum = wsum + 2.5;
        }
        final_color = final_color + acc / wsum * bloom_amount * 3.0;
    }

    // =========================================================================
    // POST-PROCESSING LAB (90s Retro CGI Mode, Grading, Vignette, Gamma)
    // =========================================================================
    let is_retro_90s = scene.postprocess_params.x > 0.5;
    let contrast = scene.postprocess_params.z;
    let saturation = scene.postprocess_params.w;

    if (is_retro_90s) {
        // Classic 1990s CGI vibrant synthetic color characteristics
        final_color = pow(max(final_color, vec3<f32>(0.0)), vec3<f32>(0.85)); // Punchy midtones
        let gray = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
        final_color = mix(vec3<f32>(gray), final_color, saturation * 1.35); // Super saturated
        final_color = (final_color - 0.5) * (contrast * 1.25) + 0.5;
        final_color = max(final_color, vec3<f32>(0.0));
    } else {
        // Modern filmic tone mapping
        let gray = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
        final_color = mix(vec3<f32>(gray), final_color, saturation);
        final_color = max((final_color - 0.5) * contrast + 0.5, vec3<f32>(0.0));
        final_color = final_color / (final_color + vec3<f32>(1.0));
    }

    // Gamma encode exactly once (skipped when the surface is an sRGB format,
    // where the hardware performs the encode)
    let gamma = select(max(scene.post_params2.y, 0.1), 1.0, scene.post_params2.w > 0.5);
    final_color = pow(final_color, vec3<f32>(1.0 / gamma));

    // Vignette
    let uv_dist = length(in.uv * 0.7);
    let vignette = clamp(1.0 - uv_dist * scene.post_params2.x * 1.8, 0.0, 1.0);
    final_color = final_color * vignette;

    // Optional ordered dither to 5-6-5 (16-bit) levels
    if (scene.post_params2.z > 0.5) {
        let t = bayer4x4(vec2<u32>(in.clip_position.xy));
        let levels = vec3<f32>(31.0, 63.0, 31.0);
        final_color = floor(clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0)) * levels + vec3<f32>(t)) / levels;
    }

    return vec4<f32>(final_color, 1.0);
}
