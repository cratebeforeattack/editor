use serde_derive::{Deserialize, Serialize};
use realtime_drawing::MiniquadBatch;
use realtime_drawing::VertexPos3UvColor as Vertex;
use std::collections::HashMap;
use crate::FontContext;

pub type FontKey = usize;

#[derive(Serialize, Deserialize)]
struct Glyph {
    uv: [u16; 4],
    pre: [f32; 2],
    post: [f32; 2],
}

#[derive(Serialize, Deserialize, Default)]
pub struct Font {
    sprite: String,
    #[serde(skip)]
    texture_key: Option<FontTextureKey>,
    metrics: FontMetrics,
    char_to_glyph: HashMap<String, Glyph>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct FontMetrics {
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
}

pub struct FontManager {
    fonts: Vec<Font>,
    fonts_by_name: HashMap<String, FontKey>,
    textures: Vec<FontTexture>,
    load_file: Box<dyn Fn(&str)->Result<Vec<u8>, String>>,
}

#[derive(Copy, Clone)]
struct Word {
    start: i32,
    end: i32,
    start_x: i32,
    end_x: i32,
}

type FontTextureKey = usize;
struct FontTexture {
    pixels: Vec<u8>,
    size: [u32; 2],
    texture: Option<miniquad::Texture>,
}

impl FontManager {
    pub fn new<F>(load_file: F) -> Self
        where F: Fn(&str)->Result<Vec<u8>, String> + 'static {
        Self {
            fonts: Vec::new(),
            fonts_by_name: HashMap::default(),
            textures: Vec::new(),
            load_file: Box::new(load_file)
        }
    }
    pub fn load_font(&mut self, name: &str) -> FontKey {
        if let Some(existing_font) = self.fonts_by_name.get(name) {
            return *existing_font;
        }
        let content = (*self.load_file)(name).expect("failed to load font");
        let font = serde_json::from_slice::<Font>(&content).expect("failed to deserialize font");
        let key = self.fonts.len();
        self.fonts.push(font);
        self.fonts_by_name.insert(name.to_owned(), key);
        key
    }

    fn load_font_texture(bytes: &[u8], context: &mut miniquad::Context)->Result<FontTexture, String> {
        let mut bytes_slice = bytes;
        let decoder = png::Decoder::new(&mut bytes_slice);
        let (info, mut reader) = decoder.read_info().map_err(|e| format!("PNG read_info failed: {}", &e))?;
        let mut pixels = vec![0; info.buffer_size()];
        reader.next_frame(&mut pixels).map_err(|e| format!("Failed to read PNG frame: {}", &e))?;
        let size = [info.width as u32, info.height as u32];
        let texture = Some(miniquad::Texture::from_rgba8(context, size[0] as u16, size[1] as u16, &pixels));
        Ok(FontTexture{
            pixels,
            size,
            texture,
        })
    }

    pub fn load_textures(&mut self, context: &mut miniquad::Context) {
        for font in self.fonts.iter_mut() {
            let texture = (*self.load_file)(&font.sprite)
                .and_then(|image_bytes| Self::load_font_texture(&image_bytes, context));
            match texture {
                Ok(texture) => {
                    let texture_key = self.textures.len();
                    self.textures.push(texture);
                    font.texture_key = Some(texture_key);
                }
                Err(err) => {
                    eprintln!("Font texture loading failed: {}", err);
                }
            }
        }
    }

    pub fn font_metrics(&self, font: FontKey) -> FontMetrics {
        let font = self.fonts.get(font).expect("Invalid font key");
        font.metrics
    }

    pub fn measure_text(&self, font: FontKey, text: &str, scale: f32) -> [f32; 2] {
        let font = self.fonts.get(font).unwrap();

        let mut cur_pos = 0f32;
        let mut buf: [u8; 6] = [0u8, 0, 0, 0, 0, 0];

        for ch in text.chars() {
            if let Some(glyph) = font.char_to_glyph.get(ch.encode_utf8(&mut buf)) {
                cur_pos += glyph.pre[0] * scale;
                cur_pos += glyph.post[0] * scale;
            }
        }

        [cur_pos, font.metrics.height]
    }

    pub fn hit_character(&self, font: FontKey, text: &str, scale: f32, pos: f32) -> Option<u32> {
        let font = self.fonts.get(font).unwrap();

        let mut cur_pos = 0f32;
        let mut buf: [u8; 6] = [0u8, 0, 0, 0, 0, 0];

        if pos < 0.0 {
            return Some(0);
        }

        for (offset, ch) in text.char_indices() {
            if let Some(glyph) = font.char_to_glyph.get(ch.encode_utf8(&mut buf)) {
                let next_pos = cur_pos + glyph.pre[0] * scale + glyph.post[0] * scale;
                if pos >= cur_pos && pos < next_pos {
                    return Some(offset as u32);
                }
                cur_pos = next_pos;
            }
        }

        Some(text.len() as u32)
    }

    pub fn wrap_text(
        &self,
        wrapped_lines: &mut Vec<(i32, i32, i32)>,
        font: FontKey,
        text: &str,
        width: i32,
    ) -> i32 {
        let font = self.fonts.get(font).unwrap();
        let lines = text.split('\n');
        let mut longest_line = 0;
        for line in lines {
            let mut words = Vec::new();
            let line_start_byte = (line.as_ptr() as usize - text.as_ptr() as usize) as i32;
            let mut word = Word {
                start: line_start_byte,
                end: line_start_byte,
                start_x: 0,
                end_x: 0,
            };
            let mut was_word_part = false;

            let mut cur_pos = 0f32;
            for (pos, ch) in line.char_indices() {
                let pos = pos as i32;
                let is_word_part = ch != ' ';
                if was_word_part && !is_word_part {
                    // word end
                    word.end = pos + line_start_byte;
                    word.end_x = cur_pos.ceil() as i32;
                    words.push(word);
                }
                if !was_word_part && is_word_part {
                    // word start
                    word.start = pos + line_start_byte;
                    word.start_x = cur_pos.ceil() as i32;
                }

                // advance position
                let mut buf: [u8; 6] = [0u8, 0, 0, 0, 0, 0];
                if let Some(glyph) = font.char_to_glyph.get(ch.encode_utf8(&mut buf)) {
                    cur_pos += glyph.pre[0];
                    cur_pos += glyph.post[0];
                }
                was_word_part = is_word_part;
            }
            let last_pos = cur_pos;
            if word.end != line_start_byte + line.len() as i32 {
                word.end = line_start_byte + line.len() as i32;
                word.end_x = last_pos.ceil() as i32;
                words.push(word);
            }

            let mut line_start_x: i32 = 0;
            let mut line_end_x = line_start_x + width;
            let mut wrapped_line = (line_start_byte, line_start_byte, line_start_x);
            for word in &words {
                let line_break = word.end_x > line_end_x;
                if line_break {
                    wrapped_lines.push(wrapped_line);
                    line_start_x = word.start_x;
                    line_end_x = word.start_x + width;
                    wrapped_line.0 = word.start;
                }
                wrapped_line.1 = word.end;
                wrapped_line.2 = word.end_x - line_start_x;
                longest_line = longest_line.max(wrapped_line.2);
            }
            wrapped_line.1 = if !words.is_empty() {
                words.last().unwrap().end
            } else {
                line_start_byte
            };
            wrapped_lines.push(wrapped_line);
        }
        longest_line
    }

    pub fn draw_text(
        &self,
        batch: &mut MiniquadBatch<Vertex>,
        font: FontKey,
        text: &str,
        pos: [f32; 2],
        color: [u8; 4],
        scale: f32,
    ) {
        let font = self.fonts.get(font).expect("invalid font");
        let texture: &FontTexture = self.textures.get(font.texture_key
            .expect("Using font without a texture"))
            .expect("Textures were not created");

        let sprite_tex = texture.texture.expect("Missing texture");
        let image_height = sprite_tex.height;
        let image_width = sprite_tex.width;
        let mut cur_pos = pos;
        let mut buf: [u8; 6] = [0u8, 0, 0, 0, 0, 0];

        let sprite_uv = [0.0, 0.0, 1.0, 1.0];
        batch.set_image(sprite_tex);

        for ch in text.chars() {
            if let Some(glyph) = font.char_to_glyph.get(ch.encode_utf8(&mut buf)) {
                cur_pos[0] += glyph.pre[0] * scale;
                cur_pos[1] += glyph.pre[1] * scale;
                let w = glyph.uv[2] as f32;
                let h = glyph.uv[3] as f32;
                let x = cur_pos[0];
                let y = cur_pos[1];
                let uv = [
                    glyph.uv[0] as f32 / image_width as f32,
                    glyph.uv[1] as f32 / image_height as f32,
                    (glyph.uv[0] + glyph.uv[2]) as f32 / image_width as f32,
                    (glyph.uv[1] + glyph.uv[3]) as f32 / image_height as f32,
                ];
                let r = rect_round([x, y, x + w, y + h]);
                let uv = uv_mul(sprite_uv, uv);
                batch
                    .geometry
                    .fill_rect_uv(r, uv, color);
                cur_pos[0] += glyph.post[0] * scale;
                cur_pos[1] += glyph.post[1] * scale;
            }
        }
    }
}

fn uv_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let a_s = [a[2] - a[0], a[3] - a[1]];
    [
        a[0] + a_s[0] * b[0],
        a[1] + a_s[1] * b[1],
        a[0] + a_s[0] * b[2],
        a[1] + a_s[1] * b[3],
    ]
}

fn rect_round(r: [f32; 4]) -> [f32; 4] {
    [r[0].round(), r[1].round(), r[2].round(), r[3].round()]
}

fn rect_intersect_f(a: [f32; 4], b: [i32; 4]) -> [f32; 4] {
    let b = [b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32];
    [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]
}

impl FontContext for FontManager {
    fn load_font(&mut self, name: &str) -> FontKey {
        self.load_font(name)
    }
    fn measure_text(&self, font: FontKey, label: &str, scale: f32) -> [f32; 2] {
        self.measure_text(font, label, scale)
    }
    fn hit_character(&self, font: FontKey, label: &str, scale: f32, pos: f32) -> Option<u32> {
        self.hit_character(font, label, scale, pos)
    }
    fn font_height(&self, font: FontKey) -> f32 {
        self.font_metrics(font).height
    }
    fn font_ascent(&self, font: FontKey) -> f32 {
        self.font_metrics(font).ascent
    }
    fn font_descent(&self, font: FontKey) -> f32 {
        self.font_metrics(font).descent
    }
    fn wrap_text(
        &self,
        wrapped_lines: &mut Vec<(i32, i32, i32)>,
        font: FontKey,
        text: &str,
        width: i32,
    ) -> i32 {
        self.wrap_text(wrapped_lines, font, text, width)
    }
}
