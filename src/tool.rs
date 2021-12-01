use crate::document::{GraphKey, GridKey, Layer, LayerContent};
use crate::graph::Graph;
use std::mem::{discriminant, Discriminant};

#[derive(Clone, Copy)]
pub enum Tool {
    Pan,
    Paint,
    Fill,
    Rectangle,
    Zone,
    Graph,
}

#[derive(Clone, Copy)]
pub enum ToolGroup {
    Paint,
    Graph,
}

impl ToolGroup {
    pub(crate) fn from_tool(tool: Tool) -> Option<ToolGroup> {
        match tool {
            Tool::Pan => None,
            Tool::Paint => Some(ToolGroup::Paint),
            Tool::Fill => Some(ToolGroup::Paint),
            Tool::Rectangle => Some(ToolGroup::Paint),
            Tool::Zone => None,
            Tool::Graph => Some(ToolGroup::Graph),
        }
    }

    pub fn from_layer_content(layer: &LayerContent) -> ToolGroup {
        match layer {
            LayerContent::Grid { .. } => ToolGroup::Paint,
            LayerContent::Graph { .. } => ToolGroup::Graph,
            LayerContent::Field { .. } => ToolGroup::Paint,
        }
    }

    pub fn layer_content_discriminant(&self) -> Discriminant<LayerContent> {
        match self {
            ToolGroup::Paint => discriminant(&LayerContent::Grid(GridKey::default())),
            ToolGroup::Graph => discriminant(&LayerContent::Graph(GraphKey::default())),
        }
    }
}

pub const NUM_TOOL_GROUPS: usize = 2;

pub struct ToolGroupState {
    pub(crate) tool: Tool,
    pub(crate) layer: Option<usize>,
}
