use crate::app::ShaderUniforms;
use crate::document::{ChangeMask, Document, Grid, TraceMethod, View};
use crate::material::Material;
use glam::{vec2, Vec2};
use miniquad::{
    BlendFactor, BlendState, BlendValue, BufferLayout, Context, Equation, FilterMode, PassAction,
    Pipeline, PipelineParams, RenderPass, Shader, ShaderMeta, Texture, TextureFormat,
    TextureParams, TextureWrap, UniformBlockLayout, UniformDesc, UniformType, VertexAttribute,
    VertexFormat,
};
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use std::cmp::Ordering::{Equal, Greater};
use std::collections::{BTreeMap, BTreeSet};

pub struct VertexBatch {
    value: u8,
    vertices: Vec<Vec2>,
}

pub struct OutlineBatch {
    value: u8,
    points: Vec<Vec2>,
}

pub struct DocumentGraphics {
    pub outline_points: Vec<OutlineBatch>,
    pub outline_fill_indices: Vec<Vec<u16>>,

    pub loose_vertices: Vec<VertexBatch>,
    pub loose_indices: Vec<Vec<u16>>,
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

fn trace_grid(
    outline_points: &mut Vec<OutlineBatch>,
    vertices: &mut Vec<VertexBatch>,
    indices: &mut Vec<Vec<u16>>,
    grid: &Grid,
    value: u8,
) {
    assert!(vertices.len() == indices.len());
    if vertices.is_empty() {
        vertices.push(VertexBatch {
            value,
            vertices: Vec::new(),
        });
        indices.push(Vec::new());
    }

    let bounds = grid.bounds;
    let w = bounds[2] - bounds[0];

    let mut add_vertices = |vs: &[Vec2], is: &[u16]| {
        let VertexBatch {
            vertices: last_vertices,
            value: last_value,
        } = vertices.last_mut().unwrap();

        let last_indices = indices.last_mut().unwrap();

        if last_vertices.len() < u16::MAX as usize - vs.len() - 1
            && last_indices.len() < u16::MAX as usize - is.len() - 1
            && *last_value == value
        {
            let base = last_vertices.len() as u16;
            last_vertices.extend_from_slice(vs);
            last_indices.extend(is.iter().cloned().map(|i| base + i));
        } else {
            vertices.push(VertexBatch {
                value,
                vertices: vs.to_owned(),
            });
            indices.push(is.to_owned());
        };
    };

    let sample_or_zero = |[x, y]: [i32; 2]| -> bool {
        if x < bounds[0] || x >= bounds[2] {
            return false;
        }
        if y < bounds[1] || y >= bounds[3] {
            return false;
        }
        let i = x - bounds[0];
        let j = y - bounds[1];
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

    let mut edges: BTreeMap<[i32; 2], [i32; 2]> = BTreeMap::new();
    let orientation_offsets = [[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]];

    let cell_size_f = grid.cell_size as f32;
    for y in bounds[1]..bounds[3] {
        for x in bounds[0]..bounds[2] {
            let mut tiles = [TraceTile::Empty; 4];
            for (orientation, tile) in tiles.iter_mut().enumerate() {
                let wall_bits = sample_bits(x, y, orientation as i32);
                *tile = match wall_bits {
                    0b0110 | 0b0011 | 0b0111 | 0b1011 | 0b1110 | 0b1010 | 0b1111 | 0b0010 => {
                        TraceTile::Fill
                    }
                    0b1100 | 0b0100 => TraceTile::TopEdge,
                    0b0001 | 0b1001 => TraceTile::LeftEdge,
                    0b1101 | 0b0101 => TraceTile::DiagonalOuter,
                    0b0000 | 0b1000 | _ => TraceTile::Empty,
                };

                *tile = match wall_bits {
                    0b0110 | 0b0011 | 0b0111 | 0b1011 | 0b1110 | 0b1010 | 0b1111 => TraceTile::Fill,
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
                                    2 => ([(x + 1) * 2, y * 2 + 1], [x * 2 + 1, (y + 1) * 2]),
                                    _ => ([x * 2 + 1, (y + 1) * 2], [x * 2, y * 2 + 1]),
                                };
                                edges.insert(from, to);

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
                                    2 => ([x * 2 + 1, (y + 1) * 2], [(x + 1) * 2, y * 2 + 1]),
                                    _ => ([x * 2, y * 2 + 1], [x * 2 + 1, (y + 1) * 2]),
                                };
                                edges.insert(from, to);

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
                                    2 => ([(x + 1) * 2, y * 2 + 1], [(x + 1) * 2, (y + 1) * 2]),
                                    _ => ([x * 2 + 1, (y + 1) * 2], [x * 2, (y + 1) * 2]),
                                };
                                edges.insert(from, to);
                            }
                            TraceTile::TopEdge => {
                                let (from, to) = match orientation {
                                    0 => ([x * 2, y * 2], [x * 2 + 1, y * 2]),
                                    1 => ([(x + 1) * 2, y * 2], [(x + 1) * 2, y * 2 + 1]),
                                    2 => ([(x + 1) * 2, (y + 1) * 2], [x * 2 + 1, (y + 1) * 2]),
                                    _ => ([x * 2, (y + 1) * 2], [x * 2, y * 2 + 1]),
                                };
                                edges.insert(from, to);
                            }
                        }
                    }
                }
            }
        }
    }

    loop {
        let (from, mut to) = match pop_first_map(&mut edges) {
            Some(kv) => kv,
            None => break,
        };

        let mut path = Vec::new();
        path.push(from);
        while let Some(next_to) = edges.remove(&to) {
            path.push(to);
            to = next_to;
        }
        if from != to {
            path.push(to);
        }

        // remove redundant points
        if path.len() > 2 {
            let mut restart = Some(1);
            while let Some(start) = restart {
                restart = None;
                for i in start..path.len() {
                    let dx = path[(i + 1) % path.len()][0] - path[i - 1][0];
                    let dy = path[(i + 1) % path.len()][1] - path[i - 1][1];
                    let vx = path[i][0] - path[i - 1][0];
                    let vy = path[i][1] - path[i - 1][1];
                    if vx.abs() <= dx.abs() && vy.abs() <= dy.abs() {
                        if dx * vy == vx * dy {
                            path.remove(i);
                            restart = Some(i.max(2) - 1);
                            break;
                        }
                    }
                }
            }
        }

        let path = path
            .into_iter()
            .map(|[x, y]| {
                vec2(
                    x as f32 * 0.5 * grid.cell_size as f32,
                    y as f32 * 0.5 * grid.cell_size as f32,
                )
            })
            .collect();
        outline_points.push(OutlineBatch {
            points: path,
            value,
        });
    }
}

impl DocumentGraphics {
    pub(crate) fn generate(
        &mut self,
        doc: &Document,
        change_mask: ChangeMask,
        context: Option<&mut Context>,
    ) {
        if change_mask.cells {
            self.generate_cells(doc);
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

    fn generate_cells(&mut self, doc: &Document) {
        let start_time = miniquad::date::now();
        self.outline_points.clear();
        self.outline_fill_indices.clear();
        self.loose_vertices.clear();
        self.loose_indices.clear();

        let bounds = doc.layer.bounds;

        match doc.layer.trace_method {
            TraceMethod::Walk => {
                let islands = Self::find_islands(
                    &doc.layer.cells,
                    bounds[2] - bounds[0],
                    bounds[3] - bounds[1],
                );
                let origin = vec2(doc.layer.bounds[0] as f32, doc.layer.bounds[1] as f32);
                for (_kind, island) in islands {
                    let outline = Self::find_island_outline(&island);
                    let outline: Vec<Vec2> = outline
                        .into_iter()
                        .map(|p| (p + origin) * Vec2::splat(doc.layer.cell_size as f32))
                        .collect();

                    let point_coordinates: Vec<f64> = outline
                        .iter()
                        .map(|p| vec![p.x as f64, p.y as f64].into_iter())
                        .flatten()
                        .collect();

                    let fill_indices: Vec<u16> =
                        earcutr::earcut(&point_coordinates, &Vec::new(), 2)
                            .into_iter()
                            .map(|i| i as u16)
                            .collect();
                    self.outline_points.push(OutlineBatch {
                        value: 1,
                        points: outline,
                    });
                    self.outline_fill_indices.push(fill_indices);
                }
            }
            TraceMethod::Grid => {
                for (index, _material) in doc.materials.iter().enumerate().skip(1).take(254) {
                    trace_grid(
                        &mut self.outline_points,
                        &mut self.loose_vertices,
                        &mut self.loose_indices,
                        &doc.layer,
                        index as u8,
                    );
                }
            }
        }
        println!(
            "generated in {} ms",
            (miniquad::date::now() - start_time) * 1000.0
        );
    }

    pub fn find_islands(grid: &[u8], w: i32, h: i32) -> Vec<(u8, BTreeSet<(i32, i32)>)> {
        let mut counts = [0; 3];
        for &cell in grid {
            counts[cell as usize] += 1;
        }

        let mut open_set = BTreeSet::new();
        for y in 0..h {
            for x in 0..w {
                if grid[(y * w + x) as usize] != 0 as u8 {
                    open_set.insert((x, y));
                }
            }
        }

        let offsets = [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (1, 1),
            (-1, 1),
        ];

        let mut result = Vec::new();

        while let Some(first) = pop_first(&mut open_set) {
            let mut fringe = BTreeSet::new();
            fringe.insert(first);

            let (first_x, first_y) = first;
            let island_value = grid[(first_y * w + first_x) as usize];
            let mut island_set = BTreeSet::new();

            while let Some((x, y)) = pop_first(&mut fringe) {
                open_set.remove(&(x, y));
                island_set.insert((x, y));
                for &o in &offsets {
                    let (dx, dy) = o;
                    let next = (x + dx, y + dy);
                    let (nx, ny) = next;
                    if nx >= w || nx < 0 {
                        continue;
                    }

                    if ny >= h || ny < 0 {
                        continue;
                    }

                    if open_set.contains(&next) && grid[(ny * w + nx) as usize] == island_value {
                        open_set.remove(&next);
                        fringe.insert(next);
                    }
                }
            }

            if island_set.len() > 0 {
                result.push((island_value, island_set));
            }
        }

        return result;
    }

    pub fn find_island_outline(island_cells: &BTreeSet<(i32, i32)>) -> Vec<Vec2> {
        let mut trace = Vec::new();

        let start = island_cells.iter().next().cloned().unwrap();
        let cut_diagonals = true;

        let start_dir = (0, -1);
        let mut dir = start_dir;
        let mut pos = start;

        loop {
            if Some(pos) != trace.last().cloned() {
                trace.push(pos);
            }

            let cell_offset = cell_offset(dir);

            let forward_cell = (pos.0 + dir.0 + cell_offset.0, pos.1 + dir.1 + cell_offset.1);
            let normal = (-dir.1, dir.0);

            let left_forward_cell = (
                pos.0 - normal.0 + dir.0 + cell_offset.0,
                pos.1 - normal.1 + dir.1 + cell_offset.1,
            );

            let new_pos;
            let new_dir;

            if island_cells.contains(&left_forward_cell) {
                new_pos = (pos.0 - normal.0, pos.1 - normal.1);
                new_dir = (-normal.0, -normal.1);
            } else if island_cells.contains(&forward_cell) {
                let is_inner_diagonal = if cut_diagonals {
                    let diagonal_cell = (
                        pos.0 + dir.0 * 2 - normal.0 + cell_offset.0,
                        pos.1 + dir.1 * 2 - normal.1 + cell_offset.1,
                    );
                    let side_cell = (
                        pos.0 + dir.0 - normal.0 * 2 + cell_offset.0,
                        pos.1 + dir.1 - normal.1 * 2 + cell_offset.1,
                    );
                    island_cells.contains(&diagonal_cell) && !island_cells.contains(&side_cell)
                } else {
                    false
                };
                if is_inner_diagonal {
                    new_pos = (pos.0 + dir.0 - normal.0, pos.1 + dir.1 - normal.1);
                    new_dir = dir;
                } else {
                    new_pos = (pos.0 + dir.0, pos.1 + dir.1);
                    new_dir = dir;
                }
            } else {
                let is_outer_diagonal = if cut_diagonals {
                    let side_cell = (
                        pos.0 + dir.0 + normal.0 + cell_offset.0,
                        pos.1 + dir.1 + normal.1 + cell_offset.1,
                    );
                    let next_forward_cell = (
                        pos.0 + dir.0 * 2 + cell_offset.0,
                        pos.1 + dir.1 * 2 + cell_offset.1,
                    );
                    island_cells.contains(&side_cell) && !island_cells.contains(&next_forward_cell)
                } else {
                    false
                };
                if is_outer_diagonal {
                    new_pos = (pos.0 + normal.0 + dir.0, pos.1 + normal.1 + dir.1);
                    new_dir = normal;
                } else {
                    new_pos = (pos.0 + normal.0, pos.1 + normal.1);
                    new_dir = normal;
                }
            }

            pos = new_pos;
            dir = new_dir;
            if dir == start_dir && pos == start {
                break;
            }
        }

        // remove redundant points
        if trace.len() > 2 {
            let mut restart = Some(1);
            while let Some(start) = restart {
                restart = None;
                for i in start..trace.len() {
                    let dx = trace[(i + 1) % trace.len()].0 - trace[i - 1].0;
                    let dy = trace[(i + 1) % trace.len()].1 - trace[i - 1].1;
                    let vx = trace[i].0 - trace[i - 1].0;
                    let vy = trace[i].1 - trace[i - 1].1;
                    if vx.abs() <= dx.abs() && vy.abs() <= dy.abs() {
                        if dx * vy == vx * dy {
                            trace.remove(i);
                            restart = Some(i.max(2) - 1);
                            break;
                        }
                    }
                }
            }
        }

        // push concave diagonal edges to ensure round thickness of walls
        let trace: Vec<Vec2> = trace
            .into_iter()
            .map(|(x, y)| vec2(x as f32, y as f32))
            .collect();

        let mut result = trace.clone();

        let num = trace.len();
        if trace.len() > 4 {
            let mut directions: Vec<Vec2> = Vec::with_capacity(num);
            for i in 0..num {
                let a = trace[i + 0];
                let b = trace[(i + 1) % trace.len()];
                let dir = b - a;
                directions.push(dir.normalize_or_zero());
            }

            for i in 0..num {
                let dir_a = directions[i];
                let dir = directions[(i + 1) % num];
                let dir_b = directions[(i + 2) % num];
                if dir_a.x != 0.0 && dir_a.y != 0.0 {
                    continue;
                }
                if dir_b.x != 0.0 && dir_b.y != 0.0 {
                    continue;
                }
                let points = [
                    trace[i + 0],
                    trace[(i + 1) % num],
                    trace[(i + 2) % num],
                    trace[(i + 3) % num],
                ];

                let center = ((points[1] + points[2]) * 0.5).floor();
                let len_square = (points[2] - points[1]).length_squared();
                let n = directions[(i + 1) % num].perp();

                let max_thickness = 2.5;
                let n_segment = [center, center + n * max_thickness];
                let mut hit = None;
                for j in 0..num {
                    if (j as isize - i as isize).abs() < 2 {
                        continue;
                    }
                    let s = [trace[j], trace[(j + 1) % num]];
                    if let Some(t) = intersect_segment_segment(n_segment, s) {
                        let t_world = t * max_thickness;
                        if let Some((hit_t, _)) = hit {
                            if t_world < hit_t {
                                hit = Some((t_world, j));
                            }
                        } else {
                            hit = Some((t_world, j));
                        }
                    }
                }

                if let Some((hit, j)) = hit {
                    let s = [trace[j], trace[(j + 1) % num]];
                    let is_parallel = directions[j].dot(dir).abs() > 0.99;
                    let edge_comparison = len_square
                        .partial_cmp(&(s[1] - s[0]).length_squared())
                        .unwrap();

                    if is_parallel && edge_comparison != Greater {
                        let fract = hit.fract();
                        let push_distance = if edge_comparison != Equal {
                            fract
                        } else {
                            fract * 0.5
                        };
                        if push_distance < hit * 0.99 {
                            let descale_a = dir_a.dot(n);
                            let descale_b = dir_b.dot(n);
                            let push_a = push_distance / descale_a;
                            let push_b = push_distance / descale_b;
                            let new_a = points[1] + dir_a * push_a;
                            let new_b = points[2] + dir_b * push_b;
                            result[(i + 1) % num] = new_a;
                            result[(i + 2) % num] = new_b;
                        }
                    }
                }
            }
        }

        result
    }

    pub(crate) fn draw(&self, batch: &mut MiniquadBatch<VertexPos3UvColor>, view: &View) {
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
            let positions_screen: Vec<_> = loose_vertices
                .iter()
                .map(|p| world_to_screen.transform_point2(*p))
                .collect();
            batch
                .geometry
                .add_position_indices(&positions_screen, loose_indices, fill_color);
        }

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
        _pipeline: Pipeline,
        context: &mut Context,
    ) -> (Vec<u8>, usize, usize) {
        let pipeline = create_pipeline(context);

        // find used bounds
        let bounds = doc.layer.find_used_bounds();

        let map_width = ((bounds[2] - bounds[0]) * doc.layer.cell_size) as usize;
        let map_height = ((bounds[3] - bounds[1]) * doc.layer.cell_size) as usize;

        let center = vec2(
            ((bounds[2] + bounds[0]) * doc.layer.cell_size) as f32 * 0.5,
            ((bounds[3] + bounds[1]) * doc.layer.cell_size) as f32 * 0.5,
        );

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
        self.draw(&mut batch, &view);

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

        (flipped_pixels, map_width, map_height)
    }
}

fn cell_offset(dir: (i32, i32)) -> (i32, i32) {
    match dir {
        (0, -1) => (0, 0),
        (1, 0) => (-1, 0),
        (0, 1) => (-1, -1),
        _ => (0, -1),
    }
}

fn pop_first<K: Ord + Clone>(set: &mut BTreeSet<K>) -> Option<K> {
    // TODO replace with BTreeSet::pop_first when it stabilizes
    let first = set.iter().next().cloned()?;
    if !set.remove(&first) {
        return None;
    }
    Some(first)
}

fn pop_first_map<K: Ord + Clone, V: Clone>(map: &mut BTreeMap<K, V>) -> Option<(K, V)> {
    // TODO replace with BTreeSet::pop_first when it stabilizes
    let (key, value) = map.iter().next()?;
    let key = key.clone();
    let value = value.clone();
    map.remove(&key);
    Some((key, value))
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
