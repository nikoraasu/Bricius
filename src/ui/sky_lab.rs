//! Bryce-style Sky & Fog lab: preset thumbnails, live preview, sun trackball and atmosphere sections.
use egui::{Color32, RichText, Sense, Slider, Ui, Vec2};

use crate::editor::EditorState;
use crate::preview::PreviewScene;
use crate::scene::SceneController;
use crate::sky::SkyConfig;
use crate::ui::theme;
use crate::water::WaterConfig;

const MAIN_ID: u64 = 3000;
const PRESET_ID: u64 = 3100;
const THUMB: f32 = 96.0;

pub fn sky_presets() -> [(&'static str, SkyConfig); 7] {
    [
        ("Clear Blue", SkyConfig::clear_blue()),
        ("Retro Sunset", SkyConfig::retro_sunset()),
        ("Dramatic Storm", SkyConfig::dramatic_storm()),
        ("Twilight Glow", SkyConfig::twilight_glow()),
        ("Starry Night", SkyConfig::starry_night()),
        ("Alien Violet", SkyConfig::alien_violet()),
        ("Cyberpunk Neon", SkyConfig::cyberpunk_neon()),
    ]
}

/// Draws a preview texture (or a placeholder) in `rect`, cropping the square render to `aspect` (w/h).
pub fn draw_preview_tex(ui: &Ui, tid: Option<egui::TextureId>, rect: egui::Rect, aspect: f32) {
    if let Some(tid) = tid {
        let crop = (1.0 - 1.0 / aspect).max(0.0) * 0.5;
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, crop), egui::pos2(1.0, 1.0 - crop));
        ui.painter().image(tid, rect, uv, Color32::WHITE);
    } else {
        ui.painter().rect_filled(rect, 2.0, theme::PANEL_DARK);
    }
}

fn colour(ui: &mut Ui, label: &str, c: &mut [f32; 4]) {
    ui.horizontal(|ui| {
        let mut rgb = [c[0], c[1], c[2]];
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            c[..3].copy_from_slice(&rgb);
        }
        ui.label(label);
    });
}

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(theme::ACCENT));
            body(ui);
        });
    ui.add_space(4.0);
}

/// Azimuth / elevation disc: centre = zenith, rim = -15 deg, ring = horizon. Returns true on change.
fn sun_trackball(ui: &mut Ui, sky: &mut SkyConfig) -> bool {
    const MIN_EL: f32 = -15.0;
    let size = 150.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let c = rect.center();
    let rad = size * 0.5 - 3.0;
    let p = ui.painter();
    let enc = |col: [f32; 4]| Color32::from_rgb(
        (col[0].clamp(0.0, 1.0).powf(0.45) * 255.0) as u8,
        (col[1].clamp(0.0, 1.0).powf(0.45) * 255.0) as u8,
        (col[2].clamp(0.0, 1.0).powf(0.45) * 255.0) as u8,
    );
    let horizon_frac = 90.0 / (90.0 - MIN_EL);
    p.circle_filled(c, rad, enc(sky.ground_color));
    let steps = 16;
    for i in 0..steps {
        let f = 1.0 - i as f32 / steps as f32; // 1 at the horizon ring .. towards centre
        let k = f;
        let col = [0, 1, 2, 3].map(|j| sky.zenith_color[j] + (sky.horizon_color[j] - sky.zenith_color[j]) * k * k);
        p.circle_filled(c, rad * horizon_frac * f, enc(col));
    }
    p.circle_stroke(c, rad, egui::Stroke::new(1.5, theme::SHADOW));
    let horizon_r = rad * horizon_frac;
    p.circle_stroke(c, horizon_r, egui::Stroke::new(1.0, Color32::from_white_alpha(90)));
    p.line_segment([c - Vec2::new(rad, 0.0), c + Vec2::new(rad, 0.0)], egui::Stroke::new(1.0, Color32::from_white_alpha(30)));
    p.line_segment([c - Vec2::new(0.0, rad), c + Vec2::new(0.0, rad)], egui::Stroke::new(1.0, Color32::from_white_alpha(30)));

    let mut changed = false;
    if (resp.dragged() || resp.clicked()) && resp.interact_pointer_pos().is_some() {
        let d = resp.interact_pointer_pos().unwrap() - c;
        let r = (d.length() / rad).clamp(0.0, 1.0);
        sky.sun_elevation = (90.0 - r * (90.0 - MIN_EL)).to_radians();
        if d.length() > 2.0 {
            sky.sun_azimuth = d.y.atan2(d.x).rem_euclid(std::f32::consts::TAU);
        }
        changed = true;
    }
    let r = ((90.0 - sky.sun_elevation.to_degrees()) / (90.0 - MIN_EL)).clamp(0.0, 1.0) * rad;
    let sp = c + Vec2::angled(sky.sun_azimuth) * r;
    let sc = Color32::from_rgb(
        (sky.sun_color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (sky.sun_color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (sky.sun_color[2].clamp(0.0, 1.0) * 255.0) as u8,
    );
    ui.painter().circle_filled(sp, 7.0, sc);
    ui.painter().circle_stroke(sp, 7.0, egui::Stroke::new(1.5, Color32::WHITE));
    resp.on_hover_text("Drag to place the sun: angle = azimuth, distance from centre = how far below the zenith");
    changed
}

fn xorshift_seed() -> u32 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(12345);
    n ^ n.rotate_left(13)
}

fn sun_section(ui: &mut Ui, sky: &mut SkyConfig, pp: &mut crate::postprocess::PostProcessConfig) {
    section(ui, "Sun", |ui| {
        ui.horizontal(|ui| {
            sun_trackball(ui, sky);
            ui.vertical(|ui| {
                let mut az = sky.sun_azimuth.to_degrees().rem_euclid(360.0);
                let mut el = sky.sun_elevation.to_degrees();
                ui.horizontal(|ui| {
                    ui.label("Azimuth");
                    if ui.add(egui::DragValue::new(&mut az).range(0.0..=360.0).suffix(" deg")).changed() {
                        sky.sun_azimuth = az.to_radians();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Elevation");
                    if ui.add(egui::DragValue::new(&mut el).range(-15.0..=90.0).suffix(" deg")).changed() {
                        sky.sun_elevation = el.to_radians();
                    }
                });
                colour(ui, "Sun colour", &mut sky.sun_color);
            });
        });
        ui.add(Slider::new(&mut sky.sun_intensity, 0.2..=5.0).text("Intensity"));
        ui.add(Slider::new(&mut pp.sun_angular_radius_deg, 0.1..=5.0).text("Angular size (deg)"));
        ui.add(Slider::new(&mut sky.mie_coeff, 0.1..=3.0).text("Corona glow"));
    });
}

fn colours_section(ui: &mut Ui, sky: &mut SkyConfig) {
    section(ui, "Sky colours", |ui| {
        colour(ui, "Zenith", &mut sky.zenith_color);
        colour(ui, "Horizon", &mut sky.horizon_color);
        colour(ui, "Ground", &mut sky.ground_color);
    });
}

fn haze_section(ui: &mut Ui, sky: &mut SkyConfig) {
    section(ui, "Haze & Fog", |ui| {
        ui.add(Slider::new(&mut sky.fog_density, 0.0..=0.05).text("Density"));
        ui.add(Slider::new(&mut sky.fog_height_falloff, 0.0..=0.2).text("Height falloff"));
        colour(ui, "Fog colour", &mut sky.fog_color);
    });
}

fn cloud_section(ui: &mut Ui, sky: &mut SkyConfig) {
    section(ui, "Clouds", |ui| {
        ui.add(Slider::new(&mut sky.cloud_coverage, 0.0..=1.5).text("Coverage"));
        ui.add(Slider::new(&mut sky.cloud_density, 0.0..=2.0).text("Opacity"));
        ui.add(Slider::new(&mut sky.cloud_altitude, 15.0..=200.0).text("Altitude"));
        ui.add(Slider::new(&mut sky.cloud_scale, 0.004..=0.06).logarithmic(true).text("Scale"));
        ui.add(Slider::new(&mut sky.cloud_softness, 0.0..=1.0).text("Edge softness"));
        ui.add(Slider::new(&mut sky.cloud_cumulus, 0.0..=1.0).text("Cumulus shading"))
            .on_hover_text("0 = flat cloud sheet, 1 = puffy clouds lit from the sun's side with darker undersides");
        let mut wd = sky.cloud_wind_dir.rem_euclid(360.0);
        if ui.add(Slider::new(&mut wd, 0.0..=360.0).suffix(" deg").text("Wind direction")).changed() {
            sky.cloud_wind_dir = wd;
        }
        ui.add(Slider::new(&mut sky.cloud_wind_speed, 0.0..=5.0).text("Wind drift"))
            .on_hover_text("Shifts the cloud pattern along the wind direction (and animates it when waves are animated)");
        colour(ui, "Cloud colour", &mut sky.cloud_color);
    });
}

fn stars_moon_section(ui: &mut Ui, sky: &mut SkyConfig) {
    section(ui, "Stars & Moon", |ui| {
        ui.checkbox(&mut sky.stars_enabled, "Stars (visible when the sun is low)");
        ui.add_enabled_ui(sky.stars_enabled, |ui| {
            ui.add(Slider::new(&mut sky.stars_intensity, 0.0..=4.0).text("Brightness"));
            ui.add(Slider::new(&mut sky.stars_density, 0.1..=5.0).logarithmic(true).text("Density"));
        });
        ui.separator();
        ui.checkbox(&mut sky.moon_enabled, "Moon");
        ui.add_enabled_ui(sky.moon_enabled, |ui| {
            let mut az = sky.moon_azimuth.to_degrees().rem_euclid(360.0);
            let mut el = sky.moon_elevation.to_degrees();
            if ui.add(Slider::new(&mut az, 0.0..=360.0).suffix(" deg").text("Azimuth")).changed() {
                sky.moon_azimuth = az.to_radians();
            }
            if ui.add(Slider::new(&mut el, -5.0..=90.0).suffix(" deg").text("Elevation")).changed() {
                sky.moon_elevation = el.to_radians();
            }
            ui.add(Slider::new(&mut sky.moon_size, 0.3..=5.0).logarithmic(true).text("Size"));
            ui.add(Slider::new(&mut sky.moon_intensity, 0.0..=3.0).text("Brightness"));
            colour(ui, "Moon colour", &mut sky.moon_color);
            ui.checkbox(&mut sky.moon_always, "Always show (even in daylight)");
        });
    });
}

pub fn draw(ed: &mut EditorState, ui: &mut Ui, scene_ctrl: &mut SceneController) {
    // ---- Preset thumbnails ----
    ui.label(RichText::new("Sky presets").strong());
    let water = WaterConfig::ocean();
    ui.horizontal_wrapped(|ui| {
        for (k, (name, cfg)) in sky_presets().into_iter().enumerate() {
            let tid = ed.request_preview(PRESET_ID + k as u64, PreviewScene::sky_thumb(&cfg, &water), THUMB as u32);
            ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(THUMB), Sense::click());
                draw_preview_tex(ui, tid, rect, 1.0);
                let active = cfg.sky_type == scene_ctrl.sky.sky_type;
                let stroke = if active { egui::Stroke::new(2.0, theme::ACCENT) } else if resp.hovered() { egui::Stroke::new(1.5, theme::HIGHLIGHT) } else { egui::Stroke::new(1.0, theme::SHADOW) };
                ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                ui.label(RichText::new(name).small());
                if resp.on_hover_text(format!("Apply the '{}' sky", name)).clicked() {
                    scene_ctrl.sky = cfg;
                    ed.status_message = format!("Applied sky preset '{}'.", name);
                }
            });
        }
    });
    ui.separator();

    let avail = ui.available_width();
    ui.horizontal_top(|ui| {
        // ---- Left: live preview ----
        ui.vertical(|ui| {
            ui.set_width(330.0);
            ui.label(RichText::new("Live preview").strong());
            let tid = ed.request_preview(MAIN_ID, PreviewScene::sky_thumb(&scene_ctrl.sky, &scene_ctrl.water), 320);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(320.0, 240.0), Sense::hover());
            draw_preview_tex(ui, tid, rect, 4.0 / 3.0);
            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme::SHADOW), egui::StrokeKind::Inside);
            ui.add_space(6.0);
            if ui.button(RichText::new("Randomize sky").strong().color(theme::ACCENT)).clicked() {
                let mut s = xorshift_seed();
                let mut next = || {
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    (s % 10_000) as f32 / 10_000.0
                };
                crate::generator::SceneGenerator::reroll_sky((next() * 1000.0) as u32, &mut scene_ctrl.sky);
                scene_ctrl.sky.sun_azimuth = next() * std::f32::consts::TAU;
                scene_ctrl.sky.sun_elevation = (-8.0 + next() * 70.0).to_radians();
                scene_ctrl.sky.cloud_coverage = next() * 1.2;
                scene_ctrl.sky.fog_density = 0.002 + next() * 0.02;
                ed.status_message = "Randomized the sky.".to_string();
            }
            ui.label(RichText::new("Camera sits at eye height over the water, facing the sun's side of the horizon.").small().color(theme::TEXT_DIM));
        });
        ui.add_space(10.0);

        // ---- Right: Bryce Sky & Fog sections ----
        ui.vertical(|ui| {
            ui.set_width((avail - 330.0 - 24.0).max(300.0));
            ui.spacing_mut().slider_width = 110.0;
            let two_col = ui.available_width() > 560.0;
            let sky = &mut scene_ctrl.sky;
            let pp = &mut scene_ctrl.postprocess;
            if two_col {
                ui.columns(2, |cols| {
                    sun_section(&mut cols[0], sky, pp);
                    cloud_section(&mut cols[0], sky);
                    colours_section(&mut cols[1], sky);
                    haze_section(&mut cols[1], sky);
                    stars_moon_section(&mut cols[1], sky);
                });
            } else {
                sun_section(ui, sky, pp);
                colours_section(ui, sky);
                haze_section(ui, sky);
                cloud_section(ui, sky);
                stars_moon_section(ui, sky);
            }
        });
    });
}
