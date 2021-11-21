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
use crate::grid::Grid;
use crate::math::Rect;
use crate::profiler::Profiler;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelExtend,
    ParallelIterator,
};
use rayon::slice::{ParallelSlice, ParallelSliceMut};
use std::iter::FromIterator;
use std::mem::{replace, take};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tracy_client::{finish_continuous_frame, message, span, start_noncontinuous_frame};

pub struct VertexBatch {
    value: u8,
    vertices: Vec<Vec2>,
}

pub struct OutlineBatch {
    value: u8,
    points: Vec<Vec2>,
}

pub struct DocumentGraphics {
    pub generated_grid: Grid<u8>,
    pub generated_distances: Vec<Grid<f32>>,

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
    let mut vertices = VertexBatch {
        value,
        vertices: vec![],
    };
    let mut add_vertices = |vs: &[Vec2], is: &[u16]| {
        if vertices.vertices.len() >= u16::MAX as usize - vs.len() - 1
            || indices.len() >= u16::MAX as usize - is.len() - 1
            || vertices.value != value
        {
            // allocate new geometry chunk
            vertex_chunks.push(replace(
                &mut vertices,
                VertexBatch {
                    vertices: Vec::new(),
                    value,
                },
            ));
            index_chunks.push(replace(&mut indices, Vec::new()));
        }

        let base = vertices.vertices.len() as u16;
        vertices.vertices.extend_from_slice(vs);
        indices.extend(is.iter().cloned().map(|i| base + i));
    };

    let disabled_bits = [/*0b1100 0b1001*/];

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
            let bits = if distances[0] <= 0.0 { 1 } else { 0 }
                | if distances[1] <= 0.0 { 2 } else { 0 }
                | if distances[2] <= 0.0 { 4 } else { 0 }
                | if distances[3] <= 0.0 { 8 } else { 0 };

            if disabled_bits.contains(&bits) {
                continue;
            }

            match bits {
                0b0000 => {}
                0b1111 => {
                    add_vertices(&positions, &[0, 1, 2, 0, 2, 3]);
                }
                // diagonals
                0b0101 => {}
                0b1010 => {}
                // corners
                0b0001 | 0b0010 | 0b0100 | 0b1000 => {
                    let (h_indices, v_indices, c_index, order) = match bits {
                        // one corner set
                        0b0001 => ([0, 1], [0, 3], 0, [0, 1, 2]),
                        0b0010 => ([1, 0], [1, 2], 1, [0, 2, 1]),
                        0b0100 => ([2, 3], [2, 1], 2, [0, 1, 2]),
                        0b1000 => ([3, 2], [3, 0], 3, [0, 2, 1]),
                        _ => unreachable!(),
                    };

                    let f_x = distances[h_indices[0]].abs()
                        / (distances[h_indices[1]] - distances[h_indices[0]]);
                    let f_y = distances[v_indices[0]].abs()
                        / (distances[v_indices[1]] - distances[v_indices[0]]);
                    let x_pos = positions[h_indices[0]].lerp(positions[h_indices[1]], f_x);
                    let y_pos = positions[v_indices[0]].lerp(positions[v_indices[1]], f_y);
                    let c_pos = positions[c_index];
                    add_vertices(&[c_pos, x_pos, y_pos], &order);
                    edges.push(if (h_indices[0] & 1) == 0 {
                        (x_pos, y_pos)
                    } else {
                        (y_pos, x_pos)
                    })
                }
                0b1110 | 0b1101 | 0b1011 | 0b0111 => {
                    let (h_indices, v_indices, c_index, order) = match bits {
                        // three corners set
                        0b1110 => ([1, 0], [3, 0], 0, [0, 1, 2, 4, 3]),
                        0b1101 => ([0, 1], [2, 1], 1, [0, 1, 2, 3, 4]),
                        0b1011 => ([3, 2], [1, 2], 2, [0, 1, 2, 4, 3]),
                        0b0111 => ([2, 3], [0, 3], 3, [0, 1, 2, 3, 4]),
                        _ => unreachable!(),
                    };
                    let f_x = distances[h_indices[0]].abs()
                        / (distances[h_indices[1]] - distances[h_indices[0]]);
                    let f_y = distances[v_indices[0]].abs()
                        / (distances[v_indices[1]] - distances[v_indices[0]]);
                    let x_pos = positions[h_indices[0]].lerp(positions[h_indices[1]], f_x);
                    let y_pos = positions[v_indices[0]].lerp(positions[v_indices[1]], f_y);
                    let c_pos0 = positions[(c_index + 1) % 4];
                    let c_pos1 = positions[(c_index + 2) % 4];
                    let c_pos2 = positions[(c_index + 3) % 4];
                    #[rustfmt::skip]
                    add_vertices(
                        &[c_pos0, c_pos1, c_pos2, x_pos, y_pos],
                        &[
                            order[0], order[1], order[2],
                            order[0], order[2], order[3],
                            order[0], order[3], order[4],
                        ],
                    );
                    edges.push(if (h_indices[0] & 1) == 1 {
                        (x_pos, y_pos)
                    } else {
                        (y_pos, x_pos)
                    })
                }

                0b1001 | 0b0011 | 0b0110 | 0b1100 => {
                    let (indices, c_indices) = match bits {
                        0b1001 => ([[0, 1], [3, 2]], [0, 3]),
                        0b0011 => ([[0, 3], [1, 2]], [0, 1]),
                        0b0110 => ([[1, 0], [2, 3]], [1, 2]),
                        0b1100 => ([[2, 1], [3, 0]], [2, 3]),
                        _ => unreachable!(),
                    };
                    let f_1 = distances[indices[0][0]].abs()
                        / (distances[indices[0][1]] - distances[indices[0][0]]);
                    let f_2 = distances[indices[1][0]].abs()
                        / (distances[indices[1][1]] - distances[indices[1][0]]);
                    let pos1 = positions[indices[0][0]].lerp(positions[indices[0][1]], f_1);
                    let pos2 = positions[indices[1][0]].lerp(positions[indices[1][1]], f_2);
                    add_vertices(
                        &[pos2, positions[c_indices[1]], positions[c_indices[0]], pos1],
                        &[0, 1, 2, 0, 2, 3],
                    );
                    edges.push((pos1, pos2));
                }
                _ => {}
            }
        }
    }
    let outline_points = edges_to_outline(cell_size, value, edges);

    vertex_chunks.push(vertices);
    index_chunks.push(indices);
    (outline_points, vertex_chunks, index_chunks)
}

fn trace_grid(
    grid: &Grid<u8>,
    cell_size: i32,
    value: u8,
    mut profiler: Option<&mut Profiler>,
) -> (Vec<OutlineBatch>, Vec<VertexBatch>, Vec<Vec<u16>>) {
    let _span = span!("trace_grid");
    let bounds = grid.bounds;
    let w = bounds.size().x;

    let sample_or_zero = |[x, y]: [i32; 2]| -> bool {
        if !bounds.contains_point(ivec2(x, y)) {
            return false;
        }
        let i = x - bounds[0].x;
        let j = y - bounds[0].y;
        grid.cells[(j * w + i) as usize] == value
    };
    let sample_bits = |x: i32, y: i32, orientation: i32| -> u8 {
        let offset = {
            match orientation {
                0 => [[-1, -1], [0, -1], [0, 0], [-1, 0]],
                1 => [[1, -1], [1, 0], [0, 0], [0, -1]],
                2 => [[1, 1], [0, 1], [0, 0], [1, 0]],
                _ => [[-1, 1], [-1, 0], [0, 0], [0, 1]],
            }
        };
        let coords = [
            [x + offset[0][0], y + offset[0][1]],
            [x + offset[1][0], y + offset[1][1]],
            [x + offset[2][0], y + offset[2][1]],
            [x + offset[3][0], y + offset[3][1]],
        ];
        (if sample_or_zero(coords[0]) { 1 << 3 } else { 0 })
            | (if sample_or_zero(coords[1]) { 1 << 2 } else { 0 })
            | (if sample_or_zero(coords[2]) { 1 << 1 } else { 0 })
            | (if sample_or_zero(coords[3]) { 1 << 0 } else { 0 })
    };

    let orientation_offsets = [[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]];

    let cell_size_f = cell_size as f32;

    let y_range = (bounds[0].y - 1)..bounds[1].y;
    let num_chunks = 16i32;
    let chunk_len = (y_range.end - y_range.start + num_chunks - 1) / num_chunks;

    let mut profile = |n| {
        if let Some(p) = &mut profiler {
            if let Some(n) = n {
                p.open_block(n);
            } else {
                p.close_block();
            }
        }
    };

    profile(Some("chunks"));
    let (vertices, indices, mut edges) = (0..num_chunks)
        .into_par_iter()
        .map(move |chunk| {
            let _span = span!("chunk");
            let y_start = y_range.start + chunk_len * chunk;
            let y_end = (y_start + chunk_len).min(y_range.end);
            let y_chunk = y_start..y_end;
            let mut edges: Vec<(Vec2, Vec2)> = Vec::new();
            let mut vertices = VertexBatch {
                value,
                vertices: vec![],
            };
            let mut vertex_chunks = Vec::new();
            let mut index_chunks = Vec::new();
            let mut indices = Vec::new();
            let mut add_vertices = |vs: &[Vec2], is: &[u16]| {
                if vertices.vertices.len() >= u16::MAX as usize - vs.len() - 1
                    || indices.len() >= u16::MAX as usize - is.len() - 1
                    || vertices.value != value
                {
                    // allocate new geometry chunk
                    vertex_chunks.push(replace(
                        &mut vertices,
                        VertexBatch {
                            vertices: Vec::new(),
                            value,
                        },
                    ));
                    index_chunks.push(replace(&mut indices, Vec::new()));
                }

                let base = vertices.vertices.len() as u16;
                vertices.vertices.extend_from_slice(vs);
                indices.extend(is.iter().cloned().map(|i| base + i));
            };

            for y in y_chunk {
                for x in bounds[0].x..bounds[1].x + 1 {
                    let mut tiles = [TraceTile::Empty; 4];
                    for (orientation, tile) in tiles.iter_mut().enumerate() {
                        let wall_bits = sample_bits(x, y, orientation as i32);
                        *tile = match wall_bits {
                            0b0110 | 0b0011 | 0b0111 | 0b1011 | 0b1110 | 0b1010 | 0b1111
                            | 0b0010 => TraceTile::Fill,
                            0b1100 | 0b0100 => TraceTile::TopEdge,
                            0b0001 | 0b1001 => TraceTile::LeftEdge,
                            0b1101 | 0b0101 => TraceTile::DiagonalOuter,
                            0b0000 | 0b1000 | _ => TraceTile::Empty,
                        };

                        *tile = match wall_bits {
                            0b0110 | 0b0011 | 0b0111 | 0b1011 | 0b1110 | 0b1010 | 0b1111 => {
                                TraceTile::Fill
                            }
                            0b1100 => TraceTile::TopEdge,
                            0b1001 => TraceTile::LeftEdge,
                            0b1101 | 0b0101 => TraceTile::DiagonalOuter,
                            0b0010 => TraceTile::DiagonalInner,
                            0b0000 | 0b1000 | _ => TraceTile::Empty,
                        };
                    }
                    use TraceTile::*;
                    match tiles {
                        [Empty, Empty, Empty, Empty] => {}
                        [Fill, Fill, Fill, Fill] => {
                            add_vertices(
                                &[
                                    vec2(x as f32, y as f32) * cell_size_f,
                                    vec2((x + 1) as f32, y as f32) * cell_size_f,
                                    vec2((x + 1) as f32, (y + 1) as f32) * cell_size_f,
                                    vec2(x as f32, (y + 1) as f32) * cell_size_f,
                                ],
                                &[0, 1, 2, 0, 2, 3],
                            );
                        }
                        tiles @ _ => {
                            for (orientation, tile) in tiles.iter().enumerate() {
                                match tile {
                                    TraceTile::Fill => {
                                        let [x_offset, y_offset] = orientation_offsets[orientation];
                                        let x = x as f32 + x_offset;
                                        let y = y as f32 + y_offset;
                                        add_vertices(
                                            &[
                                                vec2(x, y) * cell_size_f,
                                                vec2(x + 0.5, y) * cell_size_f,
                                                vec2(x + 0.5, y + 0.5) * cell_size_f,
                                                vec2(x, y + 0.5) * cell_size_f,
                                            ],
                                            &[0, 1, 2, 0, 2, 3],
                                        );
                                    }

                                    TraceTile::DiagonalOuter => {
                                        let (from, to) = match orientation {
                                            0 => ([x * 2, y * 2 + 1], [x * 2 + 1, y * 2]),
                                            1 => ([x * 2 + 1, y * 2], [(x + 1) * 2, y * 2 + 1]),
                                            2 => {
                                                ([(x + 1) * 2, y * 2 + 1], [x * 2 + 1, (y + 1) * 2])
                                            }
                                            _ => ([x * 2 + 1, (y + 1) * 2], [x * 2, y * 2 + 1]),
                                        };
                                        edges.push((
                                            IVec2::from(from).as_vec2(),
                                            IVec2::from(to).as_vec2(),
                                        ));

                                        let [x_offset, y_offset] = orientation_offsets[orientation];
                                        let x = x as f32 + x_offset;
                                        let y = y as f32 + y_offset;

                                        add_vertices(
                                            &[
                                                vec2(x, y) * cell_size_f,
                                                vec2(x + 0.5, y) * cell_size_f,
                                                vec2(x + 0.5, y + 0.5) * cell_size_f,
                                                vec2(x, y + 0.5) * cell_size_f,
                                            ],
                                            &match orientation {
                                                0 => [3, 0, 1],
                                                1 => [0, 1, 2],
                                                2 => [1, 2, 3],
                                                _ => [2, 3, 0],
                                            },
                                        );
                                    }
                                    TraceTile::DiagonalInner => {
                                        let (from, to) = match orientation {
                                            0 => ([x * 2 + 1, y * 2], [x * 2, y * 2 + 1]),
                                            1 => ([(x + 1) * 2, y * 2 + 1], [x * 2 + 1, y * 2]),
                                            2 => {
                                                ([x * 2 + 1, (y + 1) * 2], [(x + 1) * 2, y * 2 + 1])
                                            }
                                            _ => ([x * 2, y * 2 + 1], [x * 2 + 1, (y + 1) * 2]),
                                        };
                                        edges.push((
                                            IVec2::from(from).as_vec2(),
                                            IVec2::from(to).as_vec2(),
                                        ));

                                        let [x_offset, y_offset] = orientation_offsets[orientation];
                                        let x = x as f32 + x_offset;
                                        let y = y as f32 + y_offset;

                                        add_vertices(
                                            &[
                                                vec2(x, y) * cell_size_f,
                                                vec2(x + 0.5, y) * cell_size_f,
                                                vec2(x + 0.5, y + 0.5) * cell_size_f,
                                                vec2(x, y + 0.5) * cell_size_f,
                                            ],
                                            &match orientation {
                                                0 => [1, 2, 3],
                                                1 => [2, 3, 0],
                                                2 => [3, 0, 1],
                                                _ => [0, 1, 2],
                                            },
                                        );
                                    }
                                    TraceTile::Empty => {}
                                    TraceTile::LeftEdge => {
                                        let (from, to) = match orientation {
                                            0 => ([x * 2, y * 2 + 1], [x * 2, y * 2]),
                                            1 => ([x * 2 + 1, y * 2], [(x + 1) * 2, y * 2]),
                                            2 => (
                                                [(x + 1) * 2, y * 2 + 1],
                                                [(x + 1) * 2, (y + 1) * 2],
                                            ),
                                            _ => ([x * 2 + 1, (y + 1) * 2], [x * 2, (y + 1) * 2]),
                                        };
                                        edges.push((
                                            IVec2::from(from).as_vec2(),
                                            IVec2::from(to).as_vec2(),
                                        ));
                                    }
                                    TraceTile::TopEdge => {
                                        let (from, to) = match orientation {
                                            0 => ([x * 2, y * 2], [x * 2 + 1, y * 2]),
                                            1 => ([(x + 1) * 2, y * 2], [(x + 1) * 2, y * 2 + 1]),
                                            2 => (
                                                [(x + 1) * 2, (y + 1) * 2],
                                                [x * 2 + 1, (y + 1) * 2],
                                            ),
                                            _ => ([x * 2, (y + 1) * 2], [x * 2, y * 2 + 1]),
                                        };
                                        edges.push((
                                            IVec2::from(from).as_vec2(),
                                            IVec2::from(to).as_vec2(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            vertex_chunks.push(vertices);
            index_chunks.push(indices);
            (vertex_chunks, index_chunks, edges)
        })
        .fold(
            || (Vec::new(), Vec::new(), Vec::new()),
            |mut acc, (vertices, indices, edges)| {
                let _span = span!("fold");
                acc.0.extend(vertices.into_iter());
                acc.1.extend(indices.into_iter());
                acc.2.extend(edges);
                acc
            },
        )
        .reduce(
            || (Vec::new(), Vec::new(), Vec::new()),
            |mut acc, (vertices, indices, edges)| {
                let _span = span!("reduce");
                acc.0.extend(vertices.into_iter());
                acc.1.extend(indices.into_iter());
                acc.2.extend(edges);
                acc
            },
        );
    profile(None);

    let outline_points = edges_to_outline(cell_size, value, edges);

    (outline_points, vertices, indices)
}

fn edges_to_outline(cell_size: i32, value: u8, mut edges: Vec<(Vec2, Vec2)>) -> Vec<OutlineBatch> {
    let _span = span!("sort");
    edges.par_sort_unstable_by(|a, b| {
        a.0.y
            .partial_cmp(&b.0.y)
            .unwrap()
            .then(a.0.x.partial_cmp(&b.0.x).unwrap())
            .then(a.1.y.partial_cmp(&b.1.y).unwrap())
            .then(a.1.x.partial_cmp(&b.1.x).unwrap())
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

            let dir = (to - last_to);
            if dir.perp_dot(last_dir) == 0.0 {
                *path.last_mut().unwrap() = to;
            } else {
                path.push(to);
            }
            last_dir = dir;
            last_to = to;
        }
        path.pop();

        let path = path
            .into_iter()
            .map(|v| v * 0.5 * cell_size as f32)
            .collect();
        outline_points.push(OutlineBatch {
            points: path,
            value,
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
        let mut generated_distances = replace(&mut self.generated_distances, Vec::new());

        if layer_mask == u64::MAX {
            generated_bitmap.cells.clear();
            generated_bitmap.bounds = Rect::zero();
        } else {
            generated_bitmap.cells.fill(0);
        }

        while generated_distances.len() < doc.materials.len() {
            generated_distances.push(Grid::new(f32::MAX))
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
                        graph.render_distances(&mut generated_distances, cell_size);
                    }
                }
                LayerContent::Grid(grid_key) => {
                    let _span = span!("LayerContent::Graph");
                    if let Some(grid) = doc.grids.get(grid_key) {
                        let layer_bounds = grid.find_used_bounds();
                        generated_bitmap.resize_to_include_conservative(layer_bounds);
                        generated_bitmap.blit(grid, layer_bounds, 255);
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
        let (mut outline, mut vertices, mut indices) = doc
            .materials
            .par_iter()
            .enumerate()
            .skip(1)
            .take(254)
            .map(|(index, _material)| {
                if used_materials & (1 << (index - 1)) != 0 {
                    trace_grid(&self.generated_grid, doc.cell_size, index as u8, None)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            })
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

        for (index, grid) in self.generated_distances.iter().enumerate() {
            let (o, v, i) = trace_distances(grid, doc.cell_size, index as u8);
            outline.extend(o.into_iter());
            vertices.extend(v.into_iter());
            indices.extend(i.into_iter());
        }
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
            for ((dest, pos), pos_world) in vs
                .iter_mut()
                .zip(positions_screen)
                .zip(loose_vertices.iter())
            {
                *dest = VertexPos3UvColor {
                    pos: [pos.x, pos.y, 0.0],
                    uv: [
                        pos_world.x / finish_checker_size / 2.0,
                        pos_world.y / finish_checker_size / 2.0,
                    ],
                    color: fill_color,
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
                .stroke_polyline_aa(&positions_screen, true, thickness, outline_color);

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
