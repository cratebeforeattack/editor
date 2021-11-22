use crate::document::View;
use crate::grid::Grid;
use crate::math::{closest_point_on_segment, Rect};
use crate::profiler::Profiler;
use crate::sdf::{sd_box, sd_circle, sd_octogon, sd_outline, sd_segment, sd_trapezoid};
use crate::some_or::some_or;
use glam::{ivec2, vec2, IVec2, Vec2};
use ordered_float::NotNan;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use rayon::slice::ParallelSliceMut;
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use slotmap::{new_key_type, SlotMap};
use tracy_client::{message, span};

new_key_type! {
    pub struct GraphNodeKey;
    pub struct GraphEdgeKey;
}

fn outline_value_default() -> u8 {
    1
}

fn outline_width_default() -> usize {
    8
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Graph {
    #[serde(default = "Default::default")]
    pub selected: Vec<GraphRef>,
    pub nodes: SlotMap<GraphNodeKey, GraphNode>,
    pub edges: SlotMap<GraphEdgeKey, GraphEdge>,
    pub value: u8,
    #[serde(default = "outline_value_default")]
    pub outline_value: u8,
    #[serde(default = "outline_width_default")]
    pub outline_width: usize,
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct GraphNode {
    pub pos: IVec2,
    pub radius: usize,
    #[serde(default = "graph_node_shape_default")]
    pub shape: GraphNodeShape,
    #[serde(default = "Default::default")]
    pub no_outline: bool,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Ord, PartialOrd, Eq)]
pub enum GraphRef {
    Node(GraphNodeKey),
    NodeRadius(GraphNodeKey),
    Edge(GraphEdgeKey),
    EdgePoint(GraphEdgeKey, NotNan<f32>),
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            selected: Vec::new(),
            nodes: SlotMap::with_key(),
            edges: SlotMap::with_key(),
            value: 255,
            outline_value: outline_value_default(),
            outline_width: outline_width_default(),
        }
    }

    pub fn draw_graph(
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

    pub fn hit_test(&self, screen_pos: Vec2, view: &View) -> Option<GraphRef> {
        let world_to_screen = view.world_to_screen();
        let mut result = None;
        let mut best_distance = f32::MAX;
        for (key, node) in &self.nodes {
            let node_screen_pos = world_to_screen.transform_point2(node.pos.as_vec2());

            let screen_radius = world_to_screen
                .transform_vector2(vec2(node.radius as f32, 0.0))
                .x;
            let distance = (node_screen_pos - screen_pos).length() - screen_radius;
            if distance < 16.0 && distance < best_distance {
                result = Some(GraphRef::Node(key));
                best_distance = distance;
            }

            let radius_screen_pos = world_to_screen
                .transform_point2(node.pos.as_vec2() + vec2(0.0, node.radius as f32));
            let distance = ((radius_screen_pos - screen_pos).length() - 8.0).max(0.0);
            if distance < 8.0 && distance < best_distance {
                result = Some(GraphRef::NodeRadius(key));
                best_distance = distance;
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
                if dist < best_distance && dist <= r_screen {
                    let (_, position_on_segment) =
                        closest_point_on_segment(start_screen, end_screen, screen_pos);
                    result = Some(GraphRef::EdgePoint(
                        key,
                        NotNan::new(position_on_segment).unwrap(),
                    ));
                    best_distance = dist;
                }
            }
        }
        result
    }

    pub fn render_cells(&self, grid: &mut Grid<u8>, cell_size: i32, profiler: &mut Profiler) {
        let _span = span!("Graph::render_cells");
        let b = Self::bounds_in_cells(self.compute_bounds(), cell_size);
        grid.resize_to_include_conservative(b);

        let cell_size_f = cell_size as f32;
        let outline_width = self.outline_width as f32;
        let outline_value = self.outline_value;

        let height = b.size().y;

        profiler.open_block("node_cache");
        let (node_cache, edge_cache) = {
            let _span = span!("node cache");
            rayon::join(
                || {
                    let _span = span!("node_cache");
                    let mut node_cache = vec![Vec::new(); height as usize];
                    for (key, node) in &self.nodes {
                        let padding = 32.0;
                        let node_bounds = node.bounds().inflate(padding);
                        let node_cells = Self::bounds_in_cells(node_bounds, cell_size);
                        for y in node_cells[0].y.max(b[0].y)..node_cells[1].y.min(b[1].y) {
                            node_cache[(y - b[0].y) as usize].push(key);
                        }
                    }
                    node_cache
                },
                || {
                    let _span = span!("edge_cache");
                    let mut edge_cache = vec![Vec::new(); height as usize];
                    for (key, edge) in &self.edges {
                        let padding = 32.0;
                        let node_bounds = match edge.bounds(&self.nodes) {
                            Some(v) => v.inflate(padding),
                            None => continue,
                        };
                        let node_cells = Self::bounds_in_cells(node_bounds, cell_size);
                        for y in node_cells[0].y.max(b[0].y)..node_cells[1].y.min(b[1].y) {
                            edge_cache[(y - b[0].y) as usize].push(key);
                        }
                    }
                    edge_cache
                },
            )
        };

        profiler.close_block();
        drop(_span);
        profiler.open_block("cells");
        let grid_w = grid.bounds.size().x;
        {
            let _span = span!("cells");
            grid.cells
                .par_chunks_mut(grid_w.max(1) as usize)
                .with_min_len(16)
                .skip((b[0].y - grid.bounds[0].y) as usize)
                .zip(b[0].y..b[1].y)
                .for_each(|(row, y)| {
                    let _span = span!("row");
                    for x in b[0].x..b[1].x {
                        let pos = (ivec2(x, y).as_vec2() + vec2(0.5, 0.5)) * cell_size_f;
                        let mut closest_d = (f32::MAX, false);
                        for node in node_cache[(y - b[0].y) as usize]
                            .iter()
                            .map(|k| self.nodes.get(*k).unwrap())
                        {
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
                            if d <= closest_d.0 {
                                closest_d = (d, node.no_outline);
                            }
                        }
                        for edge in edge_cache[(y - b[0].y) as usize]
                            .iter()
                            .map(|k| self.edges.get(*k).unwrap())
                        {
                            let a = self
                                .nodes
                                .get(edge.start)
                                .map(|n| (n.pos.as_vec2(), n.radius as f32, n.no_outline));
                            let b = self
                                .nodes
                                .get(edge.end)
                                .map(|n| (n.pos.as_vec2(), n.radius as f32, n.no_outline));
                            if let Some(((a_pos, a_r, a_no_outline), (b_pos, b_r, b_no_outline))) =
                                a.zip(b)
                            {
                                let r = a_r.min(b_r);
                                let d = sd_trapezoid(pos, a_pos, b_pos, r, r);
                                if d <= closest_d.0 {
                                    closest_d = (d, a_no_outline || b_no_outline);
                                }
                            }
                        }
                        let (closest_d, no_outline) = closest_d;
                        if closest_d > 0.0 && closest_d < outline_width && !no_outline {
                            let index = x - grid.bounds[0].x;
                            row[index as usize] = outline_value;
                        } else if closest_d <= 0.0 {
                            let index = x - grid.bounds[0].x;
                            row[index as usize] = self.value;
                        }
                    }
                });
        }

        let _span = span!("drop");
        drop(node_cache);
        drop(edge_cache);

        profiler.close_block();
    }

    pub fn render_distances(&self, grids: &mut Vec<Grid<f32>>, cell_size: i32) {
        let grid = &mut grids[1];
        let _span = span!("Graph::render_distances");
        let b = Self::bounds_in_cells(self.compute_bounds(), cell_size);
        grid.resize_to_include_conservative(b);

        let cell_size_f = cell_size as f32;
        let outline_width = self.outline_width as f32;
        let outline_value = self.outline_value;
        let half_thickness = outline_width * 0.5;

        let height = b.size().y;

        let (node_cache, edge_cache) = {
            let _span = span!("node cache");
            rayon::join(
                || {
                    let _span = span!("node_cache");
                    let mut node_cache = vec![Vec::new(); height as usize];
                    for (key, node) in &self.nodes {
                        let padding = 32.0;
                        let node_bounds = node.bounds().inflate(padding);
                        let node_cells = Self::bounds_in_cells(node_bounds, cell_size);
                        for y in node_cells[0].y.max(b[0].y)..node_cells[1].y.min(b[1].y) {
                            node_cache[(y - b[0].y) as usize].push(key);
                        }
                    }
                    node_cache
                },
                || {
                    let _span = span!("edge_cache");
                    let mut edge_cache = vec![Vec::new(); height as usize];
                    for (key, edge) in &self.edges {
                        let padding = 32.0;
                        let node_bounds = match edge.bounds(&self.nodes) {
                            Some(v) => v.inflate(padding),
                            None => continue,
                        };
                        let node_cells = Self::bounds_in_cells(node_bounds, cell_size);
                        for y in node_cells[0].y.max(b[0].y)..node_cells[1].y.min(b[1].y) {
                            edge_cache[(y - b[0].y) as usize].push(key);
                        }
                    }
                    edge_cache
                },
            )
        };

        drop(_span);
        let grid_w = grid.bounds.size().x;
        {
            let _span = span!("cells");
            grid.cells
                .par_chunks_mut(grid_w.max(1) as usize)
                .with_min_len(16)
                .skip((b[0].y - grid.bounds[0].y) as usize)
                .zip(b[0].y..b[1].y)
                .for_each(|(row, y)| {
                    let _span = span!("row");
                    for x in b[0].x..b[1].x {
                        let pos = (ivec2(x, y).as_vec2() + vec2(0.5, 0.5)) * cell_size_f;
                        let mut closest_d = (f32::MAX, false);
                        for node in node_cache[(y - b[0].y) as usize]
                            .iter()
                            .map(|k| self.nodes.get(*k).unwrap())
                        {
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
                            if d <= closest_d.0 {
                                closest_d = (d, node.no_outline);
                            }
                        }
                        for edge in edge_cache[(y - b[0].y) as usize]
                            .iter()
                            .map(|k| self.edges.get(*k).unwrap())
                        {
                            let a = self
                                .nodes
                                .get(edge.start)
                                .map(|n| (n.pos.as_vec2(), n.radius as f32, n.no_outline));
                            let b = self
                                .nodes
                                .get(edge.end)
                                .map(|n| (n.pos.as_vec2(), n.radius as f32, n.no_outline));
                            if let Some(((a_pos, a_r, a_no_outline), (b_pos, b_r, b_no_outline))) =
                                a.zip(b)
                            {
                                let d = sd_trapezoid(pos, a_pos, b_pos, a_r, b_r);
                                if d <= closest_d.0 {
                                    closest_d = (d, a_no_outline || b_no_outline);
                                }
                            }
                        }
                        let (closest_d, no_outline) = closest_d;
                        if !no_outline {
                            let closest_d = sd_outline(closest_d, half_thickness);
                            let index = x - grid.bounds[0].x;
                            row[index as usize] = row[index as usize].min(closest_d);
                        }
                    }
                });
        }

        let _span = span!("drop");
        drop(node_cache);
        drop(edge_cache);
    }

    fn compute_bounds(&self) -> [Vec2; 2] {
        if self.nodes.is_empty() {
            return Rect::zero();
        }
        let mut b = Rect::invalid();
        for node in self.nodes.values() {
            let n = node.bounds();
            b = n.union(b);
        }
        b.inflate(self.outline_width as f32)
    }

    fn bounds_in_cells(b: [Vec2; 2], cell_size: i32) -> [IVec2; 2] {
        [
            ivec2(
                b[0].x.div_euclid(cell_size as f32).floor() as i32 - 1,
                b[0].y.div_euclid(cell_size as f32).floor() as i32 - 1,
            ),
            ivec2(
                b[1].x.div_euclid(cell_size as f32).ceil() as i32 + 1,
                b[1].y.div_euclid(cell_size as f32).ceil() as i32 + 1,
            ),
        ]
    }

    pub fn split_edge_node(
        nodes: &SlotMap<GraphNodeKey, GraphNode>,
        edges: &SlotMap<GraphEdgeKey, GraphEdge>,
        key: GraphEdgeKey,
        split_pos: SplitPos,
    ) -> GraphNode {
        let mut default_node = None;
        let mut pos = match split_pos {
            SplitPos::Explicit(pos) => pos,
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
                    _ => {}
                }
                default_node = Some(node);
            }
        }
        GraphNode {
            pos,
            ..default_node.unwrap_or_else(|| GraphNode::new())
        }
    }

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
            let node = some_or!(nodes.get(key), continue);

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

    pub fn snap_to_grid(pos: Vec2, snap_step: i32) -> Vec2 {
        let snap_step = snap_step as f32;
        (pos / snap_step).round() * snap_step
    }
}

impl GraphNode {
    pub fn new() -> GraphNode {
        GraphNode {
            pos: IVec2::ZERO,
            radius: 192,
            shape: GraphNodeShape::Octogon,
            no_outline: false,
        }
    }
    pub(crate) fn bounds(&self) -> [Vec2; 2] {
        [
            self.pos.as_vec2() - Vec2::splat(self.radius as f32),
            self.pos.as_vec2() + Vec2::splat(self.radius as f32),
        ]
    }
}

impl GraphEdge {
    pub fn position(&self, nodes: &SlotMap<GraphNodeKey, GraphNode>, pos: f32) -> Option<Vec2> {
        if let Some((start, end)) = nodes.get(self.start).zip(nodes.get(self.end)) {
            Some(start.pos.as_vec2().lerp(end.pos.as_vec2(), pos))
        } else {
            None
        }
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
    Explicit(IVec2),
    Fraction(f32),
}
