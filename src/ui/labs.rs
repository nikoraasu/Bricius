//! Full-screen lab workspaces with OK / Cancel (revert to a snapshot taken on open).
use egui::{RichText, Ui};

use super::{theme, World};
use crate::camera::Camera;
use crate::editor::{ActiveLab, EditorState};
use crate::material::GpuMaterial;
use crate::objects::SceneObject;
use crate::scene::SceneController;
use crate::terrain::TerrainObjectData;

/// Everything a lab may modify, captured when the lab opens.
pub struct LabSnapshot {
    scene: SceneController,
    materials: [GpuMaterial; GpuMaterial::NUM_PRESETS],
    objects: Vec<SceneObject>,
    terrains: Vec<TerrainObjectData>,
    cam_pos: glam::Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
    cam_fov: f32,
    cam_speed: f32,
}

impl LabSnapshot {
    pub fn take(w: &World) -> Self {
        Self {
            scene: w.scene_ctrl.clone(),
            materials: *w.materials,
            objects: w.objects.clone(),
            terrains: w.terrains.clone(),
            cam_pos: w.camera.position,
            cam_yaw: w.camera.yaw,
            cam_pitch: w.camera.pitch,
            cam_fov: w.camera.fov_y,
            cam_speed: w.camera.speed,
        }
    }

    pub fn restore(self, w: &mut World) {
        *w.scene_ctrl = self.scene;
        *w.materials = self.materials;
        *w.objects = self.objects;
        *w.terrains = self.terrains;
        let cam: &mut Camera = w.camera;
        cam.position = self.cam_pos;
        cam.yaw = self.cam_yaw;
        cam.pitch = self.cam_pitch;
        cam.fov_y = self.cam_fov;
        cam.speed = self.cam_speed;
        *w.material_dirty = true;
        *w.terrain_dirty = true;
    }
}

pub fn lab_title(lab: ActiveLab) -> &'static str {
    match lab {
        ActiveLab::Tutorial => "Studio Tutorial & Controls Guide",
        ActiveLab::Generator => "Procedural 1-Click World Generator",
        ActiveLab::Terrain => "Terrain Lab & Heightmap Sculptor",
        ActiveLab::Sky => "Sky Lab & Atmosphere",
        ActiveLab::Water => "Water & Ocean Lab",
        ActiveLab::Materials => "Materials Lab & Deep Texture Editor",
        ActiveLab::Scene => "Scene Setup, Ground & Scatter",
        ActiveLab::Render => "Render Settings & Project Export",
    }
}

/// Draws the lab workspace (below the title strip). Returns after handling OK / Cancel / Escape.
pub fn draw_lab(ed: &mut EditorState, _ctx: &egui::Context, root: &mut Ui, w: &mut World, lab: ActiveLab) {
    if ed.lab_snapshot.is_none() {
        ed.lab_snapshot = Some(LabSnapshot::take(w));
    }
    let mut ok = false;
    let mut cancel = std::mem::take(&mut ed.lab_cancel_requested);

    egui::Panel::bottom("lab_buttons")
        .frame(egui::Frame::new().fill(theme::PANEL_DARK).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(root, |ui: &mut Ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([90.0, 28.0], egui::Button::new(RichText::new("OK").strong())).clicked() {
                    ok = true;
                }
                if ui.add_sized([90.0, 28.0], egui::Button::new("Cancel")).clicked() {
                    cancel = true;
                }
                ui.label(RichText::new("OK keeps the changes. Cancel (or Esc) reverts to how things were when the lab opened.").color(theme::TEXT_DIM));
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(14)))
        .show(root, |ui: &mut Ui| {
            ui.label(RichText::new(lab_title(lab)).size(18.0).strong().color(theme::ACCENT));
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui: &mut Ui| match lab {
                ActiveLab::Tutorial => ed.draw_tutorial_lab(ui, w.scene_ctrl, w.camera, w.materials, w.objects, w.terrains, w.material_dirty),
                ActiveLab::Generator => ed.draw_generator_lab(ui, w.scene_ctrl, w.materials, w.objects),
                ActiveLab::Terrain => ed.draw_terrain_lab(ui, w.scene_ctrl, w.objects, w.terrains, w.terrain_dirty, w.camera, w.materials),
                ActiveLab::Sky => ed.draw_sky_lab(ui, w.scene_ctrl),
                ActiveLab::Water => ed.draw_water_lab(ui, w.scene_ctrl),
                ActiveLab::Materials => ed.draw_materials_lab(ui, w.materials, w.objects, w.scene_ctrl, w.material_dirty),
                ActiveLab::Scene => ed.draw_scene_lab(ui, w.scene_ctrl, w.objects, w.meshes, w.camera, w.mesh_dirty, w.terrains, w.terrain_dirty),
                ActiveLab::Render => ed.draw_render_lab(ui, w.scene_ctrl, w.camera, w.materials, w.objects, w.terrains, w.terrain_dirty),
            });
        });

    if cancel {
        if let Some(snap) = ed.lab_snapshot.take() {
            snap.restore(w);
        }
        ed.heightmap_dirty = true;
        ed.selected_object_idx = ed.selected_object_idx.min(w.objects.len().saturating_sub(1));
        ed.status_message = format!("{}: changes cancelled.", lab_title(lab));
        ed.active_lab = None;
    } else if ok {
        ed.lab_snapshot = None;
        ed.status_message = format!("{}: changes kept.", lab_title(lab));
        ed.active_lab = None;
    }
}
