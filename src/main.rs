#![windows_subsystem = "windows"]
mod app;
mod document;
mod field;
mod graph;
mod graphics;
mod grid;
mod grid_segment_iterator;
mod interaction;
mod math;
mod mouse_operation;
mod net_client_connection;
mod profiler;
mod sdf;
mod some_or;
mod tool;
mod ui;
mod undo_stack;
mod zip_fs;
mod zone;

use crate::document::{ChangeMask, Document};
use crate::math::critically_damped_spring;
use crate::net_client_connection::ConnectionEvent;
use crate::zone::AnyZone;
use anyhow::{Context, Result};
use app::*;
use bincode::Options;
use core::default::Default;
use editor_protocol::EditorServerMessage;
use glam::vec2;
use log::{error, info};
use miniquad::{conf, EventHandler, KeyMods, PassAction, UserData};
use rimui::*;
use std::path::PathBuf;
use tool::Tool;
use tracy_client::span;

// #[global_allocator]
// static GLOBAL: ProfiledAllocator<std::alloc::System> =
//     ProfiledAllocator::new(std::alloc::System, 100);

impl EventHandler for App {
    fn update(&mut self, context: &mut miniquad::Context) {
        let _span = span!("update");
        let time = (miniquad::date::now() - self.start_time) as f32;
        let dt = time - self.last_time;

        if let Err(error) = self.network_update() {
            error!("network_update: {:?}", error);
            self.report_error(Result::<()>::Err(error));
        }

        critically_damped_spring(
            &mut self.view.zoom,
            &mut self.view.zoom_velocity,
            self.view.zoom_target,
            dt,
            0.125,
        );

        self.ui(context, time, dt);

        if self.dirty_mask != ChangeMask::default() {
            self.generation_profiler.begin_frame();
            self.graphics.borrow_mut().generate(
                &self.doc,
                self.dirty_mask,
                false,
                Some(context),
                &mut self.generation_profiler,
            );
            self.dirty_mask = ChangeMask::default();
        }

        self.last_time = time;
        tracy_client::finish_continuous_frame!("update");
    }

    fn draw(&mut self, context: &mut miniquad::Context) {
        let _time = (miniquad::date::now() - self.start_time) as f32;
        context.begin_default_pass(PassAction::Clear {
            color: Some((0.0, 0.0, 0.0, 1.0)),
            depth: None,
            stencil: None,
        });

        self.batch.begin_frame();
        self.batch.clear();

        let g = self.graphics.borrow();
        g.draw_map(
            &mut self.batch,
            &self.view,
            self.window_size.into(),
            self.white_texture,
            self.finish_texture,
            self.pipeline_sdf,
            context,
        );

        context.apply_pipeline(&self.pipeline);
        context.apply_uniforms(&ShaderUniforms {
            screen_size: self.window_size,
        });
        self.batch.set_image(self.white_texture);

        let screen_origin = self.document_to_screen(vec2(0.0, 0.0));
        self.batch
            .geometry
            .fill_circle_aa(screen_origin, 4.0, 4, [255, 255, 255, 255]);

        if self.doc.show_reference {
            if let Some(reference) = g.reference_texture {
                let reference_scale = self.doc.reference_scale;
                let w = (reference.width as i32) * reference_scale;
                let h = (reference.height as i32) * reference_scale;

                let t = self.view.world_to_screen();

                let p0 = t.transform_point2(vec2(0.0, 0.0));
                let p1 = t.transform_point2(vec2(w as f32, h as f32));

                self.batch.set_image(reference);
                self.batch.geometry.fill_rect_uv(
                    [p0.x, p0.y, p1.x, p1.y],
                    [0.0, 0.0, 1.0, 1.0],
                    [255, 255, 255, 255],
                );
            }
        }

        // actual map drawing
        self.batch.set_image(self.white_texture);
        self.graphics.borrow().draw_map(
            &mut self.batch,
            &self.view,
            self.window_size.into(),
            self.white_texture,
            self.finish_texture,
            self.pipeline_sdf,
            context,
        );

        context.apply_pipeline(&self.pipeline);
        context.apply_uniforms(&ShaderUniforms {
            screen_size: self.window_size,
        });
        self.batch.set_image(self.white_texture);

        match self.tool {
            Tool::Graph => {
                let doc = &self.doc;
                let graph_key = Document::layer_graph(&doc.layers, doc.active_layer);
                if let Some(graph) = doc.graphs.get(graph_key) {
                    graph.draw_graph(&mut self.batch, self.last_mouse_pos, &self.view);
                }
            }
            Tool::Zone => {
                AnyZone::draw_zones(
                    &mut self.batch,
                    &self.doc.markup,
                    &self.view,
                    self.doc.zone_selection,
                    self.last_mouse_pos,
                );
            }
            _ => {}
        }

        self.batch.flush(None, context);

        self.operation_batch.draw(context, None);

        let white_texture = self.white_texture.clone();
        let mut render = MiniquadRender::new(&mut self.batch, &self.font_manager, |_sprite_key| {
            white_texture.clone()
        });
        self.ui.render_ui(&mut render, None);

        self.batch.flush(None, context);

        context.end_render_pass();

        context.commit_frame();
        tracy_client::finish_continuous_frame!("draw");
    }

    fn resize_event(&mut self, _context: &mut miniquad::Context, width: f32, height: f32) {
        self.window_size = [width, height];
        self.view.screen_width_px = width - 200.0;
        self.view.screen_height_px = height;
    }

    fn mouse_motion_event(&mut self, context: &mut miniquad::Context, x: f32, y: f32) {
        self.last_mouse_pos = vec2(x, y);

        self.handle_event(
            UIEvent::MouseMove {
                pos: [x as i32, y as i32],
            },
            context,
        );
    }

    fn mouse_wheel_event(&mut self, context: &mut miniquad::Context, _dx: f32, dy: f32) {
        self.handle_event(
            UIEvent::MouseWheel {
                pos: [self.last_mouse_pos[0] as i32, self.last_mouse_pos[1] as i32],
                delta: dy,
            },
            context,
        );
    }

    fn mouse_button_down_event(
        &mut self,
        context: &mut miniquad::Context,
        button: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.handle_event(
            UIEvent::MouseDown {
                pos: [x as i32, y as i32],
                button: ui_mouse_button(button),
                time: miniquad::date::now(),
            },
            context,
        );
    }

    fn mouse_button_up_event(
        &mut self,
        context: &mut miniquad::Context,
        button: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.handle_event(
            UIEvent::MouseUp {
                pos: [x as i32, y as i32],
                button: ui_mouse_button(button),
            },
            context,
        );
    }

    fn char_event(
        &mut self,
        context: &mut miniquad::Context,
        character: char,
        keymods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        if !keymods.ctrl {
            self.handle_event(
                UIEvent::TextInput {
                    text: character.to_string(),
                },
                context,
            );
        }
    }

    fn quit_requested_event(&mut self, context: &mut miniquad::Context) {
        if self.ask_to_save_changes(|_, context| context.quit()) {
            context.cancel_quit();
        }
    }

    fn key_down_event(
        &mut self,
        context: &mut miniquad::Context,
        keycode: miniquad::KeyCode,
        keymods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        let modifier_index = match keycode {
            miniquad::KeyCode::LeftControl | miniquad::KeyCode::RightControl => {
                Some(MODIFIER_CONTROL)
            }
            miniquad::KeyCode::LeftShift | miniquad::KeyCode::RightShift => Some(MODIFIER_SHIFT),
            miniquad::KeyCode::LeftAlt | miniquad::KeyCode::RightAlt => Some(MODIFIER_ALT),
            _ => None,
        };
        if let Some(modifier_index) = modifier_index {
            self.modifier_down[modifier_index] = true;
        }

        let ui_keycode = match keycode {
            miniquad::KeyCode::Enter | miniquad::KeyCode::KpEnter => Some(KeyCode::Enter),
            miniquad::KeyCode::Left => Some(KeyCode::Left),
            miniquad::KeyCode::Right => Some(KeyCode::Right),
            miniquad::KeyCode::Up => Some(KeyCode::Up),
            miniquad::KeyCode::Down => Some(KeyCode::Down),
            miniquad::KeyCode::Home => Some(KeyCode::Home),
            miniquad::KeyCode::End => Some(KeyCode::End),
            miniquad::KeyCode::PageUp => Some(KeyCode::PageUp),
            miniquad::KeyCode::PageDown => Some(KeyCode::PageDown),
            miniquad::KeyCode::Delete => Some(KeyCode::Delete),
            miniquad::KeyCode::Backspace => Some(KeyCode::Backspace),
            miniquad::KeyCode::Z => Some(KeyCode::Z),
            miniquad::KeyCode::X => Some(KeyCode::X),
            miniquad::KeyCode::C => Some(KeyCode::C),
            miniquad::KeyCode::V => Some(KeyCode::V),
            miniquad::KeyCode::Y => Some(KeyCode::Y),
            miniquad::KeyCode::A => Some(KeyCode::A),
            miniquad::KeyCode::Key1 => Some(KeyCode::Key1),
            miniquad::KeyCode::Key2 => Some(KeyCode::Key2),
            miniquad::KeyCode::Key3 => Some(KeyCode::Key3),
            miniquad::KeyCode::Key4 => Some(KeyCode::Key4),
            miniquad::KeyCode::Key5 => Some(KeyCode::Key5),
            miniquad::KeyCode::Key6 => Some(KeyCode::Key6),
            miniquad::KeyCode::Key7 => Some(KeyCode::Key7),
            miniquad::KeyCode::Key8 => Some(KeyCode::Key8),
            miniquad::KeyCode::Key9 => Some(KeyCode::Key9),
            miniquad::KeyCode::Key0 => Some(KeyCode::Key0),
            _ => None,
        };

        if let Some(ui_keycode) = ui_keycode {
            let event = UIEvent::KeyDown {
                key: ui_keycode,
                control: keymods.ctrl,
                shift: keymods.shift,
                alt: keymods.alt,
            };
            self.handle_event(event, context);
        }
    }

    fn key_up_event(
        &mut self,
        _ctx: &mut miniquad::Context,
        keycode: miniquad::KeyCode,
        _keymods: KeyMods,
    ) {
        let modifier_index = match keycode {
            miniquad::KeyCode::LeftControl | miniquad::KeyCode::RightControl => {
                Some(MODIFIER_CONTROL)
            }
            miniquad::KeyCode::LeftShift | miniquad::KeyCode::RightShift => Some(MODIFIER_SHIFT),
            miniquad::KeyCode::LeftAlt | miniquad::KeyCode::RightAlt => Some(MODIFIER_ALT),
            _ => None,
        };
        if let Some(modifier_index) = modifier_index {
            self.modifier_down[modifier_index] = false;
        }
    }
}

fn ui_mouse_button(button: miniquad::MouseButton) -> i32 {
    match button {
        miniquad::MouseButton::Left => 1,
        miniquad::MouseButton::Right => 2,
        miniquad::MouseButton::Middle => 3,
        miniquad::MouseButton::Unknown => 4,
    }
}

fn find_resources_path() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let mut resources_path = current_exe.parent()?.to_path_buf();
    loop {
        let in_target = resources_path.ends_with("target");
        if !resources_path.pop() {
            return None;
        }
        if in_target {
            resources_path.push("res");
            return Some(resources_path);
        }
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // change current directory to res/ if we are being run from target/..
        if let Some(resources_path) = find_resources_path() {
            std::env::set_current_dir(&resources_path).expect("failed to set current directory");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    simple_logger::SimpleLogger::new()
        .with_module_level("ws", log::LevelFilter::Warn)
        .with_module_level("mio", log::LevelFilter::Warn)
        .with_module_level("ureq", log::LevelFilter::Warn)
        .init()
        .unwrap();

    miniquad::start(
        conf::Conf {
            window_title: "CBA Editor".to_owned(),
            sample_count: 0,
            window_width: 1440,
            window_height: 800,
            ..Default::default()
        },
        |mut context| UserData::owning(App::new(&mut context), context),
    );
}

impl App {
    fn network_update(&mut self) -> Result<()> {
        while let Some(event) = self.connection.poll() {
            match event {
                ConnectionEvent::Received(bytes) => {
                    let message: EditorServerMessage = bincode::options()
                        .deserialize(&bytes)
                        .context("Deserializing network message")?;
                    self.on_server_message(message)?;
                }
                ConnectionEvent::Connected(_) => {
                    info!("Connected");
                }
                ConnectionEvent::FailedToConnect(_) => {
                    info!("Failed to connect");
                    self.play_state = PlayState::Offline;
                }
                ConnectionEvent::Disconnected(_) => {
                    info!("Disconnected");
                    self.play_state = PlayState::Offline;
                }
            }
        }

        if let Some(mut op) = self.network_operation.take() {
            if !op(self) && self.network_operation.is_none() {
                self.network_operation = Some(op);
            }
        }
        Ok(())
    }
    fn on_server_message(&mut self, message: EditorServerMessage) -> Result<()> {
        info!("{:?}", &message);
        match message {
            EditorServerMessage::Welcome { .. } => {}
            EditorServerMessage::ConnectionAborted { .. } => {}
            EditorServerMessage::JoinedSession {
                id: _,
                url,
                new_session,
            } => {
                self.play_state = PlayState::Connected { url: url.clone() };
                if new_session {
                    let _ = open::that(&url);
                }
            }
            EditorServerMessage::LeftSession { .. } => {
                self.play_state = PlayState::Offline;
            }
        }
        Ok(())
    }
}
