use serde::{Deserialize, Serialize};

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub enum MarkupPointKind {
    Start,
}

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub enum MarkupRectKind {
    RaceFinish,
}

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub enum MarkupSegmentKind {
    Boost,
    Bounce,
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

#[derive(Copy, Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub struct MarkupSegment {
    pub kind: MarkupSegmentKind,
    pub start: [i32; 2],
    pub end: [i32; 2],
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq)]
pub struct MapMarkup {
    pub points: Vec<MarkupPoint>,
    pub rects: Vec<MarkupRect>,
    #[serde(default)]
    pub segments: Vec<MarkupSegment>,
}

impl Default for MapMarkup {
    fn default() -> Self {
        MapMarkup::new()
    }
}

impl MapMarkup {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            rects: Vec::new(),
            segments: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.rects.is_empty() && self.segments.is_empty()
    }

    pub fn translate(&mut self, delta: [i32; 2]) {
        for point in self.points.iter_mut() {
            point.pos[0] += delta[0];
            point.pos[1] += delta[1];
        }
        for rect in self.rects.iter_mut() {
            rect.start[0] += delta[0];
            rect.start[1] += delta[1];
            rect.end[0] += delta[0];
            rect.end[1] += delta[1];
        }
        for segment in self.segments.iter_mut() {
            segment.start[0] += delta[0];
            segment.start[1] += delta[1];
            segment.end[0] += delta[0];
            segment.end[1] += delta[1];
        }
    }
}
