use std::fs;
use bryce_modern::terrain::{
    BrushMode, HeightmapGrid, TerrainAlgorithm, TerrainConfig,
};
use bryce_modern::material::GpuMaterial;
use bryce_modern::objects::{SceneObject, ScatterSystem};
use bryce_modern::sky::SkyConfig;
use bryce_modern::water::WaterConfig;
use bryce_modern::postprocess::PostProcessConfig;
use bryce_modern::io::{ProjectFile, ProjectIO};

#[test]
fn test_all_fractal_algorithms() {
    let algos = [
        TerrainAlgorithm::Fbm,
        TerrainAlgorithm::RidgedMultifractal,
        TerrainAlgorithm::Billow,
        TerrainAlgorithm::WorleyCellular,
        TerrainAlgorithm::DomainWarped,
        TerrainAlgorithm::HybridMultifractal,
    ];

    for algo in algos {
        let mut config = TerrainConfig::default();
        config.algorithm = algo;
        config.height_scale = 1.2;
        config.frequency = 0.035;
        config.octaves = 4;

        let mut grid = HeightmapGrid::new(32);
        grid.generate_from_config(&config);

        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;
        for &h in &grid.data {
            assert!(!h.is_nan(), "Height is NaN for algorithm {:?}", algo);
            assert!(!h.is_infinite(), "Height is infinite for algorithm {:?}", algo);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }

        assert!(
            max_h > min_h,
            "Algorithm {:?} produced flat terrain (min: {}, max: {})",
            algo,
            min_h,
            max_h
        );
    }
}

#[test]
fn test_brush_sculpting_operations() {
    let mut grid = HeightmapGrid::new(32);
    // Fill with base height 10.0
    for v in &mut grid.data {
        *v = 10.0;
    }

    let center = 16;
    let idx = center * 32 + center;

    // 1. Raise Brush
    grid.apply_brush(center, center, 4, 1.0, BrushMode::Raise, 10.0);
    assert!(grid.data[idx] > 10.0, "Raise brush did not increase height: {}", grid.data[idx]);

    // 2. Lower Brush
    let prev = grid.data[idx];
    grid.apply_brush(center, center, 4, 1.5, BrushMode::Lower, 10.0);
    assert!(grid.data[idx] < prev, "Lower brush did not decrease height");

    // 3. Flatten Brush
    grid.apply_brush(center, center, 4, 1.0, BrushMode::Flatten, 5.0);
    assert!(
        (grid.data[idx] - 5.0).abs() < (prev - 5.0).abs(),
        "Flatten brush did not move toward target 5.0"
    );
}

#[test]
fn test_geological_erosion_simulation() {
    let mut grid = HeightmapGrid::new(32);
    let config = TerrainConfig::default();
    grid.generate_from_config(&config);

    // Apply hydraulic rainfall erosion
    grid.simulate_hydraulic_erosion(3);
    for &h in &grid.data {
        assert!(!h.is_nan(), "Hydraulic erosion produced NaN");
        assert!(!h.is_infinite(), "Hydraulic erosion produced Inf");
    }

    // Apply thermal scree weathering
    grid.simulate_thermal_erosion(3);
    for &h in &grid.data {
        assert!(!h.is_nan(), "Thermal erosion produced NaN");
        assert!(!h.is_infinite(), "Thermal erosion produced Inf");
    }
}

#[test]
fn test_export_obj_and_png() {
    let mut grid = HeightmapGrid::new(16);
    grid.generate_from_config(&TerrainConfig::default());

    let test_obj_path = "target/test_terrain.obj";
    let test_png_path = "target/test_heightmap.png";

    let _ = fs::remove_file(test_obj_path);
    let _ = fs::remove_file(test_png_path);

    grid.export_obj(test_obj_path).expect("OBJ export failed");
    assert!(fs::metadata(test_obj_path).is_ok(), "OBJ file not created");
    let obj_content = fs::read_to_string(test_obj_path).expect("Read OBJ failed");
    assert!(obj_content.contains("v "), "OBJ missing vertices");
    assert!(obj_content.contains("f "), "OBJ missing faces");

    grid.export_png(test_png_path).expect("PNG export failed");
    assert!(fs::metadata(test_png_path).is_ok(), "PNG file not created");
    let png_meta = fs::metadata(test_png_path).unwrap();
    assert!(png_meta.len() > 50, "PNG file too small");

    let _ = fs::remove_file(test_obj_path);
    let _ = fs::remove_file(test_png_path);
}

#[test]
fn test_material_presets_and_gpu_layout() {
    // Check GPU struct size (must be 80 bytes for WGSL alignment)
    assert_eq!(
        std::mem::size_of::<GpuMaterial>(),
        80,
        "GpuMaterial must be exactly 80 bytes"
    );

    let rock = GpuMaterial::rock();
    assert!(rock.params1[0] >= 0.0 && rock.params1[0] <= 1.0, "Rock roughness invalid");

    let chrome = GpuMaterial::chrome();
    assert_eq!(chrome.params1[1], 1.0, "Chrome should be 100% metallic");
    assert_eq!(chrome.params1[2], 0.98, "Chrome should have 0.98 reflectivity");

    let glass = GpuMaterial::emerald_glass();
    assert_eq!(glass.params1[3], 0.85, "Glass should be transparent");
}

#[test]
fn test_scatter_system() {
    let terrain = TerrainConfig::default();
    let scatter = ScatterSystem {
        enabled: true,
        rock_density: 0.8,
        tree_density: 0.8,
        min_elevation: 0.0,
        max_elevation: 25.0,
        max_slope_deg: 60.0,
        random_seed: 999,
    };

    let scattered = scatter.scatter_objects(&terrain);
    assert!(!scattered.is_empty(), "Scatter system should place objects");
    for obj in &scattered {
        assert!(obj.visible);
        assert!(obj.scale[0] > 0.0);
    }
}

#[test]
fn test_project_save_load_roundtrip() {
    let test_path = "target/test_project.bryce.json";
    let _ = fs::remove_file(test_path);

    let mut project = ProjectFile {
        name: "Test Bryce Project".to_string(),
        version: "7.0.0".to_string(),
        terrain: TerrainConfig::default(),
        sky: SkyConfig::bryce_sunset(),
        water: WaterConfig::ocean(),
        postprocess: PostProcessConfig::default(),
        objects: vec![
            SceneObject::sphere("Chrome Sphere", [0.0, 5.0, 0.0], 2.5, 2),
            SceneObject::torus("Golden Ring", [0.0, 7.0, 0.0], 4.0, 0.6, 4),
        ],
        materials: vec![GpuMaterial::rock(), GpuMaterial::chrome()],
    };

    project.terrain.height_scale = 2.45;
    project.terrain.algorithm = TerrainAlgorithm::DomainWarped;
    project.postprocess.retro_90s_cgi = true;

    ProjectIO::save_project(test_path, &project).expect("Failed to save project");
    let loaded = ProjectIO::load_project(test_path).expect("Failed to load project");

    assert_eq!(loaded.version, "7.0.0");
    assert_eq!(loaded.terrain.height_scale, 2.45);
    assert_eq!(loaded.terrain.algorithm, TerrainAlgorithm::DomainWarped);
    assert_eq!(loaded.postprocess.retro_90s_cgi, true);
    assert_eq!(loaded.objects.len(), 2);
    assert_eq!(loaded.objects[0].name, "Chrome Sphere");
    assert_eq!(loaded.objects[1].name, "Golden Ring");

    let _ = fs::remove_file(test_path);
}

#[test]
fn test_camera_drag_rotation() {
    use bryce_modern::camera::Camera;
    use glam::Vec3;

    let mut camera = Camera::new(Vec3::new(0.0, 5.0, 10.0), -90.0, 0.0);
    let initial_forward = camera.forward();

    // Drag right (dx = 50.0 pixels)
    camera.rotate(50.0, 0.0);
    let right_forward = camera.forward();
    assert_ne!(initial_forward, right_forward, "Camera should rotate on horizontal drag");

    // Drag down (dy = 30.0 pixels)
    camera.rotate(0.0, 30.0);
    let down_forward = camera.forward();
    assert_ne!(right_forward, down_forward, "Camera should rotate on vertical drag");
    assert!(camera.pitch < 0.0, "Camera should pitch downwards");
}

#[test]
fn test_render_presets_and_request() {
    use bryce_modern::editor::EditorState;
    use bryce_modern::postprocess::PostProcessConfig;

    let mut editor = EditorState::new();
    assert_eq!(editor.request_render, false);

    // Trigger render request
    editor.request_render = true;
    assert_eq!(editor.request_render, true);

    let retro = PostProcessConfig::retro_bryce_preset();
    assert!(retro.retro_90s_cgi);
    assert_eq!(retro.contrast, 1.25);

    let photo = PostProcessConfig::photorealistic_preset();
    assert!(!photo.retro_90s_cgi);
    assert_eq!(photo.contrast, 1.02);
}

#[test]
fn test_model_obj_and_presets() {
    use bryce_modern::model_loader::ModelLoader;

    // Test Utah Teapot preset
    let teapot = ModelLoader::bryce_teapot();
    assert_eq!(teapot.name, "Bryce Utah Teapot");
    assert!(teapot.triangle_count() > 50, "Teapot should have triangles");
    assert_eq!(teapot.normals.len(), teapot.vertices.len());

    // Test Desert Monolith preset
    let obelisk = ModelLoader::obelisk_monolith();
    assert_eq!(obelisk.name, "Desert Monolith Obelisk");
    assert!(obelisk.triangle_count() >= 10);

    // Test Wavefront .obj parser
    let obj_sample = r#"
# Sample Cube OBJ
v -1.0 -1.0 1.0
v 1.0 -1.0 1.0
v 1.0 1.0 1.0
v -1.0 1.0 1.0
f 1 2 3
f 1 3 4
"#;
    let parsed_mesh = ModelLoader::load_obj(obj_sample, "TestCube").expect("Should parse OBJ cleanly");
    assert_eq!(parsed_mesh.triangle_count(), 2);
    assert_eq!(parsed_mesh.normals.len(), parsed_mesh.vertices.len());

    // Test load_from_file with file path
    if std::path::Path::new("test_obj.obj").exists() {
        let file_mesh = ModelLoader::load_from_file("test_obj.obj").expect("Should load test_obj.obj");
        assert!(file_mesh.triangle_count() >= 12, "Cube should have at least 12 triangles");
    }
}

#[test]
fn test_ray_picking_and_object_intersection() {
    use bryce_modern::camera::Camera;
    use bryce_modern::objects::SceneObject;
    use glam::Vec3;

    let camera = Camera::new(Vec3::new(0.0, 0.0, 10.0), -90.0, 0.0);
    let (ro, rd) = camera.screen_to_ray(400.0, 300.0, 800.0, 600.0);

    // Sphere placed right in center of view
    let sphere = SceneObject::sphere("Target Orb", [0.0, 0.0, 0.0], 2.0, 1);
    let hit = sphere.intersect_ray(ro, rd);
    assert!(hit.is_some(), "Ray should hit sphere centered along camera forward");
    assert!(hit.unwrap() > 0.0);

    // Sphere placed far off to the side (should miss)
    let off_sphere = SceneObject::sphere("Offscreen Orb", [100.0, 0.0, 0.0], 1.0, 1);
    let miss = off_sphere.intersect_ray(ro, rd);
    assert!(miss.is_none(), "Ray should miss offscreen sphere");
}

#[test]
fn test_custom_material_copy_and_serialization() {
    use bryce_modern::material::GpuMaterial;

    let mat = GpuMaterial::blue_marble();
    let json = serde_json::to_string(&mat).expect("Material should serialize to JSON");
    let deserialized: GpuMaterial = serde_json::from_str(&json).expect("Material should deserialize");

    assert_eq!(mat.params1, deserialized.params1);
    assert_eq!(mat.params2, deserialized.params2);
    assert_eq!(mat.base_color, deserialized.base_color);
}
