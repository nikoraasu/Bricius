//! Bryce-style Materials Lab: live preview, channel/texture grid, texture editor, slots and presets.
use egui::{Color32, RichText, Sense, Slider, Ui, Vec2};

use crate::editor::EditorState;
use crate::material::{GpuMaterial, CHANNEL_NAMES, NOISE_NAMES};
use crate::objects::SceneObject;
use crate::preview::{PreviewScene, PreviewShape};
use crate::scene::SceneController;
use crate::ui::theme;

const MAIN_ID: u64 = 1;
const SLOT_ID: u64 = 1000;
const PRESET_ID: u64 = 2000;
const THUMB_PX: u32 = 64;
const PRESET_NAMES: [&str; 16] = [
    "Rock", "Marble", "Chrome", "Glass", "Gold", "Obsidian", "Water", "Bio-Coral",
    "Plastic", "Sandstone", "Ice", "Wood", "Rust", "Amethyst", "Cyber", "Desert",
];

#[derive(Default, Clone, Copy, PartialEq)]
pub enum LabShape {
    #[default]
    Sphere,
    Cube,
    Plane,
}

impl LabShape {
    fn to_preview(self) -> PreviewShape {
        match self {
            LabShape::Sphere => PreviewShape::Sphere,
            LabShape::Cube => PreviewShape::Cube,
            LabShape::Plane => PreviewShape::Plane,
        }
    }
}

/// UI-only state of the Materials Lab (kept in `EditorState`).
#[derive(Default)]
pub struct MaterialLabState {
    pub shape: LabShape,
    pub tex: usize,
    swatches: [Option<egui::TextureHandle>; 4],
    swatch_keys: [Vec<u32>; 4],
}

// ---- tiny CPU noise for the 2D texture swatch (approximates the GPU texture) ----
fn h(x: i32, y: i32, z: i32) -> f32 {
    let mut n = (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ z.wrapping_mul(1274126177)) as u32;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}
fn vnoise(p: [f32; 3]) -> f32 {
    let f = [p[0].floor(), p[1].floor(), p[2].floor()];
    let u = [0, 1, 2].map(|i| {
        let t = p[i] - f[i];
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    });
    let (x, y, z) = (f[0] as i32, f[1] as i32, f[2] as i32);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| h(x + dx, y + dy, z + dz);
    l(
        l(l(c(0, 0, 0), c(1, 0, 0), u[0]), l(c(0, 1, 0), c(1, 1, 0), u[0]), u[1]),
        l(l(c(0, 0, 1), c(1, 0, 1), u[0]), l(c(0, 1, 1), c(1, 1, 1), u[0]), u[1]),
        u[2],
    )
}
fn fbm(mut p: [f32; 3], oct: i32, f: impl Fn(f32) -> f32) -> f32 {
    let (mut sum, mut amp) = (0.0, 0.5);
    for _ in 0..oct.max(1) {
        sum += amp * f(vnoise(p));
        p = [p[0] * 2.02 + 1.2, p[1] * 2.02 + 3.4, p[2] * 2.02 + 5.6];
        amp *= 0.5;
    }
    sum
}
fn cpu_tex(t: &[f32; 4], sharp: f32, x: f32, y: f32) -> f32 {
    let (ty, oct, warp) = (t[0] as u32, t[2] as i32, t[3]);
    let p = [x * t[1], y * t[1], 0.37 * t[1]];
    let v = match ty {
        1 => fbm(p, oct, |n| n),
        2 => fbm(p, oct, |n| { let r = 1.0 - (n * 2.0 - 1.0).abs(); r * r * 1.5 }),
        3 => {
            let q = [fbm(p, oct, |n| n), fbm([p[0] + 5.2, p[1] + 1.3, p[2]], oct, |n| n), 0.0];
            let w = [p[0] + q[0] * warp, p[1] + q[1] * warp, p[2]];
            (w[0] * 3.0 + w[1] * 2.0 + fbm(w, 3, |n| n) * 6.0).sin() * 0.5 + 0.5
        }
        4 => {
            let (fx, fy) = (p[0].floor(), p[1].floor());
            let mut m = 1.0f32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (cx, cy) = (fx as i32 + dx, fy as i32 + dy);
                    let d = ((cx as f32 + h(cx, cy, 7) - p[0]).powi(2) + (cy as f32 + h(cx, cy, 13) - p[1]).powi(2)).sqrt();
                    m = m.min(d);
                }
            }
            1.0 - m
        }
        5 => fbm(p, oct, |n| (n * 2.0 - 1.0).abs()) * 1.6,
        6 => 0.5 + 0.5 * ((p[1] + warp * fbm([p[0] * 0.5, p[1] * 0.5, 0.0], 3, |n| n)) * std::f32::consts::TAU).sin(),
        7 => ((p[0].floor() + p[1].floor()) as i32 & 1) as f32,
        _ => 0.5,
    };
    v.clamp(0.0, 1.0).powf(sharp.max(0.05))
}
fn swatch_image(m: &GpuMaterial, i: usize, n: usize) -> egui::ColorImage {
    let sharp = m.tex[i * 4 + 1][3];
    let ramp = |t: f32| {
        let (a, b, k) = if t < 0.5 { (m.tex_color(i, 0), m.tex_color(i, 1), t * 2.0) } else { (m.tex_color(i, 1), m.tex_color(i, 2), (t - 0.5) * 2.0) };
        [0, 1, 2].map(|c| ((a[c] + (b[c] - a[c]) * k).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8)
    };
    let mut px = Vec::with_capacity(n * n);
    for yy in 0..n {
        for xx in 0..n {
            let c = ramp(cpu_tex(&m.tex[i * 4], sharp, xx as f32 / n as f32, yy as f32 / n as f32));
            px.push(Color32::from_rgb(c[0], c[1], c[2]));
        }
    }
    egui::ColorImage { size: [n, n], source_size: Vec2::new(n as f32, n as f32), pixels: px }
}

fn preview_image(ed: &mut EditorState, ui: &mut Ui, id: u64, mat: &GpuMaterial, shape: PreviewShape, size: f32, px: u32, selected: bool) -> egui::Response {
    let tid = ed.request_preview(id, PreviewScene::material_ball(mat, shape), px);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if let Some(tid) = tid {
        ui.painter().image(tid, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        ui.painter().rect_filled(rect, 2.0, theme::PANEL_DARK);
    }
    let stroke = if selected { egui::Stroke::new(2.0, theme::ACCENT) } else { egui::Stroke::new(1.0, theme::SHADOW) };
    ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    resp
}

fn dot(ui: &mut Ui, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    let c = rect.center();
    ui.painter().circle_filled(c, 7.0, theme::SHADOW);
    ui.painter().circle_stroke(c, 7.0, egui::Stroke::new(1.0, if resp.hovered() { theme::HIGHLIGHT } else { theme::TEXT_DIM }));
    if on {
        ui.painter().circle_filled(c, 4.0, theme::ACCENT);
    }
    resp.clicked()
}

pub fn draw(
    ed: &mut EditorState,
    ui: &mut Ui,
    materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
    objects: &mut [SceneObject],
    scene_ctrl: &mut SceneController,
    dirty: &mut bool,
) {
    ed.selected_material_slot = ed.selected_material_slot.min(GpuMaterial::NUM_PRESETS - 1);
    let slot = ed.selected_material_slot;

    // ---- Top: slot thumbnails + name / copy / paste / apply ----
    ui.horizontal(|ui| {
        for i in 0..GpuMaterial::NUM_PRESETS {
            let m = materials[i];
            let r = preview_image(ed, ui, SLOT_ID + i as u64, &m, PreviewShape::Sphere, 46.0, THUMB_PX, i == slot);
            if r.on_hover_text(format!("Slot {}: {}", i, ed.material_names[i])).clicked() {
                ed.selected_material_slot = i;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(format!("Slot {}", slot));
        ui.add(egui::TextEdit::singleline(&mut ed.material_names[slot]).desired_width(160.0));
        if ui.button("Copy").clicked() {
            ed.copied_material = Some(materials[slot]);
            ed.status_message = format!("Copied material slot {}", slot);
        }
        if ui.button("Paste").clicked() {
            if let Some(c) = ed.copied_material {
                materials[slot] = c;
                *dirty = true;
            }
        }
        if ui.button("Save JSON").clicked() {
            let name = format!("custom_material_slot_{}.json", slot);
            if let Ok(j) = serde_json::to_string_pretty(&materials[slot]) {
                let _ = std::fs::write(&name, j);
                ed.status_message = format!("Exported material to {}", name);
            }
        }
        if ui.button("Load JSON").clicked() {
            let name = format!("custom_material_slot_{}.json", slot);
            if let Some(m) = std::fs::read_to_string(&name).ok().and_then(|c| GpuMaterial::from_json(&c)) {
                materials[slot] = m;
                *dirty = true;
                ed.status_message = format!("Imported material from {}", name);
            }
        }
        if !objects.is_empty() && ed.selected_object_idx < objects.len() {
            let obj = &mut objects[ed.selected_object_idx];
            if ui.button(RichText::new(format!("Apply to '{}'", obj.name)).color(theme::ACCENT).strong()).clicked() {
                obj.material_id = slot as u32;
                ed.status_message = format!("Assigned slot {} to '{}'", slot, obj.name);
            }
        }
        if ui.button("Link to Showcase Orb").clicked() {
            scene_ctrl.selected_material = slot as u32;
        }
    });
    ui.separator();

    let avail = ui.available_width();
    let wide = avail >= 1200.0;
    let left_w = if wide { 300.0 } else { 280.0 };
    ui.horizontal_top(|ui| {
        // ---- Left: big preview + shape toggles + presets ----
        ui.vertical(|ui| {
            ui.set_width(left_w);
            let m = materials[slot];
            let shape = ed.mat_lab.shape.to_preview();
            preview_image(ed, ui, MAIN_ID, &m, shape, 280.0, 320, false);
            ui.horizontal(|ui| {
                for (s, n) in [(LabShape::Sphere, "Sphere"), (LabShape::Cube, "Cube"), (LabShape::Plane, "Plane")] {
                    ui.selectable_value(&mut ed.mat_lab.shape, s, n);
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Preset library").strong());
            egui::Grid::new("preset_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                for (k, preset) in GpuMaterial::default_preset_list().iter().enumerate() {
                    if preview_image(ed, ui, PRESET_ID + k as u64, preset, PreviewShape::Sphere, 36.0, THUMB_PX, false)
                        .on_hover_text(PRESET_NAMES[k])
                        .clicked()
                    {
                        materials[slot] = *preset;
                        ed.material_names[slot] = PRESET_NAMES[k].to_string();
                        *dirty = true;
                    }
                    if k % 8 == 7 {
                        ui.end_row();
                    }
                }
            });
        });
        vsep(ui, 320.0);
        // ---- Centre: channel grid (+ texture editor below when narrow) ----
        let rest = (avail - left_w - 40.0).max(300.0);
        let right_w = if wide { (rest * 0.40).clamp(290.0, 380.0) } else { 0.0 };
        let centre_w = if wide { (rest - right_w - 20.0).max(330.0) } else { rest };
        ui.vertical(|ui| {
            ui.set_width(centre_w);
            channel_grid(ed, ui, materials, slot, dirty);
            if !wide {
                ui.add_space(8.0);
                ui.separator();
                texture_editor(ed, ui, materials, slot, dirty);
            }
        });
        if wide {
            vsep(ui, 320.0);
            ui.vertical(|ui| {
                ui.set_width(right_w);
                texture_editor(ed, ui, materials, slot, dirty);
            });
        }
    });
}

fn channel_grid(ed: &mut EditorState, ui: &mut Ui, materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS], slot: usize, dirty: &mut bool) {
    ui.spacing_mut().slider_width = 50.0;
    ui.label(RichText::new("Channels").strong());
    let mat = &mut materials[slot];
    egui::Grid::new("chan_grid").num_columns(7).spacing([4.0, 6.0]).show(ui, |ui| {
        ui.label("");
        for t in ["Value", "A", "B", "C", "D"] {
            ui.label(RichText::new(t).color(theme::TEXT_DIM));
        }
        ui.label("");
        ui.end_row();
        for ch in 0..7 {
            ui.label(CHANNEL_NAMES[ch]);
            let cur = mat.src(ch);
            for s in 0..5u32 {
                if dot(ui, cur == s) {
                    mat.set_src(ch, s);
                    if s > 0 {
                        ed.mat_lab.tex = (s - 1) as usize;
                    }
                    *dirty = true;
                }
            }
            let textured = mat.src(ch) > 0;
            if ch == 0 {
                if textured {
                    ui.label(RichText::new("texture ramp").color(theme::TEXT_DIM));
                } else {
                    let mut c = [mat.diffuse[0], mat.diffuse[1], mat.diffuse[2]];
                    if ui.color_edit_button_rgb(&mut c).changed() {
                        mat.diffuse[..3].copy_from_slice(&c);
                        *dirty = true;
                    }
                }
            } else {
                let max = if ch == 5 { 1.5 } else if ch == 6 { 3.0 } else { 1.0 };
                let tip = if textured { "Texture value x this amount" } else { "Constant value" };
                if ui.add(Slider::new(mat.value_mut(ch), 0.0..=max)).on_hover_text(tip).changed() {
                    *dirty = true;
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    if ui.add(Slider::new(mat.ior_mut(), 1.0..=2.5).text("IOR")).changed() {
        *dirty = true;
    }
    if ui.add(Slider::new(mat.metallic_mut(), 0.0..=1.0).text("Metallic")).changed() {
        *dirty = true;
    }
    ui.label(RichText::new("Pick a texture column to drive a channel; texture values are multiplied by the channel amount.").small().color(theme::TEXT_DIM));
}

fn texture_editor(ed: &mut EditorState, ui: &mut Ui, materials: &mut [GpuMaterial; GpuMaterial::NUM_PRESETS], slot: usize, dirty: &mut bool) {
    ui.label(RichText::new("Texture editor").strong());
    ui.horizontal(|ui| {
        for (i, n) in ["A", "B", "C", "D"].iter().enumerate() {
            ui.selectable_value(&mut ed.mat_lab.tex, i, *n);
        }
    });
    let ti = ed.mat_lab.tex.min(3);
    let mat = &mut materials[slot];
    // CPU swatch, regenerated when the texture parameters change
    let key: Vec<u32> = mat.tex[ti * 4..ti * 4 + 4].iter().flatten().map(|f| f.to_bits()).collect();
    if ed.mat_lab.swatch_keys[ti] != key || ed.mat_lab.swatches[ti].is_none() {
        let img = swatch_image(mat, ti, 96);
        ed.mat_lab.swatches[ti] = Some(ui.ctx().load_texture(format!("mat_swatch_{}", ti), img, egui::TextureOptions::LINEAR));
        ed.mat_lab.swatch_keys[ti] = key;
    }
    if let Some(h) = &ed.mat_lab.swatches[ti] {
        ui.add(egui::Image::new((h.id(), Vec2::splat(160.0))));
    }
    let mut ty = mat.tex[ti * 4][0] as usize;
    egui::ComboBox::from_id_salt("noise_type").selected_text(NOISE_NAMES[ty.min(7)]).show_ui(ui, |ui| {
        for (k, n) in NOISE_NAMES.iter().enumerate() {
            if ui.selectable_value(&mut ty, k, *n).changed() {
                mat.tex[ti * 4][0] = k as f32;
                *dirty = true;
            }
        }
    });
    let mut ch = false;
    ch |= ui.add(Slider::new(&mut mat.tex[ti * 4][1], 0.1..=24.0).text("Scale").logarithmic(true)).changed();
    ch |= ui.add(Slider::new(&mut mat.tex[ti * 4][2], 1.0..=8.0).text("Octaves")).changed();
    ch |= ui.add(Slider::new(&mut mat.tex[ti * 4][3], 0.0..=5.0).text("Warp")).changed();
    ch |= ui.add(Slider::new(&mut mat.tex[ti * 4 + 1][3], 0.2..=5.0).text("Sharpness")).changed();
    for (k, n) in ["Low colour", "Mid colour", "High colour"].iter().enumerate() {
        ui.horizontal(|ui| {
            let mut c = mat.tex_color(ti, k);
            if ui.color_edit_button_rgb(&mut c).changed() {
                mat.set_tex_color(ti, k, c);
                ch = true;
            }
            ui.label(*n);
        });
    }
    if ch {
        *dirty = true;
    }
}

/// Fixed-height vertical divider (a plain `ui.separator()` inside `horizontal_top` stretches full height).
fn vsep(ui: &mut Ui, height: f32) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
    ui.painter().line_segment([rect.center_top(), rect.center_bottom()], egui::Stroke::new(1.0, theme::SHADOW));
    ui.add_space(4.0);
}
