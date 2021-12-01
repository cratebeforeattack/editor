use crate::grid::Grid;
use crate::sdf::sd_trapezoid;
use glam::{ivec2, IVec2, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Field {
    pub materials: Vec<Grid<f32>>,
}

impl Field {
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
            self.materials.push(Grid::new(max_bound))
        }
        let rect = Grid::<f32>::world_to_grid_rect(world_rect, cell_size);
        for m in self.materials.iter_mut() {
            m.resize_to_include_amortized(rect);
        }

        let cell_size_f = cell_size as f32;

        for y in rect[0].y..rect[1].y {
            for x in rect[0].x..rect[1].x {
                let pos = (ivec2(x, y).as_vec2() + Vec2::splat(0.5)) * cell_size_f;
                let d = f(pos);
                for (i, grid) in self.materials.iter_mut().enumerate() {
                    let index = grid.grid_pos_index(x, y);
                    if i as u8 != material {
                        grid.cells[index] = grid.cells[index].max(d);
                    } else {
                        grid.cells[index] = grid.cells[index].min(d);
                    }
                }
            }
        }
    }
}
