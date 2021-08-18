use glam::Vec2;
use miniquad::{Texture, Context};
use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Grid {
    pub origin: [i32; 2],
    pub size: [i32; 2],
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