use std::collections::HashMap;

use anyhow::{Context, Result};
use cbmap::{BuiltinMaterial, MapMarkup, MaterialSlot, MaterialsJson};
use glam::{vec2, Affine2, Vec2};
use serde_derive::{Deserialize, Serialize};
use slotmap::new_key_type;

use crate::app::App;
use crate::field::Field;
use crate::graph::Graph;
use crate::graphics::DocumentGraphics;
use crate::grid::Grid;
use crate::math::Rect;
use crate::tool::{Tool, ToolGroup, ToolGroupState, NUM_TOOL_GROUPS};
use crate::zone::ZoneRef;
use slotmap::SlotMap;

new_key_type! {
    pub struct GridKey;
    pub struct FieldKey;
    pub struct GraphKey;
}

#[derive(Serialize, Deserialize)]
pub struct Layer {
    pub content: LayerContent,
    pub hidden: bool,
}

#[derive(Serialize, Deserialize)]
pub enum LayerContent {
    Grid(GridKey),
    Field(FieldKey),
    Graph(GraphKey),
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub materials: Vec<MaterialSlot>,
    pub cell_size: i32,

    pub layers: Vec<Layer>,

    pub active_layer: usize,

    pub selection: Grid<u8>,
    pub zone_selection: Option<ZoneRef>,

    #[serde(skip)]
    pub side_load: HashMap<String, Vec<u8>>,

    #[serde(skip_serializing_if = "MapMarkup::is_empty")]
    #[serde(default = "MapMarkup::new")]
    pub markup: MapMarkup,

    pub reference_path: Option<String>,
    pub reference_scale: i32,
    pub show_reference: bool,

    #[serde(default)]
    pub fields: SlotMap<FieldKey, Field>,
    pub grids: SlotMap<GridKey, Grid<u8>>,
    pub graphs: SlotMap<GraphKey, Graph>,
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
        let mut grids = SlotMap::with_key();
        let grid_key = grids.insert(Grid {
            default_value: 0,
            bounds: Rect::zero(),
            cells: vec![],
        });
        Document {
            reference_path: None,
            reference_scale: 2,
            show_reference: true,
            selection: Grid {
                default_value: 0,
                bounds: Rect::zero(),
                cells: vec![],
            },
            layers: vec![Layer {
                hidden: false,
                content: LayerContent::Grid(grid_key),
            }],
            materials: vec![
                MaterialSlot::None,
                MaterialSlot::BuiltIn(BuiltinMaterial::Concrete),
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
            graphs: SlotMap::with_key(),
            grids,
            fields: SlotMap::with_key(),
        }
    }
    pub fn pre_save_cleanup(&mut self) {
        for layer in self.grids.values_mut() {
            let bounds = layer.find_used_bounds().inflate(1);
            if bounds != layer.bounds {
                layer.resize(bounds);
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
        if width != 0 && height != 0 {
            let mut encoder = png::Encoder::new(&mut materials_png, width as u32, height as u32);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(
                slots
                    .iter()
                    .flat_map(|m| m.to_material().map(|m| m.fill_color).unwrap_or([0, 0, 0]))
                    .collect(),
            );
            let mut writer = encoder
                .write_header()
                .context("Writing materials image header")?;
            writer
                .write_image_data(&materials_map)
                .context("Writing materials image data")?;
        }

        let materials_json = serde_json::to_vec_pretty(&MaterialsJson { slots, map_rect })
            .context("Serializing materials")?;
        Ok((materials_png, materials_json))
    }

    pub(crate) fn set_active_layer(
        active_layer: &mut usize,
        tool: &mut Tool,
        tool_groups: &mut [ToolGroupState; NUM_TOOL_GROUPS],
        layer_index: usize,
        layer_content: &LayerContent,
    ) {
        let tool_group = ToolGroup::from_layer_content(layer_content);
        tool_groups[tool_group as usize].layer = Some(layer_index);
        *tool = tool_groups[tool_group as usize].tool;

        *active_layer = layer_index;
    }

    pub fn get_or_add_graph<'g>(
        layer_order: &mut Vec<Layer>,
        graphs: &'g mut SlotMap<GraphKey, Graph>,
        active_layer: &mut usize,
        tool: &mut Tool,
        tool_groups: &mut [ToolGroupState; 2],
    ) -> &'g mut Graph {
        let mut graph_key = match layer_order.get(*active_layer) {
            Some(Layer {
                content: LayerContent::Graph(key),
                ..
            }) => Some(*key),
            _ => None,
        };

        if graph_key.is_none() {
            for (i, layer) in layer_order.iter().enumerate() {
                match layer {
                    Layer {
                        content: LayerContent::Graph(key),
                        ..
                    } => {
                        Document::set_active_layer(
                            active_layer,
                            tool,
                            tool_groups,
                            i,
                            &LayerContent::Graph(*key),
                        );
                        graph_key = Some(*key);
                        break;
                    }
                    _ => {}
                }
            }
        }

        let graph_key = if let Some(graph_key) = graph_key {
            graph_key
        } else {
            let graph_key = graphs.insert(Graph::new());
            let new_layer = Layer {
                hidden: false,
                content: LayerContent::Graph(graph_key),
            };
            let new_layer_index = layer_order.len();
            Document::set_active_layer(
                active_layer,
                tool,
                tool_groups,
                new_layer_index,
                &new_layer.content,
            );
            layer_order.push(new_layer);
            graph_key
        };

        &mut graphs[graph_key]
    }
    pub(crate) fn layer_graph(layer_order: &Vec<Layer>, layer_index: usize) -> GraphKey {
        if let Some(layer) = layer_order.get(layer_index) {
            match layer.content {
                LayerContent::Graph(key) => return key,
                _ => {}
            }
        }
        GraphKey::default()
    }
    pub(crate) fn layer_grid(layer_order: &Vec<Layer>, layer_index: usize) -> GridKey {
        if let Some(layer) = layer_order.get(layer_index) {
            match layer.content {
                LayerContent::Grid(key) => return key,
                _ => {}
            }
        }
        GridKey::default()
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
    pub fn push_undo(&self, text: &str) {
        if *self.undo_saved_position.borrow() > self.undo.borrow().records.len() {
            // impossible to reach anymore
            self.undo_saved_position.replace(usize::MAX);
        }
        let err = self.undo.borrow_mut().push(&self.doc, text);
        self.redo.borrow_mut().clear();
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
        match self.content {
            LayerContent::Grid { .. } => "Grid",
            LayerContent::Graph { .. } => "Graph",
            LayerContent::Field { .. } => "Field,",
        }
    }
}
