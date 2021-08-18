use std::sync::Arc;
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, 
    Pipeline, PipelineParams, Shader, ShaderMeta, Texture, UniformBlockLayout,
    UniformDesc, UniformType, VertexAttribute, VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use rimui::*;
use std::cell::RefCell;
use crate::document::{Document, DocumentGraphics, Grid, ChangeMask};
use glam::Vec2;
use crate::interaction::operation_pan;

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

    pub operation: Option<(Box<dyn FnMut(&mut App, &UIEvent)>, i32)>,
    pub doc: RefCell<Document>,
    pub graphics: RefCell<DocumentGraphics>,
    pub view: View,
}

pub (crate) struct View {
    pub target: Vec2,
    pub zoom: f32,
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
            layer: Grid {
                origin: [0, 0],
                size: [0, 0],
                cells: vec![],
                cell_size: 4,
            },
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
            graphics: RefCell::new(graphics),
            view: View {
                target: Default::default(),
                zoom: 1.0,
            }
        }
    }

    pub fn handle_event(&mut self, event: UIEvent)->bool {
        // handle current mouse operation
        if let Some((mut action, start_button)) = self.operation.take() {
            action(self, &event);
            let released = match event {
                UIEvent::MouseUp { button, .. } => button == start_button,
                _ => false,
            };
            if self.operation.is_none() && !released {
                self.operation = Some((action, start_button));
            }
            return true;
        }

        // provide event to UI
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        if self.ui.handle_event(&event, render_rect, miniquad::date::now() as f32) {
            return true;
        }

        // start new operations
        match event {
            UIEvent::MouseDown { button, .. } => {
                let op = operation_pan(self);
                self.operation = Some((Box::new(op), button))
            }
            _ => {}
        }
        false
    }

    pub fn ui(&mut self, context: &mut Context, time: f32, dt: f32) {
        let window = self.ui.window("Test", WindowPlacement::Absolute{
            pos: [self.window_size[0] as i32 - 4, 4],
            size: [0, self.window_size[1] as i32 - 8],
            expand: EXPAND_LEFT,
        }, 0, 0);


        let frame = self.ui.add(window, Frame::default());
        let rows = self.ui.add(frame, vbox().padding(2).min_size([200, 0]));
        self.ui.add(rows, label("Layers"));
        self.ui.add(rows, button("Grid").down(true).align(Some(Align::Left)));



        self.ui.add(rows, label("Reference"));

        let mut doc = self.doc.borrow_mut();
        let mut buffer = String::new();
        let reference_text = doc
            .reference_path
            .as_ref()
            .map(|s| {
                if let Some((path, name)) = s.rsplit_once('/') {
                    buffer = format!(".../{}", name);
                    &buffer
                } else {
                    s.as_str()
                }
            })
            .unwrap_or("Load...");

        if self.ui.add(rows, button(reference_text)).clicked {

            let old_reference_path = doc.reference_path.clone().unwrap_or(String::new());

            use dialog::DialogBox;

            let mut file_selection = dialog::FileSelection::new("Please select a file");
            file_selection.title("File Selection");
            if !old_reference_path.is_empty() {
                let old_path = std::path::Path::new(&old_reference_path);
                if let Some(parent) = old_path.parent() {
                    if let Some(parent_str) = parent.to_str() {
                        file_selection.path(parent_str);
                    }
                }
            }

            let new_reference_path = file_selection
                .show()
                .expect("Could not display dialog box");

            if let Some(new_reference_path) = new_reference_path {
                doc.reference_path = Some(new_reference_path);
                self.graphics.borrow_mut().generate(&doc, ChangeMask{
                    reference_path: true,
                    ..Default::default()
                }, context);
            }
        }
        if let Some(path) = &doc.reference_path {
            last_tooltip(&mut self.ui, rows, path);
        }

        drop(doc);

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

fn last_tooltip(ui: &mut UI, parent: AreaRef, tooltip_text: &str) {
    if let Some(t) = ui.last_tooltip(parent, Tooltip {
        placement: TooltipPlacement::Beside,
        ..Tooltip::default()
    }) {
        let frame = ui.add(t, Frame::default());
        let rows = ui.add(frame, vbox());
        ui.add(rows, label(tooltip_text));
    }
}

