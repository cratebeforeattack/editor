use crate::font_manager::{FontKey, FontManager};
use crate::{IndexType, Render, SpriteKey};
use glam::vec2;
use realtime_drawing::MiniquadBatch;
use realtime_drawing::VertexPos3UvColor as Vertex;

pub struct MiniquadRender<'b, 'f, S>
where
    S: Fn(SpriteKey) -> miniquad::Texture,
{
    pub batch: &'b mut MiniquadBatch<Vertex>,
    pub font_manager: &'f FontManager,
    pub texture_by_key: S,
}

impl<'b, 'f, S> MiniquadRender<'b, 'f, S>
where
    S: Fn(SpriteKey) -> miniquad::Texture,
{
    pub fn new(
        batch: &'b mut MiniquadBatch<Vertex>,
        font_manager: &'f FontManager,
        texture_by_key: S,
    ) -> MiniquadRender<'b, 'f, S> {
        MiniquadRender {
            batch,
            font_manager,
            texture_by_key,
        }
    }
}

impl<'b, 'f, S> Render for MiniquadRender<'b, 'f, S>
where
    S: Fn(SpriteKey) -> miniquad::Texture,
{
    fn set_clip(&mut self, clip: Option<[i32; 4]>) {
        self.batch.set_clip(clip);
    }
    fn set_sprite(&mut self, sprite: Option<SpriteKey>) {
        let image = (self.texture_by_key)(sprite.unwrap_or(0));
        self.batch.set_image(image);
    }
    fn add_vertices(
        &mut self,
        positions: &[[f32; 2]],
        uvs: &[[f32; 2]],
        indices: &[IndexType],
        color: [u8; 4],
    ) {
        assert!(positions.len() == uvs.len());
        let (vs, is, first_vertex) =
            self.batch
                .geometry
                .allocate(positions.len(), indices.len(), Vertex::of_color(color));
        for ((v, pos), uv) in vs.iter_mut().zip(positions.iter()).zip(uvs.iter()) {
            v.pos = [pos[0], pos[1], 0.0];
            v.uv = *uv;
        }
        for (i, v) in is.iter_mut().zip(indices) {
            *i = v + first_vertex;
        }
    }
    fn draw_text(&mut self, font: FontKey, text: &str, pos: [f32; 2], color: [u8; 4], scale: f32) {
        self.font_manager
            .draw_text(self.batch, font, text, pos, color, scale);
    }

    fn draw_rounded_rect(
        &mut self,
        rect: [f32; 4],
        radius: f32,
        thickness: f32,
        outline_color: [u8; 4],
        fill_color: [u8; 4],
    ) {
        let clamped_radius = radius.min(rect[2] - rect[0]).min(rect[3] - rect[1]);
        self.batch.geometry.fill_round_rect_aa(
            vec2(rect[0], rect[1]),
            vec2(rect[2], rect[3]),
            clamped_radius,
            8,
            fill_color,
        );
        self.batch.geometry.stroke_round_rect_aa(
            vec2(rect[0], rect[1]),
            vec2(rect[2], rect[3]),
            clamped_radius,
            8,
            thickness,
            outline_color,
        );
    }
}
