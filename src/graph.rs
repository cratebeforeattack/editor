use glam::IVec2;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Graph {
    pub points: Vec<GraphNode>,
    pub edges: Vec<(usize, usize)>,
    pub value: u8,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    pos: IVec2,
    radius: usize,
}
