use std::collections::HashMap;

use anyhow::{Context, Result};
use cbmap::{BuiltinMaterial, MapMarkup, MaterialSlot, MaterialsJson};
use glam::{vec2, Affine2, Vec2};
use ordered_float::NotNan;
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use serde_derive::{Deserialize, Serialize};
use slotmap::{new_key_type, Key};

use crate::app::App;
use crate::graph::{GraphEdge, GraphEdgeKey, GraphNode, GraphNodeKey};
use crate::graphics::DocumentGraphics;
use crate::grid::Grid;
use crate::math::{closest_point_on_segment, Rect};
use crate::plant::{Plant, PlantKey};
use crate::sdf::sd_segment;
use crate::some_or::some_or;
use crate::zone::ZoneRef;
use slotmap::SlotMap;

new_key_type! {
    pub struct GridKey;
    pub struct FieldKey;
    pub struct LayerKey;
}

#[derive(Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub grid: GridKey,
    pub hidden: bool,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Ord, PartialOrd, Eq)]
pub enum GraphRef {
    Node(GraphNodeKey),
    NodeRadius(GraphNodeKey),
    Edge(GraphEdgeKey),
    EdgePoint(GraphEdgeKey, NotNan<f32>),
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub materials: Vec<MaterialSlot>,
    pub cell_size: i32,

    #[serde(default, rename = "layer_map")]
    pub layers: SlotMap<LayerKey, Layer>,
    #[serde(default)]
    pub layer_order: Vec<LayerKey>,

    #[serde(default)]
    pub current_layer: LayerKey,

    pub selection: Grid<u8>,
    pub zone_selection: Option<ZoneRef>,

    #[serde(skip)]
    pub side_load: HashMap<String, Vec<u8>>,

    #[serde(
        default = "MapMarkup::new",
        skip_serializing_if = "MapMarkup::is_empty"
    )]
    pub markup: MapMarkup,

    pub reference_path: Option<String>,
    pub reference_scale: i32,
    pub show_reference: bool,

    #[serde(default)]
    pub grids: SlotMap<GridKey, Grid<u8>>,

    #[serde(default)]
    pub selected: Vec<GraphRef>,
    #[serde(default)]
    pub nodes: SlotMap<GraphNodeKey, GraphNode>,
    #[serde(default)]
    pub edges: SlotMap<GraphEdgeKey, GraphEdge>,
    #[serde(default)]
    pub plants: SlotMap<PlantKey, Plant>,
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
        let grids = SlotMap::with_key();
        let mut layers = SlotMap::with_key();
        let current_layer = layers.insert(Layer {
            hidden: false,
            grid: GridKey::default(),
        });
        let layer_order = vec![current_layer];
        Document {
            reference_path: None,
            reference_scale: 2,
            show_reference: true,
            selection: Grid {
                default_value: 0,
                bounds: Rect::zero(),
                cells: vec![],
            },
            selected: vec![],
            layer_order,
            layers,
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
            current_layer,
            grids,
            edges: SlotMap::with_key(),
            nodes: SlotMap::with_key(),
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

    pub(crate) fn get_or_add_layer_grid(
        layers: &mut SlotMap<LayerKey, Layer>,
        layer_key: LayerKey,
        grids: &mut SlotMap<GridKey, Grid<u8>>,
    ) -> GridKey {
        let grid_key = layers
            .get(layer_key)
            .map(|layer| layer.grid)
            .unwrap_or(GridKey::default());
        if grids.contains_key(grid_key) {
            grid_key
        } else {
            if let Some(layer) = layers.get_mut(layer_key) {
                let grid_key = grids.insert(Grid::new(0));
                layer.grid = grid_key;
                grid_key
            } else {
                GridKey::default()
            }
        }
    }

    pub fn hit_test(&self, screen_pos: Vec2, view: &View) -> Option<GraphRef> {
        let world_to_screen = view.world_to_screen();
        let mut result = None;
        let mut best_distance = f32::MAX;
        let mut outside_distance = f32::MAX;
        for (key, node) in &self.nodes {
            let node_screen_pos = world_to_screen.transform_point2(node.pos.as_vec2());

            let screen_radius = world_to_screen
                .transform_vector2(vec2(node.radius as f32, 0.0))
                .x;
            let distance = (node_screen_pos - screen_pos).length();
            if distance < screen_radius + 16.0 && distance < best_distance {
                result = Some(GraphRef::Node(key));
                best_distance = distance;
                outside_distance = distance - screen_radius;
            }

            let radius_screen_pos = world_to_screen
                .transform_point2(node.pos.as_vec2() + vec2(0.0, node.radius as f32));
            let distance = ((radius_screen_pos - screen_pos).length() - 8.0).max(0.0);
            if distance < 8.0 && distance < best_distance {
                result = Some(GraphRef::NodeRadius(key));
                best_distance = distance;
                outside_distance = distance;
            }
        }

        for (key, edge) in &self.edges {
            let start = self
                .nodes
                .get(edge.start)
                .map(|n| (n.pos.as_vec2(), n.radius as f32));
            let end = self
                .nodes
                .get(edge.end)
                .map(|n| (n.pos.as_vec2(), n.radius as f32));
            if let Some(((start, start_r), (end, end_r))) = start.zip(end) {
                let start_screen = world_to_screen.transform_point2(start);
                let end_screen = world_to_screen.transform_point2(end);
                let start_r_screen = world_to_screen.transform_vector2(vec2(start_r, 0.0)).x;
                let end_r_screen = world_to_screen.transform_vector2(vec2(end_r, 0.0)).x;
                let r_screen = start_r_screen.min(end_r_screen);
                let dist = sd_segment(screen_pos, start_screen, end_screen);
                if dist < best_distance && dist <= r_screen
                    // give nodes priority, but only within their radius
                    && !(matches!(result, Some(GraphRef::Node(_))) && outside_distance < 0.0)
                {
                    let (_, position_on_segment) =
                        closest_point_on_segment(start_screen, end_screen, screen_pos);
                    result = Some(GraphRef::EdgePoint(
                        key,
                        NotNan::new(position_on_segment).unwrap(),
                    ));
                    best_distance = dist;
                    outside_distance = dist;
                }
            }
        }
        result
    }

    pub fn draw_nodes(
        &self,
        batch: &mut MiniquadBatch<VertexPos3UvColor>,
        mouse_pos: Vec2,
        view: &View,
    ) {
        let world_to_screen = view.world_to_screen();
        let hover = self.hit_test(mouse_pos, view);

        let colorize = |r| {
            if Some(r) == hover {
                ([255, 128, 0, 255], 2.0)
            } else if self.selected.contains(&r) {
                ([0, 128, 255, 255], 2.0)
            } else {
                ([128, 128, 128, 128], 1.0)
            }
        };

        for (key, node) in &self.nodes {
            let pos_screen = world_to_screen.transform_point2(node.pos.as_vec2());
            let screen_radius = world_to_screen
                .transform_vector2(vec2(node.radius as f32, 0.0))
                .x;

            let (color, thickness) = colorize(GraphRef::Node(key));
            batch
                .geometry
                .stroke_circle_aa(pos_screen, 16.0, thickness, 24, color);

            let (color, thickness) = colorize(GraphRef::NodeRadius(key));
            batch.geometry.fill_circle_aa(
                pos_screen + vec2(0.0, screen_radius),
                3.0 + thickness,
                12,
                color,
            );
        }

        for (key, edge) in &self.edges {
            let a = self
                .nodes
                .get(edge.start)
                .map(|n| (n.pos.as_vec2(), n.radius as f32));
            let b = self
                .nodes
                .get(edge.end)
                .map(|n| (n.pos.as_vec2(), n.radius as f32));
            if let Some(((pos_a, r_a), (pos_b, r_b))) = a.zip(b) {
                let a_to_b = pos_b - pos_a;
                if a_to_b.length() > r_a + r_b {
                    let a_to_b_n = a_to_b.normalize_or_zero();
                    let screen_a = world_to_screen.transform_point2(pos_a + a_to_b_n * r_a);
                    let screen_b = world_to_screen.transform_point2(pos_b - a_to_b_n * r_b);
                    let (color, thickness) = colorize(GraphRef::Edge(key));
                    batch
                        .geometry
                        .stroke_line_aa(screen_a, screen_b, thickness, color);
                }
            }
        }

        for &selection in self.selected.iter().chain(hover.iter()) {
            match selection {
                GraphRef::EdgePoint(key, pos) => {
                    let edge = some_or!(self.edges.get(key), continue);
                    let start = some_or!(self.nodes.get(edge.start), continue);
                    let end = some_or!(self.nodes.get(edge.end), continue);
                    let screen_a = world_to_screen.transform_point2(start.pos.as_vec2());
                    let screen_b = world_to_screen.transform_point2(end.pos.as_vec2());
                    let pos = screen_a.lerp(screen_b, *pos);
                    let n = (screen_b - screen_a).perp().normalize_or_zero();
                    let (color, thickness) = colorize(selection);
                    batch
                        .geometry
                        .stroke_line_aa(pos - n * 8.0, pos + n * 8.0, thickness, color)
                }
                _ => {}
            }
        }
    }

    pub fn snap_to_grid(pos: Vec2, snap_step: i32) -> Vec2 {
        let snap_step = snap_step as f32;
        (pos / snap_step).round() * snap_step
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
    pub fn mark_dirty_layer(&mut self, layer_key: LayerKey) {
        let layer_index = layer_key.data().as_ffi() & 0xffffffff;
        let bit_index = layer_index.min(63);
        let bit = 1 << bit_index;
        self.cell_layers |= bit;
    }
}

impl LayerKey {
    pub fn label(&self) -> String {
        let index = self.data().as_ffi() & 0xffffffff;
        format!("Layer {}", index)
    }
}
