mod bvh;
mod camera;
mod editor;
mod generator;
mod ground;
mod io;
mod light;
mod material;
mod preview;
mod model_loader;
mod objects;
mod postprocess;
mod scene;
mod sky;
mod state;
mod terrain;
mod ui;
mod undo;
mod water;

use std::sync::Arc;
use state::State;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Bricius - 3D Environment Creation Studio (Rust + wgpu)")
                .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 840.0));

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = pollster::block_on(State::new(Arc::clone(&window)));
            self.state = Some(state);
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        // 1. Fundamental window lifecycle events
        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(physical_size) => {
                state.resize(*physical_size);
            }
            WindowEvent::RedrawRequested => {
                state.update();
                if let Err(e) = state.render() {
                    eprintln!("Render error: {}", e);
                }
                state.window.request_redraw();
                return;
            }
            _ => {}
        }

        // 2. Global Hotkeys (Escape to exit, Tab to toggle UI, F1 for Tutorial)
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: element_state,
                    ..
                },
            ..
        } = &event
        {
            if *key_code == KeyCode::Escape {
                if *element_state == ElementState::Pressed {
                    if state.editor.active_lab.is_some() {
                        state.editor.lab_cancel_requested = true;
                    } else if state.editor.show_help {
                        state.editor.show_help = false;
                    } else {
                        event_loop.exit();
                    }
                }
                return;
            }
            if *key_code == KeyCode::Tab && *element_state == ElementState::Pressed {
                state.editor.visible = !state.editor.visible;
                return;
            }
            if *key_code == KeyCode::F1 && *element_state == ElementState::Pressed {
                state.editor.show_help = !state.editor.show_help;
                state.editor.visible = true;
                return;
            }
            if *key_code == KeyCode::F9 && *element_state == ElementState::Pressed {
                state.editor.request_render = true;
                return;
            }
        }

        // 3. Viewport Mouse Drag Camera Rotation (Supports both LMB & RMB)
        match &event {
            WindowEvent::MouseInput {
                button,
                state: element_state,
                ..
            } => {
                if *element_state == ElementState::Released {
                    state.mouse_pressed = false;
                    state.dragging_object = false;
                    state.last_mouse_pos = None;
                } else if *element_state == ElementState::Pressed {
                    if *button == MouseButton::Right {
                        // RMB drag navigates the 3D viewport (only when started over the framed viewport)
                        if state.pointer_in_viewport() {
                            state.mouse_pressed = true;
                            state.dragging_object = false;
                            state.last_mouse_pos = None;
                        }
                    } else if *button == MouseButton::Left {
                        // LMB: Click to select & drag objects, or drag viewport if clicking background
                        let over_egui = if !state.pointer_in_viewport() {
                            true
                        } else if state.editor.visible {
                            if let Some(pos) = state.egui_ctx.input(|i| i.pointer.latest_pos()) {
                                if let Some(layer) = state.egui_ctx.layer_id_at(pos) {
                                    layer.order != egui::Order::Background
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !over_egui {
                            let mx = state.cursor_pos.map(|(x, _)| x).unwrap_or(state.config.width as f64 * 0.5);
                            let my = state.cursor_pos.map(|(_, y)| y).unwrap_or(state.config.height as f64 * 0.5);
                            let (vx, vy, vw, vh) = state.viewport_px();
                            let (w, h) = (vw as f32, vh as f32);
                            let (mx, my) = (mx - vx as f64, my - vy as f64);
                            let (ro, rd) = state.cam().screen_to_ray(mx as f32, my as f32, w, h);

                            let mut closest_hit: Option<(usize, f32)> = None;
                            for (i, obj) in state.objects.iter().enumerate() {
                                if let Some(t) = obj.intersect_ray(ro, rd) {
                                    if closest_hit.map_or(true, |(_, min_t)| t < min_t) {
                                        closest_hit = Some((i, t));
                                    }
                                }
                            }

                            if let Some((hit_idx, _)) = closest_hit {
                                // Object picked!
                                state.editor.selected_object_idx = hit_idx;
                                state.dragging_object = true;
                                state.drag_start_obj_pos = glam::Vec3::from(state.objects[hit_idx].position);
                                state.drag_start_mouse = glam::Vec2::new(mx as f32, my as f32);
                                state.mouse_pressed = true;
                                state.editor.status_message = format!("Selected '{}' - Drag in viewport to move (Shift: Height, Alt: Scale)", state.objects[hit_idx].name);
                            } else {
                                state.mouse_pressed = true;
                                state.dragging_object = false;
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let over_egui = state
                    .cursor_pos
                    .map(|(x, y)| {
                        let ppp = state.egui_ctx.pixels_per_point();
                        let pos = egui::pos2(x as f32 / ppp, y as f32 / ppp);
                        state.egui_ctx.layer_id_at(pos).map_or(false, |l| l.order != egui::Order::Background)
                    })
                    .unwrap_or(false);
                if state.pointer_in_viewport() && !over_egui {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    };
                    let cam = state.cam_mut();
                    if cam.is_ortho() {
                        cam.zoom_ortho(0.9_f32.powf(lines));
                    } else {
                        let step = cam.forward() * (lines * 1.5);
                        cam.position += step;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let curr_x = position.x;
                let curr_y = position.y;
                state.cursor_pos = Some((curr_x, curr_y));

                if state.dragging_object && state.editor.selected_object_idx < state.objects.len() {
                    let (vx, vy, vw, vh) = state.viewport_px();
                    let (w, h) = (vw as f32, vh as f32);
                    let obj_idx = state.editor.selected_object_idx;

                    let shift_pressed = state.keys_pressed.contains(&KeyCode::ShiftLeft) || state.keys_pressed.contains(&KeyCode::ShiftRight);
                    let alt_pressed = state.keys_pressed.contains(&KeyCode::AltLeft) || state.keys_pressed.contains(&KeyCode::AltRight);

                    if alt_pressed {
                        // Alt + Drag: Uniform Scaling
                        if let Some((_, last_y)) = state.last_mouse_pos {
                            let dy = (last_y - curr_y) as f32 * 0.02;
                            let factor = (1.0 + dy).clamp(0.05, 5.0);
                            state.objects[obj_idx].scale[0] = (state.objects[obj_idx].scale[0] * factor).clamp(0.1, 50.0);
                            state.objects[obj_idx].scale[1] = (state.objects[obj_idx].scale[1] * factor).clamp(0.1, 50.0);
                            state.objects[obj_idx].scale[2] = (state.objects[obj_idx].scale[2] * factor).clamp(0.1, 50.0);
                        }
                    } else if shift_pressed {
                        // Shift + Drag: Vertical Y Elevation
                        if let Some((_, last_y)) = state.last_mouse_pos {
                            let dy = (last_y - curr_y) as f32 * 0.04;
                            state.objects[obj_idx].position[1] += dy;
                        }
                    } else {
                        // Normal Drag: Smooth 3D plane translation
                        let (ro, rd) = state.cam().screen_to_ray((curr_x - vx as f64) as f32, (curr_y - vy as f64) as f32, w, h);
                        // Perspective: slide on the horizontal plane through the object.
                        // Orthographic: slide on the view plane (axes perpendicular to the view direction).
                        let plane_n = if state.cam().is_ortho() { state.cam().forward() } else { glam::Vec3::Y };
                        let denom = rd.dot(plane_n);
                        if denom.abs() > 0.001 {
                            let t = (state.drag_start_obj_pos - ro).dot(plane_n) / denom;
                            if t > 0.1 && t < 600.0 {
                                let hit_point = ro + rd * t;
                                for axis in 0..3 {
                                    if plane_n[axis].abs() < 0.5 {
                                        state.objects[obj_idx].position[axis] = hit_point[axis];
                                    }
                                }
                            }
                        }
                    }
                    state.last_mouse_pos = Some((curr_x, curr_y));
                } else if state.mouse_pressed {
                    if let Some((last_x, last_y)) = state.last_mouse_pos {
                        let dx = (curr_x - last_x) as f32;
                        let dy = (curr_y - last_y) as f32;
                        state.cam_mut().rotate(dx, dy);
                    }
                    state.last_mouse_pos = Some((curr_x, curr_y));
                } else {
                    state.last_mouse_pos = Some((curr_x, curr_y));
                }
            }
            _ => {}
        }

        // 4. Send event to egui (unless we are actively dragging the camera)
        if !state.mouse_pressed {
            let _ = state.egui_winit_state.on_window_event(&state.window, &event);
        } else if let WindowEvent::MouseInput {
            state: ElementState::Released,
            ..
        } = &event
        {
            let _ = state.egui_winit_state.on_window_event(&state.window, &event);
        }

        // 5. Keyboard Navigation (when egui is not focused on text input)
        if !state.egui_ctx.egui_wants_keyboard_input() {
            if let WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: element_state,
                        ..
                    },
                ..
            } = event
            {
                match element_state {
                    ElementState::Pressed => {
                        state.keys_pressed.insert(key_code);
                    }
                    ElementState::Released => {
                        state.keys_pressed.remove(&key_code);
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    println!("============================================================");
    println!(" Bricius - Complete 3D Environment Creation Studio");
    println!(" Controls:");
    println!("  - Tab              : Toggle Studio Editor UI on/off");
    println!("  - F1               : Controls help overlay");
    println!("  - F9               : Render Still Frame to PNG (Capture artwork)");
    println!("  - LMB Click Object : Select 3D object in viewport");
    println!("  - LMB Drag Object  : Move object across landscape (XZ plane)");
    println!("  - Shift + Drag     : Move object vertically (Y elevation)");
    println!("  - Alt + Drag       : Scale object uniformly");
    println!("  - RMB / Drag Sky   : Rotate 3D Camera / 360° Mouse Look");
    println!("  - W, A, S, D       : Fly camera forward / left / back / right");
    println!("  - Space / E        : Fly camera Up");
    println!("  - Shift / Q        : Fly camera Down");
    println!("  - Arrow Left/Right : Rotate Sun Azimuth (360° celestial orbit)");
    println!("  - Arrow Up/Down    : Adjust Sun Elevation (dawn to dusk)");
    println!("  - ESC              : Cancel open lab / close help / exit Studio");
    println!("============================================================");

    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
