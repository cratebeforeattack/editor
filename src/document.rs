use std::collections::HashMap;

use anyhow::Result;
use glam::{vec2, Affine2, IVec2, Vec2};
use log::info;
use serde_derive::{Deserialize, Serialize};

use cbmap::{MapMarkup, MaterialSlot, MaterialsJson};

use crate::app::App;
use crate::graphics::DocumentGraphics;
use crate::grid::Grid;
use crate::tunnel::Tunnel;
use crate::zone::{AnyZone, ZoneRef};

#[derive(Serialize, Deserialize)]
pub enum Layer {
    Grid(Grid),
    Tunnel(Tunnel),
}

fn show_reference_default() -> bool {
    true
}

fn reference_scale_default() -> i32 {
    2
}

fn cell_size_default() -> i32 {
    8
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "Vec::new")]
    pub materials: Vec<MaterialSlot>,

    #[serde(default = "cell_size_default")]
    pub cell_size: i32,

    #[serde(default = "Vec::new")]
    pub layers: Vec<Layer>,

    #[serde(default = "Default::default")]
    pub active_layer: usize,

    #[serde(default = "Grid::new")]
    pub selection: Grid,
    #[serde(default)]
    pub zone_selection: Option<ZoneRef>,

    #[serde(skip)]
    pub side_load: HashMap<String, Vec<u8>>,

    #[serde(skip_serializing_if = "MapMarkup::is_empty")]
    #[serde(default = "MapMarkup::new")]
    pub markup: MapMarkup,

    pub reference_path: Option<String>,
    #[serde(default = "reference_scale_default")]
    pub reference_scale: i32,
    #[serde(default = "show_reference_default")]
    pub show_reference: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct View {
    pub target: Vec2,
    #[serde(skip)]
    pub zoom: f32,
    pub zoom_target: f32,
    #[serde(skip)]
    pub zoom_velocity: f32,
    #[serde(skip)]
    pub screen_width_px: f32,
    #[serde(skip)]
    pub screen_height_px: f32,
}

#[derive(Serialize, Deserialize)]
pub struct DocumentLocalState {
    pub view: View,
    pub active_material: u8,
}

#[derive(Default, Copy, Clone, PartialEq)]
pub struct ChangeMask {
    pub cell_layers: u64,
    pub reference_path: bool,
}

impl Document {
    pub fn pre_save_cleanup(&mut self) {
        for layer in &mut self.layers {
            match *layer {
                Layer::Grid(ref mut layer) => {
                    let used_bounds = layer.find_used_bounds();
                    let new_bounds = [
                        used_bounds[0] - 1,
                        used_bounds[1] - 1,
                        used_bounds[2] + 1,
                        used_bounds[3] + 1,
                    ];
                    if new_bounds != layer.bounds {
                        layer.resize(new_bounds);
                    }
                }
                Layer::Tunnel { .. } => {}
            }
        }
    }

    pub(crate) fn save_materials(&self, g: &DocumentGraphics) -> Result<(Vec<u8>, Vec<u8>)> {
        let slots: Vec<MaterialSlot> = self.materials.clone();
        let materials_map = g.generated_grid.cells.clone();

        let bounds = g.generated_grid.bounds;
        let width = bounds[2] - bounds[0];
        let height = bounds[3] - bounds[1];

        let map_rect = [
            bounds[0] * self.cell_size,
            bounds[1] * self.cell_size,
            bounds[2] * self.cell_size,
            bounds[3] * self.cell_size,
        ];

        let mut materials_png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut materials_png, width as u32, height as u32);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(
                slots
                    .iter()
                    .flat_map(|m| m.to_material().map(|m| m.fill_color).unwrap_or([0, 0, 0]))
                    .collect(),
            );
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&materials_map)?;
        }

        let materials_json = serde_json::to_vec_pretty(&MaterialsJson { slots, map_rect })?;
        Ok((materials_png, materials_json))
    }
}

impl View {
    pub fn screen_to_world(&self) -> Affine2 {
        self.world_to_screen().inverse()
    }

    pub fn world_to_screen(&self) -> Affine2 {
        Affine2::from_translation(
            (vec2(self.screen_width_px, self.screen_height_px) * 0.5).floor() - vec2(0.5, 0.5),
        ) * Affine2::from_scale(Vec2::splat(self.zoom))
            * Affine2::from_translation(-self.target)
    }
}

impl App {
    pub fn push_undo(&mut self, text: &str) {
        let doc_ref = self.doc.borrow();
        let doc: &Document = &doc_ref;
        let err = self.undo.push(doc, text);
        self.redo.clear();
        self.report_error(err);
    }
}

impl ChangeMask {
    pub fn mark_dirty_layer(&mut self, layer: usize) {
        let bit_index = layer.min(63);
        let bit = 1 << bit_index;
        self.cell_layers |= bit;
    }
}

impl Layer {
    pub(crate) fn label(&self) -> &'static str {
        match *self {
            Layer::Grid { .. } => "Grid",
            Layer::Tunnel { .. } => "Tunnel",
        }
    }
}
