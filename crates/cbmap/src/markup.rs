use serde_derive::{Deserialize, Serialize};

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub enum MarkupPointKind {
    Start,
}

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub enum MarkupRectKind {
    RaceFinish,
}

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub struct MarkupPoint {
    pub kind: MarkupPointKind,
    pub pos: [i32; 2],
}

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub struct MarkupRect {
    pub kind: MarkupRectKind,
    pub start: [i32; 2],
    pub end: [i32; 2],
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub struct MapMarkup {
    pub points: Vec<MarkupPoint>,
    pub rects: Vec<MarkupRect>,
}

impl Default for MapMarkup {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            rects: Vec::new(),
        }
    }
}

impl MapMarkup {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            rects: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.rects.is_empty()
    }
}
