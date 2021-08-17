use glam::Vec2;
use miniquad::Texture;
use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub origin: [i32; 2],
    pub size: [i32; 2],
    pub cells: Vec<u8>,

    pub reference_path: Option<String>,
}

pub struct DocumentGraphics {
    pub outline_points: Vec<Vec<Vec2>>,
    pub reference_texture: Option<Texture>,
}

pub struct ChangeMask {
    pub cells: bool,
    pub reference_path: bool,
}

impl DocumentGraphics {
    fn generate(&mut self, doc: &Document, change_mask: ChangeMask) {
        if change_mask.cells {
            self.outline_points.clear();
        }

        if change_mask.reference_path {
            if let Some(tex) = self.reference_texture.take() {
                tex.delete();
            }

        }
    }
}