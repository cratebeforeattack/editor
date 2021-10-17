use glam::Vec2;

#[derive(Serialize, Deserialize, Clone)]
pub struct Grid {
    pub bounds: [i32; 4],
    pub cells: Vec<u8>,
}

impl Grid {
    pub fn new() -> Grid {
        Grid {
            bounds: [0, 0, 0, 0],
            cells: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.bounds = [0, 0, 0, 0];
        self.cells.clear();
    }

    pub fn size(&self) -> [i32; 2] {
        [
            self.bounds[2] - self.bounds[0],
            self.bounds[3] - self.bounds[1],
        ]
    }

    pub fn find_used_bounds(&self) -> [i32; 4] {
        let mut b = self.bounds;
        for x in (b[0]..b[2]).rev() {
            let mut used = false;
            for y in self.bounds[1]..self.bounds[3] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[2] = x + 1;
        }

        for x in b[0]..b[2] {
            let mut used = false;
            for y in b[1]..b[3] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[0] = x + 1;
        }

        for y in (b[1]..b[3]).rev() {
            let mut used = false;
            for x in b[0]..b[2] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[3] = y + 1;
        }

        for y in b[1]..b[3] {
            let mut used = false;
            for x in b[0]..b[2] {
                if self.cells[self.grid_pos_index(x, y)] != 0 {
                    used = true;
                    break;
                }
            }
            if used {
                break;
            }
            b[1] = y + 1;
        }

        b
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
            let old_start =
                ((y - old_bounds[1]) * old_size[0] + (x_range.start - old_bounds[0])) as usize;
            let new_start =
                ((y - new_bounds[1]) * new_size[0] + (x_range.start - new_bounds[0])) as usize;
            let old_range = old_start..old_start + x_range.len();
            let new_range = new_start..new_start + x_range.len();
            new_cells[new_range].copy_from_slice(&self.cells[old_range]);
        }
        self.bounds = new_bounds;
        self.cells = new_cells;
        println!("resized {:?}->{:?}", old_bounds, new_bounds);
        info!("resized {:?}->{:?}", old_bounds, new_bounds);
    }

    pub fn resize_to_include(&mut self, [l, t, r, b]: [i32; 4]) {
        if l >= self.bounds[0]
            && l < self.bounds[2]
            && t >= self.bounds[1]
            && t < self.bounds[3]
            && r >= self.bounds[0]
            && r < self.bounds[2]
            && b >= self.bounds[1]
            && b < self.bounds[3]
        {
            return;
        }
        let tile_size_cells = 64;
        let tile_l = l.div_euclid(tile_size_cells);
        let tile_t = t.div_euclid(tile_size_cells);
        let tile_r = r.div_euclid(tile_size_cells);
        let tile_b = b.div_euclid(tile_size_cells);

        let tile_bounds = [
            tile_l * tile_size_cells,
            tile_t * tile_size_cells,
            (tile_r + 1) * tile_size_cells,
            (tile_b + 1) * tile_size_cells,
        ];

        let bounds = [
            self.bounds[0].min(tile_bounds[0]),
            self.bounds[1].min(tile_bounds[1]),
            self.bounds[2].max(tile_bounds[2]),
            self.bounds[3].max(tile_bounds[3]),
        ];

        self.resize(bounds);
    }

    pub fn world_to_grid_pos(
        &self,
        point: Vec2,
        cell_size: i32,
    ) -> anyhow::Result<[i32; 2], [i32; 2]> {
        let grid_pos = point / Vec2::splat(cell_size as f32);
        let x = grid_pos.x.floor() as i32;
        let y = grid_pos.y.floor() as i32;
        if x < self.bounds[0] || x >= self.bounds[2] || y < self.bounds[1] || y >= self.bounds[3] {
            return Err([x, y]);
        }
        Ok([x, y])
    }

    pub fn flood_fill(
        cells: &mut [u8],
        [l, t, r, b]: [i32; 4],
        [start_x, start_y]: [i32; 2],
        value: u8,
    ) {
        let w = r - l;
        let h = b - t;
        let start_x = start_x - l;
        let start_y = start_y - t;
        let old_value = cells[(start_y * w + start_x) as usize];
        if old_value == value {
            return;
        }
        let mut stack = Vec::new();
        stack.push([start_x, start_y]);
        let fill_diagonals = old_value != 0;
        while let Some([mut x, y]) = stack.pop() {
            while x >= 0 && cells[(y * w + x) as usize] == old_value {
                x -= 1;
            }
            let mut span_above = false;
            let mut span_below = false;

            if fill_diagonals && x > 0 {
                if y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                    span_above = true;
                }
                if y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                    span_above = true;
                }
            }
            x += 1;

            while x < w && cells[(y * w + x) as usize] == old_value {
                cells[(y * w + x) as usize] = value;
                if !span_above && y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                    span_above = true;
                } else if span_above && y > 0 && cells[((y - 1) * w + x) as usize] != old_value {
                    span_above = false;
                }

                if !span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                    span_below = true;
                } else if span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] != old_value
                {
                    span_below = false;
                }
                x += 1;
            }

            if fill_diagonals && x < w {
                if !span_above && y > 0 && cells[((y - 1) * w + x) as usize] == old_value {
                    stack.push([x, y - 1]);
                }
                if !span_below && y < h - 1 && cells[((y + 1) * w + x) as usize] == old_value {
                    stack.push([x, y + 1]);
                }
            }
        }
    }

    pub fn grid_pos_index(&self, x: i32, y: i32) -> usize {
        ((y - self.bounds[1]) * (self.bounds[2] - self.bounds[0]) + x - self.bounds[0]) as usize
    }

    pub fn rectangle_outline(&mut self, [l, t, r, b]: [i32; 4], value: u8) {
        for x in l..=r {
            let index = self.grid_pos_index(x, t);
            self.cells[index] = value;
        }

        for y in t..=b {
            let index = self.grid_pos_index(l, y);
            self.cells[index] = value;
            let index = self.grid_pos_index(r, y);
            self.cells[index] = value;
        }
        for x in l..=r {
            let index = self.grid_pos_index(x, b);
            self.cells[index] = value;
        }
    }

    pub fn rectangle_fill(&mut self, [l, t, r, b]: [i32; 4], value: u8) {
        for y in t..=b {
            for x in l..=r {
                let index = self.grid_pos_index(x, y);
                self.cells[index] = value;
            }
        }
    }

    pub fn blit(&mut self, other_grid: &Grid) {
        let ob = other_grid.bounds;
        let b = self.bounds;
        let w = b[2] - b[0];
        let ow = ob[2] - ob[0];
        for y in ob[1]..ob[3] {
            for x in ob[0]..ob[2] {
                let v = other_grid.cells[((y - ob[1]) * ow + (x - ob[0])) as usize];
                if v != 0 {
                    let new_v = if v != 255 { v } else { 0 };
                    self.cells[((y - b[1]) * w + (x - b[0])) as usize] = new_v;
                }
            }
        }
    }
}
