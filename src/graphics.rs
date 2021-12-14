use glam::{ivec2, vec2, IVec2, Vec2};
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, FilterMode, PassAction,
    Pipeline, PipelineParams, RenderPass, Shader, ShaderMeta, Texture, TextureFormat,
    TextureParams, TextureWrap, UniformBlockLayout, UniformDesc, UniformType, VertexAttribute,
    VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};

use cbmap::{BuiltinMaterial, Material, MaterialSlot};

use crate::app::ShaderUniforms;
use crate::document::{ChangeMask, Document, LayerContent, View};
use crate::field::Field;
use crate::grid::Grid;
use crate::math::Rect;
use crate::profiler::Profiler;
use crate::sdf::distance_transform;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelExtend,
    ParallelIterator,
};
use rayon::slice::{ParallelSlice, ParallelSliceMut};
use std::collections::HashMap;
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

    pub outline_points: Vec<OutlineBatch>,
    pub outline_fill_indices: Vec<Vec<u16>>,

    pub loose_vertices: Vec<VertexBatch>,
    pub loose_indices: Vec<Vec<u16>>,
    pub materials: Vec<MaterialSlot>,
    pub resolved_materials: Vec<Material>,

    pub reference_texture: Option<Texture>,
}

#[derive(Clone, Copy)]
enum TraceTile {
    Empty,
    Fill,
    TopEdge,
    LeftEdge,
    DiagonalOuter,
    DiagonalInner,
}

fn trace_distances(
    grid: &Grid<f32>,
    cell_size: i32,
    value: u8,
) -> (Vec<OutlineBatch>, Vec<VertexBatch>, Vec<Vec<u16>>) {
    let thickness = SQRT_2 * cell_size as f32;
    let half_thickness = thickness * 0.5;
    let sample_distance = |[x, y]: [i32; 2]| -> f32 {
        if !grid.bounds.contains_point(ivec2(x, y)) {
            return f32::MAX;
        }
        let index = grid.grid_pos_index(x, y);
        grid.cells[index]
    };
    let cell_size_f = cell_size as f32;
    let mut vertex_chunks = Vec::new();
    let mut index_chunks = Vec::new();
    let mut indices = Vec::new();
    let mut edges: Vec<(Vec2, Vec2)> = Vec::new();
    let mut edge_by_side = HashMap::<(IVec2, u8), Vec2>::new();
    let mut vertices = VertexBatch {
        value,
        vertices: vec![],
        colors: vec![],
    };
    let mut add_vertices = |vs: &[Vec2], is: &[u16], color: [u8; 4]| {
        if vertices.vertices.len() >= u16::MAX as usize - vs.len() - 1
            || indices.len() >= u16::MAX as usize - is.len() - 1
            || vertices.value != value
        {
            // allocate new geometry chunk
            vertex_chunks.push(replace(
                &mut vertices,
                VertexBatch {
                    vertices: Vec::new(),
                    colors: Vec::new(),
                    value,
                },
            ));
            index_chunks.push(replace(&mut indices, Vec::new()));
        }

        let base = vertices.vertices.len() as u16;
        vertices.vertices.extend_from_slice(vs);
        vertices.colors.extend(repeat(color).take(vs.len()));
        indices.extend(is.iter().cloned().map(|i| base + i));
    };

    let disabled_bits = [/*0b1100 0b1001*/];
    let validate_edges = false;

    for y in (grid.bounds[0].y - 1)..grid.bounds[1].y {
        for x in (grid.bounds[0].x - 1)..grid.bounds[1].x {
            let positions = [
                vec2(x as f32, y as f32) * cell_size_f,
                vec2(x as f32 + 1.0, y as f32) * cell_size_f,
                vec2(x as f32 + 1.0, y as f32 + 1.0) * cell_size_f,
                vec2(x as f32, y as f32 + 1.0) * cell_size_f,
            ];

            let distances = [
                sample_distance([x, y]),
                sample_distance([x + 1, y]),
                sample_distance([x + 1, y + 1]),
                sample_distance([x, y + 1]),
            ];

            let edge_by_side = &mut edge_by_side;

            let bits = if distances[0] <= 0.0 { 1 } else { 0 }
                | if distances[1] <= 0.0 { 2 } else { 0 }
                | if distances[2] <= 0.0 { 4 } else { 0 }
                | if distances[3] <= 0.0 { 8 } else { 0 };

            let mut isopoint_i = |i0: usize, i1: usize| -> Vec2 {
                let pos = isopoint(
                    [distances[i0], distances[i1]],
                    [positions[i0], positions[i1]],
                    half_thickness,
                );

                let side = match (i0.min(i1), i0.max(i1)) {
                    (0, 1) => 0,
                    (1, 2) => 1,
                    (2, 3) => 2,
                    (0, 3) => 3,
                    _ => {
                        panic!("unexpeceted indices: {}, {}", i0, i1);
                    }
                };
                let mut i_pos = pos.as_ivec2();
                let (n_pos, n_side) = match side {
                    1 => (i_pos + ivec2(1, 0), 3),
                    2 => (i_pos + ivec2(0, 1), 0),
                    _ => (i_pos, side),
                };

                if validate_edges {
                    if let Some(existing_pos) = edge_by_side.insert((n_pos, n_side), pos) {
                        let delta = pos - existing_pos;
                        if delta != Vec2::ZERO {
                            println!(
                                "edge point delta {} ({:?} - {:?}) at {:?} bits {:04b}",
                                delta, pos, existing_pos, n_pos, bits
                            );
                        }
                    }
                }

                pos
            };

            if disabled_bits.contains(&bits) {
                continue;
            }

            match bits {
                0b0000 => {}
                0b1111 => {
                    add_vertices(&positions, &[0, 1, 2, 0, 2, 3], [255, 255, 255, 255]);
                }
                // diagonals
                0b0101 | 0b1010 => {
                    let (top_indices, bottom_indices) = match bits {
                        0b0101 => ([3, 0, 1], [1, 2, 3]),
                        0b1010 => ([0, 1, 2], [2, 3, 0]),
                        _ => unreachable!(),
                    };

                    let top_1 = isopoint_i(top_indices[1], top_indices[0]);
                    let top_2 = isopoint_i(top_indices[1], top_indices[2]);
                    let bottom_1 = isopoint_i(bottom_indices[1], bottom_indices[0]);
                    let bottom_2 = isopoint_i(bottom_indices[1], bottom_indices[2]);

                    add_vertices(
                        &[top_1, positions[top_indices[1]], top_2],
                        &[0, 1, 2],
                        [255, 128, 255, 255],
                    );
                    add_vertices(
                        &[bottom_1, positions[bottom_indices[1]], bottom_2],
                        &[0, 1, 2],
                        [255, 128, 255, 255],
                    );
                    edges.push((top_1, top_2));
                    edges.push((bottom_1, bottom_2));
                }
                // corners
                0b0001 | 0b0010 | 0b0100 | 0b1000 => {
                    let indices = match bits {
                        // one corner set
                        0b0001 => [3, 0, 1],
                        0b0010 => [0, 1, 2],
                        0b0100 => [1, 2, 3],
                        0b1000 => [2, 3, 0],
                        _ => unreachable!(),
                    };

                    let pos_1 = isopoint_i(indices[1], indices[0]);
                    let pos_2 = isopoint_i(indices[1], indices[2]);

                    let c_pos = positions[indices[1]];
                    add_vertices(&[pos_1, c_pos, pos_2], &[0, 1, 2], [128, 255, 128, 255]);
                    edges.push((pos_1, pos_2));
                }
                // 3/4
                0b1110 | 0b1101 | 0b1011 | 0b0111 => {
                    let indices = match bits {
                        0b1110 => [3, 0, 1],
                        0b1101 => [0, 1, 2],
                        0b1011 => [1, 2, 3],
                        0b0111 => [2, 3, 0],
                        _ => unreachable!(),
                    };
                    let pos_1 = isopoint_i(indices[0], indices[1]);
                    let pos_2 = isopoint_i(indices[2], indices[1]);

                    let c_pos0 = positions[(indices[1] + 1) % 4];
                    let c_pos1 = positions[(indices[1] + 2) % 4];
                    let c_pos2 = positions[(indices[1] + 3) % 4];
                    #[rustfmt::skip]
                    add_vertices(
                        &[c_pos0, c_pos1, c_pos2, pos_1, pos_2],
                        &[
                            0, 1, 2,
                            0, 2, 3,
                            0, 3, 4,
                        ],
                        [128, 128, 255, 255]
                    );
                    edges.push((pos_2, pos_1));
                }

                0b1001 | 0b0011 | 0b0110 | 0b1100 => {
                    let (indices, c_indices) = match bits {
                        0b1001 => ([[3, 2], [0, 1]], [3, 0]),
                        0b0011 => ([[0, 3], [1, 2]], [0, 1]),
                        0b0110 => ([[1, 0], [2, 3]], [1, 2]),
                        0b1100 => ([[2, 1], [3, 0]], [2, 3]),
                        _ => unreachable!(),
                    };
                    let pos_1 = isopoint_i(indices[0][0], indices[0][1]);
                    let pos_2 = isopoint_i(indices[1][0], indices[1][1]);

                    add_vertices(
                        &[
                            pos_2,
                            positions[c_indices[1]],
                            positions[c_indices[0]],
                            pos_1,
                        ],
                        &[0, 1, 2, 0, 2, 3],
                        [255, 128, 128, 255],
                    );
                    edges.push((pos_1, pos_2));
                }
                _ => {}
            }
        }
    }
    let outline_points = edges_to_outline(1.0, value, edges);

    vertex_chunks.push(vertices);
    index_chunks.push(indices);
    (outline_points, vertex_chunks, index_chunks)
}

fn edges_to_outline(scale: f32, value: u8, mut edges: Vec<(Vec2, Vec2)>) -> Vec<OutlineBatch> {
    let _span = span!("sort");
    edges.par_sort_unstable_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap()
            .then(a.1.partial_cmp(&b.1).unwrap())
    });
    drop(_span);
    let mut visited = vec![false; edges.len()];

    // construct outline
    let _span = span!("outline");
    let mut outline_points = Vec::new();
    for i in 0..visited.len() {
        if visited[i] {
            continue;
        }
        let (from, to) = edges[i];
        visited[i] = true;

        let mut path = Vec::new();
        path.push(from);
        path.push(to);
        let mut last_dir = to - from;
        let mut last_to = to;
        while let Some(to_index) = edges
            .binary_search_by(|(from, _)| from.partial_cmp(&last_to).unwrap())
            .ok()
        {
            if visited[to_index] {
                break;
            }
            visited[to_index] = true;
            let to = edges[to_index].1;

            let dir = to - last_to;
            if dir.perp_dot(last_dir) == 0.0 {
                *path.last_mut().unwrap() = to;
            } else {
                path.push(to);
            }
            last_dir = dir;
            last_to = to;
        }
        let closed = if path.last() == path.first() {
            path.pop();
            true
        } else {
            false
        };

        let path = path.into_iter().map(|v| v * scale as f32).collect();
        outline_points.push(OutlineBatch {
            points: path,
            value,
            closed,
        });
    }
    drop(_span);
    outline_points
}

impl DocumentGraphics {
    pub(crate) fn generate(
        &mut self,
        doc: &Document,
        change_mask: ChangeMask,
        is_export: bool,
        context: Option<&mut Context>,
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
                .collect()
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
                grid.cells.fill(f32::MAX);
            }
        }

        while generated_distances.materials.len() < doc.materials.len() {
            generated_distances.materials.push(Grid::new(f32::MAX))
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
                        //graph.render_cells(&mut generated_bitmap, cell_size, profiler);
                        graph.render_distances(&mut generated_distances.materials, cell_size / 2);
                    }
                }
                LayerContent::Grid(grid_key) => {
                    let _span = span!("LayerContent::Graph");
                    if let Some(grid) = doc.grids.get(grid_key) {
                        let mut field = Field::new();
                        field.materials.push(Grid::new(f32::MAX));
                        field
                            .materials
                            .par_extend((1..doc.materials.len()).into_par_iter().map(
                                |material_index| {
                                    let w = grid.bounds[1].x - grid.bounds[0].x;
                                    let h = grid.bounds[1].y - grid.bounds[0].y;

                                    let mut distances =
                                        distance_transform(2 * w as u32, 2 * h as u32, |i| {
                                            let x = (i as i32 % (w * 2)) / 2;
                                            let y = (i as i32 / (w * 2)) / 2;
                                            grid.cells[(y * w + x) as usize] == material_index as u8
                                        });

                                    let neg_distances =
                                        distance_transform(2 * w as u32, 2 * h as u32, |i| {
                                            let x = (i as i32 % (w * 2)) / 2;
                                            let y = (i as i32 / (w * 2)) / 2;
                                            grid.cells[(y * w + x) as usize] != material_index as u8
                                        });
                                    for (d, neg) in
                                        distances.iter_mut().zip(neg_distances.iter().cloned())
                                    {
                                        if neg > 0.0 && neg < f32::MAX {
                                            *d = d.min(-neg);
                                        }
                                    }

                                    Grid::<f32> {
                                        default_value: f32::MAX,
                                        bounds: [grid.bounds[0] * 2, grid.bounds[1] * 2],
                                        cells: distances,
                                    }
                                },
                            ));
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

        self.outline_fill_indices.clear();

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

        profiler.open_block("trace_grids");
        let _span = span!("trace_grids");
        let (mut outline, mut vertices, mut indices) = self
            .generated_distances
            .materials
            .par_iter()
            .enumerate()
            .map(|(index, grid)| trace_distances(grid, doc.cell_size / 2, index as u8))
            .reduce(
                || Default::default(),
                |mut acc, (b_o, b_v, b_i)| {
                    let _span = span!("reduce");
                    acc.0.extend(b_o.into_iter());
                    acc.1.extend(b_v.into_iter());
                    acc.2.extend(b_i.into_iter());
                    acc
                },
            );
        self.outline_points = outline;
        self.loose_vertices = vertices;
        self.loose_indices = indices;
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

        for (
            VertexBatch {
                vertices: loose_vertices,
                colors,
                value: material_index,
            },
            loose_indices,
        ) in self.loose_vertices.iter().zip(self.loose_indices.iter())
        {
            let material = &self.resolved_materials[*material_index as usize];
            let fill_color = [
                material.fill_color[0],
                material.fill_color[1],
                material.fill_color[2],
                255,
            ];

            let texture = match self.materials[*material_index as usize] {
                MaterialSlot::BuiltIn(BuiltinMaterial::Finish) => finish_texture,
                _ => white_texture,
            };
            batch.set_image(texture);

            let positions_screen: Vec<_> = loose_vertices
                .iter()
                .map(|p| world_to_screen.transform_point2(*p))
                .collect();
            let (vs, is, first) = batch.geometry.allocate(
                positions_screen.len(),
                loose_indices.len(),
                VertexPos3UvColor::default(),
            );
            let finish_checker_size = 16.0;
            for (((dest, pos), pos_world), v_color) in vs
                .iter_mut()
                .zip(positions_screen)
                .zip(loose_vertices.iter())
                .zip(colors.iter().chain(repeat(&[255, 255, 255, 255])))
            {
                let color = fill_color;
                // let color = [
                //     ((fill_color[0] as u16 * v_color[0] as u16) / 255) as u8,
                //     ((fill_color[1] as u16 * v_color[1] as u16) / 255) as u8,
                //     ((fill_color[2] as u16 * v_color[2] as u16) / 255) as u8,
                //     ((fill_color[3] as u16 * v_color[3] as u16) / 255) as u8,
                // ];

                *dest = VertexPos3UvColor {
                    pos: [pos.x, pos.y, 0.0],
                    uv: [
                        pos_world.x / finish_checker_size / 2.0,
                        pos_world.y / finish_checker_size / 2.0,
                    ],
                    color,
                };
            }
            for (dest, index) in is.iter_mut().zip(loose_indices) {
                *dest = index + first;
            }
        }

        batch.set_image(white_texture);

        let empty_indices = Vec::new();
        let fill_iter = self
            .outline_fill_indices
            .iter()
            .chain(std::iter::repeat_with(|| &empty_indices));
        for (
            OutlineBatch {
                points: positions,
                value: material_index,
                closed,
            },
            indices,
        ) in self.outline_points.iter().zip(fill_iter)
        {
            let material = &self.resolved_materials[*material_index as usize];
            let outline_color = [
                material.outline_color[0],
                material.outline_color[1],
                material.outline_color[2],
                255,
            ];
            let fill_color = [
                material.fill_color[0],
                material.fill_color[1],
                material.fill_color[2],
                255,
            ];
            let positions_screen: Vec<_> = positions
                .iter()
                .map(|p| world_to_screen.transform_point2(*p))
                .collect();

            let thickness = outline_thickness * world_to_screen_scale;
            batch
                .geometry
                .add_position_indices(&positions_screen, &indices, fill_color);
            batch
                .geometry
                .stroke_polyline_aa(&positions_screen, *closed, thickness, outline_color);

            if false {
                for (i, point) in positions_screen.iter().cloned().enumerate() {
                    batch
                        .geometry
                        .fill_circle_aa(point, 1.0 + i as f32, 16, [0, 255, 0, 64]);
                }
            }
        }
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
