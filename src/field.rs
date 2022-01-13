use crate::grid::Grid;
use crate::math::Rect;
use crate::sdf::{distance_transform, sd_trapezoid};
use crate::some_or;
use crate::span;
use glam::{ivec2, IVec2, Vec2};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefMutIterator, ParallelExtend,
    ParallelIterator,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hint::unreachable_unchecked;

#[derive(Serialize, Deserialize)]
pub struct Field {
    pub tile_size: usize,
    pub materials: Vec<HashMap<(i32, i32), Vec<f32>>>,
}

impl Field {
    pub fn new() -> Field {
        Field {
            tile_size: 64,
            materials: Vec::new(),
        }
    }

    pub fn from_grid(grid: &Grid<u8>, num_materials: usize, cell_size: i32) -> Field {
        let _span = span!("Field::from_grid");
        let mut field = Field::new();
        let tile_size = field.tile_size as i32;
        field.materials.push(Default::default());
        field
            .materials
            .par_extend((1..num_materials).into_par_iter().map(|material_index| {
                let mut tiles = HashMap::new();

                let grid = upscale_epx(grid);
                let w = grid.bounds[1].x - grid.bounds[0].x;
                let h = grid.bounds[1].y - grid.bounds[0].y;

                let (mut distances, neg_distances) = rayon::join(
                    || {
                        distance_transform(w as u32, h as u32, |i| {
                            let x = i as i32 % w;
                            let y = i as i32 / w;
                            grid.cells[(y * w + x) as usize] == material_index as u8
                        })
                    },
                    || {
                        distance_transform(w as u32, h as u32, |i| {
                            let x = (i as i32 % w);
                            let y = (i as i32 / w);
                            grid.cells[(y * w + x) as usize] != material_index as u8
                        })
                    },
                );
                for (d, neg) in distances.iter_mut().zip(neg_distances.iter().cloned()) {
                    if neg > 0.0 && neg < f32::MAX {
                        *d = d.min(-neg);
                    }
                }

                let bounds = [grid.bounds[0], grid.bounds[1]];
                let w = bounds[1].x - bounds[0].x;
                let tile_range = Field::grid_to_tile_range(bounds, tile_size as usize);

                // split distances into tiles
                for tile_y in tile_range[0].y..tile_range[1].y {
                    for tile_x in tile_range[0].x..tile_range[1].x {
                        let mut new_tile = vec![f32::MAX; tile_size as usize * tile_size as usize];
                        let tile_rect = [
                            ivec2(tile_x * tile_size, tile_y * tile_size).max(bounds[0]),
                            ivec2((tile_x + 1) * tile_size, (tile_y + 1) * tile_size)
                                .min(bounds[1]),
                        ];

                        for y in tile_rect[0].y..tile_rect[1].y {
                            for x in tile_rect[0].x..tile_rect[1].x {
                                let tx = x & (tile_size - 1);
                                let ty = y & (tile_size - 1);
                                let sx = x - bounds[0].x;
                                let sy = y - bounds[0].y;
                                new_tile[(ty * tile_size + tx) as usize] =
                                    distances[(sy * w + sx) as usize] * cell_size as f32 * 0.25;
                            }
                        }
                        tiles.insert((tile_x, tile_y), new_tile);
                    }
                }
                tiles
            }));
        field
    }

    pub fn trapezoid(
        &mut self,
        material: u8,
        a: Vec2,
        b: Vec2,
        a_r: f32,
        b_r: f32,
        cell_size: i32,
        max_bound: f32,
        num_materials: usize,
    ) {
        let a_rect = [
            a - Vec2::splat(a_r + max_bound),
            a + Vec2::splat(a_r + max_bound),
        ];
        let b_rect = [
            b - Vec2::splat(b_r + max_bound),
            b + Vec2::splat(b_r + max_bound),
        ];
        let min_pos = a_rect[0].min(b_rect[0]);
        let max_pos = a_rect[1].max(b_rect[1]);
        let rect = [min_pos, max_pos];

        self.apply(material, rect, cell_size, max_bound, num_materials, |pos| {
            sd_trapezoid(pos, a, b, a_r, b_r)
        });
    }

    pub(crate) fn grid_to_tile_range(tile_rect: [IVec2; 2], tile_size: usize) -> [IVec2; 2] {
        [
            ivec2(
                tile_rect[0].x.div_euclid(tile_size as i32),
                tile_rect[0].y.div_euclid(tile_size as i32),
            ),
            ivec2(
                (tile_rect[1].x + tile_size as i32 - 1).div_euclid(tile_size as i32),
                (tile_rect[1].y + tile_size as i32 - 1).div_euclid(tile_size as i32),
            ),
        ]
    }

    pub fn world_to_tile_range(
        world_rect: [Vec2; 2],
        cell_size: i32,
        tile_size: usize,
    ) -> [IVec2; 2] {
        let grid_rect = Grid::<f32>::world_to_grid_rect(world_rect, cell_size);
        Field::grid_to_tile_range(grid_rect, tile_size)
    }

    fn apply(
        &mut self,
        material: u8,
        world_rect: [Vec2; 2],
        cell_size: i32,
        max_bound: f32,
        num_materials: usize,
        f: impl Fn(Vec2) -> f32,
    ) {
        while self.materials.len() < num_materials {
            self.materials.push(Default::default());
        }
        let tile_size = self.tile_size;
        let grid_rect = Grid::<f32>::world_to_grid_rect(world_rect, cell_size);
        let tile_range = Field::grid_to_tile_range(grid_rect, tile_size);

        let cell_size_f = cell_size as f32;

        let (materials_before, materials_current) = self.materials.split_at_mut(material as usize);
        let (materials_current, materials_after) = materials_current.split_at_mut(1);
        let material_tiles = &mut materials_current[0];

        for j in tile_range[0].y..tile_range[1].y {
            for i in tile_range[0].x..tile_range[1].x {
                let tile_rect = [
                    ivec2(i * tile_size as i32, j * tile_size as i32),
                    ivec2((i + 1) * tile_size as i32, (j + 1) * tile_size as i32),
                ];
                let tile_grid_rect = tile_rect.intersect(grid_rect).unwrap();

                let tile = material_tiles
                    .entry((i, j))
                    .or_insert_with(|| vec![f32::MAX; tile_size * tile_size]);

                for y in tile_grid_rect[0].y..tile_grid_rect[1].y {
                    for x in tile_grid_rect[0].x..tile_grid_rect[1].x {
                        let pos = (ivec2(x, y).as_vec2() + Vec2::splat(0.5)) * cell_size_f;
                        let d = f(pos);
                        let tx = (x & (tile_size as i32 - 1)) as usize;
                        let ty = (y & (tile_size as i32 - 1)) as usize;
                        let index = tile_size * ty + tx;
                        tile[index] = tile[index].min(d);
                    }
                }

                for other_tiles in materials_before
                    .iter_mut()
                    .chain(materials_after.iter_mut())
                {
                    let other_tile = some_or!(other_tiles.get_mut(&(i, j)), continue);
                    for y in tile_grid_rect[0].y..tile_grid_rect[1].y {
                        for x in tile_grid_rect[0].x..tile_grid_rect[1].x {
                            let tx = (x & (tile_size as i32 - 1)) as usize;
                            let ty = (y & (tile_size as i32 - 1)) as usize;
                            let index = tile_size * ty + tx;
                            let d = tile[index];
                            other_tile[index] = other_tile[index].max(-d);
                        }
                    }
                }
            }
        }
    }

    pub fn compose(&mut self, above: &Field) {
        let _span = span!("compose");
        let num_materials = above.materials.len();
        while self.materials.len() < num_materials {
            self.materials.push(Default::default());
        }

        // subtract occluding cells
        self.materials
            .par_iter_mut()
            .enumerate()
            .skip(1)
            .for_each(|(m_index, material)| {
                material.par_iter_mut().for_each(|(tile_key, dest_tile)| {
                    for m_index_src in (1..m_index).chain((m_index + 1)..num_materials) {
                        if let Some(src_tile) = above.materials[m_index_src].get(tile_key) {
                            for (d, s) in dest_tile.iter_mut().zip(src_tile.iter()) {
                                *d = d.max(-*s);
                            }
                        }
                    }
                });
            });

        // combine tiles of the same material
        self.materials
            .par_iter_mut()
            .enumerate()
            .skip(1)
            .for_each(|(material_i, material)| {
                let hash_map = some_or!(above.materials.get(material_i), return);
                for (tile_key, src_tile) in hash_map.iter() {
                    material
                        .entry(*tile_key)
                        .and_modify(|dest_tile| {
                            for (d, s) in dest_tile.iter_mut().zip(src_tile.iter()) {
                                *d = d.min(*s);
                            }
                        })
                        .or_insert_with(|| src_tile.clone());
                }
            });
    }

    pub fn calculate_bounds(&self) -> [IVec2; 2] {
        let mut bounds = [ivec2(i32::MAX, i32::MAX), ivec2(i32::MIN, i32::MIN)];

        let tile_size = self.tile_size as i32;
        for material in &self.materials {
            for (key, tile) in material {
                let mut tile_bounds = [
                    ivec2(key.0 * tile_size, key.1 * tile_size),
                    ivec2((key.0 + 1) * tile_size, (key.1 + 1) * tile_size),
                ];

                'y: for y in tile_bounds[0].y..tile_bounds[1].y {
                    let ty = y & (tile_size - 1);
                    for x in tile_bounds[0].x..tile_bounds[1].x {
                        let tx = x & (tile_size - 1);
                        if tile[(ty * tile_size + tx) as usize] <= 0.0 {
                            break 'y;
                        }
                    }
                    tile_bounds[0].y = y + 1;
                }

                'y: for y in (tile_bounds[0].y..tile_bounds[1].y).rev() {
                    let ty = y & (tile_size - 1);
                    for x in tile_bounds[0].x..tile_bounds[1].x {
                        let tx = x & (tile_size - 1);
                        if tile[(ty * tile_size + tx) as usize] <= 0.0 {
                            break 'y;
                        }
                    }
                    tile_bounds[1].y = y;
                }

                'x: for x in tile_bounds[0].x..tile_bounds[1].x {
                    let tx = x & (tile_size - 1);
                    for y in tile_bounds[0].y..tile_bounds[1].y {
                        let ty = y & (tile_size - 1);
                        if tile[(ty * tile_size + tx) as usize] <= 0.0 {
                            break 'x;
                        }
                    }
                    tile_bounds[0].x = x + 1;
                }

                'x: for x in (tile_bounds[0].x..tile_bounds[1].x).rev() {
                    let tx = x & (tile_size - 1);
                    for y in tile_bounds[0].y..tile_bounds[1].y {
                        let ty = y & (tile_size - 1);
                        if tile[(ty * tile_size + tx) as usize] <= 0.0 {
                            break 'x;
                        }
                    }
                    tile_bounds[1].x = x;
                }

                if !tile_bounds.is_empty() {
                    bounds = bounds.union(tile_bounds);
                }
            }
        }

        bounds
    }
}

fn upscale_epx(grid: &Grid<u8>) -> Grid<u8> {
    let w = grid.bounds.size().x;
    let h = grid.bounds.size().y;
    let get = |x, y| grid.cells[(y * w + x) as usize];
    let dw = w * 2;
    let dh = h * 2;
    let mut cells = vec![0; (dw * dh) as usize];
    for i in 0..w * h {
        let x = i % w;
        let y = i / w;
        let sx = x;
        let sy = y;

        let p = get(sx, sy);
        let a = if sy > 0 { get(sx, sy - 1) } else { 0 };
        let b = if sx + 1 < w { get(sx + 1, sy) } else { 0 };
        let c = if sx > 0 { get(sx - 1, sy) } else { 0 };
        let d = if sy + 1 < h { get(sx, sy + 1) } else { 0 };

        let out = [
            if c == a && c != d && a != b { a } else { p },
            if a == b && a != c && b != d { b } else { p },
            if c == d && d != b && c != a { c } else { p },
            if b == d && b != a && d != c { d } else { p },
        ];
        let dx = x * 2;
        let dy = y * 2;
        cells[(dy * dw + dx) as usize] = out[0];
        cells[(dy * dw + (dx + 1)) as usize] = out[1];
        cells[((dy + 1) * dw + dx) as usize] = out[2];
        cells[((dy + 1) * dw + (dx + 1)) as usize] = out[3];
    }

    let bounds = [grid.bounds[0] * 2, grid.bounds[1] * 2];
    Grid {
        default_value: grid.default_value,
        bounds,
        cells,
    }
}
