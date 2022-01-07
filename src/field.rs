use crate::grid::Grid;
use crate::math::Rect;
use crate::sdf::sd_trapezoid;
use crate::some_or;
use crate::span;
use glam::{ivec2, IVec2, Vec2};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
                for (tile_key, src_tile) in above.materials[material_i].iter() {
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
}
