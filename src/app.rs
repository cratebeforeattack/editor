use std::sync::Arc;
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Equation,
    Pipeline, PipelineParams, Shader, ShaderMeta, Texture, UniformBlockLayout,
    UniformDesc, UniformType, VertexAttribute, VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use std::cell::RefCell;
use crate::document::{Document, DocumentGraphics, Grid, ChangeMask, View, DocumentLocalState};
use glam::Vec2;
use anyhow::{anyhow, Result, Context};
use serde_derive::{Serialize, Deserialize};
use crate::interaction::{operation_pan, operation_stroke};
use crate::graphics::create_pipeline;
use crate::tool::Tool;
use std::path::{PathBuf, Path};
use log::error;
use rimui::{FontManager, UIEvent, UI, SpriteContext, SpriteKey};
use std::ffi::{OsStr, OsString};
use std::convert::{TryFrom, TryInto};

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

    pub tool: Tool,
    pub operation: Option<(Box<dyn FnMut(&mut App, &UIEvent)>, i32)>,
    pub error_message: RefCell<Option<String>>,
    pub doc: RefCell<Document>,
    pub doc_path: Option<PathBuf>,
    pub graphics: RefCell<DocumentGraphics>,
    pub view: View,
}

/// Persistent application state
#[derive(Serialize, Deserialize)]
struct AppState {
    doc_path: Option<PathBuf>
}

impl App {
    pub fn new(context: &mut miniquad::Context) -> Self {
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
        let pipeline = create_pipeline(context);

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

        let graphics = DocumentGraphics{
            outline_points: vec![],
            reference_texture: None
        };

        let app_state = App::load_app_state().ok().flatten();
        let (
            doc,
            local_state,
            doc_path
        ) = if let Some(doc_path) = app_state.as_ref().map(|s| s.doc_path.as_ref()).flatten() {
            let doc = App::load_doc(doc_path)
                .map_err(|e| {
                    error!("Failed to load last document: {}", e);
                }).ok();
            let got_doc = doc.is_some();
            (
                doc,
                if got_doc { App::load_local_state(&doc_path).ok() } else { None },
                if got_doc { Some(doc_path.to_owned()) } else { None }
            )
        } else {
            (
                None,
                None,
                None
            )
        };

        let doc = doc.unwrap_or_else(|| {
            // default document
            Document {
                layer: Grid {
                    origin: [0, 0],
                    size: [0, 0],
                    cells: vec![],
                    cell_size: 4,
                },
                reference_path: None,
            }
        });

        let local_state = local_state.unwrap_or_else(|| {
            DocumentLocalState {
                view: View {
                    target: Default::default(),
                    zoom: 1.0,
                }
            }
        });




        App {
            text: "Edit".into(),
            start_time: miniquad::date::now(),
            last_time: 0.0,
            batch,
            pipeline,
            white_texture,
            ui,
            tool: Tool::Pan,
            operation: None,
            error_message: RefCell::new(None),
            doc: RefCell::new(doc),
            font_manager,
            last_mouse_pos: [0.0, 0.0],
            window_size: [1280.0, 720.0],
            graphics: RefCell::new(graphics),
            view: local_state.view,
            doc_path,
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
        match self.tool {
            Tool::Pan => {
                match event {
                    UIEvent::MouseDown {
                        button, ..
                    } => {
                        let op = operation_pan(self);
                        self.operation = Some((Box::new(op), button))
                    }
                    _ => {}
                }
            }
            Tool::Paint => {
                match event {
                    UIEvent::MouseDown {
                        button, ..
                    } => {
                        let op = operation_stroke(self);
                        self.operation = Some((Box::new(op), button))
                    }
                    _ => {}
                }
            }
        }
        false
    }



    pub(crate) fn load_doc(path: &Path) ->Result<Document> {
        let content = std::fs::read(path).context("Reading document file")?;
        let document = serde_json::from_slice(&content).context("Deserializing document")?;
        Ok(document)
    }

    fn load_local_state(path: &Path)->Result<DocumentLocalState> {
        let content = std::fs::read(path).context("Reading local state file")?;
        let document = serde_json::from_slice(&content).context("Deserializing document")?;
        Ok(document)
    }

    pub(crate) fn save_doc(path: &Path, doc: &Document, view: &View)->Result<()> {
        let serialized = serde_json::to_vec_pretty(doc).context("Serializing document")?;
        std::fs::write(&path, serialized).context("Saving file")?;

        let mut sidecar_path = PathBuf::from(path);
        let mut extension = sidecar_path.extension().map(|e| e.to_owned()).unwrap_or(OsString::new());
        extension.push(OsString::try_from(".state").unwrap());
        sidecar_path.set_extension(extension);

        let local_state = DocumentLocalState {
            view: view.clone(),
        };
        let state_serialized = serde_json::to_vec_pretty(&local_state).context("Serializing local state")?;
        std::fs::write(sidecar_path, state_serialized).context("Writing local state")?;
        Ok(())
    }

    fn app_state_path()->PathBuf {
        let dirs = directories::ProjectDirs::from("com", "koalefant", "Shopper Editor")
            .expect("No home directory");
        dirs.data_local_dir().to_path_buf()
    }

    pub(crate) fn save_app_state(&mut self)->Result<()> {
        let app_state = AppState {
          doc_path: self.doc_path.clone()
        };

        let serialized = serde_json::to_vec_pretty(&app_state).context("Serializing app state")?;
        std::fs::write(App::app_state_path(), &serialized).context("Saving app state file")?;
        Ok(())
    }

    fn load_app_state()->Result<Option<AppState>> {
        let state_path = App::app_state_path();
        if !std::fs::metadata(&state_path).map(|m| m.is_file()).unwrap_or(false) {
            return Ok(None);
        }
        let app_state: AppState = serde_json::from_slice(&std::fs::read(state_path).context("Reading state file")?).context("Deserializing app state")?;
        Ok(Some(app_state))
    }


    pub (crate) fn report_error<T>(&self, result: Result<T>) -> Option<T> {
        result.map_err(|e| {
            self.error_message.replace(Some(format!("Error: {:#}", e)));
            error!("Error: {}", e);
        }).ok()
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

