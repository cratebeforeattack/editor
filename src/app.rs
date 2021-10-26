use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::OsString;
use std::fs::{rename, write};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use glam::{vec2, Vec2};
use log::error;
use miniquad::{FilterMode, Pipeline, Texture, TextureFormat, TextureParams, TextureWrap};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use rimui::{FontManager, SpriteContext, SpriteKey, UIEvent, UI};
use serde_derive::{Deserialize, Serialize};
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

use cbmap::{BuiltinMaterial, MapJson, MapMarkup, MaterialSlot};

use crate::document::{
    ChangeMask, Document, DocumentLocalState, Layer, LayerContent, ObsoleteLayer, View,
};
use crate::graphics::{create_pipeline, DocumentGraphics};
use crate::grid::Grid;
use crate::math::Rect;
use crate::mouse_operation::MouseOperation;
use crate::tool::{Tool, ToolGroupState, NUM_TOOL_GROUPS};
use crate::undo_stack::UndoStack;
use zerocopy::AsBytes;

pub struct App {
    pub start_time: f64,
    pub last_time: f32,
    pub batch: MiniquadBatch<VertexPos3UvColor>,
    pub pipeline: Pipeline,
    pub white_texture: Texture,
    pub finish_texture: Texture,
    pub font_manager: Arc<FontManager>,
    pub window_size: [f32; 2],
    pub last_mouse_pos: Vec2,
    pub modifier_down: [bool; 3],
    pub ui: UI,

    pub tool: Tool,
    pub tool_groups: [ToolGroupState; NUM_TOOL_GROUPS],
    pub active_material: u8,
    pub operation: MouseOperation,
    pub operation_batch: MiniquadBatch<VertexPos3UvColor>,
    pub error_message: RefCell<Option<String>>,
    pub dirty_mask: ChangeMask,
    pub doc: RefCell<Document>,
    pub doc_path: Option<PathBuf>,
    pub undo: RefCell<UndoStack>,
    pub redo: RefCell<UndoStack>,
    pub undo_saved_position: RefCell<usize>,
    pub confirm_unsaved_changes: Option<Box<dyn FnMut(&mut App, &mut miniquad::Context)>>,
    pub graphics: RefCell<DocumentGraphics>,
    pub view: View,
}

pub const MODIFIER_CONTROL: usize = 0;
pub const MODIFIER_SHIFT: usize = 1;
pub const MODIFIER_ALT: usize = 2;

/// Persistent application state
#[derive(Serialize, Deserialize)]
struct AppState {
    doc_path: Option<PathBuf>,
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
        #[rustfmt::skip]
        let finish_pixels: [u32; 4 * 4] = [
            0xff736556, 0xff736556, 0xff000000, 0xff000000,
            0xff736556, 0xff736556, 0xff000000, 0xff000000,
            0xff000000, 0xff000000, 0xff736556, 0xff736556,
            0xff000000, 0xff000000, 0xff736556, 0xff736556,
        ];
        let finish_texture = Texture::from_data_and_format(
            context,
            &finish_pixels.as_bytes(),
            TextureParams {
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Repeat,
                filter: FilterMode::Nearest,
                width: 4,
                height: 4,
            },
        );

        let pipeline = create_pipeline(context);

        let mut font_manager =
            FontManager::new(|name: &str| std::fs::read(name).map_err(|e| format!("{}", e)));
        let font_tiny = font_manager.load_font("fonts/BloggerSans.ttf-16.font");
        let font_normal = font_manager.load_font("fonts/BloggerSans.ttf-21.font");
        let _font_huge = font_manager.load_font("fonts/BloggerSans.ttf-64.font");
        font_manager.load_textures(context);

        let font_manager = Arc::new(font_manager);

        let mut ui = UI::new();
        ui.load_default_resources(|_sprite_name| 0, font_normal, font_tiny);

        let sprites = Arc::new(NoSprites {});

        ui.set_context(Some(font_manager.clone()), Some(sprites));

        let graphics = DocumentGraphics {
            generated_grid: Grid {
                bounds: Rect::zero(),
                cells: vec![],
            },
            outline_points: Vec::new(),
            outline_fill_indices: Vec::new(),
            reference_texture: None,
            loose_indices: Vec::new(),
            loose_vertices: Vec::new(),
            resolved_materials: Vec::new(),
            materials: Vec::new(),
        };

        let app_state = App::load_app_state().ok().flatten();
        let (doc, local_state, doc_path) =
            if let Some(doc_path) = app_state.as_ref().map(|s| s.doc_path.as_ref()).flatten() {
                let doc = App::load_doc(doc_path)
                    .map_err(|e| {
                        error!("Failed to load last document: {}", e);
                    })
                    .ok();
                let got_doc = doc.is_some();
                (
                    doc,
                    if got_doc {
                        App::load_local_state(&doc_path).ok()
                    } else {
                        None
                    },
                    if got_doc {
                        Some(doc_path.to_owned())
                    } else {
                        None
                    },
                )
            } else {
                (None, None, None)
            };

        let doc = doc.unwrap_or_else(|| Document::new());

        let DocumentLocalState {
            view,
            active_material,
        } = local_state.unwrap_or_else(|| DocumentLocalState {
            view: View {
                target: Default::default(),
                zoom: 1.0,
                zoom_target: 1.0,
                zoom_velocity: 0.0,
                screen_width_px: context.screen_size().0 - 200.0,
                screen_height_px: context.screen_size().1,
            },
            active_material: 1,
        });

        let dirty_mask = ChangeMask {
            cell_layers: u64::MAX,
            reference_path: true,
        };

        App {
            start_time: miniquad::date::now(),
            last_time: 0.0,
            batch,
            pipeline,
            white_texture,
            finish_texture,
            ui,
            tool: Tool::Pan,
            tool_groups: [
                ToolGroupState {
                    tool: Tool::Paint,
                    layer: Some(0),
                },
                ToolGroupState {
                    tool: Tool::Graph,
                    layer: None,
                },
            ],
            active_material,
            operation: MouseOperation::new(),
            operation_batch: MiniquadBatch::new(),
            error_message: RefCell::new(None),
            doc: RefCell::new(doc),
            dirty_mask,
            undo: RefCell::new(UndoStack::new()),
            redo: RefCell::new(UndoStack::new()),
            undo_saved_position: RefCell::new(0),
            font_manager,
            last_mouse_pos: vec2(0.0, 0.0),
            window_size: [context.screen_size().0, context.screen_size().1],
            graphics: RefCell::new(graphics),
            view,
            doc_path,
            modifier_down: [false; 3],
            confirm_unsaved_changes: None,
        }
    }

    pub(crate) fn load_doc(path: &Path) -> Result<Document> {
        let extension = path
            .extension()
            .map(|s| s.to_string_lossy().to_string().to_lowercase())
            .unwrap_or(String::new());

        let archive_content = std::fs::read(path).context("Reading document file")?;

        let mut side_load = HashMap::new();
        let content = if extension == "cbmap" || extension == "zip" {
            let mut zip =
                ZipArchive::new(Cursor::new(&archive_content)).context("Opening ZIP archive")?;
            let mut source_content = None;
            let num_files = zip.len();
            for index in 0..num_files {
                let mut subfile = zip
                    .by_index(index)
                    .with_context(|| format!("locating zip entry {}", index))?;

                let mut subfile_content = Vec::new();
                subfile
                    .read_to_end(&mut subfile_content)
                    .with_context(|| format!("extracting {}", subfile.name()))?;

                let name_lowercase = subfile.name().to_ascii_lowercase();
                match name_lowercase.as_str() {
                    "materials.json" | "materials.png" | "map.json" => {}
                    "source.json" => {
                        source_content = Some(subfile_content);
                    }
                    "main.png" => {
                        // will be rendered on save
                    }
                    _ => {
                        side_load.insert(subfile.name().to_owned(), subfile_content);
                    }
                }
            }

            source_content.ok_or_else(|| {
                anyhow!("This CBMAP is not made with the editor: missing source.json.")
            })?
        } else {
            archive_content
        };

        let mut document: Document =
            serde_json::from_slice(&content).context("Deserializing document")?;

        if document.materials.len() == 0 {
            document.materials.extend(
                [
                    MaterialSlot::None,
                    MaterialSlot::BuiltIn(BuiltinMaterial::Steel),
                    MaterialSlot::BuiltIn(BuiltinMaterial::Ice),
                    MaterialSlot::BuiltIn(BuiltinMaterial::Grass),
                    MaterialSlot::BuiltIn(BuiltinMaterial::Mat),
                    MaterialSlot::BuiltIn(BuiltinMaterial::Bumper),
                    MaterialSlot::BuiltIn(BuiltinMaterial::Finish),
                ]
                .iter()
                .cloned(),
            );
        }
        // convert layers from old to new format
        for layer in document.layers.drain(..) {
            match layer {
                ObsoleteLayer::Graph(graph) => {
                    let key = document.graphs.insert(graph);
                    document.layer_order.push(Layer {
                        content: LayerContent::Graph(key),
                        hidden: false,
                    })
                }
                ObsoleteLayer::Grid(grid) => {
                    let key = document.grids.insert(grid);
                    document.layer_order.push(Layer {
                        content: LayerContent::Grid(key),
                        hidden: false,
                    })
                }
            }
        }
        document.side_load = side_load;

        Ok(document)
    }

    fn load_local_state(path: &Path) -> Result<DocumentLocalState> {
        let content = std::fs::read(path).context("Reading local state file")?;
        let document = serde_json::from_slice(&content).context("Deserializing document")?;
        Ok(document)
    }

    pub(crate) fn save_doc(
        path: &Path,
        doc: &Document,
        graphics: &DocumentGraphics,
        white_pixel: Texture,
        finish_texture: Texture,
        pipeline: Pipeline,
        view: &View,
        context: &mut miniquad::Context,
        active_material: u8,
    ) -> Result<()> {
        let mut path = PathBuf::from(path);
        if path
            .extension()
            .map(|s| s.to_str() == Some("json"))
            .unwrap_or(false)
        {
            path.set_extension(OsString::try_from("cbmap")?);
        }
        let serialized = serde_json::to_vec_pretty(doc).context("Serializing document")?;

        let mut zip_bytes = Vec::new();
        let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
        zip.start_file("source.json", FileOptions::default())?;
        zip.write(&serialized)?;

        let (image, image_bounds) =
            graphics.render_map_image(doc, white_pixel, finish_texture, pipeline, context);

        if !doc.markup.is_empty() {
            let mut translated_markup = doc.markup.clone();
            // adjust all markup to match image coordinates
            translated_markup.translate([-image_bounds[0], -image_bounds[1]]);

            let map_json = serde_json::to_vec_pretty(&MapJson {
                markup: Some(translated_markup),
                ..MapJson::default()
            })
            .context("Serializing map.json")?;
            zip.start_file("map.json", FileOptions::default())?;
            zip.write_all(&map_json).context("Writing map.json")?;
        }

        let mut png_bytes = Vec::new();
        {
            let width = image_bounds[2] - image_bounds[0];
            let height = image_bounds[3] - image_bounds[1];
            let mut encoder = png::Encoder::new(&mut png_bytes, width as u32, height as u32); // Width is 2 pixels and height is 1.
            encoder.set_color(png::ColorType::RGBA);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&image)?;
        }
        zip.start_file("main.png", FileOptions::default())?;
        zip.write_all(&png_bytes)?;

        let (material_png, material_json): (Vec<u8>, Vec<u8>) = doc
            .save_materials(graphics)
            .context("Serializing materials.")?;

        zip.start_file("materials.json", FileOptions::default())?;
        zip.write_all(&material_json)?;

        zip.start_file("materials.png", FileOptions::default())?;
        zip.write_all(&material_png)?;

        for (name, content) in &doc.side_load {
            zip.start_file(name, FileOptions::default())?;
            zip.write(&content)
                .with_context(|| format!("writing {}", name))?;
        }
        zip.finish().context("Finishing zip archive.")?;
        drop(zip);

        let temp_path = PathBuf::from(&path).with_extension(OsString::try_from(".tmp")?);
        write(&temp_path, &zip_bytes)
            .with_context(|| format!("Saving {}", temp_path.to_string_lossy()))?;
        rename(&temp_path, &path).with_context(|| {
            format!(
                "Renaming {} to {}",
                temp_path.to_string_lossy(),
                path.to_string_lossy()
            )
        })?;

        let mut sidecar_path = PathBuf::from(path);
        let mut extension = sidecar_path
            .extension()
            .map(|e| e.to_owned())
            .unwrap_or(OsString::new());
        extension.push(OsString::try_from(".state")?);
        sidecar_path.set_extension(extension);

        let local_state = DocumentLocalState {
            view: view.clone(),
            active_material,
        };
        let state_serialized =
            serde_json::to_vec_pretty(&local_state).context("Serializing local state")?;
        write(sidecar_path, state_serialized).context("Writing local state")?;
        Ok(())
    }

    fn app_state_path() -> PathBuf {
        let dirs = directories::ProjectDirs::from("com", "koalefant", "Shopper Editor")
            .expect("No home directory");
        dirs.data_local_dir().to_path_buf()
    }

    pub(crate) fn save_app_state(&mut self) -> Result<()> {
        let app_state = AppState {
            doc_path: self.doc_path.clone(),
        };

        let serialized = serde_json::to_vec_pretty(&app_state).context("Serializing app state")?;
        let app_state_path = App::app_state_path();
        std::fs::create_dir_all(
            &app_state_path
                .parent()
                .ok_or_else(|| anyhow!("Failed to obtain app state path"))?,
        )
        .context("Creating app state directory")?;
        write(&app_state_path, &serialized).context("Saving app state file")?;
        Ok(())
    }

    fn load_app_state() -> Result<Option<AppState>> {
        let state_path = App::app_state_path();
        if !std::fs::metadata(&state_path)
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            return Ok(None);
        }
        let app_state: AppState =
            serde_json::from_slice(&std::fs::read(state_path).context("Reading state file")?)
                .context("Deserializing app state")?;
        Ok(Some(app_state))
    }

    pub(crate) fn report_error<T>(&self, result: Result<T>) -> Option<T> {
        result
            .map_err(|e| {
                self.error_message.replace(Some(format!("Error: {:#}", e)));
                error!("Error: {}", e);
            })
            .ok()
    }
}

struct NoSprites {}
impl SpriteContext for NoSprites {
    fn sprite_size(&self, _key: SpriteKey) -> [u32; 2] {
        [1, 1]
    }
    fn sprite_uv(&self, _key: SpriteKey) -> [f32; 4] {
        [0.0, 0.0, 1.0, 1.0]
    }
}

pub struct ShaderUniforms {
    pub screen_size: [f32; 2],
}
