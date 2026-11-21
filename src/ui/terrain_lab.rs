//! Bryce-style Terrain Editor: heightmap canvas, live 3D preview, generator, filters and export.
use egui::{Color32, RichText, Sense, Slider, Ui, Vec2};

use crate::camera::Camera;
use crate::editor::EditorState;
use crate::material::GpuMaterial;
use crate::objects::{PrimitiveType, SceneObject};
use crate::preview::PreviewScene;
use crate::scene::SceneController;
use crate::terrain::{BrushMode, TerrainObjectData};
use crate::ui::theme;

const PREVIEW_ID: u64 = 4000;
const CANVAS: f32 = 400.0;

fn mem_id(name: &str) -> egui::Id {
    egui::Id::new(("terrain_lab", name))
}

pub fn draw(
    ed: &mut EditorState,
    ui: &mut Ui,
    scene_ctrl: &mut SceneController,
    objects: &mut Vec<SceneObject>,
    terrains: &mut Vec<TerrainObjectData>,
    terrain_dirty: &mut bool,
    camera: &Camera,
    materials: &[GpuMaterial; GpuMaterial::NUM_PRESETS],
) {
    let sel_obj = (ed.selected_object_idx < objects.len() && objects[ed.selected_object_idx].primitive == PrimitiveType::Terrain)
        .then_some(ed.selected_object_idx);
    let sel_tid = sel_obj.and_then(|i| objects[i].terrain_id).filter(|&t| t < terrains.len());

    let (Some(oi), Some(tid)) = (sel_obj, sel_tid) else {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No terrain object selected").size(20.0).strong().color(theme::ACCENT));
            ui.add_space(6.0);
            ui.label("Select a Terrain in the Scene Lab (or click one in the viewport), or create a new one.");
            ui.add_space(10.0);
            if ui.add_sized([160.0, 32.0], egui::Button::new(RichText::new("Add Terrain").strong())).clicked() {
                ed.add_terrain_object(objects, terrains, terrain_dirty, scene_ctrl, camera);
            }
        });
        ui.add_space(30.0);
        ed.draw_backdrop_section(ui, scene_ctrl);
        return;
    };

    let gray = ui.data(|d| d.get_temp::<bool>(mem_id("gray"))).unwrap_or(false);
    let mut changed_hm = false; // heightmap modified this frame

    // ---- 2D canvas texture sync ----
    let res = terrains[tid].heightmap.resolution;
    let key = (tid, res);
    let last_gray = ui.data(|d| d.get_temp::<bool>(mem_id("last_gray"))).unwrap_or(gray);
    if ed.heightmap_dirty || ed.heightmap_texture.is_none() || ed.heightmap_preview_key != Some(key) || last_gray != gray {
        let hm = &terrains[tid].heightmap;
        let pixels: Vec<Color32> = hm
            .data
            .iter()
            .map(|&h| {
                if gray {
                    let g = (h.clamp(0.0, 1.0) * 255.0) as u8;
                    Color32::from_gray(g)
                } else {
                    crate::editor::height_to_hypsometric_color(h * 17.0 - 2.0)
                }
            })
            .collect();
        let img = egui::ColorImage::new([res, res], pixels);
        if let Some(ref mut handle) = ed.heightmap_texture {
            handle.set(img, egui::TextureOptions::LINEAR);
        } else {
            ed.heightmap_texture = Some(ui.ctx().load_texture("heightmap_preview", img, egui::TextureOptions::LINEAR));
        }
        ed.heightmap_dirty = false;
        ed.heightmap_preview_key = Some(key);
        ui.data_mut(|d| d.insert_temp(mem_id("last_gray"), gray));
    }

    let avail = ui.available_width();
    let side_by_side = avail >= 820.0;
    let canvas = if side_by_side { CANVAS } else { (avail - 8.0).clamp(240.0, CANVAS) };

    ui.label(
        RichText::new(format!("Editing: {} (heightmap {}x{})", objects[oi].name, res, res))
            .strong()
            .color(crate::ui::theme::ACCENT),
    );
    ui.add_space(4.0);

    let preview_ui = |ui: &mut Ui, ed: &mut EditorState, terrains: &[TerrainObjectData], objects: &[SceneObject], side: f32| {
        let mut yaw = ui.data(|d| d.get_temp::<f32>(mem_id("yaw"))).unwrap_or(0.8);
        let mut pitch = ui.data(|d| d.get_temp::<f32>(mem_id("pitch"))).unwrap_or(0.55);
        let o = &objects[oi];
        let mat = materials[(o.material_id as usize).min(GpuMaterial::NUM_PRESETS - 1)];
        let ratio = o.scale[1] / o.scale[0].max(1e-3);
        let tex = ed.request_preview(PREVIEW_ID, PreviewScene::terrain(&terrains[tid].heightmap, &mat, ratio, yaw, pitch), 320);
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::drag());
        if let Some(tex) = tex {
            ui.painter().image(tex, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            ui.painter().rect_filled(rect, 2.0, theme::PANEL_DARK);
        }
        ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme::SHADOW), egui::StrokeKind::Inside);
        if resp.dragged() {
            let d = resp.drag_delta();
            yaw -= d.x * 0.01;
            pitch = (pitch + d.y * 0.01).clamp(0.08, 1.45);
            ui.data_mut(|dd| {
                dd.insert_temp(mem_id("yaw"), yaw);
                dd.insert_temp(mem_id("pitch"), pitch);
            });
        }
        resp.on_hover_text("Drag to orbit the terrain");
        ui.label(RichText::new("Drag the preview to orbit.").small().color(theme::TEXT_DIM));
    };

    // ---- Canvas + brushes, and the 3D preview ----
    let canvas_ui = |ui: &mut Ui, ed: &mut EditorState, terrains: &mut Vec<TerrainObjectData>, terrain_dirty: &mut bool, changed: &mut bool| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Heightmap").strong());
            let mut g = gray;
            if ui.selectable_label(!g, "Hypsometric").clicked() {
                g = false;
            }
            if ui.selectable_label(g, "Grayscale").clicked() {
                g = true;
            }
            if g != gray {
                ui.data_mut(|d| d.insert_temp(mem_id("gray"), g));
            }
        });
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(canvas), Sense::click_and_drag());
        if let Some(ref texture) = ed.heightmap_texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.5, Color32::from_rgb(65, 75, 95)), egui::StrokeKind::Outside);
        let res = terrains[tid].heightmap.resolution;
        let radius_cells = ((ed.brush_radius as f32) * res as f32 / 256.0).round().max(1.0) as usize;
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let rel_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let rel_y = ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                let hx = (rel_x * (res as f32 - 1.0)).round() as usize;
                let hy = (rel_y * (res as f32 - 1.0)).round() as usize;
                let t = &mut terrains[tid];
                t.heightmap.apply_brush(hx, hy, radius_cells, ed.brush_strength, ed.brush_mode, ed.brush_target_height);
                t.clamp_heights();
                *terrain_dirty = true;
                *changed = true;
            }
        }
        if let Some(pos) = response.hover_pos() {
            let pixel_scale = rect.width() / res as f32;
            ui.painter().circle_stroke(pos, radius_cells as f32 * pixel_scale, egui::Stroke::new(1.5, crate::ui::theme::ACCENT));
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ed.brush_mode, BrushMode::Raise, "Raise");
            ui.selectable_value(&mut ed.brush_mode, BrushMode::Lower, "Lower");
            ui.selectable_value(&mut ed.brush_mode, BrushMode::Smooth, "Smooth");
            ui.selectable_value(&mut ed.brush_mode, BrushMode::Flatten, "Flatten");
            ui.selectable_value(&mut ed.brush_mode, BrushMode::Noise, "Noise");
        });
        ui.add(Slider::new(&mut ed.brush_radius, 2..=32).text("Radius"));
        ui.add(Slider::new(&mut ed.brush_strength, 0.05..=1.5).text("Strength"));
        if ed.brush_mode == BrushMode::Flatten {
            ui.add(Slider::new(&mut ed.brush_target_height, 0.0..=1.0).text("Flatten height"));
        }
    };

    if side_by_side {
        let pside = (avail - CANVAS - 30.0).clamp(240.0, 440.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(CANVAS + 4.0);
                canvas_ui(ui, ed, terrains, terrain_dirty, &mut changed_hm);
            });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("3D preview").strong());
                preview_ui(ui, ed, terrains, objects, pside);
            });
        });
    } else {
        ui.vertical(|ui| canvas_ui(ui, ed, terrains, terrain_dirty, &mut changed_hm));
        ui.add_space(6.0);
        ui.label(RichText::new("3D preview").strong());
        preview_ui(ui, ed, terrains, objects, avail.clamp(240.0, 440.0));
    }
    ui.separator();

    // ---- Generator + filters ----
    let mut gen_clicked = false;
    let mut filter: Option<&'static str> = None;
    let gen_ui = |ui: &mut Ui, ed: &mut EditorState, terrains: &mut Vec<TerrainObjectData>, scene_ctrl: &SceneController, gen_clicked: &mut bool, changed: &mut bool, terrain_dirty: &mut bool| {
        ui.label(RichText::new("Generator").strong().color(theme::ACCENT));
        EditorState::terrain_config_controls(ui, &mut terrains[tid].config, "terrain_obj_cfg");
        ui.horizontal(|ui| {
            ui.label("Resolution");
            for r in [128usize, 256, 512] {
                let cur = terrains[tid].heightmap.resolution;
                if ui.selectable_label(cur == r, format!("{}", r)).clicked() && cur != r {
                    terrains[tid].heightmap.resample(r);
                    *terrain_dirty = true;
                    *changed = true;
                    ed.status_message = format!("Resampled terrain to {}x{}.", r, r);
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Generate").strong().color(theme::ACCENT)).clicked() {
                *gen_clicked = true;
            }
            if ui.button("Random seed").clicked() {
                let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1);
                terrains[tid].config.seed = n % 100_000;
                *gen_clicked = true;
            }
            if ui.button("Copy from backdrop").clicked() {
                terrains[tid].config = scene_ctrl.terrain.clone();
                ed.status_message = "Copied backdrop fractal settings into the terrain generator.".to_string();
            }
        });
    };
    let filter_ui = |ui: &mut Ui, filter: &mut Option<&'static str>| {
        ui.label(RichText::new("Filters").strong().color(theme::ACCENT));
        let names: [(&'static str, &str); 14] = [
            ("erode", "Erode (hydraulic rain)"),
            ("thermal", "Thermal weathering"),
            ("smooth", "Smooth"),
            ("raise", "Raise all"),
            ("lower", "Lower all"),
            ("invert", "Invert"),
            ("plateau", "Sub-plateau (clamp low)"),
            ("clip", "Clip high"),
            ("spiky", "Sharpen / Spiky"),
            ("round", "Round off"),
            ("noise", "Add noise"),
            ("normalize", "Normalize range"),
            ("flatten", "Clear / flatten"),
            ("", ""),
        ];
        egui::Grid::new("terrain_filters").spacing([6.0, 6.0]).show(ui, |ui| {
            let mut n = 0;
            for (id, label) in names.iter().filter(|(i, _)| !i.is_empty()) {
                if ui.add_sized([150.0, 24.0], egui::Button::new(*label)).clicked() {
                    *filter = Some(id);
                }
                n += 1;
                if n % 2 == 0 {
                    ui.end_row();
                }
            }
        });
    };
    let seed_for_noise = ui.ctx().input(|i| (i.time * 1000.0) as u32);

    if avail >= 700.0 {
        ui.columns(2, |cols| {
            gen_ui(&mut cols[0], ed, terrains, scene_ctrl, &mut gen_clicked, &mut changed_hm, terrain_dirty);
            filter_ui(&mut cols[1], &mut filter);
            cols[1].add_space(8.0);
            export_ui(&mut cols[1], ed, objects, terrains, oi, tid);
        });
    } else {
        gen_ui(ui, ed, terrains, scene_ctrl, &mut gen_clicked, &mut changed_hm, terrain_dirty);
        ui.add_space(6.0);
        filter_ui(ui, &mut filter);
        export_ui(ui, ed, objects, terrains, oi, tid);
    }

    if gen_clicked {
        let cfg = terrains[tid].config.clone();
        terrains[tid].heightmap.generate_normalized(&cfg);
        changed_hm = true;
        ed.status_message = "Generated heightmap into selected terrain.".to_string();
    }
    if let Some(f) = filter {
        let hm = &mut terrains[tid].heightmap;
        let msg = match f {
            "erode" => { hm.simulate_hydraulic_erosion(4); "Hydraulic erosion (4 cycles)." }
            "thermal" => { hm.simulate_thermal_erosion(4); "Thermal weathering (4 cycles)." }
            "smooth" => { hm.smooth(1); "Smoothed." }
            "raise" => { hm.offset(0.05); "Raised all heights." }
            "lower" => { hm.offset(-0.05); "Lowered all heights." }
            "invert" => { hm.invert(); "Inverted elevations." }
            "plateau" => { hm.clamp_low(0.25); "Clamped lowlands to a plateau." }
            "clip" => { hm.clip_high(0.75); "Clipped high peaks." }
            "spiky" => { hm.sharpen(1.5); "Sharpened peaks." }
            "round" => { hm.sharpen(0.75); "Rounded off peaks." }
            "noise" => { hm.add_noise(0.03, seed_for_noise); "Added noise." }
            "normalize" => { hm.normalize(); "Normalized to the full range." }
            _ => { hm.data.iter_mut().for_each(|v| *v = 0.0); "Cleared terrain to a flat plane." }
        };
        terrains[tid].clamp_heights();
        changed_hm = true;
        ed.status_message = msg.to_string();
    }
    if changed_hm {
        *terrain_dirty = true;
        ed.heightmap_dirty = true;
    }

    ed.draw_backdrop_section(ui, scene_ctrl);
}

fn export_ui(ui: &mut Ui, ed: &mut EditorState, objects: &[SceneObject], terrains: &[TerrainObjectData], oi: usize, tid: usize) {
    ui.label(RichText::new("Export").strong().color(theme::ACCENT));
    ui.horizontal(|ui| {
        if ui.button("Export OBJ mesh").clicked() {
            let path = "bricius_terrain.obj";
            ed.status_message = match terrains[tid].heightmap.export_obj_scaled(path, objects[oi].scale) {
                Ok(_) => format!("Exported 3D terrain mesh to {}", path),
                Err(e) => format!("Export error: {}", e),
            };
        }
        if ui.button("Export PNG heightmap").clicked() {
            let path = "bricius_heightmap.png";
            ed.status_message = match terrains[tid].heightmap.export_png(path) {
                Ok(_) => format!("Exported elevation heightmap to {}", path),
                Err(e) => format!("Export error: {}", e),
            };
        }
    });
}
