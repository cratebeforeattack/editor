use std::sync::Arc;
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, 
    Pipeline, PipelineParams, Shader, ShaderMeta, Texture, UniformBlockLayout,
    UniformDesc, UniformType, VertexAttribute, VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use rimui::*;
use std::cell::RefCell;
use crate::document::{Document, DocumentGraphics};

pub(crate) struct App {
    pub start_time: f64,
    pub last_time: f32,
    pub batch: MiniquadBatch<VertexPos3UvColor>,
    pub pipeline: Pipeline,
    pub white_texture: Texture,
    pub font_manager: Arc<FontManager>,
    pub window_size: [f32; 2],
    pub last_mouse_pos: [f32; 2],
    pub text: String,
    pub ui: UI,

    pub operation: Option<(Box<dyn FnMut(&mut App)>, i32)>,
    pub doc: RefCell<Document>,
    pub graphics: RefCell<DocumentGraphics>,
}

impl App {
    pub fn new(context: &mut Context) -> Self {
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
        let pipeline = App::create_pipeline(context);

        let mut font_manager = FontManager::new(|name: &str| std::fs::read(name).map_err(|e| format!("{}", e)));
        let font_tiny = font_manager.load_font("fonts/BloggerSans.ttf-16.font");
        let font_normal = font_manager.load_font("fonts/BloggerSans.ttf-21.font");
        let _font_huge = font_manager.load_font("fonts/BloggerSans.ttf-64.font");
        font_manager.load_textures(context);

        let font_manager = Arc::new(font_manager);

        let mut ui = UI::new();
        ui.load_default_resources(|_sprite_name| 0, font_normal, font_tiny);

        let sprites = Arc::new(NoSprites{});

        ui.set_context(Some(font_manager.clone()), Some(sprites));
        let doc = Document{
            origin: [0, 0],
            size: [0, 0],
            cells: vec![],
            reference_path: None,
        };
        
        let graphics = DocumentGraphics{
            outline_points: vec![],
            reference_texture: None
        };

        App {
            text: "Edit".into(),
            start_time: miniquad::date::now(),
            last_time: 0.0,
            batch,
            pipeline,
            white_texture,
            ui,
            operation: None,
            doc: RefCell::new(doc),
            font_manager,
            last_mouse_pos: [0.0, 0.0],
            window_size: [1280.0, 720.0],
            graphics: RefCell::new(graphics)
        }
    }

    pub fn handle_event(&mut self, event: UIEvent)->bool {
        if let Some((mut action, start_button)) = self.operation.take() {
            action(self);
            if self.operation.is_none() {
                self.operation = Some((action, start_button));
            }
        }
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32)
    }

    pub fn ui(&mut self, _context: &mut Context, time: f32, dt: f32) {
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

struct NoSprites {}
impl SpriteContext for NoSprites {
    fn sprite_size(&self, _key: SpriteKey)->[u32; 2] { [1, 1] }
    fn sprite_uv(&self, _key: SpriteKey)->[f32; 4] { [0.0, 0.0, 1.0, 1.0] }
}


pub struct ShaderUniforms {
    pub screen_size: [f32; 2],
}

