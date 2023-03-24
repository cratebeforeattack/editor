use crate::document::LayerKey;
use crate::field::Field;
use crate::math::Rect;
use crate::plant::{Plant, PlantKey, PlantSegment, PlantSegmentKey};
use crate::sdf::{sd_box, sd_circle, sd_octogon, sd_outline, sd_trapezoid};
use glam::{ivec2, vec2, IVec2, Vec2};
use ordered_float::NotNan;
use rayon::iter::{IntoParallelRefIterator, ParallelExtend, ParallelIterator};
use slotmap::{new_key_type, SlotMap};
use std::collections::HashMap;
use tracy_client::span;

new_key_type! {
    pub struct GraphNodeKey;
    pub struct GraphEdgeKey;
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GraphEdge {
    pub start: GraphNodeKey,
    pub end: GraphNodeKey,
}

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
pub enum GraphNodeShape {
    Octogon,
    Circle,
    Square,
}

fn graph_node_shape_default() -> GraphNodeShape {
    GraphNodeShape::Octogon
}
fn graph_node_material_default() -> u8 {
    1
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GraphNode {
    pub pos: IVec2,
    pub radius: usize,
    #[serde(default = "graph_node_shape_default")]
    pub shape: GraphNodeShape,
    #[serde(default = "Default::default")]
    pub no_outline: bool,
    #[serde(default = "graph_node_material_default")]
    pub material: u8,
    #[serde(default)]
    pub layer: LayerKey,
    #[serde(default)]
    pub thickness: usize,
}

impl GraphNode {
    pub fn new() -> GraphNode {
        GraphNode {
            pos: IVec2::ZERO,
            radius: 192,
            shape: GraphNodeShape::Octogon,
            no_outline: false,
            material: graph_node_material_default(),
            layer: LayerKey::default(),
            thickness: 8,
        }
    }
    pub(crate) fn bounds(&self) -> [Vec2; 2] {
        [
            self.pos.as_vec2() - Vec2::splat(self.radius as f32),
            self.pos.as_vec2() + Vec2::splat(self.radius as f32),
        ]
    }

    pub fn render_distances(
        field: &mut Field,
        cell_size: i32,
        layer_key: LayerKey,
        nodes: &SlotMap<GraphNodeKey, GraphNode>,
        edges: &SlotMap<GraphEdgeKey, GraphEdge>,
        plants: &SlotMap<PlantKey, Plant>,
        plant_segments: &mut SlotMap<PlantSegmentKey, PlantSegment>,
    ) {
        let _span = span!("GraphNode::render_distances");
        {
            Plant::grow_plants(field, plants, plant_segments);
        }
        let plant_segments: &SlotMap<_, _> = plant_segments;

        let cell_size_f = cell_size as f32;
        let outline_width = 8.0;
        let half_thickness = outline_width * 0.5;

        let mut node_cache: Vec<HashMap<(i32, i32), Vec<_>>> = vec![];
        let mut edge_cache: Vec<HashMap<(i32, i32), Vec<_>>> = vec![];
        let mut plant_cache: Vec<HashMap<(i32, i32), Vec<_>>> = vec![];
        // material 0 has to come last for "no-outline" to work
        let mut used_materials: Vec<_> = nodes
            .values()
            .map(|n| n.material as usize)
            .chain(plants.values().map(|p| p.material as usize))
            .collect();
        used_materials.sort();
        used_materials.dedup();
        if used_materials.get(0).copied() == Some(0) {
            used_materials.remove(0);
        }
        used_materials.push(0);

        for &material in &used_materials {
            while node_cache.len() <= material as usize {
                node_cache.push(HashMap::default());
            }
            while edge_cache.len() <= material as usize {
                edge_cache.push(HashMap::default());
            }
            while plant_cache.len() <= material as usize {
                plant_cache.push(HashMap::default());
            }
        }

        {
            let _span = span!("node_cache");

            for (key, node) in nodes.iter().filter(|(_, n)| n.layer == layer_key) {
                let padding = 32.0;
                let node_bounds = node.bounds().inflate(padding);
                let tile_range =
                    Field::world_to_tile_range(node_bounds, cell_size, field.tile_size);
                let material = if node.no_outline { 0 } else { node.material };
                for y in tile_range[0].y..tile_range[1].y {
                    for x in tile_range[0].x..tile_range[1].x {
                        node_cache[material as usize]
                            .entry((x, y))
                            .or_insert_with(|| Vec::new())
                            .push(key);
                    }
                }
            }
        }
        {
            let _span = span!("edge_cache");
            for (key, edge) in edges {
                let padding = 32.0;
                let a = nodes.get(edge.start);
                let b = nodes.get(edge.end);
                if a.map(|a| a.layer) != Some(layer_key) && b.map(|b| b.layer) != Some(layer_key) {
                    continue;
                }
                let node_bounds = match edge.bounds(nodes) {
                    Some(v) => v.inflate(padding),
                    None => continue,
                };
                let tile_range =
                    Field::world_to_tile_range(node_bounds, cell_size, field.tile_size);
                let a_no_outline = a.map(|n| n.no_outline).unwrap_or(false);
                let b_no_outline = b.map(|n| n.no_outline).unwrap_or(false);
                let material = if a_no_outline | b_no_outline {
                    0
                } else {
                    a.map(|a| a.material)
                        .or_else(|| b.map(|b| b.material))
                        .unwrap_or(1)
                };
                for y in tile_range[0].y..tile_range[1].y {
                    for x in tile_range[0].x..tile_range[1].x {
                        edge_cache[material as usize]
                            .entry((x, y))
                            .or_insert_with(|| Vec::new())
                            .push(key);
                    }
                }
            }
        }
        {
            let _span = span!("plant_cache");

            for (key, segment) in plant_segments {
                let Some(plant ) = plants.get(segment.plant) else { continue };
                if plant.layer != layer_key {
                    continue;
                }
                let padding = 32.0;
                let bounds = segment.bounds().inflate(padding);
                let tile_range = Field::world_to_tile_range(bounds, cell_size, field.tile_size);
                let material = plant.material;
                for y in tile_range[0].y..tile_range[1].y {
                    for x in tile_range[0].x..tile_range[1].x {
                        plant_cache[material as usize]
                            .entry((x, y))
                            .or_insert_with(|| Vec::new())
                            .push(key);
                    }
                }
            }
        }

        drop(_span);
        let tile_size = field.tile_size;
        {
            let _span = span!("cells");
            for material in used_materials {
                let mut all_tile_keys = node_cache[material]
                    .keys()
                    .copied()
                    .chain(edge_cache[material].keys().copied())
                    .chain(plant_cache[material].keys().copied())
                    .collect::<Vec<_>>();
                all_tile_keys.sort();
                all_tile_keys.dedup();

                field.materials[material].par_extend(all_tile_keys.par_iter().copied().map(
                    |tile_key| {
                        let _span = span!("tile");
                        let mut tile = vec![f32::MAX; tile_size * tile_size];
                        for material in [material, 0] {
                            let tile_nodes = node_cache[material]
                                .get(&tile_key)
                                .map(|v| v.as_slice())
                                .unwrap_or(&[]);
                            let tile_edges = edge_cache[material]
                                .get(&tile_key)
                                .map(|v| v.as_slice())
                                .unwrap_or(&[]);
                            let tile_plant_segments = plant_cache[material]
                                .get(&tile_key)
                                .map(|v| v.as_slice())
                                .unwrap_or(&[]);
                            for index in 0..tile_size * tile_size {
                                let x = (index & (tile_size - 1)) as i32
                                    + tile_key.0 * tile_size as i32;
                                let y = (index / tile_size) as i32 + tile_key.1 * tile_size as i32;
                                let pos = (ivec2(x, y).as_vec2() + vec2(0.5, 0.5)) * cell_size_f;
                                let mut closest_d = f32::MAX;

                                for node in tile_nodes.iter().map(|k| nodes.get(*k).unwrap()) {
                                    let d = match node.shape {
                                        GraphNodeShape::Octogon => {
                                            sd_octogon(pos - node.pos.as_vec2(), node.radius as f32)
                                        }
                                        GraphNodeShape::Circle => {
                                            sd_circle(pos, node.pos.as_vec2(), node.radius as f32)
                                        }
                                        GraphNodeShape::Square => sd_box(
                                            pos - node.pos.as_vec2(),
                                            Vec2::splat(node.radius as f32),
                                        ),
                                    };
                                    closest_d = d.min(closest_d);
                                }
                                for edge in tile_edges.iter().map(|k| edges.get(*k).unwrap()) {
                                    let a = nodes
                                        .get(edge.start)
                                        .map(|n| (n.pos.as_vec2(), n.radius as f32));
                                    let b = nodes
                                        .get(edge.end)
                                        .map(|n| (n.pos.as_vec2(), n.radius as f32));
                                    if let Some(((a_pos, a_r), (b_pos, b_r))) = a.zip(b) {
                                        let d = sd_trapezoid(pos, a_pos, b_pos, a_r, b_r);
                                        closest_d = d.min(closest_d);
                                    }
                                }

                                if material != 0 {
                                    closest_d = sd_outline(closest_d, half_thickness);

                                    for segment in tile_plant_segments
                                        .iter()
                                        .map(|k| plant_segments.get(*k).unwrap())
                                    {
                                        let d = sd_trapezoid(
                                            pos,
                                            segment.start,
                                            segment.end,
                                            segment.start_thickness,
                                            segment.end_thickness,
                                        );
                                        closest_d = d.min(closest_d);
                                    }
                                }

                                if material == 0 {
                                    tile[index] = tile[index].max(-closest_d);
                                } else {
                                    tile[index] = tile[index].min(closest_d);
                                }
                            }
                        }
                        (tile_key, tile)
                    },
                ));
            }
        }

        let _span = span!("drop");
        drop(node_cache);
        drop(edge_cache);
        drop(plant_cache);
    }

    pub fn split_edge_node(
        nodes: &SlotMap<GraphNodeKey, GraphNode>,
        edges: &SlotMap<GraphEdgeKey, GraphEdge>,
        key: GraphEdgeKey,
        split_pos: SplitPos,
    ) -> GraphNode {
        let mut default_node = None;
        let mut pos = match split_pos {
            SplitPos::Fraction(_) => IVec2::ZERO,
        };
        if let Some(edge) = edges.get(key) {
            if let Some((start, end)) = nodes.get(edge.start).zip(nodes.get(edge.end)) {
                let mut node = start.clone();
                node.radius = node.radius.min(end.radius);
                if end.no_outline == false {
                    node.no_outline = false;
                }
                match split_pos {
                    SplitPos::Fraction(f) => {
                        pos = start
                            .pos
                            .as_vec2()
                            .lerp(end.pos.as_vec2(), f)
                            .floor()
                            .as_ivec2();
                    }
                }
                default_node = Some(node);
            }
        }
        GraphNode {
            pos,
            ..default_node.unwrap_or_else(|| GraphNode::new())
        }
    }

    pub fn merge_nodes(
        keys: &[GraphNodeKey],
        nodes: &SlotMap<GraphNodeKey, GraphNode>,
        distance_threshold: f32,
    ) -> Vec<(GraphNodeKey, GraphNodeKey)> {
        let mut keys = keys.to_owned();
        keys.sort_unstable();
        let mut result = Vec::new();
        let other_keys: Vec<GraphNodeKey> = nodes
            .keys()
            .filter(|k| !keys.binary_search(k).is_ok())
            .collect();
        for key in keys {
            let Some(node ) = nodes.get(key) else { continue };

            let closest_node = other_keys
                .iter()
                .cloned()
                .map(|k| {
                    (
                        NotNan::new((node.pos - nodes[k].pos).as_vec2().length() as f32).unwrap(),
                        k,
                    )
                })
                .min();

            if let Some((closest_d, closest_k)) = closest_node {
                if *closest_d < distance_threshold {
                    result.push((key, closest_k));
                }
            }
        }
        result.sort_unstable();
        result
    }
}

impl GraphEdge {
    pub(crate) fn split_edge(
        edges: &mut SlotMap<GraphEdgeKey, GraphEdge>,
        key: GraphEdgeKey,
        node_key: GraphNodeKey,
    ) -> GraphNodeKey {
        if let Some(edge) = edges.get_mut(key) {
            let old_end = edge.end;
            edge.end = node_key;
            edges.insert(GraphEdge {
                start: node_key,
                end: old_end,
            });
        }
        node_key
    }

    pub fn bounds(&self, nodes: &SlotMap<GraphNodeKey, GraphNode>) -> Option<[Vec2; 2]> {
        let mut b = Rect::invalid();
        if let Some(start_bounds) = nodes.get(self.start).map(|n| n.bounds()) {
            b = start_bounds;
        }
        if let Some(end_bounds) = nodes.get(self.end).map(|n| n.bounds()) {
            b = b.union(end_bounds);
        }
        b.valid()
    }
}

pub enum SplitPos {
    Fraction(f32),
}
