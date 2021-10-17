use crate::document::View;
use glam::{IVec2, Vec2};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct GraphNodeKey;
    pub struct GraphEdgeKey;
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Graph {
    pub nodes: SlotMap<GraphNodeKey, GraphNode>,
    pub edges: SlotMap<GraphEdgeKey, GraphEdge>,
    pub value: u8,
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
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            nodes: SlotMap::with_key(),
            edges: SlotMap::with_key(),
            value: 255,
        }
    }

    pub fn draw_graph(
        &self,
        batch: &mut MiniquadBatch<VertexPos3UvColor>,
        mouse_pos: Vec2,
        view: &View,
    ) {
        let mouse_world = view.screen_to_world().transform_point2(mouse_pos);
        let world_to_screen = view.world_to_screen();
        for node in self.nodes.values() {
            let pos_screen = world_to_screen.transform_vector2(node.pos.as_vec2());
            batch
                .geometry
                .stroke_circle(pos_screen, 16.0, 2.0, 16, [255, 255, 255, 255]);
        }
    }
}
