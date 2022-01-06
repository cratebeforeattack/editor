use glam::{ivec2, vec2, IVec2, Vec2};
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, FilterMode, PassAction,
    Pipeline, PipelineParams, RenderPass, Shader, ShaderMeta, Texture, TextureFormat,
    TextureParams, TextureWrap, UniformBlockLayout, UniformDesc, UniformType, VertexAttribute,
    VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use zerocopy::AsBytes;

use cbmap::{BuiltinMaterial, Material, MaterialSlot};

use crate::app::ShaderUniforms;
use crate::document::{ChangeMask, Document, LayerContent, View};
use crate::field::Field;
use crate::grid::Grid;
use crate::math::Rect;
use crate::profiler::Profiler;
use crate::sdf::distance_transform;
use crate::some_or;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelExtend,
    ParallelIterator,
};
use rayon::slice::{ParallelSlice, ParallelSliceMut};
use std::collections::{HashMap, HashSet};
use std::f32::consts::SQRT_2;
use std::iter::repeat;
use std::mem::replace;
use tracy_client::{finish_continuous_frame, span, start_noncontinuous_frame};

pub struct VertexBatch {
    value: u8,
    vertices: Vec<Vec2>,
    colors: Vec<[u8; 4]>,
}

pub struct OutlineBatch {
    points: Vec<Vec2>,
    value: u8,
    closed: bool,
}

pub struct DocumentGraphics {
    pub generated_grid: Grid<u8>,
    pub generated_distances: Field,
    pub distance_textures: Vec<HashMap<(i32, i32), Texture>>,

    pub materials: Vec<MaterialSlot>,
    pub resolved_materials: Vec<Material>,

    pub reference_texture: Option<Texture>,
}

impl DocumentGraphics {
    pub(crate) fn generate(
        &mut self,
        doc: &Document,
        change_mask: ChangeMask,
        is_export: bool,
        mut context: Option<&mut Context>,
        profiler: &mut Profiler,
    ) {
        start_noncontinuous_frame!("generate");
        let span = span!("DocumentGraphics::generate");
        if change_mask.cell_layers != 0 {
            self.generate_cells(doc, change_mask.cell_layers, is_export, profiler);
            self.materials = doc.materials.clone();
            self.resolved_materials = doc
                .materials
                .iter()
                .map(|m| {
                    m.to_material().unwrap_or_else(|| Material {
                        fill_color: [255, 0, 0],
                        outline_color: [255, 0, 0],
                        custom_name: String::new(),
                    })
                })
                .collect();

            if let Some(context) = &mut context {
                while self.distance_textures.len() < self.generated_distances.materials.len() {
                    self.distance_textures.push(Default::default());
                }
                for (material, tiles) in self.generated_distances.materials.iter().enumerate() {
                    let mut unused_tiles = self.distance_textures[material]
                        .keys()
                        .copied()
                        .collect::<HashSet<_>>();

                    for (&tile_key, tile) in tiles {
                        unused_tiles.remove(&tile_key);
                        let bytes_slice = tile.as_bytes();
                        let w = self.generated_distances.tile_size as u32;
                        let h = self.generated_distances.tile_size as u32;
                        let texture_params = TextureParams {
                            format: TextureFormat::Alpha32F,
                            wrap: TextureWrap::Clamp,
                            filter: FilterMode::Linear,
                            width: w,
                            height: h,
                            ..Default::default()
                        };

                        let _span = span!("texture update");

                        while material >= self.distance_textures.len() {
                            self.distance_textures.push(Default::default())
                        }

                        self.distance_textures[material]
                            .entry(tile_key)
                            .and_modify(|tex| {
                                if tex.width == w && tex.height == h {
                                    tex.update(context, bytes_slice);
                                } else {
                                    tex.delete();
                                    *tex = Texture::from_data_and_format(
                                        context,
                                        bytes_slice,
                                        texture_params,
                                    );
                                }
                            })
                            .or_insert_with(|| {
                                Texture::from_data_and_format(context, bytes_slice, texture_params)
                            });
                    }

                    for tile_key in unused_tiles {
                        if let Some(tex) = self.distance_textures[material].remove(&tile_key) {
                            tex.delete();
                        }
                    }
                }
            }
        }

        if change_mask.reference_path {
            self.generate_reference(doc, context)
        }
        finish_continuous_frame!("generate");
    }

    fn generate_reference(&mut self, doc: &Document, mut context: Option<&mut Context>) {
        if let Some(tex) = self.reference_texture.take() {
            tex.delete();
        }

        if let Some(path) = &doc.reference_path {
            if let Some(context) = &mut context {
                let (pixels, w, h) = std::fs::read(path)
                    .and_then(|e| {
                        let mut bytes_slice = e.as_slice();
                        let mut decoder = png::Decoder::new(&mut bytes_slice);
                        decoder.set_transformations(
                            png::Transformations::EXPAND | png::Transformations::GRAY_TO_RGB,
                        );
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
                    })
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to load image: {}", e);
                        (vec![0xff, 0x00, 0x00, 0xff], 1, 1)
                    });

                let texture = Texture::from_rgba8(context, w as u16, h as u16, &pixels);
                texture.set_filter(context, FilterMode::Nearest);
                self.reference_texture = Some(texture);
            }
        }
    }

    fn generate_cells(
        &mut self,
        doc: &Document,
        layer_mask: u64,
        is_export: bool,
        mut profiler: &mut Profiler,
    ) {
        let _span = span!("DocumentGraphics::generate_cells");
        profiler.open_block("generate_cells");

        let cell_size = doc.cell_size;
        let mut generated_bitmap = replace(&mut self.generated_grid, Grid::new(0));
        let mut generated_distances = replace(&mut self.generated_distances, Field::new());

        if layer_mask == u64::MAX {
            generated_bitmap.clear();

            for grid in &mut generated_distances.materials {
                grid.clear();
            }
        } else {
            generated_bitmap.cells.fill(0);

            for grid in &mut generated_distances.materials {
                grid.clear();
            }
        }

        while generated_distances.materials.len() < doc.materials.len() {
            generated_distances.materials.push(Default::default());
        }

        for layer in &doc.layers {
            if layer.hidden && !is_export {
                continue;
            }
            profiler.open_block(layer.label());
            match layer.content {
                LayerContent::Graph(graph_key) => {
                    let _span = span!("LayerContent::Graph");
                    if let Some(graph) = doc.graphs.get(graph_key) {
                        let mut field = Field::new();
                        for i in 0..doc.materials.len() {
                            field.materials.push(Default::default());
                        }
                        graph.render_distances(&mut field, cell_size / 2);
                        generated_distances.compose(&field);
                    }
                }
                LayerContent::Grid(grid_key) => {
                    let _span = span!("LayerContent::Graph");
                    if let Some(grid) = doc.grids.get(grid_key) {
                        let mut field = Field::new();
                        field.materials.push(Default::default());
                        // field
                        //     .materials
                        //     .par_extend((1..doc.materials.len()).into_par_iter().map(
                        //         |material_index| {
                        //             let w = grid.bounds[1].x - grid.bounds[0].x;
                        //             let h = grid.bounds[1].y - grid.bounds[0].y;
                        //
                        //             let (mut distances, neg_distances) = rayon::join(
                        //                 || {
                        //                     distance_transform(2 * w as u32, 2 * h as u32, |i| {
                        //                         let x = (i as i32 % (w * 2)) / 2;
                        //                         let y = (i as i32 / (w * 2)) / 2;
                        //                         grid.cells[(y * w + x) as usize]
                        //                             == material_index as u8
                        //                     })
                        //                 },
                        //                 || {
                        //                     distance_transform(2 * w as u32, 2 * h as u32, |i| {
                        //                         let x = (i as i32 % (w * 2)) / 2;
                        //                         let y = (i as i32 / (w * 2)) / 2;
                        //                         grid.cells[(y * w + x) as usize]
                        //                             != material_index as u8
                        //                     })
                        //                 },
                        //             );
                        //             for (d, neg) in
                        //                 distances.iter_mut().zip(neg_distances.iter().cloned())
                        //             {
                        //                 if neg > 0.0 && neg < f32::MAX {
                        //                     *d = d.min(-neg);
                        //                 }
                        //             }
                        //
                        //             Grid::<f32> {
                        //                 default_value: f32::MAX,
                        //                 bounds: [grid.bounds[0] * 2, grid.bounds[1] * 2],
                        //                 cells: distances
                        //                     .into_iter()
                        //                     .map(|v| v * doc.cell_size as f32 * 0.25)
                        //                     .collect(),
                        //             }
                        //         },
                        //     ));
                        generated_distances.compose(&field);
                    }
                }
                LayerContent::Field(field_key) => {
                    let _span = span!("LayerContent::Graph");
                    if let Some(field) = doc.fields.get(field_key) {
                        generated_distances.compose(field);
                    }
                }
            }
            profiler.close_block();
        }

        self.generated_grid = generated_bitmap;
        self.generated_distances = generated_distances;

        let _span = span!("used_materials");
        profiler.open_block("used_materials");
        let used_materials: u64 = self
            .generated_grid
            .cells
            .par_chunks(self.generated_grid.bounds.size().x.max(1) as usize)
            .with_min_len(64)
            .map(|chunk| {
                let _span = span!("material chunk");
                chunk
                    .iter()
                    .fold(0u64, |mut materials: u64, value: &u8| -> u64 {
                        if *value != 0 {
                            materials |= 1 << (*value - 1)
                        }
                        materials
                    })
            })
            .fold(
                || 0u64,
                |mut a, b| {
                    a |= b;
                    a
                },
            )
            .reduce(
                || 0u64,
                |mut a, b| {
                    let _span = span!("reduce");
                    a |= b;
                    a
                },
            );
        profiler.close_block();

        profiler.close_block();
    }

    pub(crate) fn draw(
        &self,
        batch: &mut MiniquadBatch<VertexPos3UvColor>,
        view: &View,
        white_texture: Texture,
        finish_texture: Texture,
    ) {
        let _span = span!("DocumentGraphics::draw");
        let world_to_screen_scale = view.zoom;
        let world_to_screen = view.world_to_screen();
        let outline_thickness = 1.0;

        batch.set_image(white_texture);
    }

    pub fn render_map_image(
        &self,
        doc: &Document,
        white_texture: Texture,
        finish_texture: Texture,
        _pipeline: Pipeline,
        context: &mut Context,
    ) -> (Vec<u8>, [i32; 4]) {
        let _span = span!("DocumentGraphics::render_map_image");

        let pipeline = create_pipeline(context);

        let bounds = self.generated_grid.bounds;

        let margin = 2;
        let pixel_bounds = [
            bounds[0].x * doc.cell_size - margin,
            bounds[0].y * doc.cell_size - margin,
            bounds[1].x * doc.cell_size + margin,
            bounds[1].y * doc.cell_size + margin,
        ];

        let map_width = (pixel_bounds[2] - pixel_bounds[0]) as usize;
        let map_height = (pixel_bounds[3] - pixel_bounds[1]) as usize;

        let center = vec2(
            (pixel_bounds[2] + pixel_bounds[0]) as f32 * 0.5,
            (pixel_bounds[3] + pixel_bounds[1]) as f32 * 0.5,
        )
        .floor();

        let color_texture = Texture::new_render_texture(
            context,
            TextureParams {
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Nearest,
                width: map_width as _,
                height: map_height as _,
            },
        );

        let render_pass = RenderPass::new(context, color_texture, None);
        context.begin_pass(
            render_pass,
            PassAction::Clear {
                color: Some((0.0, 0.0, 0.0, 0.0)),
                depth: None,
                stencil: None,
            },
        );

        let mut batch = MiniquadBatch::new();

        batch.begin_frame();
        batch.clear();
        batch.set_image(white_texture);

        // actual map drawing
        let view = View {
            target: center,
            zoom: 1.0,
            zoom_target: 1.0,
            zoom_velocity: 0.0,
            screen_width_px: map_width as f32,
            screen_height_px: map_height as f32,
        };
        batch.set_image(white_texture);
        self.draw(&mut batch, &view, white_texture, finish_texture);

        context.apply_pipeline(&pipeline);
        context.apply_uniforms(&ShaderUniforms {
            screen_size: [map_width as f32, map_height as f32],
        });

        batch.flush(Some((map_width as f32, map_height as f32)), context);

        context.end_render_pass();

        let mut pixels = vec![0u8; map_width * map_height * 4];
        color_texture.read_pixels(&mut pixels);
        color_texture.delete();

        let mut flipped_pixels = vec![];
        for y in 0..map_height {
            let start = (map_height - y - 1) * map_width * 4;
            flipped_pixels.extend(&pixels[start..start + map_width * 4]);
        }

        (flipped_pixels, pixel_bounds)
    }
}

pub fn intersect_segment_segment(a: [Vec2; 2], b: [Vec2; 2]) -> Option<f32> {
    let v1 = a[0] - b[0];
    let v2 = b[1] - b[0];
    let v3 = (a[1] - a[0]).perp();
    let denom = v2.dot(v3);
    if denom.abs() < 0.00001 {
        return None;
    }
    let t2 = v1.dot(v3) / denom;
    let eps = 0.0;
    if t2 < 0.0 - eps || t2 > 1.0 + eps {
        return None;
    }
    let t = v2.perp_dot(v1) / denom;
    if t < 0.0 || t > 1.0 {
        return None;
    }
    Some(t)
}

pub fn create_pipeline(ctx: &mut Context) -> Pipeline {
    let vertex_shader = r#"#version 100
            attribute vec2 pos;
            attribute vec2 uv;
            attribute vec4 color;
            uniform vec2 ;
            uniform vec2 screen_size;
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            void main() {
                gl_Position = vec4((pos / screen_size * 2.0 - 1.0) * vec2(1.0, -1.0), 0, 1);
                v_uv = uv;
                v_color = color / 255.0;
            }"#;
    let fragment_shader = r#"#version 100
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            uniform sampler2D tex;
            void main() {
                gl_FragColor = v_color * texture2D(tex, v_uv);
            }"#;
    let shader = Shader::new(
        ctx,
        vertex_shader,
        fragment_shader,
        ShaderMeta {
            images: vec!["tex".to_owned()],
            uniforms: UniformBlockLayout {
                // describes struct ShaderUniforms
                uniforms: vec![UniformDesc::new("screen_size", UniformType::Float2)],
            },
        },
    )
    .unwrap();

    let pipeline = Pipeline::with_params(
        ctx,
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("pos", VertexFormat::Float3),
            VertexAttribute::new("uv", VertexFormat::Float2),
            VertexAttribute::new("color", VertexFormat::Byte4),
        ],
        shader,
        PipelineParams {
            alpha_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            color_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            ..Default::default()
        },
    );
    pipeline
}

pub fn create_pipeline_sdf(ctx: &mut Context) -> Pipeline {
    let vertex_shader = r#"#version 100
            attribute vec2 pos;
            attribute vec2 uv;
            attribute vec4 color;
            uniform vec2 ;
            uniform vec2 screen_size;
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            void main() {
                gl_Position = vec4((pos / screen_size * 2.0 - 1.0) * vec2(1.0, -1.0), 0, 1);
                v_uv = uv;
                v_color = color / 255.0;
            }"#;
    let fragment_shader = r#"#version 100
            precision lowp float;    
            varying lowp vec2 v_uv;
            varying lowp vec4 v_color;
            uniform sampler2D tex;
            uniform vec4 fill_color;
            uniform vec4 outline_color;            
            uniform float pixel_size;
            float outline_mask(float d, float width) {
                float alpha1 = clamp(d + 0.5 + width * 0.5, 0.0, 1.0);
                float alpha2 = clamp(d + 0.5 - width * 0.5, 0.0, 1.0);
                return alpha1 - alpha2;
            }
            vec4 alpha_over(vec4 below, vec4 above) {
                return clamp((above + below * (1.0 - above.a)), vec4(0.0), vec4(1.0));
            }
            vec4 pma(vec4 non_pma) {
                return vec4(non_pma.rgb * non_pma.a, non_pma.a);
            }
            void main() {
                float d = texture2D(tex, v_uv).x; 
                float a = clamp(d / pixel_size, 0.0, 1.0);
                vec4 color = vec4(0.0);
                color = alpha_over(color, pma(v_color * outline_color * vec4(vec3(1.0), 1.0 - clamp(d / pixel_size, 0.0, 1.0))));
                color = alpha_over(color, pma(v_color * fill_color * vec4(vec3(1.0), 1.0 - clamp((d + 1.5) / pixel_size, 0.0, 1.0))));
                gl_FragColor = color;
            }"#;
    let shader = Shader::new(
        ctx,
        vertex_shader,
        fragment_shader,
        ShaderMeta {
            images: vec!["tex".to_owned()],
            uniforms: UniformBlockLayout {
                // describes struct ShaderUniforms
                uniforms: vec![
                    UniformDesc::new("fill_color", UniformType::Float4),
                    UniformDesc::new("outline_color", UniformType::Float4),
                    UniformDesc::new("screen_size", UniformType::Float2),
                    UniformDesc::new("pixel_size", UniformType::Float1),
                ],
            },
        },
    )
    .unwrap();

    let pipeline = Pipeline::with_params(
        ctx,
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("pos", VertexFormat::Float3),
            VertexAttribute::new("uv", VertexFormat::Float2),
            VertexAttribute::new("color", VertexFormat::Byte4),
        ],
        shader,
        PipelineParams {
            alpha_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            color_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            ..Default::default()
        },
    );
    pipeline
}

fn isopoint(d: [f32; 2], p: [Vec2; 2], thickness_half: f32) -> Vec2 {
    let thickness_half = 0.0;
    let d0 = d[0] - thickness_half;
    let d1 = d[0] + thickness_half;
    let f0 = (d0 / (d[1] - d[0])).abs();
    let f1 = (d1 / (d[1] - d[0])).abs();
    if f0 >= 0.0 && f0 <= 1.0 {
        p[0].lerp(p[1], f0)
    } else {
        p[0].lerp(p[1], f1)
    }
}
