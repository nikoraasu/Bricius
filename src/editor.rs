use egui::{Color32, RichText, Slider, Ui, Vec2};

use crate::camera::Camera;
use crate::generator::{SceneGenerator, SceneTheme};
use crate::io::{ProjectFile, ProjectIO};
use crate::material::GpuMaterial;
use crate::objects::{CsgOp, SceneObject, ScatterSystem};
use crate::scene::SceneController;
use crate::terrain::{BrushMode, HeightmapGrid, TerrainAlgorithm};
use crate::water::WaterType;

/// Classic Bryce elevation colormap: deep water -> shallow -> sand -> forest -> rock -> snow
fn height_to_bryce_color(h: f32) -> Color32 {
    if h < -0.5 {
        Color32::from_rgb(20, 60, 120) // Deep water
    } else if h < 0.2 {
        Color32::from_rgb(45, 120, 180) // Shallow turquoise water
    } else if h < 0.8 {
        Color32::from_rgb(220, 195, 135) // Beach sand
    } else if h < 3.5 {
        Color32::from_rgb(55, 125, 45) // Lowland vegetation
    } else if h < 7.0 {
        Color32::from_rgb(115, 105, 90) // Highland rock
    } else if h < 11.0 {
        Color32::from_rgb(160, 155, 150) // Mountain crest granite
    } else {
        Color32::from_rgb(240, 245, 255) // Alpine snow
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveLab {
    Terrain,
    Sky,
    Water,
    Materials,
    Scene,
    Generator,
    Render,
}

pub struct EditorState {
    pub visible: bool,
    pub active_lab: ActiveLab,
    pub selected_material_slot: usize,
    pub selected_object_idx: usize,
    pub brush_mode: BrushMode,
    pub brush_radius: usize,
    pub brush_strength: f32,
    #[allow(dead_code)]
    pub brush_target_height: f32,
    pub heightmap: HeightmapGrid,
    pub heightmap_dirty: bool,
    pub heightmap_texture: Option<egui::TextureHandle>,
    pub scatter_config: ScatterSystem,
    pub status_message: String,
    pub fps: f32,
    pub frame_time_ms: f32,
}

impl EditorState {
    pub fn new() -> Self {
        let mut heightmap = HeightmapGrid::new(128);
        heightmap.generate_from_config(&crate::terrain::TerrainConfig::default());

        Self {
            visible: true,
            active_lab: ActiveLab::Generator,
            selected_material_slot: 1,
            selected_object_idx: 0,
            brush_mode: BrushMode::Raise,
            brush_radius: 8,
            brush_strength: 0.5,
            brush_target_height: 5.0,
            heightmap,
            heightmap_dirty: true,
            heightmap_texture: None,
            scatter_config: ScatterSystem::default(),
            status_message: "Bryce 7 Environment Studio Ready. Select a lab to begin.".to_string(),
            fps: 60.0,
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
        material_dirty: &mut bool,
    ) {
        if !self.visible {
            egui::Area::new(egui::Id::new("minimized_bar"))
                .fixed_pos(egui::pos2(12.0, 12.0))
                .show(ctx, |ui: &mut Ui| {
                    if ui.button(RichText::new("⚡ Open Bryce 7 Studio").strong()).clicked() {
                        self.visible = true;
                    }
                });
            return;
        }

        // Synchronize 2D Heightmap Canvas Texture if dirty
        if self.heightmap_dirty || self.heightmap_texture.is_none() {
            let res = self.heightmap.resolution;
            let mut pixels = vec![Color32::BLACK; res * res];
            for y in 0..res {
                for x in 0..res {
                    let h = self.heightmap.data[y * res + x];
                    pixels[y * res + x] = height_to_bryce_color(h);
                }
            }
            let img = egui::ColorImage::new([res, res], pixels);
            if let Some(ref mut handle) = self.heightmap_texture {
                handle.set(img, egui::TextureOptions::LINEAR);
            } else {
                self.heightmap_texture = Some(ctx.load_texture("heightmap_preview", img, egui::TextureOptions::LINEAR));
            }
            self.heightmap_dirty = false;
        }

        // Apply dark skeuomorphic Bryce visuals
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
        // TOP BRYCE 7 SHELF (TOOLBAR)
        // =========================================================================
        egui::Area::new(egui::Id::new("bryce_top_shelf"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui: &mut Ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 22, 26))
                    .inner_margin(egui::Margin { left: 16, right: 16, top: 8, bottom: 8 })
                    .show(ui, |ui: &mut Ui| {
                        ui.set_width(screen.width() - 32.0);
                        ui.horizontal(|ui: &mut Ui| {
                            ui.label(RichText::new("BRYCE 7").size(20.0).strong().color(Color32::from_rgb(0, 225, 255)));
                            ui.label(RichText::new("PRO STUDIO").size(11.0).color(Color32::from_rgb(255, 190, 0)));
                            ui.separator();

                            let labs = [
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
                                ui.label(format!("{:.1} FPS ({:.1} ms)", self.fps, self.frame_time_ms));
                            });
                        });
                    });
            });

        // =========================================================================
        // ACTIVE LAB WORKSPACE WINDOW
        // =========================================================================
        egui::Window::new(match self.active_lab {
            ActiveLab::Generator => "🎲 Procedural 1-Click World Generator",
            ActiveLab::Terrain => "⛰️ Terrain Lab & Interactive Heightmap Sculptor",
            ActiveLab::Sky => "🌅 Sky Lab & Atmospheric Volumetrics",
            ActiveLab::Water => "🌊 Water & Ocean Environment Lab",
            ActiveLab::Materials => "🎨 Materials Lab & Deep Texture Editor (DTE)",
            ActiveLab::Scene => "🔮 Scene Hierarchy & 3D Objects",
            ActiveLab::Render => "🎥 Render Settings & Project Export",
        })
        .default_pos(egui::pos2(20.0, 65.0))
        .default_size(Vec2::new(460.0, 620.0))
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui: &mut Ui| {
            egui::ScrollArea::vertical().show(ui, |ui: &mut Ui| {
                match self.active_lab {
                    ActiveLab::Generator => {
                        self.draw_generator_lab(ui, scene_ctrl, materials, objects);
                    }
                    ActiveLab::Terrain => {
                        self.draw_terrain_lab(ui, scene_ctrl);
                    }
                    ActiveLab::Sky => {
                        self.draw_sky_lab(ui, scene_ctrl);
                    }
                    ActiveLab::Water => {
                        self.draw_water_lab(ui, scene_ctrl);
                    }
                    ActiveLab::Materials => {
                        self.draw_materials_lab(ui, materials, scene_ctrl, material_dirty);
                    }
                    ActiveLab::Scene => {
                        self.draw_scene_lab(ui, scene_ctrl, objects);
                    }
                    ActiveLab::Render => {
                        self.draw_render_lab(ui, scene_ctrl, camera, materials, objects);
                    }
                }
            });
        });

        // =========================================================================
        // BOTTOM STATUS BAR
        // =========================================================================
        egui::Area::new(egui::Id::new("bryce_status_bar"))
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
            (SceneTheme::ClassicBryce1994, "🏜️ Classic Bryce 1994 Canyon", "Signature eroded red rocks, golden sunset, and floating mirror chrome orb."),
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
                        self.heightmap.generate_from_config(&scene_ctrl.terrain);
                        self.heightmap_dirty = true;
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
                self.heightmap.generate_from_config(&scene_ctrl.terrain);
                self.heightmap_dirty = true;
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
    fn draw_terrain_lab(&mut self, ui: &mut Ui, scene_ctrl: &mut SceneController) {
        ui.heading("Ken Musgrave Fractal Algorithms");

        let mut algo = scene_ctrl.terrain.algorithm;
        ui.horizontal(|ui| {
            ui.label("Fractal Algorithm:");
            egui::ComboBox::from_id_salt("terrain_algo_select")
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
        scene_ctrl.terrain.algorithm = algo;

        let mut changed = false;
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.height_scale, 0.1..=3.0).text("Height Scale")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.frequency, 0.01..=0.08).text("Frequency / Scale")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.octaves, 1..=6).text("Fractal Octaves")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.mountain_sharpness, 0.5..=3.5).text("Mountain Sharpness")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.ridge_intensity, 0.5..=3.0).text("Ridge Crest Intensity")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.valley_depth, 0.5..=2.5).text("Valley Depth")).changed();
        changed |= ui.add(Slider::new(&mut scene_ctrl.terrain.domain_warp, 0.0..=2.0).text("Domain Warping")).changed();

        if changed || ui.button("⚡ Sync Heightmap from Fractal Parameters").clicked() {
            self.heightmap.generate_from_config(&scene_ctrl.terrain);
            self.heightmap_dirty = true;
            self.status_message = "Heightmap regenerated from fractal parameters.".to_string();
        }

        ui.separator();
        ui.heading("Interactive Heightmap Sculptor Canvas");
        ui.label("Click & drag directly on the elevation map to sculpt terrain:");

        // Live 2D Heightmap Canvas with Interactive Paint Sculpting
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

        // Canvas Border
        ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.5, Color32::from_rgb(65, 75, 95)), egui::StrokeKind::Outside);

        // Mouse Drag / Click Sculpting
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rel_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let rel_y = ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                let res = self.heightmap.resolution;
                let hx = (rel_x * (res as f32 - 1.0)).round() as usize;
                let hy = (rel_y * (res as f32 - 1.0)).round() as usize;

                self.heightmap.apply_brush(
                    hx,
                    hy,
                    self.brush_radius,
                    self.brush_strength,
                    self.brush_mode,
                    self.brush_target_height,
                );
                self.heightmap_dirty = true;
            }
        }

        // Brush Hover Circle Overlay
        if let Some(pos) = response.hover_pos() {
            let res = self.heightmap.resolution as f32;
            let pixel_scale = rect.width() / res;
            let screen_radius = (self.brush_radius as f32) * pixel_scale;
            ui.painter().circle_stroke(
                pos,
                screen_radius,
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
            ui.add(Slider::new(&mut self.brush_target_height, -5.0..=25.0).text("Flatten Target Height"));
        }

        ui.separator();
        ui.heading("Geological Simulation & Operations");
        ui.horizontal(|ui| {
            if ui.button("🌧️ Hydraulic Rain Erosion").clicked() {
                self.heightmap.simulate_hydraulic_erosion(4);
                self.heightmap_dirty = true;
                self.status_message = "Simulated 4 cycles of hydraulic rainfall & sediment deposition.".to_string();
            }
            if ui.button("🪨 Thermal Scree Weathering").clicked() {
                self.heightmap.simulate_thermal_erosion(4);
                self.heightmap_dirty = true;
                self.status_message = "Simulated 4 cycles of thermal slope weathering.".to_string();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("🔄 Invert Heights").clicked() {
                for v in &mut self.heightmap.data {
                    *v = -*v;
                }
                self.heightmap_dirty = true;
                self.status_message = "Inverted terrain elevations (valleys to ridges).".to_string();
            }
            if ui.button("⚖️ Normalize Range").clicked() {
                let mut min_h = f32::INFINITY;
                let mut max_h = f32::NEG_INFINITY;
                for &h in &self.heightmap.data {
                    min_h = min_h.min(h);
                    max_h = max_h.max(h);
                }
                let span = (max_h - min_h).max(0.001);
                for v in &mut self.heightmap.data {
                    *v = (*v - min_h) / span * 15.0 - 2.0;
                }
                self.heightmap_dirty = true;
                self.status_message = "Normalized terrain elevations to [ -2.0 .. 13.0 ].".to_string();
            }
            if ui.button("🧹 Clear / Flatten").clicked() {
                for v in &mut self.heightmap.data {
                    *v = 0.0;
                }
                self.heightmap_dirty = true;
                self.status_message = "Cleared terrain to flat plane.".to_string();
            }
        });

        ui.separator();
        ui.heading("Terrain Export");
        ui.horizontal(|ui| {
            if ui.button("💾 Export 3D Mesh (.OBJ)").clicked() {
                let path = "bryce_terrain.obj";
                match self.heightmap.export_obj(path) {
                    Ok(_) => self.status_message = format!("Exported 3D terrain mesh to {}", path),
                    Err(e) => self.status_message = format!("Export error: {}", e),
                }
            }
            if ui.button("🖼️ Export Heightmap (.PNG)").clicked() {
                let path = "bryce_heightmap.png";
                match self.heightmap.export_png(path) {
                    Ok(_) => self.status_message = format!("Exported elevation heightmap to {}", path),
                    Err(e) => self.status_message = format!("Export error: {}", e),
                }
            }
        });
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

        let mut water_type = scene_ctrl.water.water_type;
        ui.horizontal(|ui| {
            ui.label("Water Environment:");
            egui::ComboBox::from_id_salt("water_type_select")
                .selected_text(match water_type {
                    WaterType::Ocean => "Dynamic Ocean (Gerstner Waves)",
                    WaterType::Lake => "Calm Alpine Lake",
                    WaterType::CalmMirror => "Bryce 1994 Pure Mirror",
                    WaterType::AlienLiquid => "Glowing Alien Liquid",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut water_type, WaterType::Ocean, "Dynamic Ocean (Gerstner Waves)").clicked() {
                        scene_ctrl.water = crate::water::WaterConfig::ocean();
                    }
                    if ui.selectable_value(&mut water_type, WaterType::Lake, "Calm Alpine Lake").clicked() {
                        scene_ctrl.water = crate::water::WaterConfig::lake();
                    }
                    if ui.selectable_value(&mut water_type, WaterType::CalmMirror, "Bryce 1994 Pure Mirror").clicked() {
                        scene_ctrl.water = crate::water::WaterConfig::calm_mirror();
                    }
                    if ui.selectable_value(&mut water_type, WaterType::AlienLiquid, "Glowing Alien Liquid").clicked() {
                        scene_ctrl.water = crate::water::WaterConfig::alien_liquid();
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
        scene_ctrl: &mut SceneController,
        material_dirty: &mut bool,
    ) {
        ui.heading("Active Material Slot");
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("")
                .selected_text(format!("Material Slot {}", self.selected_material_slot))
                .show_ui(ui, |ui| {
                    for i in 0..GpuMaterial::NUM_PRESETS {
                        ui.selectable_value(&mut self.selected_material_slot, i, format!("Slot {}: Material", i));
                    }
                });

            if ui.button("Apply to Target Orb").clicked() {
                scene_ctrl.selected_material = self.selected_material_slot as u32;
                self.status_message = format!("Linked slot {} to showcase orb", self.selected_material_slot);
            }
        });

        ui.separator();
        ui.label(RichText::new("Preset Library (One-Click)").strong());
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
                ("Plastic", GpuMaterial::bryce_plastic()),
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
    ) {
        ui.heading("3D Scene Primitives & Composition");

        ui.horizontal(|ui| {
            if ui.button("➕ Add Sphere").clicked() {
                objects.push(SceneObject::sphere(
                    &format!("Sphere {}", objects.len() + 1),
                    [0.0, 3.0, -6.0],
                    1.5,
                    scene_ctrl.selected_material,
                ));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Sphere primitive.".to_string();
            }
            if ui.button("➕ Add Cube").clicked() {
                objects.push(SceneObject::cube(
                    &format!("Cube {}", objects.len() + 1),
                    [0.0, 2.0, -6.0],
                    [1.0, 1.0, 1.0],
                    scene_ctrl.selected_material,
                ));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Cube primitive.".to_string();
            }
            if ui.button("➕ Add Cylinder").clicked() {
                objects.push(SceneObject::cylinder(
                    &format!("Cylinder {}", objects.len() + 1),
                    [0.0, 2.5, -6.0],
                    1.0,
                    2.5,
                    scene_ctrl.selected_material,
                ));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Cylinder primitive.".to_string();
            }
            if ui.button("➕ Add Torus").clicked() {
                objects.push(SceneObject::torus(
                    &format!("Torus {}", objects.len() + 1),
                    [0.0, 4.0, -6.0],
                    2.5,
                    0.4,
                    scene_ctrl.selected_material,
                ));
                self.selected_object_idx = objects.len() - 1;
                self.status_message = "Added new Torus primitive.".to_string();
            }
        });

        ui.separator();
        ui.label(RichText::new("Scene Hierarchy").strong());

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
                    ui.label("Position (X, Y, Z):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut obj.position[0]).speed(0.1).prefix("X: "));
                        ui.add(egui::DragValue::new(&mut obj.position[1]).speed(0.1).prefix("Y: "));
                        ui.add(egui::DragValue::new(&mut obj.position[2]).speed(0.1).prefix("Z: "));
                    });
                    ui.label("Rotation (Pitch, Yaw, Roll):");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut obj.rotation[0]).speed(1.0).suffix("°"));
                        ui.add(egui::DragValue::new(&mut obj.rotation[1]).speed(1.0).suffix("°"));
                        ui.add(egui::DragValue::new(&mut obj.rotation[2]).speed(1.0).suffix("°"));
                    });
                    ui.label("Scale / Dimensions:");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut obj.scale[0]).speed(0.05).prefix("Sx: "));
                        ui.add(egui::DragValue::new(&mut obj.scale[1]).speed(0.05).prefix("Sy: "));
                        ui.add(egui::DragValue::new(&mut obj.scale[2]).speed(0.05).prefix("Sz: "));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Material Slot:");
                        ui.add(egui::DragValue::new(&mut obj.material_id).range(0..=15));
                    });
                });
            }
        }

        if !objects.is_empty() && self.selected_object_idx < objects.len() {
            if ui.button("🗑️ Delete Selected Object").clicked() {
                objects.remove(self.selected_object_idx);
                self.selected_object_idx = self.selected_object_idx.saturating_sub(1);
                self.status_message = "Deleted object.".to_string();
            }
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
        objects: &[SceneObject],
    ) {
        ui.heading("Render Aesthetics & Optics");

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

        ui.checkbox(&mut scene_ctrl.postprocess.retro_90s_cgi, "💾 Retro 1990s Bryce CGI Mode (Vibrant, Saturated, Glossy)");

        ui.add(Slider::new(&mut scene_ctrl.postprocess.contrast, 0.8..=1.8).text("Contrast"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.saturation, 0.5..=2.0).text("Saturation"));
        ui.add(Slider::new(&mut scene_ctrl.postprocess.bloom_intensity, 0.0..=1.5).text("Bloom Glow"));

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
            if ui.button("💾 Save Native Project (.json)").clicked() {
                let project = ProjectFile {
                    name: "Bryce Modern Studio Project".to_string(),
                    version: "1.0.0".to_string(),
                    terrain: scene_ctrl.terrain.clone(),
                    sky: scene_ctrl.sky.clone(),
                    water: scene_ctrl.water.clone(),
                    materials: materials.to_vec(),
                    objects: objects.to_vec(),
                    postprocess: scene_ctrl.postprocess.clone(),
                };
                match ProjectIO::save_project("bryce_project.json", &project) {
                    Ok(_) => self.status_message = "Successfully saved project to bryce_project.json".to_string(),
                    Err(e) => self.status_message = format!("Save error: {}", e),
                }
            }

            if ui.button("📂 Load Project (.json)").clicked() {
                match ProjectIO::load_project("bryce_project.json") {
                    Ok(proj) => {
                        scene_ctrl.terrain = proj.terrain;
                        scene_ctrl.sky = proj.sky;
                        scene_ctrl.water = proj.water;
                        scene_ctrl.postprocess = proj.postprocess;
                        self.heightmap.generate_from_config(&scene_ctrl.terrain);
                        self.status_message = "Successfully loaded bryce_project.json".to_string();
                    }
                    Err(e) => self.status_message = format!("Load error: {}", e),
                }
            }
        });
    }
}

fn rand_pseudo_u32() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}
