mod app;
mod document;


use core::default::Default;
use miniquad::{
    conf, Context, EventHandler, PassAction, UserData, 
};
use rimui::*;
use app::*;
use document::*;

impl EventHandler for App {
    fn update(&mut self, context: &mut Context) {
        let time = (miniquad::date::now() - self.start_time) as f32;
        let dt = time - self.last_time;

        self.ui(context, time, dt);

        self.last_time = time;
    }

    fn draw(&mut self, context: &mut Context) {
        let _time = (miniquad::date::now() - self.start_time) as f32;
        context.begin_default_pass(PassAction::Clear {
            color: Some((0.2, 0.2, 0.2, 1.0)),
            depth: None,
            stencil: None,
        });

        self.batch.begin_frame();
        self.batch.clear();
        self.batch.set_image(self.white_texture);

        let white_texture = self.white_texture.clone();
        let mut render = MiniquadRender::new(&mut self.batch, &self.font_manager, |_sprite_key| {
            white_texture.clone()
        });
        self.ui.render_ui(&mut render, None);

        context.apply_pipeline(&self.pipeline);
        context.apply_uniforms(&ShaderUniforms {
            screen_size: self.window_size,
        });
        self.batch.flush(context);

        context.end_render_pass();

        context.commit_frame();
    }


    fn resize_event(&mut self, _context: &mut Context, width: f32, height: f32) {
        self.window_size = [width, height];
    }

    fn mouse_motion_event(&mut self, _c: &mut miniquad::Context, x: f32, y: f32) {
        self.last_mouse_pos = [x, y];

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

    fn mouse_button_down_event(&mut self, _c: &mut miniquad::Context, button: miniquad::MouseButton, x: f32, y: f32) {
        self.handle_event(UIEvent::MouseDown {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
            time: miniquad::date::now(),
        });
    }

    fn mouse_button_up_event(&mut self, _c: &mut miniquad::Context, button: miniquad::MouseButton, x: f32, y: f32) {
        self.handle_event(UIEvent::MouseUp {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
        });
    }

    fn char_event(&mut self, _c: &mut miniquad::Context, character: char, keymods: miniquad::KeyMods, _repeat: bool) {
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
        repeat: bool,
    ) {
        if self.ui.consumes_key_down() || keycode == miniquad::KeyCode::PageDown || keycode == miniquad::KeyCode::PageUp {
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
                _ => None,
            };

            if let Some(ui_keycode) = ui_keycode {
                let event = UIEvent::KeyDown {
                    key: ui_keycode,
                    control: keymods.ctrl,
                    shift: keymods.shift,
                    alt: keymods.alt,
                };
                let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
                if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {

                }
            }
            return;
        }

        if repeat {
            return;
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
        let mut resources_path = current_exe.parent().expect("cannot serve from the root").to_path_buf();
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

    miniquad::start(
        conf::Conf {
            sample_count: 0,
            window_width: 1280,
            window_height: 720,
            ..Default::default()
        },
        |mut context| UserData::owning(App::new(&mut context), context),
    );
}
