use crate::app::App;
use crate::graphics::DocumentGraphics;
use crate::tunnel::Tunnel;
use crate::zone::{AnyZone, ZoneRef};
use anyhow::Result;
use cbmap::{MapMarkup, MaterialSlot, MaterialsJson};
use glam::{vec2, Affine2, IVec2, Vec2};
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct Grid {
    pub bounds: [i32; 4],
    pub cells: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum Layer {
    Grid(Grid),
    Tunnel(Tunnel),
}

fn show_reference_default() -> bool {
    true
}

fn reference_scale_default() -> i32 {
    2
}

fn cell_size_default() -> i32 {
    8
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "Vec::new")]
    pub materials: Vec<MaterialSlot>,

    #[serde(default = "cell_size_default")]
    pub cell_size: i32,

    #[serde(default = "Vec::new")]
    pub layers: Vec<Layer>,

    #[serde(default = "Default::default")]
    pub active_layer: usize,

    #[serde(default = "Grid::new")]
    pub selection: Grid,
    #[serde(default)]
    pub zone_selection: Option<ZoneRef>,

    #[serde(skip)]
    pub side_load: HashMap<String, Vec<u8>>,

    #[serde(skip_serializing_if = "MapMarkup::is_empty")]
    #[serde(default = "MapMarkup::new")]
    pub markup: MapMarkup,

    pub reference_path: Option<String>,
    #[serde(default = "reference_scale_default")]
    pub reference_scale: i32,
    #[serde(default = "show_reference_default")]
    pub show_reference: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct View {
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
pub struct DocumentLocalState {
    pub view: View,
    pub active_material: u8,
}

#[derive(Default, Copy, Clone, PartialEq)]
pub struct ChangeMask {
    pub cell_layers: u64,
    pub reference_path: bool,
}

impl Document {
    pub fn pre_save_cleanup(&mut self) {
        for layer in &mut self.layers {
            match *layer {
                Layer::Grid(ref mut layer) => {
                    let used_bounds = layer.find_used_bounds();
                    let new_bounds = [
                        used_bounds[0] - 1,
                        used_bounds[1] - 1,
                        used_bounds[2] + 1,
                        used_bounds[3] + 1,
                    ];
                    if new_bounds != layer.bounds {
                        layer.resize(new_bounds);
                    }
                }
                Layer::Tunnel { .. } => {}
            }
        }
    }

    pub(crate) fn save_materials(&self, g: &DocumentGraphics) -> Result<(Vec<u8>, Vec<u8>)> {
        let slots: Vec<MaterialSlot> = self.materials.clone();
        let materials_map = g.generated_grid.cells.clone();

        let bounds = g.generated_grid.bounds;
        let width = bounds[2] - bounds[0];
        let height = bounds[3] - bounds[1];

        let map_rect = [
            bounds[0] * self.cell_size,
            bounds[1] * self.cell_size,
            bounds[2] * self.cell_size,
            bounds[3] * self.cell_size,
        ];

        let mut materials_png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut materials_png, width as u32, height as u32);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(
                slots
                    .iter()
                    .flat_map(|m| m.to_material().map(|m| m.fill_color).unwrap_or([0, 0, 0]))
                    .collect(),
            );
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&materials_map)?;
        }

        let materials_json = serde_json::to_vec_pretty(&MaterialsJson { slots, map_rect })?;
        Ok((materials_png, materials_json))
    }
}

impl Grid {
    pub fn new() -> Grid {
        Grid {
            bounds: [0, 0, 0, 0],
            cells: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.bounds = [0, 0, 0, 0];
        self.cells.clear();
    }

    pub fn size(&self) -> [i32; 2] {
        [
            self.bounds[2] - self.bounds[0],
            self.bounds[3] - self.bounds[1],
        ]
    }

    pub fn find_used_bounds(&self) -> [i32; 4] {
        let mut b = self.bounds;
        for x in (b[0]..b[2]).rev() {
            let mut used = false;
            for y in self.bounds[1]..self.bounds[3] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[2] = x + 1;
        }

        for x in b[0]..b[2] {
            let mut used = false;
            for y in b[1]..b[3] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[0] = x + 1;
        }

        for y in (b[1]..b[3]).rev() {
            let mut used = false;
            for x in b[0]..b[2] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[3] = y + 1;
        }

        for y in b[1]..b[3] {
            let mut used = false;
            for x in b[0]..b[2] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[1] = y + 1;
        }

        b
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

    pub fn resize_to_include(&mut self, [l, t, r, b]: [i32; 4]) {
        if l >= self.bounds[0]
            && l < self.bounds[2]
            && t >= self.bounds[1]
            && t < self.bounds[3]
            && r >= self.bounds[0]
            && r < self.bounds[2]
            && b >= self.bounds[1]
            && b < self.bounds[3]
        {
            return;
        }
        let tile_size_cells = 64;
        let tile_l = l.div_euclid(tile_size_cells);
        let tile_t = t.div_euclid(tile_size_cells);
        let tile_r = r.div_euclid(tile_size_cells);
        let tile_b = b.div_euclid(tile_size_cells);

        let tile_bounds = [
            tile_l * tile_size_cells,
            tile_t * tile_size_cells,
            (tile_r + 1) * tile_size_cells,
            (tile_b + 1) * tile_size_cells,
        ];

        let bounds = [
            self.bounds[0].min(tile_bounds[0]),
            self.bounds[1].min(tile_bounds[1]),
            self.bounds[2].max(tile_bounds[2]),
            self.bounds[3].max(tile_bounds[3]),
        ];

        self.resize(bounds);
    }

    pub fn world_to_grid_pos(&self, point: Vec2, cell_size: i32) -> Result<[i32; 2], [i32; 2]> {
        let grid_pos = point / Vec2::splat(cell_size as f32);
        let x = grid_pos.x.floor() as i32;
        let y = grid_pos.y.floor() as i32;
        if x < self.bounds[0] || x >= self.bounds[2] || y < self.bounds[1] || y >= self.bounds[3] {
            return Err([x, y]);
        }
        Ok([x, y])
    }

    pub fn flood_fill(
        cells: &mut [u8],
        [l, t, r, b]: [i32; 4],
        [start_x, start_y]: [i32; 2],
        value: u8,
    ) {
        let w = r - l;
        let h = b - t;
        let start_x = start_x - l;
        let start_y = start_y - t;
        let old_value = cells[(start_y * w + start_x) as usize];
        if old_value == value {
            return;
        }
        let mut stack = Vec::new();
        stack.push([start_x, start_y]);
        let fill_diagonals = old_value != 0;
        while let Some([mut x, y]) = stack.pop() {
            while x >= 0 && cells[(y * w + x) as usize] == old_value {
                x -= 1;
            }
            let mut span_above = false;
            let mut span_below = false;

            if fill_diagonals && x > 0 {
                if y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                    span_above = true;
                }
                if y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                    span_above = true;
                }
            }
            x += 1;

            while x < w && cells[(y * w + x) as usize] == old_value {
                cells[(y * w + x) as usize] = value;
                if !span_above && y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                    span_above = true;
                } else if span_above && y > 0 && cells[((y - 1) * w + x) as usize] != old_value {
                    span_above = false;
                }

                if !span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                    span_below = true;
                } else if span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] != old_value
                {
                    span_below = false;
                }
                x += 1;
            }

            if fill_diagonals && x < w {
                if !span_above && y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                }
                if !span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                }
            }
        }
    }

    pub fn grid_pos_index(&self, x: i32, y: i32) -> usize {
        ((y - self.bounds[1]) * (self.bounds[2] - self.bounds[0]) + x - self.bounds[0]) as usize
    }

    pub fn rectangle_outline(&mut self, [l, t, r, b]: [i32; 4], value: u8) {
        for x in l..=r {
            let index = self.grid_pos_index(x, t);
            self.cells[index] = value;
        }

        for y in t..=b {
            let index = self.grid_pos_index(l, y);
            self.cells[index] = value;
            let index = self.grid_pos_index(r, y);
            self.cells[index] = value;
        }
        for x in l..=r {
            let index = self.grid_pos_index(x, b);
            self.cells[index] = value;
        }
    }

    pub fn rectangle_fill(&mut self, [l, t, r, b]: [i32; 4], value: u8) {
        for y in t..=b {
            for x in l..=r {
                let index = self.grid_pos_index(x, y);
                self.cells[index] = value;
            }
        }
    }

    pub fn blit(&mut self, other_grid: &Grid) {
        let ob = other_grid.bounds;
        let b = self.bounds;
        let w = b[2] - b[0];
        let ow = ob[2] - ob[0];
        for y in ob[1]..ob[3] {
            for x in ob[0]..ob[2] {
                let v = other_grid.cells[((y - ob[1]) * ow + (x - ob[0])) as usize];
                if v != 0 {
                    let new_v = if v != 255 { v } else { 0 };
                    self.cells[((y - b[1]) * w + (x - b[0])) as usize] = new_v;
                }
            }
        }
    }
}

impl View {
    pub fn screen_to_world(&self) -> Affine2 {
        self.world_to_screen().inverse()
    }

    pub fn world_to_screen(&self) -> Affine2 {
        Affine2::from_translation(
            (vec2(self.screen_width_px, self.screen_height_px) * 0.5).floor() - vec2(0.5, 0.5),
        ) * Affine2::from_scale(Vec2::splat(self.zoom))
            * Affine2::from_translation(-self.target)
    }
}

impl App {
    pub fn push_undo(&mut self, text: &str) {
        let doc_ref = self.doc.borrow();
        let doc: &Document = &doc_ref;
        let err = self.undo.push(doc, text);
        self.redo.clear();
        self.report_error(err);
    }
}

impl ChangeMask {
    pub fn mark_dirty_layer(&mut self, layer: usize) {
        let bit_index = layer.min(63);
        let bit = 1 << bit_index;
        self.cell_layers |= bit;
    }
}

impl Layer {
    pub(crate) fn label(&self) -> &'static str {
        match *self {
            Layer::Grid { .. } => "Grid",
            Layer::Tunnel { .. } => "Tunnel",
        }
    }
}
