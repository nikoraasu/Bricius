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

        // Pass event to Egui first
        let response = state.egui_winit_state.on_window_event(&state.window, &event);
        if response.consumed {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size);
            }
            WindowEvent::RedrawRequested => {
                state.update();
                if let Err(e) = state.render() {
                    eprintln!("Render error: {}", e);
                }
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: element_state,
                        ..
                    },
                ..
            } => {
                if key_code == KeyCode::Escape {
                    event_loop.exit();
                    return;
                }
                if key_code == KeyCode::Tab && element_state == ElementState::Pressed {
                    state.editor.visible = !state.editor.visible;
                    return;
                }
                match element_state {
                    winit::event::ElementState::Pressed => {
                        state.keys_pressed.insert(key_code);
                    }
                    winit::event::ElementState::Released => {
                        state.keys_pressed.remove(&key_code);
                    }
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: element_state,
                ..
            } => {
                state.mouse_pressed = element_state == ElementState::Pressed;
                if !state.mouse_pressed {
                    state.last_mouse_pos = None;
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
            _ => (),
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
    println!("  - Mouse Drag (LMB) : Rotate 3D Camera / Look Around");
    println!("  - W, A, S, D       : Fly camera forward / left / back / right");
    println!("  - Space / E        : Fly camera Up");
    println!("  - Shift / Q        : Fly camera Down");
    println!("  - Arrow Left/Right : Rotate Sun Azimuth (360° celestial orbit)");
    println!("  - Arrow Up/Down    : Adjust Sun Elevation (dawn to dusk)");
    println!("  - ESC              : Exit Studio");
    println!("============================================================");

    let event_loop = EventLoop::new().unwrap();
    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
