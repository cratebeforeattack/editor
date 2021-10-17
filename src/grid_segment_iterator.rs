use glam::{ivec2, IVec2, Vec2};

// Implementation of "A Fast Voxel Traversal Algorithm for Ray Tracing" by John Amanatides and Andrew Woo
// http://www.cse.yorku.ca/~amana/research/grid.pdf
pub struct GridSegmentIterator {
    iteration: usize,
    max_steps: usize,
    t: f32,
    pos: IVec2,
    step: IVec2,
    t_max: Vec2,
    t_delta: Vec2,
}

impl GridSegmentIterator {
    pub fn new(
        start: Vec2,
        end: Vec2,
        grid_offset: Vec2,
        grid_cell_size: Vec2,
        max_steps: usize,
    ) -> GridSegmentIterator {
        let start_grid = (start - grid_offset) / grid_cell_size;
        let end_grid = (end - grid_offset) / grid_cell_size;
        let relative_grid = end_grid - start_grid;
        let step = ivec2(
            if relative_grid.x > 0.0 {
                1
            } else if relative_grid.x < 0.0 {
                -1
            } else {
                0
            },
            if relative_grid.y > 0.0 {
                1
            } else if relative_grid.y < 0.0 {
                -1
            } else {
                0
            },
        );

        let mut t_max = Vec2::splat(f32::MAX);
        let mut t_delta = Vec2::ZERO;
        if relative_grid.x != 0.0 {
            t_max.x = (if step.x > 0 {
                1.0 - start_grid.x.fract()
            } else {
                start_grid.x.fract()
            }) / relative_grid.x.abs();
            t_delta.x = step.x as f32 / relative_grid.x;
        }

        if relative_grid.y != 0.0 {
            t_max.y = (if step.y > 0 {
                1.0 - start_grid.y.fract()
            } else {
                start_grid.y.fract()
            }) / relative_grid.y.abs();
            t_delta.y = step.y as f32 / relative_grid.y;
        }

        if step.x == 0 && step.y == 0 {
            // terminate after first iteration
            t_max = Vec2::splat(1.0);
        }

        let pos = start_grid.floor().as_ivec2();

        GridSegmentIterator {
            step,
            iteration: 0,
            max_steps,
            pos,
            t: 0.0,
            t_max,
            t_delta,
        }
    }
}

impl Iterator for GridSegmentIterator {
    type Item = IVec2;

    #[inline]
    fn next(&mut self) -> Option<IVec2> {
        if self.iteration == self.max_steps {
            return None;
        }
        if self.t >= 1.0 {
            return None;
        }
        let current_pos = self.pos;
        self.iteration += 1;

        // compute next pos
        if self.t_max.x < self.t_max.y {
            self.pos.x += self.step.x;
            self.t = self.t_max.x;
            self.t_max.x += self.t_delta.x;
        } else {
            self.pos.y += self.step.y;
            self.t = self.t_max.y;
            self.t_max.y += self.t_delta.y;
        }

        Some(current_pos)
    }
}
