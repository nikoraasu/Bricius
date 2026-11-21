//! Bryce-like sculpted grey theme with amber accents.
use egui::{Color32, CornerRadius, Stroke};

pub const PANEL: Color32 = Color32::from_rgb(0x5a, 0x5a, 0x5e);
pub const PANEL_DARK: Color32 = Color32::from_rgb(0x4a, 0x4a, 0x4e);
pub const SURROUND: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2e);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xb4, 0xb4, 0xb8);
pub const WIDGET: Color32 = Color32::from_rgb(0x6c, 0x6c, 0x71);
pub const WIDGET_HOVER: Color32 = Color32::from_rgb(0x7a, 0x7a, 0x80);
pub const HIGHLIGHT: Color32 = Color32::from_rgb(0x8e, 0x8e, 0x94);
pub const SHADOW: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x32);
pub const ACCENT: Color32 = Color32::from_rgb(0xd0, 0xa0, 0x3c);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x8a, 0x6a, 0x22);

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    let r = CornerRadius::same(3);
    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x3c, 0x3c, 0x40);
    v.faint_bg_color = Color32::from_rgb(0x52, 0x52, 0x56);
    v.window_stroke = Stroke::new(1.0, SHADOW);
    v.window_corner_radius = r;
    v.menu_corner_radius = r;
    v.selection.bg_fill = ACCENT_DARK;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;

    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, SHADOW);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.corner_radius = r;

    v.widgets.inactive.bg_fill = WIDGET;
    v.widgets.inactive.weak_bg_fill = WIDGET;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, HIGHLIGHT);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;

    v.widgets.hovered.bg_fill = WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xb0, 0xb0, 0xb6));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.corner_radius = r;

    v.widgets.active.bg_fill = ACCENT_DARK;
    v.widgets.active.weak_bg_fill = ACCENT_DARK;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.corner_radius = r;

    v.widgets.open.bg_fill = PANEL_DARK;
    v.widgets.open.weak_bg_fill = PANEL_DARK;
    v.widgets.open.corner_radius = r;
    ctx.set_visuals(v);

    ctx.global_style_mut(|s| {
        for (ts, font) in s.text_styles.iter_mut() {
            match ts {
                egui::TextStyle::Heading => font.size = 16.0,
                egui::TextStyle::Body | egui::TextStyle::Button => font.size = 13.0,
                egui::TextStyle::Small => font.size = 11.0,
                _ => {}
            }
        }
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
    });
}

/// Embossed (raised) bevel: light top/left edge, dark bottom/right edge.
pub fn paint_bevel(painter: &egui::Painter, rect: egui::Rect, sunken: bool) {
    let (hi, lo) = if sunken { (SHADOW, HIGHLIGHT) } else { (HIGHLIGHT, SHADOW) };
    let r = rect.shrink(0.5);
    painter.line_segment([r.left_bottom(), r.left_top()], Stroke::new(1.0, hi));
    painter.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, hi));
    painter.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, lo));
    painter.line_segment([r.right_bottom(), r.left_bottom()], Stroke::new(1.0, lo));
}
