//! Procedurally painted palette icons (no image assets, no emoji).
use egui::{epaint::PathShape, pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui};

use super::theme;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IconKind {
    Water,
    Ground,
    Terrain,
    Sphere,
    Cube,
    Cylinder,
    Torus,
    Cone,
    Pyramid,
    Stone,
    Import,
    Models,
    PointLight,
    SpotLight,
    ParallelLight,
    Materials,
    EditObject,
    Duplicate,
    Delete,
    Snap,
    Csg,
    Generator,
    Scatter,
    SkyLab,
    WaterLab,
    Help,
    Sun,
    Undo,
    Redo,
}

fn shade(c: Color32, f: f32) -> Color32 {
    Color32::from_rgb((c.r() as f32 * f).min(255.0) as u8, (c.g() as f32 * f).min(255.0) as u8, (c.b() as f32 * f).min(255.0) as u8)
}

fn poly(p: &Painter, pts: Vec<Pos2>, fill: Color32, stroke: Color32) {
    p.add(PathShape::convex_polygon(pts, fill, Stroke::new(1.0, stroke)));
}

fn ball(p: &Painter, c: Pos2, r: f32, base: Color32) {
    p.circle_filled(c, r, shade(base, 0.55));
    p.circle_filled(c + vec2(-r * 0.12, -r * 0.12), r * 0.8, shade(base, 0.85));
    p.circle_filled(c + vec2(-r * 0.3, -r * 0.3), r * 0.4, shade(base, 1.25));
    p.circle_stroke(c, r, Stroke::new(1.0, shade(base, 0.35)));
}

fn ellipse(p: &Painter, c: Pos2, rx: f32, ry: f32, fill: Color32, stroke: Stroke) {
    let pts: Vec<Pos2> = (0..28).map(|i| {
        let a = i as f32 / 28.0 * std::f32::consts::TAU;
        c + vec2(a.cos() * rx, a.sin() * ry)
    }).collect();
    p.add(PathShape::convex_polygon(pts, fill, stroke));
}

fn wave(p: &Painter, rect: Rect, y: f32, amp: f32, col: Color32) {
    let pts: Vec<Pos2> = (0..=16).map(|i| {
        let t = i as f32 / 16.0;
        pos2(rect.left() + t * rect.width(), y + (t * std::f32::consts::TAU * 2.0).sin() * amp)
    }).collect();
    p.add(egui::Shape::line(pts, Stroke::new(2.0, col)));
}

fn star(p: &Painter, c: Pos2, r: f32, n: usize, col: Color32) {
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        p.line_segment([c + vec2(a.cos(), a.sin()) * r * 0.55, c + vec2(a.cos(), a.sin()) * r], Stroke::new(1.5, col));
    }
}

pub fn paint_icon(p: &Painter, rect: Rect, kind: IconKind, tint: Color32) {
    let r = rect.shrink(rect.width() * 0.16);
    let c = r.center();
    let hw = r.width() * 0.5;
    let blue = Color32::from_rgb(90, 150, 210);
    let stone = Color32::from_rgb(170, 160, 150);
    let amber = Color32::from_rgb(235, 190, 70);
    let outline = shade(tint, 0.4);
    match kind {
        IconKind::Water | IconKind::WaterLab => {
            for i in 0..3 {
                wave(p, r, r.top() + r.height() * (0.25 + 0.25 * i as f32), 2.5, shade(blue, 1.0 + 0.15 * i as f32));
            }
        }
        IconKind::Ground => {
            poly(p, vec![pos2(c.x, r.top() + hw * 0.5), pos2(r.right(), c.y), pos2(c.x, r.bottom() - hw * 0.1), pos2(r.left(), c.y)], Color32::from_rgb(120, 140, 80), outline);
            p.line_segment([pos2(r.left(), c.y), pos2(c.x, r.bottom() - hw * 0.1)], Stroke::new(1.0, shade(Color32::from_rgb(120, 140, 80), 0.6)));
        }
        IconKind::Terrain => {
            poly(p, vec![pos2(r.left(), r.bottom()), pos2(r.left() + hw * 0.7, r.top() + hw * 0.5), pos2(c.x, c.y + hw * 0.1), pos2(r.right() - hw * 0.5, r.top()), pos2(r.right(), r.bottom())], Color32::from_rgb(120, 110, 100), outline);
            poly(p, vec![pos2(r.right() - hw * 0.78, r.top() + hw * 0.4), pos2(r.right() - hw * 0.5, r.top()), pos2(r.right() - hw * 0.25, r.top() + hw * 0.5)], Color32::from_rgb(235, 235, 240), outline);
        }
        IconKind::Sphere => ball(p, c, hw, tint),
        IconKind::Cube => {
            let s = hw * 0.55;
            let top = vec![pos2(c.x, c.y - hw), pos2(c.x + hw * 0.9, c.y - s), pos2(c.x, c.y), pos2(c.x - hw * 0.9, c.y - s)];
            let left = vec![pos2(c.x - hw * 0.9, c.y - s), pos2(c.x, c.y), pos2(c.x, c.y + hw), pos2(c.x - hw * 0.9, c.y + s)];
            let right = vec![pos2(c.x + hw * 0.9, c.y - s), pos2(c.x, c.y), pos2(c.x, c.y + hw), pos2(c.x + hw * 0.9, c.y + s)];
            poly(p, top, shade(tint, 1.15), outline);
            poly(p, left, shade(tint, 0.75), outline);
            poly(p, right, shade(tint, 0.5), outline);
        }
        IconKind::Cylinder => {
            let rx = hw * 0.7;
            let ry = hw * 0.3;
            let (t, b) = (c.y - hw * 0.65, c.y + hw * 0.65);
            p.rect_filled(Rect::from_min_max(pos2(c.x - rx, t), pos2(c.x + rx, b)), 0.0, shade(tint, 0.7));
            ellipse(p, pos2(c.x, b), rx, ry, shade(tint, 0.7), Stroke::new(1.0, outline));
            p.rect_filled(Rect::from_min_max(pos2(c.x - rx + 1.0, t), pos2(c.x + rx - 1.0, b)), 0.0, shade(tint, 0.7));
            p.line_segment([pos2(c.x - rx, t), pos2(c.x - rx, b)], Stroke::new(1.0, outline));
            p.line_segment([pos2(c.x + rx, t), pos2(c.x + rx, b)], Stroke::new(1.0, outline));
            ellipse(p, pos2(c.x, t), rx, ry, shade(tint, 1.15), Stroke::new(1.0, outline));
        }
        IconKind::Torus => {
            ellipse(p, c, hw, hw * 0.7, shade(tint, 0.6), Stroke::new(1.0, outline));
            ellipse(p, c + vec2(0.0, hw * 0.05), hw * 0.45, hw * 0.25, theme::WIDGET, Stroke::new(1.0, outline));
            p.add(egui::Shape::line((0..=12).map(|i| {
                let a = std::f32::consts::PI * (1.1 + 0.8 * i as f32 / 12.0);
                c + vec2(a.cos() * hw * 0.75, a.sin() * hw * 0.5)
            }).collect(), Stroke::new(2.0, shade(tint, 1.3))));
        }
        IconKind::Cone => {
            poly(p, vec![pos2(c.x, c.y - hw), pos2(c.x + hw * 0.7, c.y + hw * 0.6), pos2(c.x - hw * 0.7, c.y + hw * 0.6)], shade(tint, 0.8), outline);
            ellipse(p, pos2(c.x, c.y + hw * 0.6), hw * 0.7, hw * 0.22, shade(tint, 0.55), Stroke::new(1.0, outline));
            p.line_segment([pos2(c.x, c.y - hw), pos2(c.x - hw * 0.25, c.y + hw * 0.55)], Stroke::new(1.5, shade(tint, 1.3)));
        }
        IconKind::Pyramid => {
            let apex = pos2(c.x, c.y - hw);
            let (l, m, rr) = (pos2(c.x - hw * 0.85, c.y + hw * 0.6), pos2(c.x - hw * 0.1, c.y + hw), pos2(c.x + hw * 0.85, c.y + hw * 0.5));
            poly(p, vec![apex, l, m], shade(tint, 1.0), outline);
            poly(p, vec![apex, m, rr], shade(tint, 0.6), outline);
        }
        IconKind::Stone => {
            // Lumpy grey polygon with a lit top-left facet and a shaded base
            let grey = Color32::from_rgb(128, 126, 122);
            let radii = [0.95, 0.78, 0.9, 0.7, 0.92, 0.82, 0.97, 0.74, 0.88];
            let n = radii.len();
            let pts: Vec<Pos2> = radii.iter().enumerate().map(|(i, k)| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU - 0.6;
                c + vec2(a.cos() * hw * k * 1.05, a.sin() * hw * k * 0.8 + hw * 0.1)
            }).collect();
            p.add(PathShape::convex_polygon(pts.clone(), shade(grey, 0.6), Stroke::new(1.0, outline)));
            let top: Vec<Pos2> = vec![pts[5], pts[6], pts[7], pts[8], c + vec2(-hw * 0.05, -hw * 0.05)];
            p.add(PathShape::convex_polygon(top, shade(grey, 1.15), Stroke::NONE));
            let hi: Vec<Pos2> = vec![pts[0], pts[1], c + vec2(hw * 0.1, hw * 0.1), c + vec2(-hw * 0.05, -hw * 0.05), pts[8]];
            p.add(PathShape::convex_polygon(hi, shade(grey, 0.85), Stroke::NONE));
            p.line_segment([pts[2], c + vec2(hw * 0.1, hw * 0.1)], Stroke::new(1.0, shade(grey, 0.4)));
        }
        IconKind::Import => {
            poly(p, vec![pos2(r.left(), r.top() + hw * 0.4), pos2(c.x - hw * 0.1, r.top() + hw * 0.4), pos2(c.x + hw * 0.1, r.top() + hw * 0.7), pos2(r.right(), r.top() + hw * 0.7), pos2(r.right(), r.bottom()), pos2(r.left(), r.bottom())], amber, shade(amber, 0.4));
            let g = Color32::from_rgb(40, 40, 44);
            p.line_segment([pos2(c.x, c.y - hw * 0.1), pos2(c.x, c.y + hw * 0.6)], Stroke::new(2.5, g));
            p.add(PathShape::convex_polygon(vec![pos2(c.x - hw * 0.35, c.y + hw * 0.35), pos2(c.x + hw * 0.35, c.y + hw * 0.35), pos2(c.x, c.y + hw * 0.8)], g, Stroke::NONE));
        }
        IconKind::Models => {
            let s = hw * 0.55;
            let top = vec![pos2(c.x, c.y - hw * 0.8), pos2(c.x + hw * 0.75, c.y - s * 0.7), pos2(c.x, c.y), pos2(c.x - hw * 0.75, c.y - s * 0.7)];
            poly(p, top, shade(stone, 1.1), outline);
            poly(p, vec![pos2(c.x - hw * 0.75, c.y - s * 0.7), pos2(c.x, c.y), pos2(c.x, c.y + hw * 0.8), pos2(c.x - hw * 0.75, c.y + s * 0.1)], shade(stone, 0.75), outline);
            poly(p, vec![pos2(c.x + hw * 0.75, c.y - s * 0.7), pos2(c.x, c.y), pos2(c.x, c.y + hw * 0.8), pos2(c.x + hw * 0.75, c.y + s * 0.1)], shade(stone, 0.5), outline);
            p.add(PathShape::convex_polygon(vec![pos2(r.right() - 7.0, r.bottom() - 5.0), pos2(r.right(), r.bottom() - 5.0), pos2(r.right() - 3.5, r.bottom())], tint, Stroke::NONE));
        }
        IconKind::PointLight => {
            star(p, c, hw, 8, amber);
            ball(p, c, hw * 0.45, amber);
        }
        IconKind::SpotLight => {
            let top = pos2(c.x, c.y - hw * 0.8);
            poly(p, vec![top, pos2(c.x + hw * 0.9, c.y + hw * 0.9), pos2(c.x - hw * 0.9, c.y + hw * 0.9)], Color32::from_rgba_unmultiplied(235, 190, 70, 110), amber);
            ball(p, top, hw * 0.3, amber);
        }
        IconKind::ParallelLight => {
            for i in 0..3 {
                let x = c.x + (i as f32 - 1.0) * hw * 0.6;
                p.line_segment([pos2(x - hw * 0.2, c.y - hw * 0.8), pos2(x + hw * 0.1, c.y + hw * 0.7)], Stroke::new(2.0, amber));
                p.add(PathShape::convex_polygon(vec![pos2(x - hw * 0.1, c.y + hw * 0.5), pos2(x + hw * 0.3, c.y + hw * 0.5), pos2(x + hw * 0.1, c.y + hw * 0.95)], amber, Stroke::NONE));
            }
        }
        IconKind::Materials => {
            ball(p, c, hw, Color32::from_rgb(190, 90, 70));
            p.circle_filled(c + vec2(hw * 0.35, hw * 0.3), hw * 0.28, Color32::from_rgb(80, 150, 200));
            p.circle_filled(c + vec2(-hw * 0.3, hw * 0.4), hw * 0.2, Color32::from_rgb(220, 200, 80));
        }
        IconKind::EditObject => {
            ball(p, c + vec2(-hw * 0.15, hw * 0.1), hw * 0.75, tint);
            let (a, b) = (c + vec2(hw * 0.15, hw * 0.35), c + vec2(hw * 0.95, -hw * 0.75));
            p.line_segment([a, b], Stroke::new(3.0, amber));
            p.circle_filled(b, 2.5, Color32::WHITE);
        }
        IconKind::Duplicate => {
            let a = Rect::from_min_size(pos2(r.left(), r.top()), vec2(hw * 1.3, hw * 1.3));
            let b = a.translate(vec2(hw * 0.7, hw * 0.7));
            p.rect_filled(a, 2.0, shade(tint, 0.6));
            p.rect_stroke(a, 2.0, Stroke::new(1.0, outline), egui::StrokeKind::Inside);
            p.rect_filled(b, 2.0, tint);
            p.rect_stroke(b, 2.0, Stroke::new(1.0, outline), egui::StrokeKind::Inside);
        }
        IconKind::Delete => {
            let col = Color32::from_rgb(210, 80, 70);
            p.line_segment([c + vec2(-hw * 0.7, -hw * 0.7), c + vec2(hw * 0.7, hw * 0.7)], Stroke::new(4.0, col));
            p.line_segment([c + vec2(hw * 0.7, -hw * 0.7), c + vec2(-hw * 0.7, hw * 0.7)], Stroke::new(4.0, col));
        }
        IconKind::Snap => {
            p.line_segment([pos2(r.left(), r.bottom()), pos2(r.right(), r.bottom())], Stroke::new(3.0, Color32::from_rgb(120, 140, 80)));
            ball(p, pos2(c.x, c.y - hw * 0.1), hw * 0.45, tint);
            p.add(PathShape::convex_polygon(vec![pos2(c.x - hw * 0.3, r.bottom() - hw * 0.45), pos2(c.x + hw * 0.3, r.bottom() - hw * 0.45), pos2(c.x, r.bottom() - 2.0)], amber, Stroke::NONE));
        }
        IconKind::Csg => {
            let red = Color32::from_rgba_unmultiplied(220, 100, 80, 200);
            let blu = Color32::from_rgba_unmultiplied(90, 150, 220, 200);
            p.circle_filled(c + vec2(-hw * 0.3, 0.0), hw * 0.7, red);
            p.circle_filled(c + vec2(hw * 0.3, 0.0), hw * 0.7, blu);
            p.circle_stroke(c + vec2(-hw * 0.3, 0.0), hw * 0.7, Stroke::new(1.0, outline));
            p.circle_stroke(c + vec2(hw * 0.3, 0.0), hw * 0.7, Stroke::new(1.0, outline));
        }
        IconKind::Generator => {
            let s = hw * 0.9;
            let die = Rect::from_center_size(c, vec2(s * 1.6, s * 1.6));
            p.rect_filled(die, 4.0, shade(tint, 1.1));
            p.rect_stroke(die, 4.0, Stroke::new(1.0, outline), egui::StrokeKind::Inside);
            for (dx, dy) in [(-0.4, -0.4), (0.4, -0.4), (0.0, 0.0), (-0.4, 0.4), (0.4, 0.4)] {
                p.circle_filled(c + vec2(dx, dy) * s, s * 0.14, Color32::from_rgb(40, 40, 44));
            }
        }
        IconKind::Scatter => {
            for (dx, dy, k) in [(-0.5, 0.3, 0.45), (0.2, 0.5, 0.35), (0.5, -0.1, 0.4), (-0.1, -0.4, 0.5)] {
                poly(p, vec![pos2(c.x + dx * hw, c.y + dy * hw - k * hw), pos2(c.x + dx * hw + k * hw * 0.7, c.y + dy * hw + k * hw * 0.4), pos2(c.x + dx * hw - k * hw * 0.7, c.y + dy * hw + k * hw * 0.4)], Color32::from_rgb(90, 150, 80), outline);
            }
        }
        IconKind::SkyLab => {
            p.rect_filled(r, 3.0, Color32::from_rgb(90, 140, 200));
            p.circle_filled(c + vec2(-hw * 0.3, -hw * 0.2), hw * 0.4, amber);
            ellipse(p, c + vec2(hw * 0.2, hw * 0.35), hw * 0.55, hw * 0.25, Color32::from_rgb(240, 240, 245), Stroke::NONE);
        }
        IconKind::Sun => {
            star(p, c, hw, 8, amber);
            p.circle_filled(c, hw * 0.45, amber);
        }
        IconKind::Undo | IconKind::Redo => {
            // Half-circle arc over the top with an arrow head at the far end (mirrored for Redo)
            let m = if kind == IconKind::Undo { 1.0 } else { -1.0 };
            let rad = hw * 0.62;
            let cc = c + vec2(0.0, hw * 0.15);
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = 0.15 + (std::f32::consts::PI - 0.15) * i as f32 / 16.0;
                    cc + vec2(m * a.cos() * rad, -a.sin() * rad)
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(2.5, tint)));
            let tip = cc + vec2(-m * rad, hw * 0.45);
            p.add(PathShape::convex_polygon(
                vec![tip, tip + vec2(-hw * 0.38, -hw * 0.3), tip + vec2(hw * 0.38, -hw * 0.3)],
                tint,
                Stroke::NONE,
            ));
        }
        IconKind::Help => {
            p.circle_filled(c, hw, shade(tint, 0.6));
            p.text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(hw * 1.5), Color32::WHITE);
        }
    }
}

/// Square icon button with embossed look; returns the response (tooltip text shown on hover).
pub fn icon_button(ui: &mut Ui, kind: IconKind, size: f32, selected: bool, hover: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let painter = ui.painter();
    let fill = if selected {
        theme::ACCENT_DARK
    } else if resp.hovered() {
        theme::WIDGET_HOVER
    } else {
        theme::WIDGET
    };
    painter.rect_filled(rect, 3.0, fill);
    theme::paint_bevel(painter, rect, selected || resp.is_pointer_button_down_on());
    let inner = if resp.is_pointer_button_down_on() { rect.translate(vec2(0.0, 1.0)) } else { rect };
    paint_icon(painter, inner, kind, Color32::from_rgb(190, 190, 196));
    resp.on_hover_text(hover)
}
