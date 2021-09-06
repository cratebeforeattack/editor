use glam::{Vec2, vec2, Affine2};
use miniquad::{Texture, Context};
use serde_derive::{Serialize, Deserialize};
use log::info;
use std::collections::BTreeSet;
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};
use std::cmp::Ordering::*;

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
    pub outline_fill_indices: Vec<Vec<u16>>,
    pub reference_texture: Option<Texture>,
}


#[derive(Clone, Serialize, Deserialize)]
pub (crate) struct View {
    pub target: Vec2,
    pub zoom: f32,
    pub zoom_target: f32,
    pub zoom_velocity: f32,
    pub screen_width_px: f32,
    pub screen_height_px: f32,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentLocalState {
    pub view: View
}

#[derive(Default, Copy, Clone, PartialEq)]
pub(crate) struct ChangeMask {
    pub cells: bool,
    pub reference_path: bool,
}

impl DocumentGraphics {
    pub(crate) fn generate(&mut self, doc: &Document, change_mask: ChangeMask, mut context: Option<&mut Context>) {
        if change_mask.cells {
            let start_time = miniquad::date::now();
            self.outline_points.clear();
            self.outline_fill_indices.clear();

            let bounds = doc.layer.bounds;
            let islands = Self::find_islands(&doc.layer.cells, bounds[2] - bounds[0], bounds[3] - bounds[1]);
            let origin = vec2(doc.layer.bounds[0] as f32, doc.layer.bounds[1] as f32);
            for (_kind, island) in islands {
                let outline = Self::find_island_outline(&island);
                let outline: Vec<Vec2> = outline.into_iter().map(|p| (p + origin) * Vec2::splat(doc.layer.cell_size as f32)).collect();

                let point_coordinates: Vec<f64> = outline.iter().map(|p| vec![p.x as f64, p.y as f64].into_iter()).flatten().collect();

                let fill_indices: Vec<u16> = earcutr::earcut(&point_coordinates, &Vec::new(), 2).into_iter().map(|i| i as u16).collect();
                self.outline_points.push(outline);
                self.outline_fill_indices.push(fill_indices);
            }
            println!("generated in {} ms", (miniquad::date::now() - start_time) * 1000.0);
        }

        if change_mask.reference_path {
            if let Some(tex) = self.reference_texture.take() {
                tex.delete();
            }

            if let Some(path) = &doc.reference_path {
                if let Some(context) = &mut context {
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

    pub fn find_islands(grid: &[u8], w: i32, h: i32)->Vec<(u8, BTreeSet<(i32, i32)>)>  {
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

    pub fn find_island_outline(island_cells: &BTreeSet<(i32, i32)>)->Vec<Vec2>  {
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

            let left_forward_cell = (pos.0 - normal.0 + dir.0 + cell_offset.0,
                                     pos.1 - normal.1 + dir.1 + cell_offset.1);

            let new_pos;
            let new_dir;

            if island_cells.contains(&left_forward_cell) {
                new_pos = (pos.0 - normal.0, pos.1 - normal.1);
                new_dir = (-normal.0, -normal.1);
            } else if island_cells.contains(&forward_cell) {
                let is_inner_diagonal = if cut_diagonals {
                    let diagonal_cell = (pos.0 + dir.0 * 2 - normal.0 + cell_offset.0,
                                         pos.1 + dir.1 * 2 - normal.1 + cell_offset.1);
                    let side_cell = (pos.0 + dir.0 - normal.0 * 2 + cell_offset.0,
                                     pos.1 + dir.1 - normal.1 * 2 + cell_offset.1);
                    island_cells.contains(&diagonal_cell) && !island_cells.contains(&side_cell)
                } else {
                    false
                };
                if is_inner_diagonal {
                    new_pos = (pos.0 + dir.0 - normal.0,
                               pos.1 + dir.1 - normal.1);
                    new_dir = dir;
                } else {
                    new_pos = (pos.0 + dir.0, pos.1 + dir.1);
                    new_dir = dir;
                }
            } else {
                let is_outer_diagonal = if cut_diagonals {
                    let side_cell = (pos.0 + dir.0 + normal.0 + cell_offset.0,
                                     pos.1 + dir.1 + normal.1 + cell_offset.1);
                    let next_forward_cell = (pos.0 + dir.0 * 2 + cell_offset.0,
                                             pos.1 + dir.1 * 2 + cell_offset.1);
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
                    let dx = trace[(i+1) % trace.len()].0 - trace[i-1].0;
                    let dy = trace[(i+1) % trace.len()].1 - trace[i-1].1;
                    let vx = trace[i].0 - trace[i-1].0;
                    let vy = trace[i].1 - trace[i-1].1;
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
        let trace: Vec<Vec2> = trace.into_iter().map(|(x, y)| vec2(x as f32, y as f32)).collect();

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
                    trace[(i + 3) % num]
                ];

                let center = (points[1] + points[2]) * 0.5;
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
                    let edge_comparison = len_square.partial_cmp(&(s[1] - s[0]).length_squared()).unwrap();

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
        for (positions, indices) in self.outline_points.iter().zip(self.outline_fill_indices.iter()) {
            let fill_color = [64, 64, 64, 255];
            let positions_screen: Vec<_> = positions.iter()
                .map(|p| world_to_screen.transform_point2(*p))
                .collect();

            let color = [200, 200, 200, 255];
            let thickness = outline_thickness * world_to_screen_scale;
            batch.geometry.add_position_indices(&positions_screen, &indices, fill_color);
            batch.geometry.stroke_polyline_aa(&positions_screen, true, thickness, color);

            if false {
                for (i, point) in positions_screen.iter().cloned().enumerate() {
                    batch.geometry.fill_circle_aa(point, 1.0 + i as f32, 16, [0, 255, 0, 64]);
                }
            }

        }

    }
}

impl Grid {
    pub fn size(&self) -> [i32; 2] {
        [self.bounds[2] - self.bounds[0], self.bounds[3] - self.bounds[1]]
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

        let tile_bounds = [
            tile_x * tile_size_cells,
            tile_y * tile_size_cells,
            (tile_x + 1) * tile_size_cells,
            (tile_y + 1) * tile_size_cells
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


fn cell_offset(dir: (i32, i32))->(i32, i32) {
    match dir {
        (0, -1) => (0, 0),
        (1, 0) => (-1, 0),
        (0, 1) => (-1, -1),
        _ => (0, -1)
    }
}

pub fn pop_first<K: Ord + Clone>(set: &mut BTreeSet<K>)->Option<K> {
    // TODO replace with BTreeSet::pop_first when it stabilizes
    let first = set.iter().next().cloned()?;
    if !set.remove(&first) {
        return None;
    }
    Some(first)
}

pub fn intersect_segment_segment(a: [Vec2; 2], b: [Vec2; 2])->Option<f32> {
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


impl View {
    pub fn screen_to_world(&self)->Affine2 {
        self.world_to_screen().inverse()
    }

    pub fn world_to_screen(&self)->Affine2 {
        Affine2::from_translation( vec2(self.screen_width_px, self.screen_height_px) * 0.5) *
        Affine2::from_scale(Vec2::splat(self.zoom)) *
        Affine2::from_translation(-self.target)
    }
}