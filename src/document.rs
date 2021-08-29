use glam::Vec2;
use miniquad::{Texture, Context};
use serde_derive::{Serialize, Deserialize};
use log::info;

#[derive(Serialize, Deserialize)]
pub(crate) struct Grid {
    pub bounds: [i32; 4],
    pub cell_size: i32,
    pub cells: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Document {
    pub layer: Grid,

    pub reference_path: Option<String>,
}

pub(crate) struct DocumentGraphics {
    pub outline_points: Vec<Vec<Vec2>>,
    pub reference_texture: Option<Texture>,
}


#[derive(Clone, Serialize, Deserialize)]
pub (crate) struct View {
    pub target: Vec2,
    pub zoom: f32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentLocalState {
    pub view: View
}

#[derive(Default)]
pub(crate) struct ChangeMask {
    pub cells: bool,
    pub reference_path: bool,
}

impl DocumentGraphics {
    pub(crate) fn generate(&mut self, doc: &Document, change_mask: ChangeMask, context: &mut Context) {
        if change_mask.cells {
            self.outline_points.clear();
        }

        if change_mask.reference_path {
            if let Some(tex) = self.reference_texture.take() {
                tex.delete();
            }

            if let Some(path) = &doc.reference_path {
                let (pixels, w, h) = std::fs::read(path)
                    .and_then(|e| {
                        let mut bytes_slice = e.as_slice();
                        let mut decoder = png::Decoder::new(&mut bytes_slice);
                        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::GRAY_TO_RGB);
                        let (info, mut reader) = decoder.read_info()?;
                        let mut pixels = vec![0; info.buffer_size()];
                        reader.next_frame(&mut pixels)?;
                        if info.color_type == png::ColorType::RGB {
                            let mut rgba = vec![0; info.width as usize * info.height as usize * 4];
                            for pixel_index in 0..info.width as usize * info.height as usize {
                                rgba[pixel_index * 4 + 0] = pixels[pixel_index * 3 + 0];
                                rgba[pixel_index * 4 + 1] = pixels[pixel_index * 3 + 1];
                                rgba[pixel_index * 4 + 2] = pixels[pixel_index * 3 + 2];
                                rgba[pixel_index * 4 + 3] = 255;
                            }
                            pixels = rgba;
                        }
                        Ok((pixels, info.width, info.height))
                    }).unwrap_or_else(|e| {
                        eprintln!("Failed to load image: {}", e);
                        (vec![0xff, 0x00, 0x00, 0xff], 1, 1)
                    });

                self.reference_texture = Some(Texture::from_rgba8(context, w as u16, h as u16, &pixels));
            }
        }
    }
}

impl Grid {
    pub fn size(&self) -> [i32; 2] {
        [self.bounds[2] - self.bounds[0], self.bounds[3] - self.bounds[1]]
    }

    pub fn resize(&mut self, new_bounds: [i32; 4]) {
        let old_bounds = self.bounds;
        let old_size = [old_bounds[2] - old_bounds[0], old_bounds[3] - old_bounds[1]];
        let new_size = [new_bounds[2] - new_bounds[0], new_bounds[3] - new_bounds[1]];
        let mut new_cells = vec![0u8; new_size[0] as usize * new_size[1] as usize];
        let offset = [new_bounds[0] - old_bounds[0], new_bounds[1] - old_bounds[1]];
        let y_range = old_bounds[1].max(new_bounds[1])..old_bounds[3].min(new_bounds[3]);
        let x_range = old_bounds[0].max(new_bounds[0])..old_bounds[2].min(new_bounds[2]);
        for y in y_range {
            let old_start = ((y - old_bounds[1]) * old_size[0] + (x_range.start - old_bounds[0])) as usize;
            let new_start = ((y - new_bounds[1]) * new_size[0] + (x_range.start - new_bounds[0])) as usize;
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

        let old_tile_bounds = [
            self.bounds[0].div_euclid(tile_size_cells),
            self.bounds[1].div_euclid(tile_size_cells),
            self.bounds[2].div_euclid(tile_size_cells),
            self.bounds[3].div_euclid(tile_size_cells)
        ];

        let tile_bounds = [
            tile_x.min(old_tile_bounds[0]),
            tile_y.min(old_tile_bounds[1]),
            tile_x.max(old_tile_bounds[0]),
            tile_y.max(old_tile_bounds[1]),
        ];

        let bounds = [
            tile_bounds[0] * tile_size_cells,
            tile_bounds[1] * tile_size_cells,
            (tile_bounds[2] + 1) * tile_size_cells,
            (tile_bounds[3] + 1) * tile_size_cells,
        ];

        self.resize(bounds);
    }

}