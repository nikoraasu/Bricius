//! Bryce-style shell: title strip + palettes (top), views/navigation/render (left),
//! attributes/objects/lights (right), status bar (bottom) and the framed 3D viewport (center).
use egui::{pos2, vec2, Align, Color32, Layout, Rect, RichText, Slider, Stroke, Ui};

use super::icons::{icon_button, IconKind};
use super::{labs, theme, World};
use crate::editor::{ActiveLab, EditorState};
use crate::light::{LightConfig, LightKind, MAX_LIGHTS};
use crate::material::GpuMaterial;
use crate::objects::{CsgOp, PrimitiveType, SceneObject};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Palette {
    Create,
    Edit,
    Sky,
}

fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(8)).stroke(Stroke::new(1.0, theme::SHADOW))
}

pub fn draw(ed: &mut EditorState, ctx: &egui::Context, w: &mut World) {
    if !ed.theme_applied {
        theme::apply(ctx);
        ed.theme_applied = true;
    }
    ed.pixels_per_point = ctx.pixels_per_point();
    let screen = ctx.viewport_rect();

    if !ed.visible {
        ed.lab_cancel_requested = false;
        ed.viewport_rect = screen;
        egui::Area::new(egui::Id::new("minimized_bar")).fixed_pos(pos2(8.0, 8.0)).show(ctx, |ui: &mut Ui| {
            if ui.button(RichText::new("Show UI (Tab)").strong()).clicked() {
                ed.visible = true;
            }
        });
        ed.draw_light_markers(ctx, w.camera, &w.scene_ctrl.lights, screen);
        ed.draw_viewport_gizmo(ctx, w.camera, w.objects, screen);
        return;
    }

    let mut root_ui = Ui::new(
        ctx.clone(),
        egui::Id::new("bricius_root_ui"),
        egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(screen),
    );
    let root = &mut root_ui;
    let lab = ed.active_lab;
    let mut hover = String::new();
    top_strip(ed, root, w, lab.is_none(), &mut hover);

    if let Some(lab) = lab {
        labs::draw_lab(ed, ctx, root, w, lab);
        return;
    }
    ed.lab_snapshot = None;
    ed.lab_cancel_requested = false;

    left_panel(ed, ctx, root, w);
    right_panel(ed, root, w);

    egui::Panel::bottom("status_bar")
        .frame(egui::Frame::new().fill(theme::PANEL_DARK).inner_margin(egui::Margin::symmetric(10, 3)))
        .show(root, |ui: &mut Ui| {
            // Right-aligned fixed slots (samples | FPS | undo); the status text gets the rest, truncated with an ellipsis
            const SLOT_SAMPLES: f32 = 130.0;
            const SLOT_FPS: f32 = 70.0;
            const SLOT_UNDO: f32 = 110.0;
            let sep = ui.spacing().item_spacing.x * 2.0 + 2.0;
            let slots_w = SLOT_SAMPLES + SLOT_FPS + SLOT_UNDO + sep * 2.0;
            let full = ui.available_rect_before_wrap();
            let text_w = (full.width() - slots_w - 12.0).max(40.0);
            let text = if hover.is_empty() { ed.status_message.clone() } else { hover.clone() };
            let text_rect = Rect::from_min_size(full.min, vec2(text_w, full.height()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                ui.set_clip_rect(text_rect.intersect(ui.clip_rect()));
                ui.add(egui::Label::new(RichText::new(text).color(theme::TEXT)).truncate());
            });
            let slots_rect = Rect::from_min_max(pos2(full.max.x - slots_w, full.min.y), full.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(slots_rect).layout(Layout::right_to_left(Align::Center)), |ui| {
                let slot = |ui: &mut Ui, w: f32, text: String| {
                    ui.allocate_ui_with_layout(vec2(w, ui.available_height()), Layout::right_to_left(Align::Center), |ui| {
                        ui.set_width(w);
                        ui.add(egui::Label::new(text).truncate());
                    });
                };
                slot(ui, SLOT_UNDO, format!("Undo: {} steps", ed.undo_steps));
                ui.separator();
                slot(ui, SLOT_FPS, format!("{:.0} FPS", ed.fps));
                ui.separator();
                slot(ui, SLOT_SAMPLES, format!("Samples {} / {}", ed.samples_done, ed.sample_target));
            });
        });

    // Center: letterboxed "document" window
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui: &mut Ui| {
        let avail = ui.max_rect();
        let pp = &w.scene_ctrl.postprocess;
        let aspect = pp.render_width.max(1) as f32 / pp.render_height.max(1) as f32;
        let inner = avail.shrink(14.0);
        let (mut vw, mut vh) = (inner.width().max(16.0), inner.height().max(16.0));
        if vw / vh > aspect {
            vw = vh * aspect;
        } else {
            vh = vw / aspect;
        }
        let ppp = ctx.pixels_per_point();
        let snap = |v: f32| (v * ppp).round() / ppp;
        let c = inner.center();
        let vp = Rect::from_min_max(
            pos2(snap(c.x - vw * 0.5), snap(c.y - vh * 0.5)),
            pos2(snap(c.x + vw * 0.5), snap(c.y + vh * 0.5)),
        );
        let p = ui.painter();
        let s = theme::SURROUND;
        p.rect_filled(Rect::from_min_max(avail.min, pos2(avail.max.x, vp.min.y)), 0.0, s);
        p.rect_filled(Rect::from_min_max(pos2(avail.min.x, vp.max.y), avail.max), 0.0, s);
        p.rect_filled(Rect::from_min_max(pos2(avail.min.x, vp.min.y), pos2(vp.min.x, vp.max.y)), 0.0, s);
        p.rect_filled(Rect::from_min_max(pos2(vp.max.x, vp.min.y), pos2(avail.max.x, vp.max.y)), 0.0, s);
        p.rect_stroke(vp.expand(1.0), 0.0, Stroke::new(1.0, Color32::from_rgb(0x78, 0x78, 0x7e)), egui::StrokeKind::Outside);
        ed.viewport_rect = vp;
    });

    let vp = ed.viewport_rect;
    ed.draw_light_markers(ctx, w.camera, &w.scene_ctrl.lights, vp);
    ed.draw_viewport_gizmo(ctx, w.camera, w.objects, vp);

    if ed.show_help {
        help_window(ed, ctx);
    }
}

// -------------------------------------------------------------------------------------------------
// TOP: title strip + palettes
// -------------------------------------------------------------------------------------------------
fn top_strip(ed: &mut EditorState, root: &mut Ui, w: &mut World, show_palette: bool, hover: &mut String) {
    egui::Panel::top("top_strip")
        .frame(egui::Frame::new().fill(theme::PANEL_DARK).inner_margin(egui::Margin::symmetric(10, 4)).stroke(Stroke::new(1.0, theme::SHADOW)))
        .show(root, |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("BRICIUS").size(17.0).strong().color(theme::ACCENT));
                ui.add_space(14.0);
                if show_palette {
                    for (pal, name) in [(Palette::Create, "Create"), (Palette::Edit, "Edit"), (Palette::Sky, "Sky & Fog")] {
                        let sel = ed.palette == pal;
                        let txt = if sel { RichText::new(name).strong().color(theme::ACCENT) } else { RichText::new(name) };
                        if ui.selectable_label(sel, txt).clicked() {
                            ed.palette = pal;
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("?").on_hover_text("Controls help (F1)").clicked() {
                        ed.show_help = !ed.show_help;
                    }
                    if ui.button("Hide UI").on_hover_text("Hide all panels (Tab)").clicked() {
                        ed.visible = false;
                    }
                    if show_palette && ui.button("Render Settings").on_hover_text("Open the Render lab").clicked() {
                        ed.active_lab = Some(ActiveLab::Render);
                    }
                    if show_palette {
                        ui.add_space(8.0);
                        ui.add_enabled_ui(ed.redo_steps > 0, |ui| {
                            if icon_button(ui, IconKind::Redo, 26.0, false, "Redo (Ctrl+Shift+Z / Ctrl+Y)").clicked() {
                                ed.redo_requested = true;
                            }
                        });
                        ui.add_enabled_ui(ed.undo_steps > 0, |ui| {
                            if icon_button(ui, IconKind::Undo, 26.0, false, "Undo (Ctrl+Z)").clicked() {
                                ed.undo_requested = true;
                            }
                        });
                    }
                });
            });
            if !show_palette {
                return;
            }
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| match ed.palette {
                Palette::Create => create_palette(ed, ui, w, hover),
                Palette::Edit => edit_palette(ed, ui, w, hover),
                Palette::Sky => sky_palette(ed, ui, w, hover),
            });
            ui.label(RichText::new(if hover.is_empty() { " " } else { hover.as_str() }).color(theme::TEXT_DIM).small());
        });
}

fn pbtn(ui: &mut Ui, hover: &mut String, kind: IconKind, selected: bool, text: &str) -> bool {
    let r = icon_button(ui, kind, 40.0, selected, text);
    if r.hovered() {
        *hover = text.to_string();
    }
    r.clicked()
}

fn spawn_pos(w: &World, dist: f32) -> [f32; 3] {
    let p = w.camera.position + w.camera.forward() * dist;
    [p.x, p.y, p.z]
}

fn add_object(ed: &mut EditorState, w: &mut World, obj: SceneObject, msg: &str) {
    w.objects.push(obj);
    ed.selected_object_idx = w.objects.len() - 1;
    ed.status_message = msg.to_string();
}

fn random_seed() -> u32 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(12345);
    (n ^ n.rotate_left(13)).max(1)
}

fn create_palette(ed: &mut EditorState, ui: &mut Ui, w: &mut World, hover: &mut String) {
    let n = w.objects.len() + 1;
    let mat = w.scene_ctrl.selected_material;
    if pbtn(ui, hover, IconKind::Water, w.scene_ctrl.water.enabled, "Water plane: click to toggle the water plane on/off") {
        w.scene_ctrl.water.enabled = !w.scene_ctrl.water.enabled;
    }
    if pbtn(ui, hover, IconKind::Ground, w.scene_ctrl.ground.enabled, "Ground plane: click to toggle the ground plane on/off") {
        w.scene_ctrl.ground.enabled = !w.scene_ctrl.ground.enabled;
    }
    if pbtn(ui, hover, IconKind::Terrain, false, "Terrain: add a new terrain object in front of the camera") {
        ed.add_terrain_object(w.objects, w.terrains, w.terrain_dirty, w.scene_ctrl, w.camera);
    }
    ui.separator();
    if pbtn(ui, hover, IconKind::Sphere, false, "Sphere: add a sphere primitive") {
        let o = SceneObject::sphere(&format!("Sphere {}", n), spawn_pos(w, 6.0), 1.5, mat);
        add_object(ed, w, o, "Added new Sphere primitive.");
    }
    if pbtn(ui, hover, IconKind::Cube, false, "Cube: add a cube primitive") {
        let o = SceneObject::cube(&format!("Cube {}", n), spawn_pos(w, 6.0), [1.0, 1.0, 1.0], mat);
        add_object(ed, w, o, "Added new Cube primitive.");
    }
    if pbtn(ui, hover, IconKind::Cylinder, false, "Cylinder: add a cylinder primitive") {
        let o = SceneObject::cylinder(&format!("Cylinder {}", n), spawn_pos(w, 6.0), 1.0, 2.0, mat);
        add_object(ed, w, o, "Added new Cylinder primitive.");
    }
    if pbtn(ui, hover, IconKind::Torus, false, "Torus: add a torus primitive") {
        let o = SceneObject::torus(&format!("Torus {}", n), spawn_pos(w, 6.0), 2.0, 0.4, mat);
        add_object(ed, w, o, "Added new Torus primitive.");
    }
    if pbtn(ui, hover, IconKind::Cone, false, "Cone: add a cone primitive") {
        let o = SceneObject::cone(&format!("Cone {}", n), spawn_pos(w, 6.0), 1.0, 1.5, mat);
        add_object(ed, w, o, "Added new Cone primitive.");
    }
    if pbtn(ui, hover, IconKind::Pyramid, false, "Pyramid: add a square pyramid primitive") {
        let o = SceneObject::pyramid(&format!("Pyramid {}", n), spawn_pos(w, 6.0), [1.2, 1.2], 1.5, mat);
        add_object(ed, w, o, "Added new Pyramid primitive.");
    }
    if pbtn(ui, hover, IconKind::Stone, false, "Stone: add a random lumpy rock resting on the ground, water or terrain") {
        let seed = random_seed();
        let mut rng = seed;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (rng % 10_000) as f32 / 10_000.0
        };
        let (sx, sy, sz) = (0.9 + next() * 0.9, 0.6 + next() * 0.7, 0.9 + next() * 0.9);
        let mut o = SceneObject::stone(&format!("Stone {}", n), spawn_pos(w, 6.0), [sx, sy, sz], seed % 100_000, 0);
        o.rotation[1] = next() * 360.0;
        // Rest on the highest of ground / water / terrain beneath the spawn point, sunk slightly
        let terrain_objs: Vec<SceneObject> = w.objects.iter().filter(|o| o.primitive == PrimitiveType::Terrain && o.visible).cloned().collect();
        let mut gy = EditorState::ground_height_at(o.position[0], o.position[2], &terrain_objs, w.terrains, &w.scene_ctrl.terrain, None);
        if w.scene_ctrl.ground.enabled {
            gy = gy.max(w.scene_ctrl.ground.height);
        }
        if w.scene_ctrl.water.enabled {
            gy = gy.max(w.scene_ctrl.water.water_height);
        }
        o.position[1] = if gy.is_finite() { gy } else { 0.0 } + sy * 0.8;
        add_object(ed, w, o, "Added a new Stone.");
    }
    ui.separator();
    if pbtn(ui, hover, IconKind::Import, false, "Import model: pick an .obj / .glb / .gltf / .fbx file") {
        import_model_dialog(ed, w);
    }
    let r = icon_button(ui, IconKind::Models, 40.0, false, "Built-in models: Teapot, Monolith, Geode, Starfighter");
    if r.hovered() {
        *hover = "Built-in models: Teapot, Monolith, Geode, Starfighter".to_string();
    }
    egui::Popup::menu(&r).show(|ui| {
        for (i, (name, dist, lift)) in [("Utah Teapot", 6.0, 1.8), ("Desert Monolith", 7.0, 2.2), ("Crystal Geode", 6.5, 1.8), ("Starfighter Interceptor", 7.5, 3.0)].into_iter().enumerate() {
            if ui.button(name).clicked() {
                let f = w.camera.forward();
                let (x, z) = (w.camera.position.x + f.x * dist, w.camera.position.z + f.z * dist);
                let gy = crate::terrain::evaluate_procedural_height(x, z, &w.scene_ctrl.terrain);
                let o = SceneObject::mesh(name, [x, gy + lift, z], [1.5, 1.5, 1.5], mat, i);
                add_object(ed, w, o, &format!("Spawned {} model", name));
                ui.close();
            }
        }
    });
    ui.separator();
    for (kind, icon, text) in [
        (LightKind::Point, IconKind::PointLight, "Point light: add an omnidirectional light"),
        (LightKind::Spot, IconKind::SpotLight, "Spot light: add a cone light"),
        (LightKind::Parallel, IconKind::ParallelLight, "Parallel light: add a directional light"),
    ] {
        if pbtn(ui, hover, icon, false, text) {
            add_light(ed, w, kind);
        }
    }
}

fn add_light(ed: &mut EditorState, w: &mut World, kind: LightKind) {
    if w.scene_ctrl.lights.len() >= MAX_LIGHTS {
        ed.status_message = format!("Light limit reached ({}).", MAX_LIGHTS);
        return;
    }
    let pos = w.camera.position + w.camera.forward() * 8.0;
    let mut light = LightConfig::new(kind, [pos.x, pos.y + 1.0, pos.z]);
    light.name = format!("{} {}", kind.label(), w.scene_ctrl.lights.len() + 1);
    w.scene_ctrl.lights.push(light);
    ed.selected_light_idx = w.scene_ctrl.lights.len() - 1;
    ed.status_message = format!("Added {} light.", kind.label());
}

fn import_model_dialog(ed: &mut EditorState, w: &mut World) {
    let Some(file_path) = rfd::FileDialog::new()
        .add_filter("3D Models (*.obj, *.glb, *.gltf, *.fbx)", &["obj", "glb", "gltf", "fbx"])
        .pick_file()
    else {
        return;
    };
    let path_str = file_path.to_string_lossy().to_string();
    ed.model_file_path_input = path_str.clone();
    match crate::model_loader::ModelLoader::load_from_file(&path_str) {
        Ok(loaded_mesh) => {
            let name = loaded_mesh.name.clone();
            let tri_cnt = loaded_mesh.triangle_count();
            let mesh_idx = w.meshes.len();
            w.meshes.push(loaded_mesh);
            *w.mesh_dirty = true;
            let f = w.camera.forward();
            let (x, z) = (w.camera.position.x + f.x * 6.5, w.camera.position.z + f.z * 6.5);
            let gy = crate::terrain::evaluate_procedural_height(x, z, &w.scene_ctrl.terrain);
            let o = SceneObject::mesh(&name, [x, gy + 1.8, z], [1.5, 1.5, 1.5], w.scene_ctrl.selected_material, mesh_idx);
            add_object(ed, w, o, &format!("Successfully imported '{}' ({} triangles)", name, tri_cnt));
        }
        Err(e) => ed.status_message = format!("Import error: {}", e),
    }
}

fn snap_to_ground(ed: &mut EditorState, w: &mut World, i: usize) {
    let terrain_objs: Vec<SceneObject> = w.objects.iter().filter(|o| o.primitive == PrimitiveType::Terrain && o.visible).cloned().collect();
    let obj = &mut w.objects[i];
    let h = EditorState::ground_height_at(obj.position[0], obj.position[2], &terrain_objs, w.terrains, &w.scene_ctrl.terrain, obj.terrain_id);
    obj.position[1] = h + if obj.primitive == PrimitiveType::Terrain { 0.0 } else { obj.scale[1] };
    ed.status_message = format!("Snapped '{}' to terrain ground height {:.2}", obj.name, obj.position[1]);
}

fn edit_palette(ed: &mut EditorState, ui: &mut Ui, w: &mut World, hover: &mut String) {
    let sel = ed.selected_object_idx < w.objects.len();
    if pbtn(ui, hover, IconKind::Materials, false, "Materials: open the Materials lab") {
        ed.active_lab = Some(ActiveLab::Materials);
    }
    if pbtn(ui, hover, IconKind::EditObject, false, "Edit object: open the Terrain lab for the selected terrain") {
        if sel && w.objects[ed.selected_object_idx].primitive == PrimitiveType::Terrain {
            ed.active_lab = Some(ActiveLab::Terrain);
        } else {
            ed.status_message = "Select a terrain object to edit it.".to_string();
        }
    }
    ui.separator();
    if pbtn(ui, hover, IconKind::Duplicate, false, "Duplicate: copy the selected object") && sel {
        let mut cloned = w.objects[ed.selected_object_idx].clone();
        cloned.name = format!("{} (Copy)", cloned.name);
        cloned.position[0] += 2.0;
        if let Some(tid) = cloned.terrain_id.filter(|&t| t < w.terrains.len()) {
            let copy = w.terrains[tid].clone();
            w.terrains.push(copy);
            cloned.terrain_id = Some(w.terrains.len() - 1);
            *w.terrain_dirty = true;
        }
        add_object(ed, w, cloned, "Duplicated selected object.");
    }
    if pbtn(ui, hover, IconKind::Delete, false, "Delete: remove the selected object") && sel {
        w.objects.remove(ed.selected_object_idx);
        if crate::terrain::gc_terrains(w.objects, w.terrains) > 0 {
            *w.terrain_dirty = true;
        }
        ed.selected_object_idx = ed.selected_object_idx.saturating_sub(1);
        ed.status_message = "Deleted object.".to_string();
    }
    if pbtn(ui, hover, IconKind::Snap, false, "Snap to ground: drop the selected object onto the terrain") && sel {
        snap_to_ground(ed, w, ed.selected_object_idx);
    }
    if pbtn(ui, hover, IconKind::Csg, false, "CSG mode: cycle Positive / Negative / Intersecting for the selected object") && sel {
        let o = &mut w.objects[ed.selected_object_idx];
        o.csg_op = match o.csg_op {
            CsgOp::Positive => CsgOp::Negative,
            CsgOp::Negative => CsgOp::Intersecting,
            CsgOp::Intersecting => CsgOp::Positive,
        };
        ed.status_message = format!("'{}' CSG mode: {}", o.name, csg_label(o.csg_op));
    }
    ui.separator();
    if pbtn(ui, hover, IconKind::Generator, false, "Generator: 1-click procedural worlds") {
        ed.active_lab = Some(ActiveLab::Generator);
    }
    if pbtn(ui, hover, IconKind::Scatter, false, "Scene & Scatter: ground plane settings and vegetation / rock scatter") {
        ed.active_lab = Some(ActiveLab::Scene);
    }
}

fn csg_label(op: CsgOp) -> &'static str {
    match op {
        CsgOp::Positive => "Positive (Solid Union)",
        CsgOp::Negative => "Negative (Subtractive Cutout)",
        CsgOp::Intersecting => "Intersecting (Intersection)",
    }
}

fn sky_palette(ed: &mut EditorState, ui: &mut Ui, w: &mut World, hover: &mut String) {
    use crate::sky::SkyConfig;
    let sky = &mut w.scene_ctrl.sky;
    let presets: [(&str, fn() -> SkyConfig); 7] = [
        ("Clear Blue", SkyConfig::clear_blue),
        ("Retro Sunset", SkyConfig::retro_sunset),
        ("Storm", SkyConfig::dramatic_storm),
        ("Twilight", SkyConfig::twilight_glow),
        ("Starry Night", SkyConfig::starry_night),
        ("Alien Violet", SkyConfig::alien_violet),
        ("Cyberpunk", SkyConfig::cyberpunk_neon),
    ];
    ui.vertical(|ui| {
        ui.label(RichText::new("Sky presets").small().color(theme::TEXT_DIM));
        ui.horizontal(|ui| {
            for (name, f) in presets {
                let r = ui.button(name);
                if r.hovered() {
                    *hover = format!("Apply the '{}' sky preset", name);
                }
                if r.clicked() {
                    *sky = f();
                    ed.status_message = format!("Applied '{}' sky preset.", name);
                }
            }
        });
    });
    ui.separator();
    ui.vertical(|ui| {
        let mut az = sky.sun_azimuth.to_degrees();
        let mut el = sky.sun_elevation.to_degrees();
        if ui.add(Slider::new(&mut az, 0.0..=360.0).text("Sun azimuth").fixed_decimals(0)).changed() {
            sky.sun_azimuth = az.to_radians();
        }
        if ui.add(Slider::new(&mut el, -15.0..=90.0).text("Sun elevation").fixed_decimals(0)).changed() {
            sky.sun_elevation = el.to_radians();
        }
    });
    ui.separator();
    ui.vertical(|ui| {
        ui.add(Slider::new(&mut sky.fog_density, 0.001..=0.05).text("Fog"));
        ui.add(Slider::new(&mut sky.mie_coeff, 0.1..=3.0).text("Haze"));
        ui.add(Slider::new(&mut sky.cloud_coverage, 0.0..=1.5).text("Clouds"));
    });
    ui.separator();
    if pbtn(ui, hover, IconKind::SkyLab, false, "Sky Lab: full sky, sun, atmosphere and cloud editor") {
        ed.active_lab = Some(ActiveLab::Sky);
    }
    if pbtn(ui, hover, IconKind::WaterLab, false, "Water Lab: ocean, lake and liquid settings") {
        ed.active_lab = Some(ActiveLab::Water);
    }
}

// -------------------------------------------------------------------------------------------------
// LEFT: views, navigation, render
// -------------------------------------------------------------------------------------------------
fn hold_arrow(ui: &mut Ui, dir: egui::Vec2) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::click_and_drag());
    let down = resp.is_pointer_button_down_on();
    ui.painter().rect_filled(rect, 3.0, if down { theme::ACCENT_DARK } else if resp.hovered() { theme::WIDGET_HOVER } else { theme::WIDGET });
    theme::paint_bevel(ui.painter(), rect, down);
    let c = rect.center();
    let perp = vec2(-dir.y, dir.x);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![c + dir * 8.0, c - dir * 6.0 + perp * 7.0, c - dir * 6.0 - perp * 7.0],
        theme::TEXT,
        Stroke::NONE,
    ));
    down
}

fn left_panel(ed: &mut EditorState, ctx: &egui::Context, root: &mut Ui, w: &mut World) {
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    egui::Panel::left("left_panel").exact_size(150.0).resizable(false).frame(panel_frame()).show(root, |ui: &mut Ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui: &mut Ui| {
        ui.label(RichText::new("Views").strong().color(theme::ACCENT));
        use crate::camera::ViewKind;
        let view_defs: [(&str, ViewKind, &str); 5] = [
            ("Camera", ViewKind::Camera, "Render camera: this is what F9 / Render captures"),
            ("Director", ViewKind::Director, "Director camera: fly around freely without moving the render camera"),
            ("Top", ViewKind::Top, "Orthographic top view (pan with arrows / drag, zoom with the wheel)"),
            ("Front", ViewKind::Front, "Orthographic front view (pan with arrows / drag, zoom with the wheel)"),
            ("Side", ViewKind::Side, "Orthographic side view (pan with arrows / drag, zoom with the wheel)"),
        ];
        for (name, kind, tip) in view_defs {
            let active = ed.view == kind;
            let label = if active { RichText::new(name).color(Color32::from_rgb(0x2a, 0x20, 0x08)) } else { RichText::new(name) };
            let mut btn = egui::Button::new(label);
            if active {
                btn = btn.fill(theme::ACCENT);
            }
            if ui.add_sized([ui.available_width(), 22.0], btn).on_hover_text(tip).clicked() && !active {
                ed.pending_view = Some(kind);
                ed.status_message = format!("{} view.", name);
            }
        }
        ui.add_space(2.0);
        let ortho = w.camera.is_ortho();
        ui.add_enabled_ui(!ortho, |ui| {
            let presets: [(&str, glam::Vec3, f32); 3] = [
                ("Default Vista", glam::Vec3::new(0.0, 7.5, 24.0), -8.0),
                ("Ground Low-Angle", glam::Vec3::new(0.0, 1.2, 14.0), 18.0),
                ("Aerial", glam::Vec3::new(0.0, 34.0, 22.0), -50.0),
            ];
            egui::ComboBox::from_id_salt("view_presets").width(ui.available_width() - 8.0).selected_text("Presets").show_ui(ui, |ui| {
                for (name, pos, pitch) in presets {
                    if ui.selectable_label(false, name).clicked() {
                        w.camera.position = pos;
                        w.camera.yaw = -90.0_f32.to_radians();
                        w.camera.pitch = pitch.to_radians();
                        ed.status_message = format!("Camera set to {}.", name);
                    }
                }
            });
        });
        ui.separator();
        ui.label(RichText::new("Navigate").strong().color(theme::ACCENT));
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 94.0) * 0.5);
                ui.add_space(32.0);
                if hold_arrow(ui, vec2(0.0, -1.0)) {
                    w.camera.move_forward(dt);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 94.0) * 0.5);
                if hold_arrow(ui, vec2(-1.0, 0.0)) {
                    w.camera.move_left(dt);
                }
                ui.add_space(32.0);
                if hold_arrow(ui, vec2(1.0, 0.0)) {
                    w.camera.move_right(dt);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 94.0) * 0.5);
                ui.add_space(32.0);
                if hold_arrow(ui, vec2(0.0, 1.0)) {
                    w.camera.move_backward(dt);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 62.0) * 0.5);
                if hold_arrow(ui, vec2(0.0, -1.0)) {
                    w.camera.move_up(dt);
                }
                ui.add_space(2.0);
                if hold_arrow(ui, vec2(0.0, 1.0)) {
                    w.camera.move_down(dt);
                }
            });
            ui.label(RichText::new(if w.camera.is_ortho() { "Pan  |  Zoom" } else { "Pan / dolly  |  Up / Down" }).small().color(theme::TEXT_DIM));
            ui.add_space(4.0);
            // Trackball: drag to orbit/look like LMB-dragging the viewport
            let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 96.0), egui::Sense::drag());
            let c = rect.center();
            let r = 46.0;
            let p = ui.painter();
            p.circle_filled(c, r, Color32::from_rgb(0x38, 0x38, 0x3c));
            p.circle_filled(c + vec2(-3.0, -3.0), r - 4.0, Color32::from_rgb(0x70, 0x70, 0x76));
            p.circle_filled(c + vec2(-10.0, -10.0), r * 0.45, Color32::from_rgb(0x8c, 0x8c, 0x92));
            let g = Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 70));
            p.circle_stroke(c, r * 0.66, g);
            p.line_segment([c - vec2(r, 0.0) * 0.95, c + vec2(r, 0.0) * 0.95], g);
            p.line_segment([c - vec2(0.0, r) * 0.95, c + vec2(0.0, r) * 0.95], g);
            p.circle_stroke(c, r, Stroke::new(1.5, if resp.dragged() { theme::ACCENT } else { theme::SHADOW }));
            if resp.dragged() {
                let d = resp.drag_delta();
                w.camera.rotate(d.x * 1.5, d.y * 1.5);
            }
            resp.on_hover_text("Trackball: drag to rotate the camera");
        });

        ui.separator();
        ui.vertical_centered(|ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(84.0, 84.0), egui::Sense::click());
            let down = resp.is_pointer_button_down_on();
            let c = rect.center();
            let p = ui.painter();
            p.circle_filled(c + vec2(0.0, 2.0), 41.0, theme::SHADOW);
            p.circle_filled(c, 41.0, if down { theme::ACCENT_DARK } else if resp.hovered() { Color32::from_rgb(0xe0, 0xb0, 0x4c) } else { theme::ACCENT });
            p.circle_stroke(c, 41.0, Stroke::new(1.5, Color32::from_rgb(0xf0, 0xd0, 0x80)));
            p.circle_filled(c + vec2(-10.0, -14.0), 14.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40));
            p.text(c, egui::Align2::CENTER_CENTER, "Render", egui::FontId::proportional(17.0), Color32::from_rgb(0x2a, 0x20, 0x08));
            if resp.on_hover_text("Render a still image to PNG (F9, always uses the Camera view)").clicked() {
                ed.request_render = true;
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Quality / Look").small().color(theme::TEXT_DIM));
            let sc = &mut *w.scene_ctrl;
            use crate::postprocess::RenderQualityPreset as Q;
            egui::ComboBox::from_id_salt("render_quality_select").width(ui.available_width() - 8.0).selected_text(format!("{:?}", sc.postprocess.quality)).show_ui(ui, |ui| {
                for (q, n) in [(Q::InteractivePreview, "Interactive Preview"), (Q::Draft, "Draft"), (Q::Standard, "Standard"), (Q::HighQuality, "High Quality"), (Q::MaximumProduction, "Maximum Production")] {
                    ui.selectable_value(&mut sc.postprocess.quality, q, n);
                }
            });
            if ui.add_sized([ui.available_width(), 22.0], egui::Button::new("Classic '90s")).clicked() {
                sc.postprocess.apply_look_preset(crate::postprocess::PostProcessConfig::classic_90s_preset());
                ed.status_message = "Applied Classic '90s look (retro grade, hard shadows, 4 bounces).".to_string();
            }
            if ui.add_sized([ui.available_width(), 22.0], egui::Button::new("Modern 2000s")).clicked() {
                sc.postprocess.apply_look_preset(crate::postprocess::PostProcessConfig::modern_2000s_preset());
                ed.status_message = "Applied Modern 2000s look (filmic, soft shadows, bloom).".to_string();
            }
        });
        });
    });
}

// -------------------------------------------------------------------------------------------------
// RIGHT: attributes, objects, lights
// -------------------------------------------------------------------------------------------------
fn right_panel(ed: &mut EditorState, root: &mut Ui, w: &mut World) {
    egui::Panel::right("right_panel").exact_size(260.0).resizable(false).frame(panel_frame()).show(root, |ui: &mut Ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui: &mut Ui| {
            ui.label(RichText::new("Attributes").strong().color(theme::ACCENT));
            ui.separator();
            attributes(ed, ui, w);
            ui.add_space(6.0);
            ui.label(RichText::new("Objects").strong().color(theme::ACCENT));
            ui.separator();
            egui::ScrollArea::vertical().id_salt("obj_list").max_height(170.0).show(ui, |ui| {
                for (i, obj) in w.objects.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut obj.visible, "");
                        let tag = match obj.csg_op {
                            CsgOp::Positive => "+",
                            CsgOp::Negative => "-",
                            CsgOp::Intersecting => "&",
                        };
                        if ui.selectable_label(ed.selected_object_idx == i, format!("[{}] {}: {}", tag, i + 1, obj.name)).clicked() {
                            ed.selected_object_idx = i;
                        }
                    });
                }
                if w.objects.is_empty() {
                    ui.label(RichText::new("No objects. Use the Create palette.").color(theme::TEXT_DIM));
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Lights").strong().color(theme::ACCENT));
            ui.separator();
            ed.draw_lights_section(ui, w.scene_ctrl, w.camera);
        });
    });
}

fn attributes(ed: &mut EditorState, ui: &mut Ui, w: &mut World) {
    let i = ed.selected_object_idx;
    if i >= w.objects.len() {
        ui.label(RichText::new("No object selected.").color(theme::TEXT_DIM));
        return;
    }
    let lock = &mut ed.uniform_scale_locked;
    let names = &ed.material_names;
    let status = &mut ed.status_message;
    let obj = &mut w.objects[i];
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut obj.name);
    });
    ui.horizontal(|ui| {
        ui.label("Type");
        ui.label(RichText::new(format!("{:?}", obj.primitive)).color(theme::TEXT_DIM));
        ui.checkbox(&mut obj.visible, "Visible");
    });
    ui.label(RichText::new("Position").small().color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        for v in obj.position.iter_mut() {
            ui.add(egui::DragValue::new(v).speed(0.1));
        }
    });
    ui.label(RichText::new("Rotation").small().color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        for v in obj.rotation.iter_mut() {
            ui.add(egui::DragValue::new(v).speed(1.0).suffix("\u{b0}"));
        }
        if ui.small_button("0").on_hover_text("Reset rotation").clicked() {
            obj.rotation = [0.0; 3];
        }
    });
    ui.label(RichText::new("Scale").small().color(theme::TEXT_DIM));
    ui.horizontal(|ui| {
        ui.checkbox(lock, "Uniform");
        if *lock {
            let mut u = obj.scale[0];
            if ui.add(egui::DragValue::new(&mut u).speed(0.05)).changed() {
                let c = u.clamp(0.05, 50.0);
                obj.scale = [c, c, c];
            }
        }
    });
    if !*lock {
        ui.horizontal(|ui| {
            for v in obj.scale.iter_mut() {
                ui.add(egui::DragValue::new(v).speed(0.05));
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Material");
        let id = obj.material_id.min(GpuMaterial::NUM_PRESETS as u32 - 1) as usize;
        egui::ComboBox::from_id_salt("attr_mat").width(150.0).selected_text(format!("{}: {}", id, names[id])).show_ui(ui, |ui| {
            for m in 0..GpuMaterial::NUM_PRESETS {
                if ui.selectable_value(&mut obj.material_id, m as u32, format!("{}: {}", m, names[m])).clicked() {
                    *status = format!("Set material of '{}' to Slot {}", obj.name, m);
                }
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label("CSG");
        egui::ComboBox::from_id_salt("attr_csg").width(150.0).selected_text(csg_label(obj.csg_op)).show_ui(ui, |ui| {
            for op in [CsgOp::Positive, CsgOp::Negative, CsgOp::Intersecting] {
                ui.selectable_value(&mut obj.csg_op, op, csg_label(op));
            }
        });
    });
    if obj.primitive == PrimitiveType::Stone {
        ui.horizontal(|ui| {
            ui.label("Seed");
            let mut sd = obj.stone_seed;
            if ui.add(egui::DragValue::new(&mut sd).speed(1.0).range(0..=99_999)).changed() {
                obj.stone_seed = sd;
            }
            if ui.button("Reroll").clicked() {
                obj.stone_seed = random_seed() % 100_000;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lumpiness");
            ui.add(egui::Slider::new(&mut obj.stone_lumpiness, 0.0..=3.0));
        });
    }
    if obj.primitive == PrimitiveType::Terrain && ui.button("Edit Terrain...").clicked() {
        ed.active_lab = Some(ActiveLab::Terrain);
    }
    ui.horizontal(|ui| {
        if ui.button("Snap to Ground").clicked() {
            snap_to_ground(ed, w, i);
        }
        if ui.button("Focus Camera").clicked() {
            let target = glam::Vec3::from(w.objects[i].position);
            w.camera.position = target - w.camera.forward() * 7.0 + glam::Vec3::Y * 2.0;
            ed.status_message = format!("Framed camera onto '{}'", w.objects[i].name);
        }
    });
}

// -------------------------------------------------------------------------------------------------
// HELP overlay (F1)
// -------------------------------------------------------------------------------------------------
fn help_window(ed: &mut EditorState, ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Controls").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui: &mut Ui| {
        egui::Grid::new("help_grid").spacing(vec2(14.0, 4.0)).show(ui, |ui| {
            for (k, d) in [
                ("W, A, S, D", "Fly camera forward / left / back / right"),
                ("Space / Shift", "Fly altitude (ascend / descend)"),
                ("RMB / LMB drag", "Mouse look / view orbit (LMB on empty space)"),
                ("LMB on object", "Select; drag to move (Shift: height, Alt: scale)"),
                ("Left / Right", "Sun azimuth orbit"),
                ("Up / Down", "Sun elevation"),
                ("Ctrl+Z / Ctrl+Y", "Undo / redo (Ctrl+Shift+Z also redoes)"),
                ("Tab", "Hide / show all UI"),
                ("F1", "This help"),
                ("F9", "Render still image (.png)"),
                ("Esc", "Cancel the open lab (quits when no lab is open)"),
            ] {
                ui.label(RichText::new(k).monospace().strong().color(theme::ACCENT));
                ui.label(d);
                ui.end_row();
            }
        });
        ui.separator();
        if ui.button("Open step-by-step tutorial").clicked() {
            ed.active_lab = Some(ActiveLab::Tutorial);
            ed.show_help = false;
        }
    });
    if !open {
        ed.show_help = false;
    }
}
