use std::collections::HashMap;

use anyhow::Result;
use glam::{vec2, Affine2, Vec2};
use serde_derive::{Deserialize, Serialize};

use cbmap::{BuiltinMaterial, MapMarkup, MaterialSlot, MaterialsJson};

use crate::app::App;
use crate::graph::Graph;
use crate::graphics::DocumentGraphics;
use crate::grid::Grid;
use crate::math::Rect;
use crate::tool::{Tool, ToolGroup, ToolGroupState, NUM_TOOL_GROUPS};
use crate::zone::ZoneRef;

#[derive(Serialize, Deserialize)]
pub enum Layer {
    Grid(Grid),
    Graph(Graph),
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
    pub fn new() -> Document {
        Document {
            reference_path: None,
            reference_scale: 2,
            show_reference: true,
            selection: Grid {
                bounds: Rect::zero(),
                cells: vec![],
            },
            layers: vec![Layer::Grid(Grid {
                bounds: Rect::zero(),
                cells: vec![],
            })],
            materials: vec![
                MaterialSlot::None,
                MaterialSlot::BuiltIn(BuiltinMaterial::Steel),
                MaterialSlot::BuiltIn(BuiltinMaterial::Ice),
                MaterialSlot::BuiltIn(BuiltinMaterial::Grass),
                MaterialSlot::BuiltIn(BuiltinMaterial::Mat),
                MaterialSlot::BuiltIn(BuiltinMaterial::Bumper),
                MaterialSlot::BuiltIn(BuiltinMaterial::Finish),
            ],
            side_load: HashMap::new(),
            markup: MapMarkup::new(),
            zone_selection: None,
            cell_size: 8,
            active_layer: 0,
        }
    }
    pub fn pre_save_cleanup(&mut self) {
        for layer in &mut self.layers {
            match *layer {
                Layer::Grid(ref mut layer) => {
                    let bounds = layer.find_used_bounds().inflate(1);
                    if bounds != layer.bounds {
                        layer.resize(bounds);
                    }
                }
                Layer::Graph { .. } => {}
            }
        }
    }

    pub(crate) fn save_materials(&self, g: &DocumentGraphics) -> Result<(Vec<u8>, Vec<u8>)> {
        let slots: Vec<MaterialSlot> = self.materials.clone();
        let materials_map = g.generated_grid.cells.clone();

        let bounds = g.generated_grid.bounds.to_array();
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

    pub(crate) fn set_active_layer(
        active_layer: &mut usize,
        tool: &mut Tool,
        tool_groups: &mut [ToolGroupState; NUM_TOOL_GROUPS],
        layer_index: usize,
        layer: &Layer,
    ) {
        let tool_group = ToolGroup::from_layer(layer);
        tool_groups[tool_group as usize].layer = Some(layer_index);
        *tool = tool_groups[tool_group as usize].tool;

        *active_layer = layer_index;
    }

    pub fn get_or_add_graph<'l>(
        layers: &'l mut Vec<Layer>,
        active_layer: &mut usize,
        tool: &mut Tool,
        tool_groups: &mut [ToolGroupState; 2],
    ) -> &'l mut Graph {
        let is_graph_selected = match layers.get_mut(*active_layer) {
            Some(Layer::Graph(_)) => true,
            _ => false,
        };

        if !is_graph_selected {
            let mut found = false;
            for i in 0..layers.len() {
                match layers.get_mut(i) {
                    Some(Layer::Graph(graph)) => {
                        Document::set_active_layer(active_layer, tool, tool_groups, i, &layers[i]);
                        found = true;
                        break;
                    }
                    _ => {}
                }
            }

            if !found {
                let new_layer = Layer::Graph(Graph::new());
                let new_layer_index = layers.len();
                Document::set_active_layer(
                    active_layer,
                    tool,
                    tool_groups,
                    new_layer_index,
                    &new_layer,
                );
                layers.push(new_layer);
            }
        }

        match layers.get_mut(*active_layer) {
            Some(Layer::Graph(graph)) => graph,
            _ => panic!("Unexpected"),
        }
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
        if self.undo_saved_position > self.undo.records.len() {
            // impossible to reach anymore
            self.undo_saved_position = usize::MAX;
        }
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
            Layer::Graph { .. } => "Graph",
        }
    }
}
