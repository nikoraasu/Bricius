use egui::{Color32, RichText, Slider, Ui, Vec2};
use glam::Vec3;

use crate::camera::Camera;
use crate::generator::{SceneGenerator, SceneTheme};
use crate::io::{ProjectFile, ProjectIO};
use crate::light::{LightConfig, LightKind, MAX_LIGHTS};
use crate::material::GpuMaterial;
use crate::objects::{CsgOp, SceneObject, ScatterSystem};
use crate::scene::SceneController;
use crate::terrain::{BrushMode, TerrainAlgorithm, TerrainConfig, TerrainObjectData};
use crate::water::WaterType;

/// Classic hypsometric elevation colormap with smooth linear interpolation:
/// deep water -> shallow -> sand -> forest -> rock -> granite -> snow
fn height_to_hypsometric_color(h: f32) -> Color32 {
    let stops: &[(f32, Color32)] = &[
        (-2.0, Color32::from_rgb(14, 38, 85)),    // Deep oceanic trench
        (-0.5, Color32::from_rgb(22, 65, 130)),   // Deep water
        (0.2, Color32::from_rgb(45, 135, 195)),   // Shallow turquoise lagoon
        (0.8, Color32::from_rgb(225, 200, 140)),  // Beach shoreline sand
        (3.5, Color32::from_rgb(50, 130, 45)),    // Lush green lowlands
        (7.0, Color32::from_rgb(115, 105, 90)),   // Highland rock & scree
        (11.0, Color32::from_rgb(165, 160, 155)), // Granite peaks
        (15.0, Color32::from_rgb(245, 250, 255)), // Glacial snow
    ];

    if h <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops.len() - 1;
    if h >= stops[last].0 {
        return stops[last].1;
    }

    for i in 0..last {
        let (h0, c0) = stops[i];
        let (h1, c1) = stops[i + 1];
        if h >= h0 && h <= h1 {
            let t = ((h - h0) / (h1 - h0)).clamp(0.0, 1.0);
            let r = (c0.r() as f32 * (1.0 - t) + c1.r() as f32 * t) as u8;
            let g = (c0.g() as f32 * (1.0 - t) + c1.g() as f32 * t) as u8;
            let b = (c0.b() as f32 * (1.0 - t) + c1.b() as f32 * t) as u8;
            return Color32::from_rgb(r, g, b);
        }
    }
    stops[last].1
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveLab {
    Tutorial,
    Generator,
    Terrain,
    Sky,
    Water,
    Materials,
    Scene,
    Render,
}

pub struct EditorState {
    pub visible: bool,
    pub active_lab: ActiveLab,
    pub selected_material_slot: usize,
    pub selected_object_idx: usize,
    pub selected_light_idx: usize,
    pub brush_mode: BrushMode,
    pub brush_radius: usize,
    pub brush_strength: f32,
    #[allow(dead_code)]
    pub brush_target_height: f32,
    /// Preview texture of the selected terrain object's heightmap (Terrain lab canvas)
    pub heightmap_dirty: bool,
    pub heightmap_texture: Option<egui::TextureHandle>,
    pub heightmap_preview_key: Option<(usize, usize)>,
    pub scatter_config: ScatterSystem,
    pub tutorial_step: usize,
    pub show_controls_hud: bool,
    pub request_render: bool,
    pub material_names: [String; GpuMaterial::NUM_PRESETS],
    #[allow(dead_code)]
    pub custom_mat_name_input: String,
    pub model_file_path_input: String,
    pub uniform_scale_locked: bool,
    pub copied_material: Option<GpuMaterial>,
    pub status_message: String,
    pub fps: f32,
    pub samples_done: u32,
    pub sample_target: u32,
    pub frame_time_ms: f32,
}

impl EditorState {
    pub fn new() -> Self {
        let material_names = [
            "Mountain Rock".to_string(),
            "Blue Marble".to_string(),
            "Mirror Chrome".to_string(),
            "Emerald Glass".to_string(),
            "Hammered Gold".to_string(),
            "Basalt Obsidian".to_string(),
            "Ocean Water".to_string(),
            "Bioluminescent".to_string(),
            "Retro Plastic".to_string(),
            "Sandstone Strata".to_string(),
            "Glacial Ice".to_string(),
            "Wood Grain".to_string(),
            "Rusted Iron".to_string(),
            "Amethyst Crystal".to_string(),
            "Cyber Grid".to_string(),
            "Red Desert Rock".to_string(),
        ];

        Self {
            visible: true,
            active_lab: ActiveLab::Tutorial,
            selected_material_slot: 1,
            selected_object_idx: 0,
            selected_light_idx: 0,
            brush_mode: BrushMode::Raise,
            brush_radius: 8,
            brush_strength: 0.5,
            brush_target_height: 0.4,
            heightmap_dirty: true,
            heightmap_texture: None,
            heightmap_preview_key: None,
            scatter_config: ScatterSystem::default(),
            tutorial_step: 0,
            show_controls_hud: true,
            request_render: false,
            material_names,
            custom_mat_name_input: String::new(),
            model_file_path_input: String::new(),
            uniform_scale_locked: true,
            copied_material: None,
            status_message: "Welcome to Bricius! Follow the tutorial or press F1 anytime.".to_string(),
            fps: 60.0,
            samples_done: 0,
            sample_target: 1,
            frame_time_ms: 16.6,
        }
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        scene_ctrl: &mut SceneController,
        camera: &mut Camera,
        materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
        objects: &mut Vec<SceneObject>,
        meshes: &mut Vec<crate::model_loader::LoadedMesh>,
        material_dirty: &mut bool,
        mesh_dirty: &mut bool,
        terrains: &mut Vec<TerrainObjectData>,
        terrain_dirty: &mut bool,
    ) {
        if !self.visible {
            egui::Area::new(egui::Id::new("minimized_bar"))
                .fixed_pos(egui::pos2(12.0, 12.0))
                .show(ctx, |ui: &mut Ui| {
                    if ui.button(RichText::new("⚡ Open Bricius").strong()).clicked() {
                        self.visible = true;
                    }
                });
            return;
        }

        // Apply dark skeuomorphic visuals
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(22, 24, 28);
        visuals.window_fill = Color32::from_rgba_unmultiplied(26, 29, 36, 245);
        visuals.window_stroke = egui::Stroke::new(1.5, Color32::from_rgb(52, 58, 70));
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 36, 44);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 43, 53);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 60, 75);
        visuals.widgets.active.bg_fill = Color32::from_rgb(0, 185, 230);
        ctx.set_visuals(visuals);

        let screen = ctx.viewport_rect();

        // =========================================================================
        // TOP BRICIUS SHELF (TOOLBAR)
        // =========================================================================
        egui::Area::new(egui::Id::new("bricius_top_shelf"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui: &mut Ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 22, 26))
                    .inner_margin(egui::Margin { left: 16, right: 16, top: 8, bottom: 8 })
                    .show(ui, |ui: &mut Ui| {
                        ui.set_width(screen.width() - 32.0);
                        ui.horizontal(|ui: &mut Ui| {
                            ui.label(RichText::new("BRICIUS").size(20.0).strong().color(Color32::from_rgb(0, 225, 255)));
                            ui.label(RichText::new("STUDIO").size(11.0).color(Color32::from_rgb(255, 190, 0)));
                            ui.separator();

                            let labs = [
                                (ActiveLab::Tutorial, "🎓 Tutorial & Controls (F1)"),
                                (ActiveLab::Generator, "🎲 1-Click Generator"),
                                (ActiveLab::Terrain, "⛰️ Terrain Lab & Sculptor"),
                                (ActiveLab::Sky, "🌅 Sky & Atmosphere"),
                                (ActiveLab::Water, "🌊 Water & Ocean"),
                                (ActiveLab::Materials, "🎨 Materials & DTE"),
                                (ActiveLab::Scene, "🔮 Scene Objects"),
                                (ActiveLab::Render, "🎥 Render & Export"),
                            ];

                            for (lab, label) in labs {
                                let is_active = self.active_lab == lab;
                                let text = if is_active {
                                    RichText::new(label).strong().color(Color32::from_rgb(0, 235, 255))
                                } else {
                                    RichText::new(label).color(Color32::from_rgb(180, 190, 205))
                                };

                                if ui.selectable_label(is_active, text).clicked() {
                                    self.active_lab = lab;
                                }
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut Ui| {
                                if ui.button("Hide UI [Tab]").clicked() {
                                    self.visible = false;
                                }
                                if ui.button(if self.show_controls_hud { "⌨️ Hide HUD" } else { "⌨️ Controls HUD" }).clicked() {
                                    self.show_controls_hud = !self.show_controls_hud;
                                }
                                if ui.button(RichText::new("📸 Render [F9]").color(Color32::from_rgb(0, 240, 210)).strong()).clicked() {
                                    self.request_render = true;
                                }
                                ui.label(format!("{:.1} FPS ({:.1} ms)", self.fps, self.frame_time_ms));
                                ui.label(format!("Samples: {} / {}", self.samples_done, self.sample_target));
                            });
                        });
                    });
            });

        // =========================================================================
        // FLOATING CONTROLS QUICK REFERENCE HUD OVERLAY
        // =========================================================================
        if self.show_controls_hud {
            egui::Area::new(egui::Id::new("bricius_controls_hud_card"))
                .fixed_pos(egui::pos2(screen.width() - 315.0, 65.0))
                .show(ctx, |ui: &mut Ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(16, 18, 24, 235))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 60, 80)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin { left: 12, right: 12, top: 10, bottom: 10 })
                        .show(ui, |ui: &mut Ui| {
                            ui.set_width(290.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⌨️ Controls Quick Reference").strong().size(12.0).color(Color32::from_rgb(0, 220, 240)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("✕").clicked() {
                                        self.show_controls_hud = false;
                                    }
                                });
                            });
                            ui.separator();
                            egui::Grid::new("hud_grid").spacing(egui::vec2(10.0, 4.0)).show(ui, |ui| {
                                ui.label(RichText::new("W, A, S, D").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Fly Camera Forward/Left/Back/Right").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("Space / Shift").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Fly Altitude (Ascend / Descend)").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("LMB Drag").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("360° Mouse Look / View Orbit").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("Left / Right").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Sun Azimuth Orbit (360°)").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("Up / Down").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Sun Elevation (Dawn/Noon/Dusk)").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("Keys 1 – 8").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Quick Material Swap on Orb").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("Tab").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Toggle Studio UI On / Off").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("F1").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                                ui.label(RichText::new("Open Studio Tutorial & Guide").size(11.0));
                                ui.end_row();

                                ui.label(RichText::new("F9").monospace().strong().color(Color32::from_rgb(0, 240, 210)));
                                ui.label(RichText::new("📸 Render Still Image (.png)").size(11.0));
                                ui.end_row();
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("🎓 Open Step-by-Step Tutorial").clicked() {
                                    self.active_lab = ActiveLab::Tutorial;
                                }
                            });
                        });
                });
        }

        // =========================================================================
        // ACTIVE LAB WORKSPACE WINDOW
        // =========================================================================
        egui::Window::new(match self.active_lab {
            ActiveLab::Tutorial => "🎓 In-Studio Interactive Tutorial & Controls Guide",
            ActiveLab::Generator => "🎲 Procedural 1-Click World Generator",
            ActiveLab::Terrain => "⛰️ Terrain Lab & Interactive Heightmap Sculptor",
            ActiveLab::Sky => "🌅 Sky Lab & Atmospheric Volumetrics",
            ActiveLab::Water => "🌊 Water & Ocean Environment Lab",
            ActiveLab::Materials => "🎨 Materials Lab & Deep Texture Editor (DTE)",
            ActiveLab::Scene => "🔮 Scene Hierarchy & 3D Objects",
            ActiveLab::Render => "🎥 Render Settings & Project Export",
        })
        .default_pos(egui::pos2(20.0, 65.0))
        .default_size(Vec2::new(500.0, 640.0))
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui: &mut Ui| {
            egui::ScrollArea::vertical().show(ui, |ui: &mut Ui| {
                match self.active_lab {
                    ActiveLab::Tutorial => {
                        self.draw_tutorial_lab(ui, scene_ctrl, camera, materials, objects, terrains, material_dirty);
                    }
                    ActiveLab::Generator => {
                        self.draw_generator_lab(ui, scene_ctrl, materials, objects);
                    }
                    ActiveLab::Terrain => {
                        self.draw_terrain_lab(ui, scene_ctrl, objects, terrains, terrain_dirty, camera);
                    }
                    ActiveLab::Sky => {
                        self.draw_sky_lab(ui, scene_ctrl);
                    }
                    ActiveLab::Water => {
                        self.draw_water_lab(ui, scene_ctrl);
                    }
                    ActiveLab::Materials => {
                        self.draw_materials_lab(ui, materials, objects, scene_ctrl, material_dirty);
                    }
                    ActiveLab::Scene => {
                        self.draw_scene_lab(ui, scene_ctrl, objects, meshes, camera, mesh_dirty, terrains, terrain_dirty);
                    }
                    ActiveLab::Render => {
                        self.draw_render_lab(ui, scene_ctrl, camera, materials, objects, terrains, terrain_dirty);
                    }
                }
            });
        });

        // =========================================================================
        // 3D VIEWPORT SELECTION BRACKETS & TRANSFORM GIZMO OVERLAY
        // =========================================================================
        self.draw_light_markers(ctx, camera, &scene_ctrl.lights, screen.width(), screen.height());
        self.draw_viewport_gizmo(ctx, camera, objects, screen.width(), screen.height());

        // =========================================================================
        // BOTTOM STATUS BAR
        // =========================================================================
        egui::Area::new(egui::Id::new("bricius_status_bar"))
            .fixed_pos(egui::pos2(0.0, screen.height() - 28.0))
            .show(ctx, |ui: &mut Ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 20, 24))
                    .inner_margin(egui::Margin { left: 16, right: 16, top: 4, bottom: 4 })
                    .show(ui, |ui: &mut Ui| {
                        ui.set_width(screen.width() - 32.0);
                        ui.horizontal(|ui: &mut Ui| {
                            ui.label(RichText::new("STATUS:").size(11.0).color(Color32::from_rgb(0, 200, 220)));
                            ui.label(RichText::new(&self.status_message).size(11.0).color(Color32::from_rgb(205, 210, 220)));
                        });
                    });
            });
    }

    // -------------------------------------------------------------------------
    // IN-STUDIO INTERACTIVE TUTORIAL & CONTROLS GUIDE
    // -------------------------------------------------------------------------
    fn draw_tutorial_lab(
        &mut self,
        ui: &mut Ui,
        scene_ctrl: &mut SceneController,
        camera: &mut Camera,
        materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
        objects: &mut Vec<SceneObject>,
        terrains: &[TerrainObjectData],
        material_dirty: &mut bool,
    ) {
        ui.heading("🎓 Bricius Interactive Studio Tutorial");
        ui.label(RichText::new("A complete hands-on guide to 3D environment creation, procedural algorithms, and studio controls.").size(12.0).color(Color32::from_rgb(180, 195, 215)));

        ui.separator();

        const TOTAL_STEPS: usize = 9;
        let step_titles = [
            "1. 🚀 Viewport & Fly Camera Navigation",
            "2. 🎲 1-Click Procedural Worlds & Rerolling",
            "3. ⛰️ Ken Musgrave Fractal Terrains & 2D Sculptor",
            "4. 🌅 Sky Lab & Celestial Atmospheric Volumetrics",
            "5. 🌊 Infinite Water Plane & Ocean Physics",
            "6. 🎨 Deep Texture Editor (DTE) & 3D Materials",
            "7. 🔮 3D Scene Composition & CSG Booleans",
            "8. 🌲 Vegetation & Rock Scatter System",
            "9. 🎥 Render Lab, Retro 1990s CGI & Project Export",
        ];

        // Step navigation selector
        ui.horizontal(|ui| {
            if ui.button("⬅ Previous").clicked() {
                self.tutorial_step = self.tutorial_step.saturating_sub(1);
            }

            for s in 0..TOTAL_STEPS {
                let is_cur = self.tutorial_step == s;
                let btn = if is_cur {
                    RichText::new(format!("{}", s + 1)).strong().color(Color32::from_rgb(0, 240, 255))
                } else {
                    RichText::new(format!("{}", s + 1)).color(Color32::from_rgb(160, 170, 185))
                };
                if ui.selectable_label(is_cur, btn).clicked() {
                    self.tutorial_step = s;
                }
            }

            if ui.button("Next ➡").clicked() {
                if self.tutorial_step + 1 < TOTAL_STEPS {
                    self.tutorial_step += 1;
                }
            }
        });

        ui.add(
            egui::ProgressBar::new((self.tutorial_step + 1) as f32 / TOTAL_STEPS as f32)
                .text(RichText::new(step_titles[self.tutorial_step]).strong().size(12.0))
        );

        ui.separator();

        match self.tutorial_step {
            0 => {
                // Step 1: Navigation
                ui.heading("Step 1: 6-DOF Fly Camera & Navigation Controls");
                ui.label("Bricius uses a continuous 3D coordinate system where you can freely navigate and frame your scene from any angle.");

                ui.group(|ui| {
                    ui.label(RichText::new("⌨️ Core Navigation Keys:").strong().color(Color32::from_rgb(0, 220, 240)));
                    egui::Grid::new("nav_keys_grid").spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
                        ui.label(RichText::new("W, A, S, D").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                        ui.label("Fly Camera forward, left, backward, and right in 3D space.");
                        ui.end_row();

                        ui.label(RichText::new("Space / Shift").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                        ui.label("Fly Camera altitude vertically: Space ascends, Shift descends.");
                        ui.end_row();

                        ui.label(RichText::new("LMB Drag").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                        ui.label("Click and drag Left Mouse Button in the viewport for 360° mouse look.");
                        ui.end_row();

                        ui.label(RichText::new("Tab").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                        ui.label("Toggle Studio UI on/off to enjoy an unobstructed cinematic view.");
                        ui.end_row();

                        ui.label(RichText::new("F1").monospace().strong().color(Color32::from_rgb(255, 205, 50)));
                        ui.label("Return to this interactive tutorial at any time.");
                        ui.end_row();
                    });
                });

                ui.add_space(8.0);
                ui.label(RichText::new("⚡ Interactive Camera Framing Presets:").strong());
                ui.label("Click any preset below to instantly move the camera to a classic framing angle:");

                ui.horizontal(|ui| {
                    if ui.button("🎥 Default Vista").clicked() {
                        camera.position = Vec3::new(0.0, 7.5, 24.0);
                        camera.yaw = -90.0_f32.to_radians();
                        camera.pitch = -8.0_f32.to_radians();
                        self.status_message = "Camera set to Default Landscape Vista.".to_string();
                    }
                    if ui.button("🏔️ Ground Low-Angle").clicked() {
                        camera.position = Vec3::new(0.0, 1.2, 14.0);
                        camera.yaw = -90.0_f32.to_radians();
                        camera.pitch = 18.0_f32.to_radians();
                        self.status_message = "Camera set to Dramatic Ground Low-Angle.".to_string();
                    }
                    if ui.button("🦅 Aerial Bird's Eye").clicked() {
                        camera.position = Vec3::new(0.0, 34.0, 22.0);
                        camera.yaw = -90.0_f32.to_radians();
                        camera.pitch = -50.0_f32.to_radians();
                        self.status_message = "Camera set to Aerial Bird's Eye View.".to_string();
                    }
                });

                ui.add_space(10.0);
                if ui.button("Next: 1-Click World Generation ➡").clicked() {
                    self.tutorial_step = 1;
                }
            }
            1 => {
                // Step 2: World Generation
                ui.heading("Step 2: One-Click Procedural Worlds & Themes");
                ui.label("One of Bricius's signature hallmarks is the ability to instantly generate an entire surreal or photorealistic world with a single click.");
                ui.label("A generated world configures fractal terrain elevation, atmospheric Rayleigh/Mie scattering, Gerstner ocean waves, and 3D compositions.");

                ui.group(|ui| {
                    ui.label(RichText::new("⚡ Try One-Click Themes:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.horizontal(|ui| {
                        if ui.button("🏜️ Classic 1994").clicked() {
                            let seed = rand_pseudo_u32();
                            SceneGenerator::generate_world(
                                SceneTheme::Classic1994Canyon,
                                seed,
                                &mut scene_ctrl.terrain,
                                &mut scene_ctrl.sky,
                                &mut scene_ctrl.water,
                                objects,
                                &mut scene_ctrl.postprocess,
                            );
                            self.status_message = "Generated Classic 1994 Canyon.".to_string();
                        }
                        if ui.button("🏔️ Alpine Fjord").clicked() {
                            let seed = rand_pseudo_u32();
                            SceneGenerator::generate_world(
                                SceneTheme::AlpineFjord,
                                seed,
                                &mut scene_ctrl.terrain,
                                &mut scene_ctrl.sky,
                                &mut scene_ctrl.water,
                                objects,
                                &mut scene_ctrl.postprocess,
                            );
                            self.status_message = "Generated Alpine Fjord & Glacial Lake.".to_string();
                        }
                        if ui.button("🪐 Alien Pandora").clicked() {
                            let seed = rand_pseudo_u32();
                            SceneGenerator::generate_world(
                                SceneTheme::AlienPandora,
                                seed,
                                &mut scene_ctrl.terrain,
                                &mut scene_ctrl.sky,
                                &mut scene_ctrl.water,
                                objects,
                                &mut scene_ctrl.postprocess,
                            );
                            self.status_message = "Generated Alien World of Pandora.".to_string();
                        }
                    });
                });

                ui.add_space(8.0);
                ui.label(RichText::new("🎲 Independent Component Rerolling:").strong());
                ui.label("You can reroll individual components without destroying the rest of your scene:");

                ui.horizontal(|ui| {
                    if ui.button("🎲 Reroll Sky Only").clicked() {
                        SceneGenerator::reroll_sky(rand_pseudo_u32(), &mut scene_ctrl.sky);
                        self.status_message = "Rerolled sky without touching terrain or objects.".to_string();
                    }
                    if ui.button("🎲 Reroll Terrain Only").clicked() {
                        SceneGenerator::reroll_terrain(rand_pseudo_u32(), &mut scene_ctrl.terrain);
                        self.status_message = "Rerolled terrain without touching sky or objects.".to_string();
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to 1-Click Generator Lab").clicked() {
                        self.active_lab = ActiveLab::Generator;
                    }
                    if ui.button("Next: Terrain Sculpting ➡").clicked() {
                        self.tutorial_step = 2;
                    }
                });
            }
            2 => {
                // Step 3: Terrain & Sculpting
                ui.heading("Step 3: Ken Musgrave Fractals & 2D Heightmap Canvas");
                ui.label("Terrain uses procedural noise algorithms pioneered by graphics researcher Ken Musgrave. You can choose from 6 procedural noise algorithms:");
                ui.label("• Fractional Brownian Motion (fBm): classic natural mountain ranges.\n• Ridged Multifractal: sharp mountain crests, peaks, and steep canyon gorges.\n• Billow Noise: smooth rolling sand dunes and soft hills.\n• Worley Cellular: basalt columns, rock fractures, and desert mesas.\n• Domain Warping: alien fluid swirls and folded rock strata.\n• Hybrid Multifractal: smooth valley floors transitioning into jagged cliffs.");

                ui.group(|ui| {
                    ui.label(RichText::new("⛰️ Live 2D Heightmap Sculptor Canvas:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.label("The Terrain Lab includes a live 2D elevation canvas. Left-click and drag on the map with procedural brushes to sculpt hills and valleys in real time:");
                    ui.label("• ⬆️ Raise / ⬇️ Lower: directly elevate or depress the landscape.\n• 🌊 Smooth: blend harsh spikes into gentle plateaus.\n• ⏹️ Flatten: level terrain to a specific altitude plane.\n• 🎲 Noise: inject rough procedural micro-relief.");
                });

                ui.add_space(8.0);
                ui.label(RichText::new("🌧️ Geological Erosion Passes:").strong());
                ui.label("Simulate natural weathering processes with a single click:");

                ui.label("Select a Terrain object (Scene Lab > Add Terrain), then open the Terrain Lab to sculpt it and run hydraulic / thermal erosion passes on its heightmap.");

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Terrain Lab & Sculptor").clicked() {
                        self.active_lab = ActiveLab::Terrain;
                    }
                    if ui.button("Next: Sky & Atmosphere ➡").clicked() {
                        self.tutorial_step = 3;
                    }
                });
            }
            3 => {
                // Step 4: Sky & Atmosphere
                ui.heading("Step 4: Sky Lab, Atmospheric Volumetrics & Celestial Dials");
                ui.label("The atmosphere is a physical volumetric simulation of light scattering through planetary air:");
                ui.label("• Rayleigh Scattering: Wavelength-selective scattering that creates deep azure blue zenith skies and fiery golden/red sunset horizons.\n• Mie Scattering: Aerosol particle scattering producing the intense solar corona glow, flare, and disc halo.\n• Volumetric Fog & Haze: Height-based ground fog and distance atmospheric perspective that provides majestic scale.");

                ui.group(|ui| {
                    ui.label(RichText::new("☀️ Celestial Mechanics Shortcuts:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.label("• Arrow Left / Right: Rotate Sun Azimuth 360° across the horizon.\n• Arrow Up / Down: Adjust Sun Elevation from dawn to high noon to twilight.");
                });

                ui.add_space(8.0);
                ui.label(RichText::new("🌅 Try Atmospheric Presets:").strong());

                ui.horizontal(|ui| {
                    if ui.button("🌅 Retro 90s Sunset").clicked() {
                        scene_ctrl.sky = crate::sky::SkyConfig::retro_sunset();
                        self.status_message = "Applied Retro 90s Sunset sky preset.".to_string();
                    }
                    if ui.button("☀️ Clear Midday").clicked() {
                        scene_ctrl.sky = crate::sky::SkyConfig::clear_blue();
                        self.status_message = "Applied Clear Blue Midday sky preset.".to_string();
                    }
                    if ui.button("⚡ Dramatic Storm").clicked() {
                        scene_ctrl.sky = crate::sky::SkyConfig::dramatic_storm();
                        self.status_message = "Applied Dramatic Storm sky preset.".to_string();
                    }
                    if ui.button("🌌 Starry Night").clicked() {
                        scene_ctrl.sky = crate::sky::SkyConfig::starry_night();
                        self.status_message = "Applied Starry Night sky preset.".to_string();
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Sky Lab").clicked() {
                        self.active_lab = ActiveLab::Sky;
                    }
                    if ui.button("Next: Water & Ocean ➡").clicked() {
                        self.tutorial_step = 4;
                    }
                });
            }
            4 => {
                // Step 5: Water & Ocean
                ui.heading("Step 5: Infinite Water Plane & Ocean Physics");
                ui.label("Water here is an infinite ocean plane with procedural waves and physical optics:");
                ui.label("• Multi-Component Gerstner Waves: Procedural sinusoidal wave displacement with controllable amplitude, frequency, and drift speed.\n• Physical Fresnel Reflections: At shallow grazing angles, water acts as a mirror reflecting the sky and mountains; at perpendicular view angles it reveals seabed depths.\n• Beer-Lambert Extinction: Light absorption causes water to transition naturally from turquoise shallows to deep navy abysses.");

                ui.group(|ui| {
                    ui.label(RichText::new("🌊 Try Ocean Presets:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.horizontal(|ui| {
                        if ui.button("🌊 Open Ocean Waves").clicked() {
                            { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::ocean(); scene_ctrl.water.enabled = keep_enabled; }
                            self.status_message = "Configured Open Ocean with Gerstner waves.".to_string();
                        }
                        if ui.button("🪞 Calm Mirror Glass").clicked() {
                            { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::calm_mirror(); scene_ctrl.water.enabled = keep_enabled; }
                            self.status_message = "Configured Calm Mirror-like water surface.".to_string();
                        }
                        if ui.button("🧪 Alien Bioluminescent").clicked() {
                            { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::alien_liquid(); scene_ctrl.water.enabled = keep_enabled; }
                            self.status_message = "Configured Alien Bioluminescent liquid.".to_string();
                        }
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Water Lab").clicked() {
                        self.active_lab = ActiveLab::Water;
                    }
                    if ui.button("Next: Materials & DTE ➡").clicked() {
                        self.tutorial_step = 5;
                    }
                });
            }
            5 => {
                // Step 6: Materials & DTE
                ui.heading("Step 6: Deep Texture Editor (DTE) & 3D Spatial Texturing");
                ui.label("The Deep Texture Editor (DTE) is the core of Bricius's material system. Instead of wrapping 2D image textures around objects, DTE materials evaluate continuously in 3D object and world space $(X, Y, Z)$:");
                ui.label("• Zero UV Unwrapping: No texture stretching, pinching at poles, or visible seam lines!\n• Optical Parameters: Base and Secondary color ramps, roughness, metallic, reflectivity (mirror bounces), transparency, and physical Index of Refraction (IOR).\n• Procedural Bump Relief: Surface normal perturbations evaluated via spatial gradient estimation without requiring normal map files.");

                ui.group(|ui| {
                    ui.label(RichText::new("🔮 Central Orb Material Quick-Swap:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.label("Press keys [1] through [8] anytime to instantly swap the material on the central showcase orb, or test them here:");
                    ui.horizontal(|ui| {
                        if ui.button("1: 🪨 Rock").clicked() {
                            materials[1] = GpuMaterial::rock();
                            *material_dirty = true;
                            self.status_message = "Applied Mountain Rock material.".to_string();
                        }
                        if ui.button("3: 🪞 90s Chrome").clicked() {
                            materials[1] = GpuMaterial::chrome();
                            *material_dirty = true;
                            self.status_message = "Applied 90s Mirror Chrome material.".to_string();
                        }
                        if ui.button("4: 💎 Emerald Glass").clicked() {
                            materials[1] = GpuMaterial::emerald_glass();
                            *material_dirty = true;
                            self.status_message = "Applied Emerald Glass material.".to_string();
                        }
                        if ui.button("8: 🪐 Bioluminescent").clicked() {
                            materials[1] = GpuMaterial::alien_bioluminescent();
                            *material_dirty = true;
                            self.status_message = "Applied Bioluminescent Coral material.".to_string();
                        }
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Materials Lab & DTE").clicked() {
                        self.active_lab = ActiveLab::Materials;
                    }
                    if ui.button("Next: 3D Objects & CSG ➡").clicked() {
                        self.tutorial_step = 6;
                    }
                });
            }
            6 => {
                // Step 7: Scene Objects & CSG
                ui.heading("Step 7: 3D Scene Composition & CSG Booleans");
                ui.label("Bricius allows composing intricate geometric forms using 3D primitives and Constructive Solid Geometry (CSG):");
                ui.label("• Primitives: Spheres, Cubes, Cylinders, and Toruses with full 3-axis Position, Rotation (Pitch, Yaw, Roll), and Scale.\n• Constructive Solid Geometry (CSG):\n  - Positive (Solid Union): standard solid object.\n  - Negative (Subtractive Cutout): punches physical cavities through solid primitives and terrain, with inverted normals and interior cutter material inheritance!\n  - Intersecting: preserves only the overlapping boolean volume.");

                ui.group(|ui| {
                    ui.label(RichText::new("➕ Try Adding Objects:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.horizontal(|ui| {
                        if ui.button("➕ Add Mirror Sphere").clicked() {
                            objects.push(SceneObject::sphere("Tutorial Sphere", [0.0, 4.0, -8.0], 1.8, 2));
                            self.status_message = "Added floating mirror sphere.".to_string();
                        }
                        if ui.button("➕ Add CSG Cutout Demo").clicked() {
                            let mut cube = SceneObject::cube("Outer Cube", [0.0, 3.5, -6.0], [2.0, 2.0, 2.0], 0);
                            cube.csg_op = CsgOp::Positive;
                            let mut cutter = SceneObject::sphere("Negative Cutter", [0.0, 3.5, -6.0], 2.3, 7);
                            cutter.csg_op = CsgOp::Negative;
                            objects.push(cube);
                            objects.push(cutter);
                            self.status_message = "Added CSG Demo: Negative sphere carving hollow interior inside cube!".to_string();
                        }
                        if ui.button("➕ Add Golden Torus").clicked() {
                            objects.push(SceneObject::torus("Golden Torus", [0.0, 5.0, -8.0], 3.0, 0.5, 4));
                            self.status_message = "Added golden torus primitive.".to_string();
                        }
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Scene Objects Lab").clicked() {
                        self.active_lab = ActiveLab::Scene;
                    }
                    if ui.button("Next: Scatter System ➡").clicked() {
                        self.tutorial_step = 7;
                    }
                });
            }
            7 => {
                // Step 8: Scatter System
                ui.heading("Step 8: Vegetation & Rock Scatter System");
                ui.label("Instead of placing hundreds of trees and boulders by hand, Bricius provides an automated procedural scatter system:");
                ui.label("• Altitude Masking: Set min and max elevation to ensure trees only grow in fertile valleys and don't spawn underwater or on snow peaks.\n• Slope Filtering: Restrict scatter to gentler slopes (e.g. max 35°), preventing trees from clinging to vertical sheer cliffs.\n• Density Controls: Independently adjust rock boulder density and pine tree spire density.");

                ui.group(|ui| {
                    ui.label(RichText::new("🌲 Scatter Simulation:").strong().color(Color32::from_rgb(0, 220, 240)));
                    if ui.button("🎲 Scatter Rocks & Trees across Terrain").clicked() {
                        let scattered = self.scatter_config.scatter_objects(&scene_ctrl.terrain);
                        let count = scattered.len();
                        objects.extend(scattered);
                        self.status_message = format!("Generated and scattered {} vegetation spires and rock boulders across terrain.", count);
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("🚀 Jump to Scene Lab Scatter Controls").clicked() {
                        self.active_lab = ActiveLab::Scene;
                    }
                    if ui.button("Next: Render & Export ➡").clicked() {
                        self.tutorial_step = 8;
                    }
                });
            }
            8 => {
                // Step 9: Render & Export
                ui.heading("Step 9: Render Lab, Retro 1990s CGI & Project Export");
                ui.label("You're ready to render and export your artwork! Bricius supports both modern high-fidelity rendering and authentic retro CGI aesthetics:");
                ui.label("• Retro 1990s CGI Mode: Emulates the punchy contrast, vibrant saturated color, and 16-bit / 24-bit color quantization of legendary 90s CGI renderers.\n• Render Quality Presets: Interactive Preview, Draft, Standard, High Quality, and Maximum Production.\n• Native Project Serialization: Save and restore entire worlds using human-readable .bricius.json project files.\n• 3D Mesh & Heightmap Export: Export 3D OBJ meshes and PNG elevation maps for external DCC software.");

                ui.group(|ui| {
                    ui.label(RichText::new("🎥 Render Lab Quick Actions:").strong().color(Color32::from_rgb(0, 220, 240)));
                    ui.horizontal(|ui| {
                        let retro_text = if scene_ctrl.postprocess.retro_90s_cgi { "Disable 90s Retro CGI" } else { "📺 Enable 90s Retro CGI" };
                        if ui.button(retro_text).clicked() {
                            scene_ctrl.postprocess.retro_90s_cgi = !scene_ctrl.postprocess.retro_90s_cgi;
                            self.status_message = format!("Retro 1990s CGI mode set to {}", scene_ctrl.postprocess.retro_90s_cgi);
                        }
                        if ui.button("💾 Save Project (.bricius.json)").clicked() {
                            let proj = ProjectFile {
                                name: "My Bricius World".to_string(),
                                version: "7.0.0".to_string(),
                                terrain: scene_ctrl.terrain.clone(),
                                sky: scene_ctrl.sky.clone(),
                                water: scene_ctrl.water.clone(),
                                ground: scene_ctrl.ground.clone(),
                                lights: scene_ctrl.lights.clone(),
                                materials: materials.to_vec(),
                                objects: objects.clone(),
                                postprocess: scene_ctrl.postprocess.clone(),
                                terrain_objects: terrains.iter().map(|t| t.to_save()).collect(),
                            };
                            let path = "my_bricius_world.bricius.json";
                            match ProjectIO::save_project(path, &proj) {
                                Ok(_) => self.status_message = format!("Saved project to {}", path),
                                Err(e) => self.status_message = format!("Save error: {}", e),
                            }
                        }
                    });
                });

                ui.add_space(10.0);
                ui.label(RichText::new("🎉 You are now fully trained in Bricius! Enjoy creating surreal landscapes and photorealistic worlds.").strong().color(Color32::from_rgb(0, 230, 160)));

                ui.horizontal(|ui| {
                    if ui.button("🚀 Open Render & Export Lab").clicked() {
                        self.active_lab = ActiveLab::Render;
                    }
                    if ui.button("🔄 Restart Tutorial from Step 1").clicked() {
                        self.tutorial_step = 0;
                    }
                });
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------------------
    // 1-CLICK WORLD GENERATOR LAB
    // -------------------------------------------------------------------------
    fn draw_generator_lab(
        &mut self,
        ui: &mut Ui,
        scene_ctrl: &mut SceneController,
        _materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
        objects: &mut Vec<SceneObject>,
    ) {
        ui.heading("One-Click Procedural Worlds");
        ui.label("Generate complete environments featuring terrain, skies, water, materials, and 3D objects:");

        ui.separator();

        let themes = [
            (SceneTheme::Classic1994Canyon, "🏜️ Classic 1994 Canyon", "Signature eroded red rocks, golden sunset, and floating mirror chrome orb."),
            (SceneTheme::AlpineFjord, "🏔️ Alpine Fjord & Glacial Lake", "Photorealistic mountain peaks, clear turquoise glacial water, and midday sun."),
            (SceneTheme::AlienPandora, "🪐 Alien World of Pandora", "Bioluminescent spires, purple atmosphere, cyan sun, and floating void ring."),
            (SceneTheme::SaharaOasis, "🐪 Sahara Desert Oasis", "Rolling billow sand dunes, desert haze, ancient altar, and emerald oasis pond."),
            (SceneTheme::VolcanicCaldera, "🌋 Volcanic Caldera & Ash Sky", "Obsidian rock cliffs, dark moody storm clouds, and magma core."),
            (SceneTheme::SurrealistChrome, "🔮 Surrealist Geometric Gallery", "Abstract floating chrome spheres, golden toruses, and mirror plane."),
        ];

        for (theme, title, desc) in themes {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).strong().size(15.0));
                    if ui.button("⚡ Generate").clicked() {
                        let seed = rand_pseudo_u32();
                        SceneGenerator::generate_world(
                            theme,
                            seed,
                            &mut scene_ctrl.terrain,
                            &mut scene_ctrl.sky,
                            &mut scene_ctrl.water,
                            objects,
                            &mut scene_ctrl.postprocess,
                        );
                        self.status_message = format!("Generated complete world: {}", title);
                    }
                });
                ui.label(RichText::new(desc).size(11.0).color(Color32::from_rgb(170, 180, 195)));
            });
        }

        ui.separator();
        ui.heading("Component-Wise Independent Regeneration");
        ui.label("Regenerate single aspects of the world without destroying other components:");

        ui.horizontal(|ui| {
            if ui.button("🎲 Reroll Sky Only").clicked() {
                SceneGenerator::reroll_sky(rand_pseudo_u32(), &mut scene_ctrl.sky);
                self.status_message = "Rerolled sky and atmosphere parameters.".to_string();
            }
            if ui.button("🎲 Reroll Terrain Only").clicked() {
                SceneGenerator::reroll_terrain(rand_pseudo_u32(), &mut scene_ctrl.terrain);
                self.status_message = "Rerolled terrain geometry.".to_string();
            }
            if ui.button("🎲 Reroll Water Only").clicked() {
                scene_ctrl.water.water_height = ((rand_pseudo_u32() % 20) as f32) * 0.1 - 1.0;
                scene_ctrl.water.wave_amplitude = 0.05 + ((rand_pseudo_u32() % 20) as f32) * 0.01;
                self.status_message = "Rerolled ocean wave parameters.".to_string();
            }
        });
    }

    // -------------------------------------------------------------------------
    // TERRAIN LAB & INTERACTIVE SCULPTOR
    // -------------------------------------------------------------------------
    fn draw_terrain_lab(
        &mut self,
        ui: &mut Ui,
        scene_ctrl: &mut SceneController,
        objects: &mut Vec<SceneObject>,
        terrains: &mut Vec<TerrainObjectData>,
        terrain_dirty: &mut bool,
        camera: &Camera,
    ) {
        // Selected terrain object (if any)
        let sel_obj = if self.selected_object_idx < objects.len()
            && objects[self.selected_object_idx].primitive == crate::objects::PrimitiveType::Terrain
        {
            Some(self.selected_object_idx)
        } else {
            None
        };
        let sel_tid = sel_obj
            .and_then(|i| objects[i].terrain_id)
            .filter(|&t| t < terrains.len());

        ui.heading("Terrain Objects");
        ui.label("Terrains are real scene objects: select one, move/scale/rotate it, and sculpt its heightmap here.");
        if ui.button("⛰️ Add Terrain").clicked() {
            self.add_terrain_object(objects, terrains, terrain_dirty, scene_ctrl, camera);
        }

        let Some(tid) = sel_tid else {
            ui.add_space(6.0);
            ui.label(RichText::new("No terrain object selected. Select a Terrain in the Scene Lab (or click it in the viewport), or add a new one above.").color(Color32::from_rgb(240, 190, 90)));
            self.draw_backdrop_section(ui, scene_ctrl);
            return;
        };
        let obj_name = objects[sel_obj.unwrap()].name.clone();
        ui.label(RichText::new(format!("Editing: {} (heightmap {}x{})", obj_name, terrains[tid].heightmap.resolution, terrains[tid].heightmap.resolution)).strong().color(Color32::from_rgb(0, 220, 240)));

        // Resolution
        ui.horizontal(|ui| {
            ui.label("Resolution:");
            for r in [128usize, 256, 512] {
                if ui.selectable_label(terrains[tid].heightmap.resolution == r, format!("{}", r)).clicked()
                    && terrains[tid].heightmap.resolution != r
                {
                    terrains[tid].heightmap.resample(r);
                    *terrain_dirty = true;
                    self.heightmap_dirty = true;
                    self.status_message = format!("Resampled terrain to {}x{}.", r, r);
                }
            }
        });

        ui.separator();
        ui.heading("Fractal Generator (into selected terrain)");
        Self::terrain_config_controls(ui, &mut terrains[tid].config, "terrain_obj_cfg");
        ui.horizontal(|ui| {
            if ui.button("⚡ Generate into selected terrain").clicked() {
                let cfg = terrains[tid].config.clone();
                terrains[tid].heightmap.generate_normalized(&cfg);
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Generated heightmap into selected terrain.".to_string();
            }
            if ui.button("📋 Copy settings from Backdrop").clicked() {
                terrains[tid].config = scene_ctrl.terrain.clone();
                self.status_message = "Copied backdrop fractal settings into the terrain generator.".to_string();
            }
        });

        // Keep the 2D preview texture in sync
        let key = (tid, terrains[tid].heightmap.resolution);
        if self.heightmap_dirty || self.heightmap_texture.is_none() || self.heightmap_preview_key != Some(key) {
            let res = terrains[tid].heightmap.resolution;
            let mut pixels = vec![Color32::BLACK; res * res];
            for i in 0..res * res {
                pixels[i] = height_to_hypsometric_color(terrains[tid].heightmap.data[i] * 17.0 - 2.0);
            }
            let img = egui::ColorImage::new([res, res], pixels);
            if let Some(ref mut handle) = self.heightmap_texture {
                handle.set(img, egui::TextureOptions::LINEAR);
            } else {
                self.heightmap_texture = Some(ui.ctx().load_texture("heightmap_preview", img, egui::TextureOptions::LINEAR));
            }
            self.heightmap_dirty = false;
            self.heightmap_preview_key = Some(key);
        }

        ui.separator();
        ui.heading("Interactive Heightmap Sculptor Canvas");
        ui.label("Click & drag directly on the elevation map to sculpt the selected terrain (updates the 3D view live):");

        let canvas_size = 256.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(canvas_size, canvas_size), egui::Sense::click_and_drag());

        if let Some(ref texture) = self.heightmap_texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.5, Color32::from_rgb(65, 75, 95)), egui::StrokeKind::Outside);

        let res = terrains[tid].heightmap.resolution;
        // Brush radius is expressed in 256-grid cells; scale to the actual resolution
        let radius_cells = ((self.brush_radius as f32) * res as f32 / 256.0).round().max(1.0) as usize;

        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rel_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let rel_y = ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                let hx = (rel_x * (res as f32 - 1.0)).round() as usize;
                let hy = (rel_y * (res as f32 - 1.0)).round() as usize;
                let t = &mut terrains[tid];
                t.heightmap.apply_brush(hx, hy, radius_cells, self.brush_strength, self.brush_mode, self.brush_target_height);
                t.clamp_heights();
                *terrain_dirty = true;
                self.heightmap_dirty = true;
            }
        }

        if let Some(pos) = response.hover_pos() {
            let pixel_scale = rect.width() / res as f32;
            ui.painter().circle_stroke(
                pos,
                radius_cells as f32 * pixel_scale,
                egui::Stroke::new(1.5, Color32::from_rgb(0, 240, 255)),
            );
        }

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.brush_mode, BrushMode::Raise, "⬆️ Raise");
            ui.selectable_value(&mut self.brush_mode, BrushMode::Lower, "⬇️ Lower");
            ui.selectable_value(&mut self.brush_mode, BrushMode::Smooth, "🌊 Smooth");
            ui.selectable_value(&mut self.brush_mode, BrushMode::Flatten, "⏹️ Flatten");
            ui.selectable_value(&mut self.brush_mode, BrushMode::Noise, "🎲 Noise");
        });

        ui.add(Slider::new(&mut self.brush_radius, 2..=32).text("Brush Radius"));
        ui.add(Slider::new(&mut self.brush_strength, 0.05..=1.5).text("Brush Strength"));
        if self.brush_mode == BrushMode::Flatten {
            ui.add(Slider::new(&mut self.brush_target_height, 0.0..=1.0).text("Flatten Target Height (0..1)"));
        }

        ui.separator();
        ui.heading("Geological Simulation & Operations");
        ui.horizontal(|ui| {
            if ui.button("🌧️ Hydraulic Rain Erosion").clicked() {
                terrains[tid].heightmap.simulate_hydraulic_erosion(4);
                terrains[tid].clamp_heights();
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Simulated 4 cycles of hydraulic rainfall & sediment deposition.".to_string();
            }
            if ui.button("🪨 Thermal Scree Weathering").clicked() {
                terrains[tid].heightmap.simulate_thermal_erosion(4);
                terrains[tid].clamp_heights();
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Simulated 4 cycles of thermal slope weathering.".to_string();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("🔄 Invert Heights").clicked() {
                for v in &mut terrains[tid].heightmap.data {
                    *v = 1.0 - *v;
                }
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Inverted terrain elevations (valleys to ridges).".to_string();
            }
            if ui.button("⚖️ Normalize Range").clicked() {
                let mut min_h = f32::INFINITY;
                let mut max_h = f32::NEG_INFINITY;
                for &h in &terrains[tid].heightmap.data {
                    min_h = min_h.min(h);
                    max_h = max_h.max(h);
                }
                let span = (max_h - min_h).max(0.001);
                for v in &mut terrains[tid].heightmap.data {
                    *v = (*v - min_h) / span;
                }
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Normalized terrain elevations to the full [0..1] range.".to_string();
            }
            if ui.button("🧹 Clear / Flatten").clicked() {
                for v in &mut terrains[tid].heightmap.data {
                    *v = 0.0;
                }
                *terrain_dirty = true;
                self.heightmap_dirty = true;
                self.status_message = "Cleared terrain to flat plane.".to_string();
            }
        });

        ui.separator();
        ui.heading("Terrain Export (selected terrain)");
        ui.horizontal(|ui| {
            if ui.button("💾 Export 3D Mesh (.OBJ)").clicked() {
                let path = "bricius_terrain.obj";
                let scale = objects[sel_obj.unwrap()].scale;
                match terrains[tid].heightmap.export_obj_scaled(path, scale) {
                    Ok(_) => self.status_message = format!("Exported 3D terrain mesh to {}", path),
                    Err(e) => self.status_message = format!("Export error: {}", e),
                }
            }
            if ui.button("🖼️ Export Heightmap (.PNG)").clicked() {
                let path = "bricius_heightmap.png";
                match terrains[tid].heightmap.export_png(path) {
                    Ok(_) => self.status_message = format!("Exported elevation heightmap to {}", path),
                    Err(e) => self.status_message = format!("Export error: {}", e),
                }
            }
        });

        self.draw_backdrop_section(ui, scene_ctrl);
    }

    /// Highest ground surface under (x, z): backdrop mountains (if enabled) and terrain objects
    /// (skipping `skip_tid`, the terrain being moved itself).
    fn ground_height_at(
        x: f32,
        z: f32,
        terrain_objs: &[SceneObject],
        terrains: &[TerrainObjectData],
        config: &TerrainConfig,
        skip_tid: Option<usize>,
    ) -> f32 {
        let mut h = if config.backdrop_enabled {
            crate::terrain::evaluate_procedural_height(x, z, config)
        } else {
            f32::NEG_INFINITY
        };
        for o in terrain_objs {
            let Some(tid) = o.terrain_id.filter(|&t| t < terrains.len() && Some(t) != skip_tid) else { continue };
            let yaw = o.rotation[1].to_radians();
            let local = glam::Mat3::from_rotation_y(yaw).transpose() * (glam::Vec3::new(x, 0.0, z) - glam::Vec3::new(o.position[0], 0.0, o.position[2]));
            let u = local.x / o.scale[0].max(1e-4) * 0.5 + 0.5;
            let v = local.z / o.scale[2].max(1e-4) * 0.5 + 0.5;
            if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                h = h.max(o.position[1] + terrains[tid].heightmap.sample_bilinear(u, v) * o.scale[1]);
            }
        }
        if h == f32::NEG_INFINITY { 0.0 } else { h }
    }

    /// Spawn a new terrain object in front of the camera, generated from the current backdrop config
    pub fn add_terrain_object(
        &mut self,
        objects: &mut Vec<SceneObject>,
        terrains: &mut Vec<TerrainObjectData>,
        terrain_dirty: &mut bool,
        scene_ctrl: &SceneController,
        camera: &Camera,
    ) {
        let fwd = camera.forward();
        let mut pos = [camera.position.x + fwd.x * 28.0, 0.0, camera.position.z + fwd.z * 28.0];
        pos[1] = scene_ctrl.water.water_height.max(scene_ctrl.ground.height) + 0.0;
        let tid = terrains.len();
        terrains.push(TerrainObjectData::new(256, &scene_ctrl.terrain));
        objects.push(SceneObject::terrain(&format!("Terrain {}", tid + 1), pos, [20.0, 6.0, 20.0], 0, tid));
        self.selected_object_idx = objects.len() - 1;
        self.heightmap_dirty = true;
        *terrain_dirty = true;
        self.status_message = "Added new Terrain object.".to_string();
    }

    fn draw_backdrop_section(&mut self, ui: &mut Ui, scene_ctrl: &mut SceneController) {
        ui.separator();
        ui.collapsing("🏔️ Backdrop mountains (global procedural terrain)", |ui| {
            ui.checkbox(&mut scene_ctrl.terrain.backdrop_enabled, "Backdrop mountains enabled");
            ui.label("Infinite procedural mountains raymarched behind everything. Independent of Terrain objects.");
            Self::terrain_config_controls(ui, &mut scene_ctrl.terrain, "terrain_backdrop_cfg");
        });
    }

    /// Fractal generator controls shared by the backdrop and terrain objects. Returns true if changed.
    fn terrain_config_controls(ui: &mut Ui, cfg: &mut TerrainConfig, salt: &str) -> bool {
        let mut algo = cfg.algorithm;
        ui.horizontal(|ui| {
            ui.label("Fractal Algorithm:");
            egui::ComboBox::from_id_salt(salt)
                .selected_text(match algo {
                    TerrainAlgorithm::Fbm => "Fractional Brownian Motion (fBm)",
                    TerrainAlgorithm::RidgedMultifractal => "Ridged Multifractal (Peaks & Canyons)",
                    TerrainAlgorithm::Billow => "Billow Noise (Rolling Hills / Dunes)",
                    TerrainAlgorithm::WorleyCellular => "Worley Cellular (Mesa Columns)",
                    TerrainAlgorithm::DomainWarped => "Domain-Warped Fractal (Alien Swirl)",
                    TerrainAlgorithm::HybridMultifractal => "Hybrid Multifractal (Smooth to Jagged)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut algo, TerrainAlgorithm::Fbm, "Fractional Brownian Motion (fBm)");
                    ui.selectable_value(&mut algo, TerrainAlgorithm::RidgedMultifractal, "Ridged Multifractal (Peaks & Canyons)");
                    ui.selectable_value(&mut algo, TerrainAlgorithm::Billow, "Billow Noise (Rolling Hills / Dunes)");
                    ui.selectable_value(&mut algo, TerrainAlgorithm::WorleyCellular, "Worley Cellular (Mesa Columns)");
                    ui.selectable_value(&mut algo, TerrainAlgorithm::DomainWarped, "Domain-Warped Fractal (Alien Swirl)");
                    ui.selectable_value(&mut algo, TerrainAlgorithm::HybridMultifractal, "Hybrid Multifractal (Smooth to Jagged)");
                });
        });
        let mut changed = algo != cfg.algorithm;
        cfg.algorithm = algo;
        changed |= ui.add(Slider::new(&mut cfg.height_scale, 0.1..=3.0).text("Height Scale")).changed();
        changed |= ui.add(Slider::new(&mut cfg.frequency, 0.01..=0.08).text("Frequency / Scale")).changed();
        changed |= ui.add(Slider::new(&mut cfg.octaves, 1..=6).text("Fractal Octaves")).changed();
        changed |= ui.add(Slider::new(&mut cfg.mountain_sharpness, 0.5..=3.5).text("Mountain Sharpness")).changed();
        changed |= ui.add(Slider::new(&mut cfg.ridge_intensity, 0.5..=3.0).text("Ridge Crest Intensity")).changed();
        changed |= ui.add(Slider::new(&mut cfg.valley_depth, 0.5..=2.5).text("Valley Depth")).changed();
        changed |= ui.add(Slider::new(&mut cfg.domain_warp, 0.0..=2.0).text("Domain Warping")).changed();
        ui.horizontal(|ui| {
            ui.label("Seed:");
            changed |= ui.add(egui::DragValue::new(&mut cfg.seed).speed(1.0)).changed();
        });
        changed
    }

    // -------------------------------------------------------------------------
    // SKY & ATMOSPHERE LAB
    // -------------------------------------------------------------------------
    fn draw_sky_lab(&mut self, ui: &mut Ui, scene_ctrl: &mut SceneController) {
        ui.heading("Celestial Mechanics");

        let mut sun_az_deg = scene_ctrl.sky.sun_azimuth.to_degrees();
        if ui.add(Slider::new(&mut sun_az_deg, 0.0..=360.0).text("Sun Azimuth (Orbit)")).changed() {
            scene_ctrl.sky.sun_azimuth = sun_az_deg.to_radians();
        }

        let mut sun_el_deg = scene_ctrl.sky.sun_elevation.to_degrees();
        if ui.add(Slider::new(&mut sun_el_deg, -15.0..=90.0).text("Sun Elevation (Height)")).changed() {
            scene_ctrl.sky.sun_elevation = sun_el_deg.to_radians();
        }

        ui.add(Slider::new(&mut scene_ctrl.sky.sun_intensity, 0.2..=5.0).text("Sun Radiance Intensity"));
        ui.add(Slider::new(&mut scene_ctrl.sky.sun_size, 0.5..=4.0).text("Sun Disc & Corona Scale"));

        ui.separator();
        ui.heading("Atmospheric Scattering & Volumetrics");
        ui.add(Slider::new(&mut scene_ctrl.sky.fog_density, 0.001..=0.05).text("Distance Fog / Haze Density"));
        ui.add(Slider::new(&mut scene_ctrl.sky.mie_coeff, 0.1..=3.0).text("Mie Solar Corona Glow"));

        ui.separator();
        ui.heading("Procedural Cloud Layer");
        ui.add(Slider::new(&mut scene_ctrl.sky.cloud_coverage, 0.0..=1.5).text("Cloud Coverage"));
        ui.add(Slider::new(&mut scene_ctrl.sky.cloud_altitude, 20.0..=120.0).text("Cloud Ceiling Altitude"));
        ui.add(Slider::new(&mut scene_ctrl.sky.cloud_wind_speed, 0.0..=4.0).text("Cloud Wind Speed"));

        ui.separator();
        ui.heading("Atmospheric Color Gradients");
        ui.horizontal(|ui| {
            ui.label("Zenith Sky Color:   ");
            ui.color_edit_button_rgba_unmultiplied(&mut scene_ctrl.sky.zenith_color);
        });
        ui.horizontal(|ui| {
            ui.label("Horizon Tint Color: ");
            ui.color_edit_button_rgba_unmultiplied(&mut scene_ctrl.sky.horizon_color);
        });
        ui.horizontal(|ui| {
            ui.label("Distance Fog Color: ");
            ui.color_edit_button_rgba_unmultiplied(&mut scene_ctrl.sky.fog_color);
        });
    }

    // -------------------------------------------------------------------------
    // WATER & OCEAN LAB
    // -------------------------------------------------------------------------
    fn draw_water_lab(&mut self, ui: &mut Ui, scene_ctrl: &mut SceneController) {
        ui.heading("Ocean & Liquid Surface Properties");
        ui.checkbox(&mut scene_ctrl.water.enabled, "Water plane enabled");
        ui.checkbox(&mut scene_ctrl.water.animate, "Animate waves (disables progressive accumulation)");

        let mut water_type = scene_ctrl.water.water_type;
        ui.horizontal(|ui| {
            ui.label("Water Environment:");
            egui::ComboBox::from_id_salt("water_type_select")
                .selected_text(match water_type {
                    WaterType::Ocean => "Dynamic Ocean (Gerstner Waves)",
                    WaterType::Lake => "Calm Alpine Lake",
                    WaterType::CalmMirror => "Retro 1994 Pure Mirror",
                    WaterType::AlienLiquid => "Glowing Alien Liquid",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut water_type, WaterType::Ocean, "Dynamic Ocean (Gerstner Waves)").clicked() {
                        { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::ocean(); scene_ctrl.water.enabled = keep_enabled; }
                    }
                    if ui.selectable_value(&mut water_type, WaterType::Lake, "Calm Alpine Lake").clicked() {
                        { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::lake(); scene_ctrl.water.enabled = keep_enabled; }
                    }
                    if ui.selectable_value(&mut water_type, WaterType::CalmMirror, "Retro 1994 Pure Mirror").clicked() {
                        { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::calm_mirror(); scene_ctrl.water.enabled = keep_enabled; }
                    }
                    if ui.selectable_value(&mut water_type, WaterType::AlienLiquid, "Glowing Alien Liquid").clicked() {
                        { let keep_enabled = scene_ctrl.water.enabled; scene_ctrl.water = crate::water::WaterConfig::alien_liquid(); scene_ctrl.water.enabled = keep_enabled; }
                    }
                });
        });

        ui.add(Slider::new(&mut scene_ctrl.water.water_height, -3.0..=4.0).text("Water Surface Height"));
        ui.add(Slider::new(&mut scene_ctrl.water.wave_amplitude, 0.0..=0.5).text("Wave Amplitude"));
        ui.add(Slider::new(&mut scene_ctrl.water.wave_frequency, 0.2..=4.0).text("Wave Frequency"));
        ui.add(Slider::new(&mut scene_ctrl.water.wave_speed, 0.0..=4.0).text("Wave Motion Speed"));
        ui.add(Slider::new(&mut scene_ctrl.water.reflection_strength, 0.0..=1.0).text("Fresnel Reflection"));
        ui.add(Slider::new(&mut scene_ctrl.water.ior, 1.0..=2.0).text("Index of Refraction (IOR)"));

        ui.separator();
        ui.heading("Water Optical Absorption Colors");
        ui.horizontal(|ui| {
            ui.label("Deep Abyss Color:   ");
            ui.color_edit_button_rgba_unmultiplied(&mut scene_ctrl.water.deep_color);
        });
        ui.horizontal(|ui| {
            ui.label("Shallow Shore Color:");
            ui.color_edit_button_rgba_unmultiplied(&mut scene_ctrl.water.shallow_color);
        });
    }

    // -------------------------------------------------------------------------
    // MATERIALS LAB & DEEP TEXTURE EDITOR (DTE)
    // -------------------------------------------------------------------------
    fn draw_materials_lab(
        &mut self,
        ui: &mut Ui,
        materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
        objects: &mut [SceneObject],
        scene_ctrl: &mut SceneController,
        material_dirty: &mut bool,
    ) {
        ui.heading("Custom Material Studio & DTE");

        ui.horizontal(|ui| {
            let slot_name = &self.material_names[self.selected_material_slot];
            egui::ComboBox::from_label("")
                .selected_text(format!("Slot {}: {}", self.selected_material_slot, slot_name))
                .show_ui(ui, |ui| {
                    for i in 0..GpuMaterial::NUM_PRESETS {
                        ui.selectable_value(
                            &mut self.selected_material_slot,
                            i,
                            format!("Slot {}: {}", i, self.material_names[i]),
                        );
                    }
                });

            if !objects.is_empty() && self.selected_object_idx < objects.len() {
                let obj_name = &objects[self.selected_object_idx].name;
                if ui.button(RichText::new(format!("🎯 Assign to '{}'", obj_name)).color(Color32::from_rgb(0, 230, 255)).strong()).clicked() {
                    objects[self.selected_object_idx].material_id = self.selected_material_slot as u32;
                    self.status_message = format!("Assigned Material Slot {} ({}) to '{}'", self.selected_material_slot, self.material_names[self.selected_material_slot], obj_name);
                }
            }

            if ui.button("Link to Showcase Orb").clicked() {
                scene_ctrl.selected_material = self.selected_material_slot as u32;
                self.status_message = format!("Linked slot {} to central showcase orb", self.selected_material_slot);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Material Label:");
            ui.text_edit_singleline(&mut self.material_names[self.selected_material_slot]);

            if ui.button("📋 Copy").clicked() {
                self.copied_material = Some(materials[self.selected_material_slot]);
                self.status_message = format!("Copied Material Slot {} to clipboard", self.selected_material_slot);
            }
            if ui.button("📥 Paste").clicked() {
                if let Some(copied) = self.copied_material {
                    materials[self.selected_material_slot] = copied;
                    *material_dirty = true;
                    self.status_message = format!("Pasted copied material into Slot {}", self.selected_material_slot);
                }
            }
            if ui.button("💾 Save JSON").clicked() {
                let mat = &materials[self.selected_material_slot];
                if let Ok(json) = serde_json::to_string_pretty(mat) {
                    let filename = format!("custom_material_slot_{}.json", self.selected_material_slot);
                    let _ = std::fs::write(&filename, json);
                    self.status_message = format!("Exported material to {}", filename);
                }
            }
            if ui.button("📂 Load JSON").clicked() {
                let filename = format!("custom_material_slot_{}.json", self.selected_material_slot);
                if let Ok(content) = std::fs::read_to_string(&filename) {
                    if let Ok(mat) = serde_json::from_str::<GpuMaterial>(&content) {
                        materials[self.selected_material_slot] = mat;
                        *material_dirty = true;
                        self.status_message = format!("Imported material from {}", filename);
                    }
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("Preset Material Library (One-Click)").strong());
        ui.horizontal_wrapped(|ui| {
            let presets = [
                ("Rock", GpuMaterial::rock()),
                ("Marble", GpuMaterial::blue_marble()),
                ("Chrome", GpuMaterial::chrome()),
                ("Glass", GpuMaterial::emerald_glass()),
                ("Gold", GpuMaterial::gold()),
                ("Obsidian", GpuMaterial::basalt_obsidian()),
                ("Water", GpuMaterial::ocean_water()),
                ("Bio-Coral", GpuMaterial::alien_bioluminescent()),
                ("Plastic", GpuMaterial::retro_plastic()),
                ("Sandstone", GpuMaterial::sandstone()),
                ("Ice", GpuMaterial::glacial_ice()),
                ("Wood", GpuMaterial::wood_grain()),
                ("Rust", GpuMaterial::rusted_iron()),
                ("Amethyst", GpuMaterial::amethyst_crystal()),
                ("Cyber", GpuMaterial::cyber_grid()),
                ("Desert", GpuMaterial::red_desert_rock()),
            ];

            for (name, mat) in presets {
                if ui.button(name).clicked() {
                    materials[self.selected_material_slot] = mat;
                    self.material_names[self.selected_material_slot] = name.to_string();
                    *material_dirty = true;
                    self.status_message = format!("Loaded preset '{}' into slot {}", name, self.selected_material_slot);
                }
            }
        });

        ui.separator();
        let mat = &mut materials[self.selected_material_slot];

        ui.collapsing("🎨 Color & Strata Channels", |ui| {
            ui.horizontal(|ui| {
                ui.label("Base Diffuse Color:     ");
                if ui.color_edit_button_rgba_unmultiplied(&mut mat.base_color).changed() {
                    *material_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Secondary Vein Color: ");
                if ui.color_edit_button_rgba_unmultiplied(&mut mat.secondary_color).changed() {
                    *material_dirty = true;
                }
            });
        });

        ui.collapsing("💎 Optical & Surface Properties", |ui| {
            if ui.add(Slider::new(&mut mat.params1[0], 0.01..=1.0).text("Roughness")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params1[1], 0.0..=1.0).text("Metallic")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params1[2], 0.0..=1.0).text("Mirror Reflectivity")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params1[3], 0.0..=1.0).text("Transparency")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params2[0], 1.0..=2.5).text("Index of Refraction (IOR)")).changed() {
                *material_dirty = true;
            }
        });

        ui.collapsing("🌀 Deep Texture Editor (3D Procedural Engine)", |ui| {
            let mut noise_type = mat.params2[3] as u32;
            ui.horizontal(|ui| {
                ui.label("3D Noise Algorithm:");
                egui::ComboBox::from_id_salt("dte_noise_type_lab")
                    .selected_text(match noise_type {
                        0 => "Solid / Uniform",
                        1 => "Ken Musgrave fBm Granite",
                        2 => "Ken Musgrave Ridged Multifractal",
                        3 => "Marble Swirl (Domain Warping)",
                        4 => "Worley Cellular / Rock Fractures",
                        _ => "Custom",
                    })
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut noise_type, 0, "Solid / Uniform").clicked() {
                            mat.params2[3] = 0.0;
                            *material_dirty = true;
                        }
                        if ui.selectable_value(&mut noise_type, 1, "Ken Musgrave fBm Granite").clicked() {
                            mat.params2[3] = 1.0;
                            *material_dirty = true;
                        }
                        if ui.selectable_value(&mut noise_type, 2, "Ken Musgrave Ridged Multifractal").clicked() {
                            mat.params2[3] = 2.0;
                            *material_dirty = true;
                        }
                        if ui.selectable_value(&mut noise_type, 3, "Marble Swirl (Domain Warping)").clicked() {
                            mat.params2[3] = 3.0;
                            *material_dirty = true;
                        }
                        if ui.selectable_value(&mut noise_type, 4, "Worley Cellular / Rock Fractures").clicked() {
                            mat.params2[3] = 4.0;
                            *material_dirty = true;
                        }
                    });
            });

            if ui.add(Slider::new(&mut mat.params2[2], 0.1..=12.0).text("Noise Frequency / Scale")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params3[1], 1.0..=6.0).text("Fractal Octaves")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params3[0], 0.0..=5.0).text("Domain Warp / Turbulence")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params3[2], 0.2..=5.0).text("Blend Sharpness / Contrast")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params2[1], 0.0..=1.5).text("Procedural Bump Relief")).changed() {
                *material_dirty = true;
            }
            if ui.add(Slider::new(&mut mat.params3[3], 0.0..=3.0).text("Emissive Glow")).changed() {
                *material_dirty = true;
            }
        });
    }

    // -------------------------------------------------------------------------
    // SCENE OBJECTS & HIERARCHY LAB
    // -------------------------------------------------------------------------
    fn draw_scene_lab(
        &mut self,
        ui: &mut Ui,
        scene_ctrl: &mut SceneController,
        objects: &mut Vec<SceneObject>,
        meshes: &mut Vec<crate::model_loader::LoadedMesh>,
        camera: &mut Camera,
        mesh_dirty: &mut bool,
        terrains: &mut Vec<TerrainObjectData>,
        terrain_dirty: &mut bool,
    ) {
        ui.heading("3D Scene Composition & Custom Models");

        self.draw_lights_section(ui, scene_ctrl, camera);
        ui.collapsing("Ground plane", |ui| {
            ui.checkbox(&mut scene_ctrl.ground.enabled, "Ground plane enabled");
            ui.add(Slider::new(&mut scene_ctrl.ground.height, -20.0..=20.0).text("Ground height"));
            let gid = (scene_ctrl.ground.material_id as usize).min(GpuMaterial::NUM_PRESETS - 1);
            egui::ComboBox::from_id_salt("ground_mat_select")
                .selected_text(format!("Slot {}: {}", gid, self.material_names[gid]))
                .show_ui(ui, |ui| {
                    for m_i in 0..GpuMaterial::NUM_PRESETS {
                        ui.selectable_value(&mut scene_ctrl.ground.material_id, m_i as u32, format!("Slot {}: {}", m_i, self.material_names[m_i]));
                    }
                });
        });
        ui.separator();

        ui.label(RichText::new("➕ Create Geometric Primitives:").strong());
        ui.horizontal(|ui| {
            if ui.button("⚪ Sphere").clicked() {
                let spawn_pos = [camera.position.x + camera.forward().x * 6.0, camera.position.y + camera.forward().y * 6.0, camera.position.z + camera.forward().z * 6.0];
                objects.push(SceneObject::sphere(&format!("Sphere {}", objects.len() + 1), spawn_pos, 1.5, scene_ctrl.selected_material));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Sphere primitive.".to_string();
            }
            if ui.button("🧊 Cube").clicked() {
                let spawn_pos = [camera.position.x + camera.forward().x * 6.0, camera.position.y + camera.forward().y * 6.0, camera.position.z + camera.forward().z * 6.0];
                objects.push(SceneObject::cube(&format!("Cube {}", objects.len() + 1), spawn_pos, [1.0, 1.0, 1.0], scene_ctrl.selected_material));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Cube primitive.".to_string();
            }
            if ui.button("🛢️ Cylinder").clicked() {
                let spawn_pos = [camera.position.x + camera.forward().x * 6.0, camera.position.y + camera.forward().y * 6.0, camera.position.z + camera.forward().z * 6.0];
                objects.push(SceneObject::cylinder(&format!("Cylinder {}", objects.len() + 1), spawn_pos, 1.0, 2.0, scene_ctrl.selected_material));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Cylinder primitive.".to_string();
            }
            if ui.button("🍩 Torus").clicked() {
                let spawn_pos = [camera.position.x + camera.forward().x * 6.0, camera.position.y + camera.forward().y * 6.0, camera.position.z + camera.forward().z * 6.0];
                objects.push(SceneObject::torus(&format!("Torus {}", objects.len() + 1), spawn_pos, 2.0, 0.4, scene_ctrl.selected_material));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Torus primitive.".to_string();
            }
            if ui.button("⛰️ Add Terrain").clicked() {
                self.add_terrain_object(objects, terrains, terrain_dirty, scene_ctrl, camera);
            }
        });

        ui.separator();
        ui.label(RichText::new("🗿 Import & Spawn 3D Mesh Models (.obj, .glb, .gltf, .fbx):").strong().color(Color32::from_rgb(0, 220, 240)));
        ui.horizontal_wrapped(|ui| {
            if ui.button("🫖 Utah Teapot").clicked() {
                let spawn_x = camera.position.x + camera.forward().x * 6.0;
                let spawn_z = camera.position.z + camera.forward().z * 6.0;
                let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                let spawn_y = ground_y + 1.8;
                objects.push(SceneObject::mesh("Utah Teapot", [spawn_x, spawn_y, spawn_z], [1.5, 1.5, 1.5], scene_ctrl.selected_material, 0));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Spawned Utah Teapot model".to_string();
            }
            if ui.button("🏛️ Desert Monolith").clicked() {
                let spawn_x = camera.position.x + camera.forward().x * 7.0;
                let spawn_z = camera.position.z + camera.forward().z * 7.0;
                let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                let spawn_y = ground_y + 2.2;
                objects.push(SceneObject::mesh("Desert Monolith", [spawn_x, spawn_y, spawn_z], [1.5, 1.5, 1.5], scene_ctrl.selected_material, 1));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Spawned Desert Monolith model".to_string();
            }
            if ui.button("💎 Crystal Geode").clicked() {
                let spawn_x = camera.position.x + camera.forward().x * 6.5;
                let spawn_z = camera.position.z + camera.forward().z * 6.5;
                let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                let spawn_y = ground_y + 1.8;
                objects.push(SceneObject::mesh("Crystal Geode", [spawn_x, spawn_y, spawn_z], [1.5, 1.5, 1.5], scene_ctrl.selected_material, 2));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Spawned Crystal Geode model".to_string();
            }
            if ui.button("🚀 Starfighter").clicked() {
                let spawn_x = camera.position.x + camera.forward().x * 7.5;
                let spawn_z = camera.position.z + camera.forward().z * 7.5;
                let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                let spawn_y = ground_y + 3.0; // Hovering flight altitude
                objects.push(SceneObject::mesh("Starfighter Interceptor", [spawn_x, spawn_y, spawn_z], [1.5, 1.5, 1.5], scene_ctrl.selected_material, 3));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Spawned Starfighter model".to_string();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("📂 Browse File...").clicked() {
                if let Some(file_path) = rfd::FileDialog::new()
                    .add_filter("3D Models (*.obj, *.glb, *.gltf, *.fbx)", &["obj", "glb", "gltf", "fbx"])
                    .pick_file()
                {
                    let path_str = file_path.to_string_lossy().to_string();
                    self.model_file_path_input = path_str.clone();
                    match crate::model_loader::ModelLoader::load_from_file(&path_str) {
                        Ok(loaded_mesh) => {
                            let name = loaded_mesh.name.clone();
                            let tri_cnt = loaded_mesh.triangle_count();
                            let mesh_idx = meshes.len();
                            meshes.push(loaded_mesh);
                            *mesh_dirty = true;
                            let spawn_x = camera.position.x + camera.forward().x * 6.5;
                            let spawn_z = camera.position.z + camera.forward().z * 6.5;
                            let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                            let spawn_y = ground_y + 1.8;
                            objects.push(SceneObject::mesh(
                                &name,
                                [spawn_x, spawn_y, spawn_z],
                                [1.5, 1.5, 1.5],
                                scene_ctrl.selected_material,
                                mesh_idx,
                            ));
                            self.selected_object_idx = objects.len() - 1;
                            self.status_message = format!("Successfully imported '{}' ({} triangles)", name, tri_cnt);
                        }
                        Err(e) => {
                            self.status_message = format!("Import error: {}", e);
                        }
                    }
                }
            }

            ui.label("Path:");
            ui.add(egui::TextEdit::singleline(&mut self.model_file_path_input).hint_text("model.obj, .glb, .gltf, .fbx"));
            if ui.button("📥 Load").clicked() {
                let trimmed = self.model_file_path_input.trim().trim_matches('"').trim();
                if !trimmed.is_empty() {
                    match crate::model_loader::ModelLoader::load_from_file(trimmed) {
                        Ok(loaded_mesh) => {
                            let name = loaded_mesh.name.clone();
                            let tri_cnt = loaded_mesh.triangle_count();
                            let mesh_idx = meshes.len();
                            meshes.push(loaded_mesh);
                            *mesh_dirty = true;
                            let spawn_x = camera.position.x + camera.forward().x * 6.5;
                            let spawn_z = camera.position.z + camera.forward().z * 6.5;
                            let ground_y = crate::terrain::evaluate_procedural_height(spawn_x, spawn_z, &scene_ctrl.terrain);
                            let spawn_y = ground_y + 1.8;
                            objects.push(SceneObject::mesh(
                                &name,
                                [spawn_x, spawn_y, spawn_z],
                                [1.5, 1.5, 1.5],
                                scene_ctrl.selected_material,
                                mesh_idx,
                            ));
                            self.selected_object_idx = objects.len() - 1;
                            self.status_message = format!("Successfully imported '{}' ({} triangles)", name, tri_cnt);
                        }
                        Err(e) => {
                            self.status_message = format!("Import error: {}", e);
                        }
                    }
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("Scene Objects & Hierarchy").strong());

        let terrain_objs: Vec<SceneObject> = objects
            .iter()
            .filter(|o| o.primitive == crate::objects::PrimitiveType::Terrain && o.visible)
            .cloned()
            .collect();
        for (i, obj) in objects.iter_mut().enumerate() {
            let is_sel = self.selected_object_idx == i;
            ui.horizontal(|ui| {
                ui.checkbox(&mut obj.visible, "");
                let csg_tag = match obj.csg_op {
                    CsgOp::Positive => " [+] ",
                    CsgOp::Negative => " [-] ",
                    CsgOp::Intersecting => " [∩] ",
                };
                if ui.selectable_label(is_sel, format!("{}{}: {}", csg_tag, i + 1, obj.name)).clicked() {
                    self.selected_object_idx = i;
                }
            });

            if is_sel {
                ui.indent(format!("obj_edit_{}", i), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut obj.name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("CSG Boolean Op:");
                        egui::ComboBox::from_id_salt(format!("csg_select_{}", i))
                            .selected_text(match obj.csg_op {
                                CsgOp::Positive => "Positive (Solid Union)",
                                CsgOp::Negative => "Negative (Subtractive Cutout)",
                                CsgOp::Intersecting => "Intersecting (Intersection)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut obj.csg_op, CsgOp::Positive, "Positive (Solid Union)");
                                ui.selectable_value(&mut obj.csg_op, CsgOp::Negative, "Negative (Subtractive Cutout)");
                                ui.selectable_value(&mut obj.csg_op, CsgOp::Intersecting, "Intersecting (Intersection)");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Material:");
                        let mat_id = obj.material_id.min(15) as usize;
                        let mat_name = &self.material_names[mat_id];
                        egui::ComboBox::from_id_salt(format!("obj_mat_select_{}", i))
                            .selected_text(format!("Slot {}: {}", mat_id, mat_name))
                            .show_ui(ui, |ui| {
                                for m_i in 0..GpuMaterial::NUM_PRESETS {
                                    if ui.selectable_value(&mut obj.material_id, m_i as u32, format!("Slot {}: {}", m_i, self.material_names[m_i])).clicked() {
                                        self.status_message = format!("Set material of '{}' to Slot {}", obj.name, m_i);
                                    }
                                }
                            });
                    });

                    ui.label(RichText::new("Transform & Alignment:").strong());
                    ui.horizontal(|ui| {
                        ui.label("Position:");
                        ui.add(egui::DragValue::new(&mut obj.position[0]).speed(0.1).prefix("X: "));
                        ui.add(egui::DragValue::new(&mut obj.position[1]).speed(0.1).prefix("Y: "));
                        ui.add(egui::DragValue::new(&mut obj.position[2]).speed(0.1).prefix("Z: "));
                        if ui.button("🌱 Snap to Ground").clicked() {
                            let h = Self::ground_height_at(obj.position[0], obj.position[2], &terrain_objs, terrains, &scene_ctrl.terrain, obj.terrain_id);
                            obj.position[1] = h + if obj.primitive == crate::objects::PrimitiveType::Terrain { 0.0 } else { obj.scale[1] };
                            self.status_message = format!("Snapped '{}' to terrain ground height {:.2}", obj.name, obj.position[1]);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rotation:");
                        ui.add(egui::DragValue::new(&mut obj.rotation[0]).speed(1.0).suffix("°"));
                        ui.add(egui::DragValue::new(&mut obj.rotation[1]).speed(1.0).suffix("°"));
                        ui.add(egui::DragValue::new(&mut obj.rotation[2]).speed(1.0).suffix("°"));
                        if ui.small_button("🔄 0°").clicked() {
                            obj.rotation = [0.0, 0.0, 0.0];
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Scale:   ");
                        ui.checkbox(&mut self.uniform_scale_locked, "🔒 Uniform");
                        if self.uniform_scale_locked {
                            let mut uniform_val = obj.scale[0];
                            if ui.add(egui::DragValue::new(&mut uniform_val).speed(0.05).prefix("Size: ")).changed() {
                                let clamped = uniform_val.clamp(0.05, 50.0);
                                obj.scale = [clamped, clamped, clamped];
                            }
                        } else {
                            ui.add(egui::DragValue::new(&mut obj.scale[0]).speed(0.05).prefix("Sx: "));
                            ui.add(egui::DragValue::new(&mut obj.scale[1]).speed(0.05).prefix("Sy: "));
                            ui.add(egui::DragValue::new(&mut obj.scale[2]).speed(0.05).prefix("Sz: "));
                        }
                    });
                });
            }
        }

        if !objects.is_empty() && self.selected_object_idx < objects.len() {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("📋 Duplicate Object").clicked() {
                    let mut cloned = objects[self.selected_object_idx].clone();
                    cloned.name = format!("{} (Copy)", cloned.name);
                    cloned.position[0] += 2.0;
                    if let Some(tid) = cloned.terrain_id.filter(|&t| t < terrains.len()) {
                        let copy = terrains[tid].clone();
                        terrains.push(copy);
                        cloned.terrain_id = Some(terrains.len() - 1);
                        *terrain_dirty = true;
                    }
                    objects.push(cloned);
                    self.selected_object_idx = objects.len() - 1;
                    self.status_message = "Duplicated selected object.".to_string();
                }
                if ui.button("🎯 Frame / Focus Camera").clicked() {
                    let target = glam::Vec3::from(objects[self.selected_object_idx].position);
                    camera.position = target - camera.forward() * 7.0 + glam::Vec3::Y * 2.0;
                    self.status_message = format!("Framed camera onto '{}'", objects[self.selected_object_idx].name);
                }
                if ui.button("🗑️ Delete Selected Object").clicked() {
                    objects.remove(self.selected_object_idx);
                    self.selected_object_idx = self.selected_object_idx.saturating_sub(1);
                    self.status_message = "Deleted object.".to_string();
                }
            });
        }

        ui.separator();
        ui.heading("🌲 Vegetation & Rock Scatter System");
        ui.label("Distribute procedural rocks and vegetation clusters across terrain elevation and slope:");
        ui.checkbox(&mut self.scatter_config.enabled, "Enable Scatter System");
        ui.add(Slider::new(&mut self.scatter_config.rock_density, 0.0..=1.0).text("Rock Boulder Density"));
        ui.add(Slider::new(&mut self.scatter_config.tree_density, 0.0..=1.0).text("Tree / Spire Density"));
        ui.add(Slider::new(&mut self.scatter_config.min_elevation, -5.0..=25.0).text("Min Elevation"));
        ui.add(Slider::new(&mut self.scatter_config.max_elevation, 0.0..=50.0).text("Max Elevation"));
        ui.add(Slider::new(&mut self.scatter_config.max_slope_deg, 5.0..=80.0).text("Max Slope Angle"));

        if ui.button("🎲 Scatter Rocks & Trees across Terrain").clicked() {
            let scattered = self.scatter_config.scatter_objects(&scene_ctrl.terrain);
            let count = scattered.len();
            objects.extend(scattered);
            self.status_message = format!("Generated and scattered {} vegetation spires and rock boulders across terrain.", count);
        }
    }

    // -------------------------------------------------------------------------
    // RENDER SETTINGS & PROJECT EXPORT LAB
    // -------------------------------------------------------------------------
    fn draw_render_lab(
        &mut self,
        ui: &mut Ui,
        scene_ctrl: &mut SceneController,
        camera: &mut Camera,
        materials: &[GpuMaterial; GpuMaterial::NUM_PRESETS],
        objects: &mut Vec<SceneObject>,
        terrains: &mut Vec<TerrainObjectData>,
        terrain_dirty: &mut bool,
    ) {
        ui.heading("Render Aesthetics & Production Output");

        ui.horizontal(|ui| {
            if ui.button(RichText::new("📸 Render Still Frame to PNG [F9]").strong().size(14.0).color(Color32::from_rgb(0, 240, 220))).clicked() {
                self.request_render = true;
            }
            ui.label(RichText::new("Renders pristine raytraced artwork at native resolution without UI overlays.").italics());
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Render Style Presets:").strong());
            if ui.button("Classic '90s (Bryce 4)").clicked() {
                scene_ctrl.postprocess.apply_look_preset(crate::postprocess::PostProcessConfig::classic_90s_preset());
                self.status_message = "Applied Classic '90s look (retro grade, hard shadows, 4 bounces).".to_string();
            }
            if ui.button("Modern 2000s (Bryce 7)").clicked() {
                scene_ctrl.postprocess.apply_look_preset(crate::postprocess::PostProcessConfig::modern_2000s_preset());
                self.status_message = "Applied Modern 2000s look (filmic, soft shadows, bloom).".to_string();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Render Quality:");
            egui::ComboBox::from_id_salt("render_quality_select")
                .selected_text(format!("{:?}", scene_ctrl.postprocess.quality))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut scene_ctrl.postprocess.quality, crate::postprocess::RenderQualityPreset::InteractivePreview, "Interactive Preview");
                    ui.selectable_value(&mut scene_ctrl.postprocess.quality, crate::postprocess::RenderQualityPreset::Draft, "Draft");
                    ui.selectable_value(&mut scene_ctrl.postprocess.quality, crate::postprocess::RenderQualityPreset::Standard, "Standard");
                    ui.selectable_value(&mut scene_ctrl.postprocess.quality, crate::postprocess::RenderQualityPreset::HighQuality, "High Quality");
                    ui.selectable_value(&mut scene_ctrl.postprocess.quality, crate::postprocess::RenderQualityPreset::MaximumProduction, "Maximum Production");
                });
        });

        ui.checkbox(&mut scene_ctrl.postprocess.retro_90s_cgi, "💾 Retro 1990s CGI Mode (Vibrant midtones & synthetic grading)");

        ui.add(Slider::new(&mut scene_ctrl.postprocess.contrast, 0.8..=1.8).text("Contrast"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.saturation, 0.5..=2.0).text("Saturation"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.bloom_intensity, 0.0..=1.5).text("Bloom Glow"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.chromatic_aberration, 0.0..=0.01).text("Chromatic aberration"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.vignette_strength, 0.0..=1.0).text("Vignette"));
        ui.checkbox(&mut scene_ctrl.postprocess.dither_16bit, "16-bit (5-6-5) ordered dither");

        ui.separator();
        ui.label(RichText::new("Ray Tracing").strong());
        ui.add(Slider::new(&mut scene_ctrl.postprocess.max_reflection_bounces, 1..=8).text("Max bounces"));
        ui.checkbox(&mut scene_ctrl.postprocess.soft_shadows, "Soft shadows (sun disc)");
        if scene_ctrl.postprocess.soft_shadows {
            ui.add(Slider::new(&mut scene_ctrl.postprocess.sun_angular_radius_deg, 0.1..=5.0).text("Sun size (deg)"));
        }
        ui.checkbox(&mut scene_ctrl.postprocess.blurry_reflections, "Blurry reflections (by material roughness)");

        ui.separator();
        ui.label(RichText::new("Still Render (F9)").strong());
        ui.horizontal(|ui| {
            ui.label("Resolution:");
            ui.add(egui::DragValue::new(&mut scene_ctrl.postprocess.render_width).range(64..=8192).speed(8.0));
            ui.label("x");
            ui.add(egui::DragValue::new(&mut scene_ctrl.postprocess.render_height).range(64..=8192).speed(8.0));
        });
        ui.horizontal(|ui| {
            for (w, h) in [(640u32, 480u32), (1024, 768), (1920, 1080)] {
                if ui.button(format!("{}x{}", w, h)).clicked() {
                    scene_ctrl.postprocess.render_width = w;
                    scene_ctrl.postprocess.render_height = h;
                }
            }
        });
        ui.add(Slider::new(&mut scene_ctrl.postprocess.still_samples, 1..=1024).logarithmic(true).text("Still samples"));

        ui.separator();
        ui.heading("Camera System");
        let mut fov_deg = camera.fov_y.to_degrees();
        if ui.add(Slider::new(&mut fov_deg, 20.0..=100.0).text("Field of View (FOV)")).changed() {
            camera.fov_y = fov_deg.to_radians();
        }
        ui.add(Slider::new(&mut camera.speed, 1.0..=30.0).text("Camera Flight Speed"));

        ui.separator();
        ui.heading("Project File Management");
        ui.horizontal(|ui| {
            if ui.button("💾 Save Native Project (.bricius.json)").clicked() {
                let project = ProjectFile {
                    name: "Bricius Studio Project".to_string(),
                    version: "1.0.0".to_string(),
                    terrain: scene_ctrl.terrain.clone(),
                    sky: scene_ctrl.sky.clone(),
                    water: scene_ctrl.water.clone(),
                    ground: scene_ctrl.ground.clone(),
                                lights: scene_ctrl.lights.clone(),
                    materials: materials.to_vec(),
                    objects: objects.to_vec(),
                    postprocess: scene_ctrl.postprocess.clone(),
                    terrain_objects: terrains.iter().map(|t| t.to_save()).collect(),
                };
                match ProjectIO::save_project("bricius_project.bricius.json", &project) {
                    Ok(_) => self.status_message = "Successfully saved project to bricius_project.bricius.json".to_string(),
                    Err(e) => self.status_message = format!("Save error: {}", e),
                }
            }

            if ui.button("📂 Load Project (.json)").clicked() {
                // Prefer the current name; fall back to the legacy .bryce.json quick-save.
                let path = ["bricius_project.bricius.json", "bryce_project.json"]
                    .into_iter()
                    .find(|p| std::path::Path::new(p).exists())
                    .unwrap_or("bricius_project.bricius.json");
                match ProjectIO::load_project(path) {
                    Ok(proj) => {
                        scene_ctrl.terrain = proj.terrain;
                        scene_ctrl.sky = proj.sky;
                        scene_ctrl.water = proj.water;
                        scene_ctrl.ground = proj.ground;
                        scene_ctrl.lights = proj.lights;
                        scene_ctrl.postprocess = proj.postprocess;
                        if !proj.terrain_objects.is_empty() {
                            // Projects with terrain objects restore the object list and heightmaps together
                            *terrains = proj.terrain_objects.iter().map(TerrainObjectData::from_save).collect();
                            *objects = proj.objects;
                            *terrain_dirty = true;
                            self.heightmap_dirty = true;
                            self.selected_object_idx = 0;
                        }
                        self.status_message = format!("Successfully loaded {}", path);
                    }
                    Err(e) => self.status_message = format!("Load error: {}", e),
                }
            }
        });
    }

    /// Draw 3D Transform Gizmo, Selection Brackets, and Orientation Arrows in the Viewport
    // -------------------------------------------------------------------------
    // LIGHTS
    // -------------------------------------------------------------------------
    fn draw_lights_section(&mut self, ui: &mut Ui, scene_ctrl: &mut SceneController, camera: &Camera) {
        let header = format!("Lights ({}/{})", scene_ctrl.lights.len(), MAX_LIGHTS);
        egui::CollapsingHeader::new(header).id_salt("lights_section").show(ui, |ui| {
            ui.horizontal(|ui| {
                for kind in [LightKind::Point, LightKind::Spot, LightKind::Parallel] {
                    let can_add = scene_ctrl.lights.len() < MAX_LIGHTS;
                    if ui.add_enabled(can_add, egui::Button::new(format!("+ {}", kind.label()))).clicked() {
                        let pos = camera.position + camera.forward() * 8.0;
                        let mut light = LightConfig::new(kind, [pos.x, pos.y + 1.0, pos.z]);
                        light.name = format!("{} {}", kind.label(), scene_ctrl.lights.len() + 1);
                        scene_ctrl.lights.push(light);
                        self.selected_light_idx = scene_ctrl.lights.len() - 1;
                        self.status_message = format!("Added {} light.", kind.label());
                    }
                }
            });

            let mut delete_idx: Option<usize> = None;
            for i in 0..scene_ctrl.lights.len() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut scene_ctrl.lights[i].enabled, "");
                    let label = format!("{} ({})", scene_ctrl.lights[i].name, scene_ctrl.lights[i].kind.label());
                    if ui.selectable_label(self.selected_light_idx == i, label).clicked() {
                        self.selected_light_idx = i;
                    }
                    if ui.small_button("Delete").clicked() {
                        delete_idx = Some(i);
                    }
                });
            }
            if let Some(i) = delete_idx {
                scene_ctrl.lights.remove(i);
                self.selected_light_idx = self.selected_light_idx.min(scene_ctrl.lights.len().saturating_sub(1));
            }

            if self.selected_light_idx < scene_ctrl.lights.len() {
                let l = &mut scene_ctrl.lights[self.selected_light_idx];
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut l.name);
                });
                egui::ComboBox::from_id_salt("light_kind_select")
                    .selected_text(l.kind.label())
                    .show_ui(ui, |ui| {
                        for kind in [LightKind::Point, LightKind::Spot, LightKind::Parallel] {
                            ui.selectable_value(&mut l.kind, kind, kind.label());
                        }
                    });
                ui.checkbox(&mut l.enabled, "Enabled");
                ui.horizontal(|ui| {
                    ui.label("Color:");
                    ui.color_edit_button_rgb(&mut l.color);
                });
                ui.horizontal(|ui| {
                    ui.label("Intensity:");
                    ui.add(egui::DragValue::new(&mut l.intensity).range(0.0..=10000.0).speed(0.5));
                });
                if l.kind != LightKind::Parallel {
                    ui.horizontal(|ui| {
                        ui.label("Position:");
                        for v in l.position.iter_mut() {
                            ui.add(egui::DragValue::new(v).speed(0.1));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Range:");
                        ui.add(egui::DragValue::new(&mut l.range).range(0.5..=1000.0).speed(0.2));
                    });
                }
                if l.kind != LightKind::Point {
                    ui.horizontal(|ui| {
                        ui.label("Direction:");
                        for v in l.direction.iter_mut() {
                            ui.add(egui::DragValue::new(v).speed(0.02));
                        }
                    });
                }
                if l.kind == LightKind::Spot {
                    ui.add(Slider::new(&mut l.spot_angle_deg, 1.0..=89.0).text("Cone angle (deg)"));
                    ui.add(Slider::new(&mut l.spot_softness, 0.0..=1.0).text("Penumbra softness"));
                }
                ui.checkbox(&mut l.cast_shadows, "Cast shadows");
            }
        });
        ui.separator();
    }

    /// Yellow markers (and direction lines) for enabled lights in the viewport.
    fn draw_light_markers(&self, ctx: &egui::Context, camera: &Camera, lights: &[LightConfig], screen_w: f32, screen_h: f32) {
        if lights.is_empty() {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("light_markers")));
        let yellow = Color32::from_rgb(255, 220, 40);
        for (i, l) in lights.iter().enumerate().filter(|(_, l)| l.enabled) {
            let dir = Vec3::from(l.direction).normalize_or_zero();
            // Parallel lights have no position: anchor the marker a few units in front of the camera
            let pos = if l.kind == LightKind::Parallel {
                camera.position + camera.forward() * 10.0 + camera.up() * 3.0
            } else {
                Vec3::from(l.position)
            };
            let Some(sp) = camera.project_to_screen(pos, screen_w, screen_h) else { continue };
            let selected = i == self.selected_light_idx;
            painter.circle_stroke(sp, if selected { 9.0 } else { 7.0 }, egui::Stroke::new(2.0, yellow));
            painter.circle_filled(sp, 2.5, yellow);
            painter.text(sp + egui::vec2(11.0, -9.0), egui::Align2::LEFT_CENTER, &l.name, egui::FontId::proportional(12.0), yellow);
            if l.kind != LightKind::Point {
                if let Some(ep) = camera.project_to_screen(pos + dir * 2.0, screen_w, screen_h) {
                    painter.line_segment([sp, ep], egui::Stroke::new(2.0, yellow));
                    painter.circle_filled(ep, 3.0, yellow);
                }
            }
        }
    }

    pub fn draw_viewport_gizmo(
        &mut self,
        ctx: &egui::Context,
        camera: &Camera,
        objects: &[SceneObject],
        screen_w: f32,
        screen_h: f32,
    ) {
        if self.selected_object_idx >= objects.len() {
            return;
        }
        let obj = &objects[self.selected_object_idx];
        if !obj.visible {
            return;
        }

        let center_3d = glam::Vec3::from(obj.position);
        let center_screen = match camera.project_to_screen(center_3d, screen_w, screen_h) {
            Some(pos) => pos,
            None => return,
        };

        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("3d_viewport_gizmo")));

        // 1. Draw 3D Transform Gizmo Axes
        let axis_len = (obj.scale[0].max(obj.scale[1]).max(obj.scale[2]) * 1.5).clamp(1.5, 6.0);
        let px_3d = center_3d + glam::Vec3::X * axis_len;
        let py_3d = center_3d + glam::Vec3::Y * axis_len;
        let pz_3d = center_3d + glam::Vec3::Z * axis_len;

        let stroke_w = 2.5;

        // X Axis (Red)
        if let Some(sx) = camera.project_to_screen(px_3d, screen_w, screen_h) {
            painter.line_segment([center_screen, sx], egui::Stroke::new(stroke_w, Color32::from_rgb(255, 60, 60)));
            painter.circle_filled(sx, 5.0, Color32::from_rgb(255, 70, 70));
            painter.text(sx + egui::vec2(7.0, -7.0), egui::Align2::LEFT_CENTER, "X", egui::FontId::proportional(12.0), Color32::from_rgb(255, 100, 100));
        }

        // Y Axis (Green)
        if let Some(sy) = camera.project_to_screen(py_3d, screen_w, screen_h) {
            painter.line_segment([center_screen, sy], egui::Stroke::new(stroke_w, Color32::from_rgb(50, 230, 80)));
            painter.circle_filled(sy, 5.0, Color32::from_rgb(50, 230, 80));
            painter.text(sy + egui::vec2(7.0, -7.0), egui::Align2::LEFT_CENTER, "Y", egui::FontId::proportional(12.0), Color32::from_rgb(100, 255, 120));
        }

        // Z Axis (Blue)
        if let Some(sz) = camera.project_to_screen(pz_3d, screen_w, screen_h) {
            painter.line_segment([center_screen, sz], egui::Stroke::new(stroke_w, Color32::from_rgb(50, 140, 255)));
            painter.circle_filled(sz, 5.0, Color32::from_rgb(50, 140, 255));
            painter.text(sz + egui::vec2(7.0, -7.0), egui::Align2::LEFT_CENTER, "Z", egui::FontId::proportional(12.0), Color32::from_rgb(100, 180, 255));
        }

        // Center Origin Ring
        painter.circle_stroke(center_screen, 6.0, egui::Stroke::new(2.0, Color32::WHITE));

        // 2. Selection Brackets around object
        let box_radius = 24.0;
        let bracket_len = 8.0;
        let cyan = Color32::from_rgb(0, 230, 255);
        let b_stroke = egui::Stroke::new(1.8, cyan);

        // Top-left
        painter.line_segment([center_screen + egui::vec2(-box_radius, -box_radius + bracket_len), center_screen + egui::vec2(-box_radius, -box_radius)], b_stroke);
        painter.line_segment([center_screen + egui::vec2(-box_radius, -box_radius), center_screen + egui::vec2(-box_radius + bracket_len, -box_radius)], b_stroke);

        // Top-right
        painter.line_segment([center_screen + egui::vec2(box_radius - bracket_len, -box_radius), center_screen + egui::vec2(box_radius, -box_radius)], b_stroke);
        painter.line_segment([center_screen + egui::vec2(box_radius, -box_radius), center_screen + egui::vec2(box_radius, -box_radius + bracket_len)], b_stroke);

        // Bottom-left
        painter.line_segment([center_screen + egui::vec2(-box_radius, box_radius - bracket_len), center_screen + egui::vec2(-box_radius, box_radius)], b_stroke);
        painter.line_segment([center_screen + egui::vec2(-box_radius, box_radius), center_screen + egui::vec2(-box_radius + bracket_len, box_radius)], b_stroke);

        // Bottom-right
        painter.line_segment([center_screen + egui::vec2(box_radius - bracket_len, box_radius), center_screen + egui::vec2(box_radius, box_radius)], b_stroke);
        painter.line_segment([center_screen + egui::vec2(box_radius, box_radius), center_screen + egui::vec2(box_radius, box_radius - bracket_len)], b_stroke);

        // Name tag above object
        painter.text(
            center_screen + egui::vec2(0.0, -box_radius - 8.0),
            egui::Align2::CENTER_BOTTOM,
            &obj.name,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(255, 220, 80),
        );
    }
}

fn rand_pseudo_u32() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}
