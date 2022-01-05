use crate::grid::Grid;
use crate::sdf::sd_trapezoid;
use crate::span;
use glam::{ivec2, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Field {
    pub materials: Vec<Grid<f32>>,
}

impl Field {
    pub fn new() -> Field {
        Field {
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

    pub fn compose(&mut self, other: &Field) {
        let _span = span!("compose");
        for g in self.materials.iter_mut() {
            g.resize_to_include_amortized(other.materials[1].bounds);
        }
        let num = self.materials.len().min(other.materials.len());
        for m_index in 0..num {
            let mut o = &mut self.materials[m_index];
            let mut i = &other.materials[m_index];
            let b = i.bounds;
            for y in b[0].y..b[1].y {
                for x in b[0].x..b[1].x {
                    let o_i = o.grid_pos_index(x, y);
                    let i_i = i.grid_pos_index(x, y);
                    let mut d = o.cells[o_i];
                    d = d.min(i.cells[i_i]);
                    // substract remaining materials
                    for j in (m_index + 1)..num {
                        let mut j_grid = &other.materials[j];
                        if x >= j_grid.bounds[0].x
                            && x < j_grid.bounds[1].x
                            && y >= j_grid.bounds[0].y
                            && y < j_grid.bounds[1].y
                        {
                            let j_i = j_grid.grid_pos_index(x, y);
                            let j_d = j_grid.cells[j_i];
                            d = d.max(-j_d);
                        }
                    }
                    o.cells[o_i] = d;
                }
            }
        }
    }
}
