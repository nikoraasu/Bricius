mod camera;
mod editor;
mod generator;
mod io;
mod material;
mod objects;
mod postprocess;
mod scene;
mod sky;
mod state;
mod terrain;
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
                .with_title("Bryce 7 Pro - 3D Environment Creation Studio (Rust + wgpu)")
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
                event_loop.exit();
                return;
            }
            if *key_code == KeyCode::Tab && *element_state == ElementState::Pressed {
                state.editor.visible = !state.editor.visible;
                return;
            }
            if *key_code == KeyCode::F1 && *element_state == ElementState::Pressed {
                state.editor.active_lab = crate::editor::ActiveLab::Tutorial;
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
                    state.last_mouse_pos = None;
                } else if *element_state == ElementState::Pressed {
                    if *button == MouseButton::Right {
                        // RMB drag always navigates the 3D viewport
                        state.mouse_pressed = true;
                        state.last_mouse_pos = None;
                    } else if *button == MouseButton::Left {
                        // LMB drag navigates the viewport if not clicking an egui widget
                        let over_egui = if state.editor.visible {
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
                            state.mouse_pressed = true;
                            state.last_mouse_pos = None;
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if state.mouse_pressed {
                    if let Some((last_x, last_y)) = state.last_mouse_pos {
                        let dx = (position.x - last_x) as f32;
                        let dy = (position.y - last_y) as f32;
                        state.camera.rotate(dx, dy);
                    }
                    state.last_mouse_pos = Some((position.x, position.y));
                } else {
                    state.last_mouse_pos = Some((position.x, position.y));
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

        // 5. Keyboard Navigation & Material Shortcuts (when egui is not focused on text input)
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

                        // Quick material swap keys [1] through [8] on central showcase orb
                        use crate::material::GpuMaterial;
                        match key_code {
                            KeyCode::Digit1 => {
                                state.materials[1] = GpuMaterial::rock();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Mountain Rock material.".to_string();
                            }
                            KeyCode::Digit2 => {
                                state.materials[1] = GpuMaterial::blue_marble();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Blue Marble material.".to_string();
                            }
                            KeyCode::Digit3 => {
                                state.materials[1] = GpuMaterial::chrome();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied 90s Mirror Chrome material.".to_string();
                            }
                            KeyCode::Digit4 => {
                                state.materials[1] = GpuMaterial::emerald_glass();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Emerald Glass material.".to_string();
                            }
                            KeyCode::Digit5 => {
                                state.materials[1] = GpuMaterial::gold();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Hammered Gold material.".to_string();
                            }
                            KeyCode::Digit6 => {
                                state.materials[1] = GpuMaterial::basalt_obsidian();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Basalt Obsidian material.".to_string();
                            }
                            KeyCode::Digit7 => {
                                state.materials[1] = GpuMaterial::ocean_water();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Ocean Water material.".to_string();
                            }
                            KeyCode::Digit8 => {
                                state.materials[1] = GpuMaterial::alien_bioluminescent();
                                state.material_dirty = true;
                                state.editor.status_message = "Applied Alien Bioluminescent material.".to_string();
                            }
                            _ => {}
                        }
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
    println!(" Bryce 7 Pro - Complete 3D Environment Creation Studio");
    println!(" Controls:");
    println!("  - Tab              : Toggle Studio Editor UI on/off");
    println!("  - F1               : Open In-Studio Interactive Tutorial & Guide");
    println!("  - F9               : Render Still Frame to PNG (Capture artwork)");
    println!("  - Mouse Drag (LMB) : Rotate 3D Camera / Look Around");
    println!("  - W, A, S, D       : Fly camera forward / left / back / right");
    println!("  - Space / E        : Fly camera Up");
    println!("  - Shift / Q        : Fly camera Down");
    println!("  - Arrow Left/Right : Rotate Sun Azimuth (360° celestial orbit)");
    println!("  - Arrow Up/Down    : Adjust Sun Elevation (dawn to dusk)");
    println!("  - 1 through 8      : Quick-swap procedural materials on showcase orb");
    println!("  - ESC              : Exit Studio");
    println!("============================================================");

    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
