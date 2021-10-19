use crate::document::View;
use crate::grid::Grid;
use crate::sdf::sd_trapezoid;
use glam::{ivec2, vec2, IVec2, Vec2};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use slotmap::{new_key_type, SlotMap};

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
    pub selection: Option<GraphRef>,
    pub nodes: SlotMap<GraphNodeKey, GraphNode>,
    pub edges: SlotMap<GraphEdgeKey, GraphEdge>,
    pub value: u8,
    #[serde(default = "outline_value_default")]
    pub outline_value: u8,
    #[serde(default = "outline_width_default")]
    pub outline_width: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub start: GraphNodeKey,
    pub end: GraphNodeKey,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    pub pos: IVec2,
    pub radius: usize,
    #[serde(default = "Default::default")]
    pub no_outline: bool,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum GraphRef {
    Node(GraphNodeKey),
    NodeRadius(GraphNodeKey),
    Edge(GraphEdgeKey),
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            selection: None,
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
            } else if Some(r) == self.selection {
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
                let a_to_b = (pos_b - pos_a);
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
    }

    pub fn hit_test(&self, screen_pos: Vec2, view: &View) -> Option<GraphRef> {
        let world_pos = view.screen_to_world().transform_point2(screen_pos);
        let world_to_screen = view.world_to_screen();
        let mut result = None;
        let mut best_distance = f32::MAX;
        for (key, node) in &self.nodes {
            let node_screen_pos = world_to_screen.transform_point2(node.pos.as_vec2());

            let screen_radius = world_to_screen
                .transform_vector2(vec2(node.radius as f32, 0.0))
                .x;
            let distance = (node_screen_pos - screen_pos).length();
            if distance < screen_radius + 16.0 && distance < best_distance {
                result = Some(GraphRef::Node(key));
                best_distance = distance;
            }

            let radius_screen_pos = world_to_screen
                .transform_point2(node.pos.as_vec2() + vec2(0.0, node.radius as f32));
            let distance = (radius_screen_pos - screen_pos).length();
            if distance < 8.0 && distance < best_distance {
                result = Some(GraphRef::NodeRadius(key));
                best_distance = distance;
            }
        }
        result
    }

    pub fn render_cells(&self, grid: &mut Grid, cell_size: i32) {
        let b = self.compute_bounds_cell(cell_size);
        grid.resize_to_include_conservative(b);

        let cell_size_f = cell_size as f32;
        let outline_width = self.outline_width as f32;
        let outline_value = self.outline_value;

        for y in b[1]..b[3] {
            for x in b[0]..b[2] {
                let pos = (ivec2(x, y).as_vec2() + vec2(0.5, 0.5)) * cell_size_f;
                let mut closest_d = f32::MAX;
                for node in self.nodes.values() {
                    let d = (pos - node.pos.as_vec2()).length() - node.radius as f32;
                    closest_d = closest_d.min(d);
                }
                for edge in self.edges.values() {
                    let a = self
                        .nodes
                        .get(edge.start)
                        .map(|n| (n.pos.as_vec2(), n.radius as f32));
                    let b = self
                        .nodes
                        .get(edge.end)
                        .map(|n| (n.pos.as_vec2(), n.radius as f32));
                    if let Some(((a_pos, a_r), (b_pos, b_r))) = a.zip(b) {
                        let r = a_r.min(b_r);
                        let d = sd_trapezoid(pos, a_pos, b_pos, r, r);
                        closest_d = closest_d.min(d);
                    }
                }
                if closest_d > 0.0 && closest_d < outline_width {
                    let index = grid.grid_pos_index(x, y);
                    grid.cells[index] = outline_value;
                } else if closest_d <= 0.0 {
                    let index = grid.grid_pos_index(x, y);
                    grid.cells[index] = self.value;
                }
            }
        }
    }
    fn compute_bounds(&self) -> [f32; 4] {
        if self.nodes.is_empty() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for node in self.nodes.values() {
            let n = node.bounds();
            b[0] = b[0].min(n[0]);
            b[1] = b[1].min(n[1]);
            b[2] = b[2].max(n[2]);
            b[3] = b[3].max(n[3]);
        }
        b
    }

    fn compute_bounds_cell(&self, cell_size: i32) -> [i32; 4] {
        let b = self.compute_bounds();
        [
            b[0].div_euclid(cell_size as f32).floor() as i32 - 1,
            b[1].div_euclid(cell_size as f32).floor() as i32 - 1,
            b[2].div_euclid(cell_size as f32).ceil() as i32 + 1,
            b[3].div_euclid(cell_size as f32).ceil() as i32 + 1,
        ]
    }
}

impl GraphNode {
    fn bounds(&self) -> [f32; 4] {
        [
            self.pos.x as f32 - self.radius as f32,
            self.pos.y as f32 - self.radius as f32,
            self.pos.x as f32 + self.radius as f32,
            self.pos.y as f32 + self.radius as f32,
        ]
    }
}
