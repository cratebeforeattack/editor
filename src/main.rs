mod app;
mod document;
mod graph;
mod graphics;
mod grid;
mod grid_segment_iterator;
mod interaction;
mod math;
mod mouse_operation;
mod sdf;
mod some_or;
mod tool;
mod ui;
mod undo_stack;
mod zone;

use crate::document::{ChangeMask, Layer};
use crate::math::critically_damped_spring;
use crate::zone::AnyZone;
use app::*;
use core::default::Default;
use glam::vec2;
use miniquad::{conf, Context, EventHandler, KeyMods, PassAction, UserData};
use rimui::*;
use tool::Tool;

impl EventHandler for App {
    fn update(&mut self, context: &mut Context) {
        let time = (miniquad::date::now() - self.start_time) as f32;
        let dt = time - self.last_time;

        critically_damped_spring(
            &mut self.view.zoom,
            &mut self.view.zoom_velocity,
            self.view.zoom_target,
            dt,
            0.125,
        );

        self.ui(context, time, dt);

        if self.dirty_mask != ChangeMask::default() {
            self.graphics
                .borrow_mut()
                .generate(&self.doc.borrow(), self.dirty_mask, Some(context));
            self.dirty_mask = ChangeMask::default();
        }

        self.last_time = time;
    }

    fn draw(&mut self, context: &mut Context) {
        let _time = (miniquad::date::now() - self.start_time) as f32;
        context.begin_default_pass(PassAction::Clear {
            color: Some((0.0, 0.0, 0.0, 1.0)),
            depth: None,
            stencil: None,
        });

        self.batch.begin_frame();
        self.batch.clear();
        let g = self.graphics.borrow();
        self.batch.set_image(self.white_texture);
        let screen_origin = self.document_to_screen(vec2(0.0, 0.0));
        self.batch
            .geometry
            .fill_circle_aa(screen_origin, 4.0, 4, [255, 255, 255, 255]);

        if self.doc.borrow().show_reference {
            if let Some(reference) = g.reference_texture {
                let reference_scale = self.doc.borrow().reference_scale;
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
        self.graphics.borrow().draw(
            &mut self.batch,
            &self.view,
            self.white_texture,
            self.finish_texture,
        );
        match self.tool {
            Tool::Graph => {
                let doc = self.doc.borrow();
                if let Some(Layer::Graph(graph)) = doc.layers.get(doc.active_layer) {
                    graph.draw_graph(&mut self.batch, self.last_mouse_pos, &self.view);
                }
            }
            Tool::Zone => {
                let doc = self.doc.borrow();
                AnyZone::draw_zones(
                    &mut self.batch,
                    &doc.markup,
                    &self.view,
                    doc.zone_selection,
                    self.last_mouse_pos,
                );
            }
            _ => {}
        }

        context.apply_pipeline(&self.pipeline);
        context.apply_uniforms(&ShaderUniforms {
            screen_size: self.window_size,
        });
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
    }

    fn resize_event(&mut self, _context: &mut Context, width: f32, height: f32) {
        self.window_size = [width, height];
        self.view.screen_width_px = width - 200.0;
        self.view.screen_height_px = height;
    }

    fn mouse_motion_event(&mut self, _c: &mut miniquad::Context, x: f32, y: f32) {
        self.last_mouse_pos = vec2(x, y);

        self.handle_event(UIEvent::MouseMove {
            pos: [x as i32, y as i32],
        });
    }

    fn mouse_wheel_event(&mut self, _c: &mut miniquad::Context, _dx: f32, dy: f32) {
        self.handle_event(UIEvent::MouseWheel {
            pos: [self.last_mouse_pos[0] as i32, self.last_mouse_pos[1] as i32],
            delta: dy,
        });
    }

    fn mouse_button_down_event(
        &mut self,
        _c: &mut miniquad::Context,
        button: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.handle_event(UIEvent::MouseDown {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
            time: miniquad::date::now(),
        });
    }

    fn mouse_button_up_event(
        &mut self,
        _c: &mut miniquad::Context,
        button: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.handle_event(UIEvent::MouseUp {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
        });
    }

    fn char_event(
        &mut self,
        _c: &mut miniquad::Context,
        character: char,
        keymods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        if !keymods.ctrl {
            self.handle_event(UIEvent::TextInput {
                text: character.to_string(),
            });
        }
    }

    fn key_down_event(
        &mut self,
        _c: &mut miniquad::Context,
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
            self.handle_event(event);
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: miniquad::KeyCode, _keymods: KeyMods) {
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

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let current_exe = std::env::current_exe().expect("missing exe path");
        let mut resources_path = current_exe
            .parent()
            .expect("cannot serve from the root")
            .to_path_buf();
        loop {
            let in_target = resources_path.ends_with("target");
            if !resources_path.pop() {
                panic!(
                    "cannot find target in the exe path {}",
                    current_exe.to_str().expect("unprintable chars in path")
                );
            }
            if in_target {
                resources_path.push("res");
                break;
            }
        }
        std::env::set_current_dir(&resources_path).expect("failed to set current directory");
    }

    #[cfg(not(target_arch = "wasm32"))]
    simple_logger::SimpleLogger::new()
        .with_module_level("ws", log::LevelFilter::Warn)
        .with_module_level("mio", log::LevelFilter::Warn)
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
