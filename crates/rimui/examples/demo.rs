use core::default::Default;
use miniquad::{
    conf, BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, EventHandler,
    PassAction, Pipeline, PipelineParams, Shader, ShaderMeta, Texture, UniformBlockLayout,
    UniformDesc, UniformType, UserData, VertexAttribute, VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use rimui::*;
use std::sync::Arc;

struct Example {
    start_time: f64,
    last_time: f32,
    batch: MiniquadBatch<VertexPos3UvColor>,
    pipeline: Pipeline,
    white_texture: Texture,
    font_manager: Arc<FontManager>,
    window_size: [f32; 2],
    last_mouse_pos: [f32; 2],
    text: String,
    ui: UI,
}

impl EventHandler for Example {
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

    fn update(&mut self, _context: &mut Context) {
        let time = (miniquad::date::now() - self.start_time) as f32;
        let dt = time - self.last_time;

        let window = self.ui.window("Test", WindowPlacement::Center{
            offset: [0, 0],
            size: [0, 0],
            expand: EXPAND_ALL,
        }, 0, 0);

        let frame = self.ui.add(window, Frame::default());
        let rows = self.ui.add(frame, vbox().padding(2));

        self.ui.add(rows, Label::new("Label"));
        if self.ui.add(rows, Button::new("First Button")).clicked {
        }
        if let Some(t) = self.ui.last_tooltip(rows, Tooltip{
            placement: TooltipPlacement::Beside ,
            ..Tooltip::default()
        }) {
            let frame = self.ui.add(t, Frame::default());
            let rows = self.ui.add(frame, vbox());
            self.ui.add(rows, label("How about this button"));
        }
        if self.ui.add(rows, Button::new("Second Button")).clicked {
        }
        self.ui.add(rows, progress()
            .progress(((time / 10.0).fract() * 2.0 - 1.0).abs())
            .min_size([8, 8]));
        self.ui.add(rows, edit("text", &mut self.text));

        self.ui.layout_ui(dt, [0, 0, self.window_size[0] as i32, self.window_size[1] as i32], None);

        self.last_time = time;
    }


    fn resize_event(&mut self, _context: &mut Context, width: f32, height: f32) {
        self.window_size = [width, height];
    }

    fn mouse_motion_event(&mut self, _c: &mut miniquad::Context, x: f32, y: f32) {
        let event = UIEvent::MouseMove {
            pos: [x as i32, y as i32],
        };
        self.last_mouse_pos = [x, y];

        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {
            return;
        }
    }

    fn mouse_wheel_event(&mut self, _c: &mut miniquad::Context, _dx: f32, dy: f32) {
        let event = UIEvent::MouseWheel {
            pos: [self.last_mouse_pos[0] as i32, self.last_mouse_pos[1] as i32],
            delta: dy,
        };
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {
            return;
        }
    }

    fn mouse_button_up_event(&mut self, _c: &mut miniquad::Context, button: miniquad::MouseButton, x: f32, y: f32) {
        let event = UIEvent::MouseUp {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
        };
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32);
    }

    fn mouse_button_down_event(&mut self, _c: &mut miniquad::Context, button: miniquad::MouseButton, x: f32, y: f32) {
        let event = UIEvent::MouseDown {
            pos: [x as i32, y as i32],
            button: ui_mouse_button(button),
            time: miniquad::date::now(),
        };
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {
            return;
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

    fn char_event(&mut self, _c: &mut miniquad::Context, character: char, keymods: miniquad::KeyMods, _repeat: bool) {
        if !keymods.ctrl {
            let event = UIEvent::TextInput {
                text: character.to_string(),
            };
            let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
            if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {
                return;
            }
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

struct ExampleSprites {}
impl SpriteContext for ExampleSprites {
    fn sprite_size(&self, _key: SpriteKey)->[u32; 2] { [1, 1] }
    fn sprite_uv(&self, _key: SpriteKey)->[f32; 4] { [0.0, 0.0, 1.0, 1.0] }
}

impl Example {
    pub fn new(context: &mut Context) -> Example {
        let batch = MiniquadBatch::new();

        let white_texture = Texture::from_rgba8(
            context,
            4,
            4,
            &[
                // white RGBA-image 4x4 pixels
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            ],
        );
        let pipeline = Example::create_pipeline(context);

        let mut font_manager = FontManager::new(|name: &str| std::fs::read(name).map_err(|e| format!("{}", e)));
        let font_tiny = font_manager.load_font("fonts/BloggerSans.ttf-16.font");
        let font_normal = font_manager.load_font("fonts/BloggerSans.ttf-21.font");
        let _font_huge = font_manager.load_font("fonts/BloggerSans.ttf-64.font");
        font_manager.load_textures(context);

        let font_manager = Arc::new(font_manager);

        let mut ui = UI::new();
        ui.load_default_resources(|_sprite_name| 0, font_normal, font_tiny);

        let sprites = Arc::new(ExampleSprites{});

        ui.set_context(Some(font_manager.clone()), Some(sprites));

        Example {
            text: "Edit".into(),
            start_time: miniquad::date::now(),
            last_time: 0.0,
            batch,
            pipeline,
            white_texture,
            ui,
            font_manager,
            last_mouse_pos: [0.0, 0.0],
            window_size: [1280.0, 720.0],
        }
    }

    fn create_pipeline(ctx: &mut Context) -> Pipeline {
        let vertex_shader = r#"#version 100
            attribute vec2 pos;
            attribute vec2 uv;
            attribute vec4 color;
            uniform vec2 ;
            uniform vec2 screen_size;
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            void main() {
                gl_Position = vec4((pos / screen_size * 2.0 - 1.0) * vec2(1.0, -1.0), 0, 1);
                v_uv = uv;
                v_color = color / 255.0;
            }"#;
        let fragment_shader = r#"#version 100
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            uniform sampler2D tex;
            void main() {
                gl_FragColor = v_color * texture2D(tex, v_uv);
            }"#;
        let shader = Shader::new(
            ctx,
            vertex_shader,
            fragment_shader,
            ShaderMeta {
                images: vec!["tex".to_owned()],
                uniforms: UniformBlockLayout {
                    // describes struct ShaderUniforms
                    uniforms: vec![UniformDesc::new("screen_size", UniformType::Float2)],
                },
            },
        )
        .unwrap();

        let pipeline = Pipeline::with_params(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float3),
                VertexAttribute::new("uv", VertexFormat::Float2),
                VertexAttribute::new("color", VertexFormat::Byte4),
            ],
            shader,
            PipelineParams {
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );
        pipeline
    }
}

pub struct ShaderUniforms {
    pub screen_size: [f32; 2],
}


fn main() {
    miniquad::start(
        conf::Conf {
            sample_count: 0,
            window_width: 1280,
            window_height: 720,
            ..Default::default()
        },
        |mut context| UserData::owning(Example::new(&mut context), context),
    );
}
