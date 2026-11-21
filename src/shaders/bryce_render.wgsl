// =============================================================================
// BRYCE 7-INSPIRED COMPLETE 3D ENVIRONMENT CREATION STUDIO (WGSL)
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
    // x: width, y: height, z: frame_count, w: num_active_primitives
    resolution: vec4<f32>,
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
    geom_normal: vec3<f32>
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
        let bump_factor = bump_strength * 0.12;
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

    return (mountain - 2.0) * mask;
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
    var closest: HitInfo;
    closest.hit = false;
    closest.t = 1e6;
    closest.mat_id = 0u;
    closest.is_water = false;

    // 1. Water Plane (Y = water_params1.x)
    let water_y = scene.water_params1.x;
    if (abs(rd.y) > 0.0001) {
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

            closest.normal = normalize(vec3<f32>(-ripple_dx, 1.0, -ripple_dz));
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

        if (prim_type == 0u) {
            // SPHERE
            let oc = ro - center;
            let rad = scale.x;
            let b = dot(oc, rd);
            let c = dot(oc, oc) - rad * rad;
            let disc = b * b - c;
            if (disc > 0.0) {
                let t_sph = -b - sqrt(disc);
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

            if (t_max >= max(0.05, t_min) && t_min < closest.t) {
                closest.hit = true;
                closest.t = t_min;
                closest.pos = ro + rd * t_min;

                let p_local = ro_local + rd_local * t_min;
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

            if (disc > 0.0 && a > 0.0001) {
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
    var tm = 10.0;
    let tm_max = 120.0;
    for (var i = 0; i < 75; i = i + 1) {
        if (tm > tm_max || tm >= closest.t) { break; }
        let p = ro + rd * tm;
        let h = terrain_height(p.xz);
        let dist_to_surface = p.y - h;
        if (dist_to_surface < 0.14) {
            closest.hit = true;
            closest.t = tm;
            closest.pos = p;
            closest.normal = terrain_normal(p.xz);
            closest.mat_id = 0u; // Mountain Rock
            closest.is_water = false;
            break;
        }
        tm = tm + max(dist_to_surface * 0.5, 0.35);
    }

    return closest;
}

// =============================================================================
// MAIN RECURSIVE RAYTRACING SHADER
// =============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tan_half_fov = camera.params.x;
    let aspect_ratio = camera.params.y;

    let ray_dir = normalize(
        camera.forward.xyz +
        camera.right.xyz * (in.uv.x * tan_half_fov * aspect_ratio) +
        camera.up.xyz * (in.uv.y * tan_half_fov)
    );
    var ro = camera.pos.xyz;
    var rd = ray_dir;

    let sun_dir = normalize(scene.sun_direction.xyz);
    let sun_color = scene.sun_color_intensity.rgb * scene.sun_color_intensity.w;

    let hit = intersect_scene(ro, rd);

    if (!hit.hit) {
        let sky = render_sky(rd, sun_dir);
        return vec4<f32>(sky, 1.0);
    }

    let render_mode = u32(scene.time_and_flags.w + 0.5);
    if (render_mode == 2u) {
        // Normals Debug Mode
        return vec4<f32>(hit.normal * 0.5 + 0.5, 1.0);
    }

    let mat = materials[hit.mat_id];
    let surf = evaluate_material(mat, hit.pos, hit.normal);

    // Direct lighting & specular
    let n_dot_l = max(dot(surf.normal, sun_dir), 0.0);
    let diffuse = surf.albedo * n_dot_l * sun_color;
    let ambient = surf.albedo * scene.ground_color.rgb * 0.35;

    let half_vec = normalize(sun_dir - rd);
    let n_dot_h = max(dot(surf.normal, half_vec), 0.0);
    let spec_power = (1.0 - surf.roughness) * 128.0 + 2.0;
    let specular = pow(n_dot_h, spec_power) * (1.0 - surf.roughness) * sun_color * surf.reflectivity;

    var direct_color = ambient + diffuse + specular + surf.emission;

    // Fresnel reflection
    let f0 = mix(vec3<f32>(0.04), surf.albedo, surf.metallic);
    let cos_theta = max(dot(-rd, surf.normal), 0.0);
    let fresnel = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - cos_theta, 5.0);

    if (hit.is_water) {
        // Bryce Water: Fresnel reflection + depth extinction
        let reflect_dir = reflect(rd, surf.normal);
        let reflected_sky = render_sky(reflect_dir, sun_dir);
        let water_deep = scene.water_deep_color.rgb;
        let water_shallow = scene.water_shallow_color.rgb;
        let water_fresnel = 0.04 + 0.96 * pow(1.0 - cos_theta, 5.0);

        let water_base = mix(water_deep, water_shallow, clamp(surf.normal.y * 0.6, 0.0, 1.0));
        direct_color = mix(water_base, reflected_sky, water_fresnel * scene.water_params2.w) + specular * 2.0;
    } else if (surf.reflectivity > 0.05) {
        // True secondary recursive ray bounce
        let bounce_ro = hit.pos + surf.normal * 0.03;
        let bounce_rd = reflect(rd, surf.normal);

        let bounce_hit = intersect_scene(bounce_ro, bounce_rd);
        var bounce_color: vec3<f32>;
        if (bounce_hit.hit) {
            let b_mat = materials[bounce_hit.mat_id];
            let b_surf = evaluate_material(b_mat, bounce_hit.pos, bounce_hit.normal);
            let b_ndotl = max(dot(b_surf.normal, sun_dir), 0.0);
            bounce_color = b_surf.albedo * (b_ndotl * sun_color + vec3<f32>(0.12));
        } else {
            bounce_color = render_sky(bounce_rd, sun_dir);
        }
        direct_color = mix(direct_color, bounce_color, surf.reflectivity * fresnel);
    }

    if (render_mode == 1u) {
        // Specular reflections only
        return vec4<f32>(specular + direct_color * surf.reflectivity, 1.0);
    }

    // Atmospheric Distance Fog & Haze
    let fog_density = scene.fog_params.x;
    let fog_amount = 1.0 - exp(-hit.t * fog_density);
    let fog_color = scene.fog_color.rgb;
    var final_color = mix(direct_color, fog_color, clamp(fog_amount, 0.0, 1.0));

    // =========================================================================
    // POST-PROCESSING LAB (90s Retro CGI Mode, Bloom, Vignette, Grading)
    // =========================================================================
    let is_retro_90s = scene.postprocess_params.x > 0.5;
    let bloom_amt = scene.postprocess_params.y;
    let contrast = scene.postprocess_params.z;
    let saturation = scene.postprocess_params.w;

    if (is_retro_90s) {
        // Classic 1990s Bryce vibrant synthetic color characteristics
        final_color = pow(final_color, vec3<f32>(0.85)); // Punchy midtones
        let gray = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
        final_color = mix(vec3<f32>(gray), final_color, saturation * 1.35); // Super saturated
        final_color = (final_color - 0.5) * (contrast * 1.25) + 0.5;
        // Subtle color banding/quantization typical of 90s 16/24-bit framebuffers
        final_color = floor(final_color * 64.0) / 64.0;
    } else {
        // Modern filmic tone mapping
        let gray = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
        final_color = mix(vec3<f32>(gray), final_color, saturation);
        final_color = (final_color - 0.5) * contrast + 0.5;
        final_color = final_color / (final_color + vec3<f32>(1.0));
        final_color = pow(max(final_color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    }

    // Vignette
    let uv_dist = length(in.uv * 0.7);
    let vignette = clamp(1.0 - uv_dist * 0.45, 0.0, 1.0);
    final_color = final_color * vignette;

    return vec4<f32>(final_color, 1.0);
}
