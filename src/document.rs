use glam::{vec2, Affine2, Vec2};
use log::info;
use miniquad::{Context, FilterMode, Texture};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering::*;
use std::collections::BTreeSet;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum TraceMethod {
    Walk,
    Grid,
}
fn default_trace_method() -> TraceMethod {
    TraceMethod::Walk
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Grid {
    pub bounds: [i32; 4],
    pub cell_size: i32,
    pub cells: Vec<u8>,
    #[serde(default = "default_trace_method")]
    pub trace_method: TraceMethod,
}

fn show_reference_default() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Document {
    pub layer: Grid,

    pub reference_path: Option<String>,
    #[serde(default = "show_reference_default")]
    pub show_reference: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct View {
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
pub(crate) struct DocumentLocalState {
    pub view: View,
}

#[derive(Default, Copy, Clone, PartialEq)]
pub(crate) struct ChangeMask {
    pub cells: bool,
    pub reference_path: bool,
}

impl Grid {
    pub fn size(&self) -> [i32; 2] {
        [
            self.bounds[2] - self.bounds[0],
            self.bounds[3] - self.bounds[1],
        ]
    }

    pub fn resize(&mut self, new_bounds: [i32; 4]) {
        if self.bounds == new_bounds {
            return;
        }
        let old_bounds = self.bounds;
        let old_size = [old_bounds[2] - old_bounds[0], old_bounds[3] - old_bounds[1]];
        let new_size = [new_bounds[2] - new_bounds[0], new_bounds[3] - new_bounds[1]];
        let mut new_cells = vec![0u8; new_size[0] as usize * new_size[1] as usize];
        let y_range = old_bounds[1].max(new_bounds[1])..old_bounds[3].min(new_bounds[3]);
        let x_range = old_bounds[0].max(new_bounds[0])..old_bounds[2].min(new_bounds[2]);
        for y in y_range {
            let old_start =
                ((y - old_bounds[1]) * old_size[0] + (x_range.start - old_bounds[0])) as usize;
            let new_start =
                ((y - new_bounds[1]) * new_size[0] + (x_range.start - new_bounds[0])) as usize;
            let old_range = old_start..old_start + x_range.len();
            let new_range = new_start..new_start + x_range.len();
            new_cells[new_range].copy_from_slice(&self.cells[old_range]);
        }
        self.bounds = new_bounds;
        self.cells = new_cells;
        println!("resized {:?}->{:?}", old_bounds, new_bounds);
        info!("resized {:?}->{:?}", old_bounds, new_bounds);
    }

    pub(crate) fn resize_to_include(&mut self, point: [i32; 2]) {
        let [x, y] = point;
        let tile_size_cells = 64;
        let tile_x = x.div_euclid(tile_size_cells);
        let tile_y = y.div_euclid(tile_size_cells);

        let tile_bounds = [
            tile_x * tile_size_cells,
            tile_y * tile_size_cells,
            (tile_x + 1) * tile_size_cells,
            (tile_y + 1) * tile_size_cells,
        ];

        let bounds = [
            self.bounds[0].min(tile_bounds[0]),
            self.bounds[1].min(tile_bounds[1]),
            self.bounds[2].max(tile_bounds[2]),
            self.bounds[3].max(tile_bounds[3]),
        ];

        self.resize(bounds);
    }
}

impl View {
    pub fn screen_to_world(&self) -> Affine2 {
        self.world_to_screen().inverse()
    }

    pub fn world_to_screen(&self) -> Affine2 {
        Affine2::from_translation(vec2(self.screen_width_px, self.screen_height_px) * 0.5)
            * Affine2::from_scale(Vec2::splat(self.zoom))
            * Affine2::from_translation(-self.target)
    }
}
