use crate::app::App;
use crate::tool::Tool;
use glam::{vec2, Vec2};
use rimui::UIEvent;

impl App {
    pub(crate) fn screen_to_document(&self, screen_pos: Vec2) -> Vec2 {
        self.view.screen_to_world().transform_point2(screen_pos)
    }
    pub(crate) fn document_to_screen(&self, world_pos: Vec2) -> Vec2 {
        self.view.world_to_screen().transform_point2(world_pos)
    }

    pub fn handle_event(&mut self, event: UIEvent) -> bool {
        // handle zoom
        match event {
            UIEvent::MouseWheel { pos: _, delta } => {
                let mult = if delta < 0.0 { 0.5 } else { 2.0 };
                self.view.zoom_target = (self.view.zoom_target * mult).clamp(0.125, 16.0);
            }
            _ => {}
        }

        // handle current mouse operation
        if let Some((mut action, start_button)) = self.operation.take() {
            action(self, &event);
            let released = match event {
                UIEvent::MouseUp { button, .. } => button == start_button,
                _ => false,
            };
            if self.operation.is_none() && !released {
                self.operation = Some((action, start_button));
            }
            return true;
        }

        // provide event to UI
        let render_rect = [0, 0, self.window_size[0] as i32, self.window_size[1] as i32];
        if self
            .ui
            .handle_event(&event, render_rect, miniquad::date::now() as f32)
        {
            return true;
        }

        // start new operations
        match self.tool {
            Tool::Pan => match event {
                UIEvent::MouseDown { button, .. } => {
                    let op = operation_pan(self);
                    self.operation = Some((Box::new(op), button))
                }
                _ => {}
            },
            Tool::Paint => match event {
                UIEvent::MouseDown { button, .. } => {
                    if button == 1 || button == 2 {
                        let op = operation_stroke(self, if button == 1 { 1 } else { 0 });
                        self.operation = Some((Box::new(op), button))
                    } else {
                        let op = operation_pan(self);
                        self.operation = Some((Box::new(op), button))
                    }
                }
                _ => {}
            },
        }
        false
    }
}

pub(crate) fn operation_pan(app: &App) -> impl FnMut(&mut App, &UIEvent) {
    let start_mouse_pos: Vec2 = app.last_mouse_pos.into();
    let start_target = app.view.target;
    move |app, event| match event {
        UIEvent::MouseMove { pos } => {
            let delta = vec2(pos[0] as f32, pos[1] as f32) - start_mouse_pos;
            app.view.target = start_target - delta / app.view.zoom;
        }
        _ => {}
    }
}

pub(crate) fn operation_stroke(app: &App, value: u8) -> impl FnMut(&mut App, &UIEvent) {
    let mut last_mouse_pos: Vec2 = app.last_mouse_pos.into();
    let mut undo_pushed = false;
    move |app, event| {
        match event {
            UIEvent::MouseMove { pos } => {
                let mouse_pos = Vec2::new(pos[0] as f32, pos[1] as f32);
                let document_pos = app.screen_to_document(mouse_pos);
                let mut doc = app.doc.borrow_mut();
                let layer = &mut doc.layer;
                let grid_pos = document_pos / Vec2::splat(layer.cell_size as f32);
                let x = grid_pos.x.floor() as i32;
                let y = grid_pos.y.floor() as i32;
                if x < layer.bounds[0]
                    || x >= layer.bounds[2]
                    || y < layer.bounds[1]
                    || y >= layer.bounds[3]
                {
                    println!("out of bounds: {}, {}", x, y);
                    // Drawing outside of the grid? Resize it.
                    layer.resize_to_include([x, y]);

                    assert!(
                        x >= layer.bounds[0]
                            && x < layer.bounds[2]
                            && y >= layer.bounds[1]
                            && y < layer.bounds[3]
                    );
                }
                let [w, _] = layer.size();

                let cell_index =
                    (y - layer.bounds[1]) as usize * w as usize + (x - layer.bounds[0]) as usize;
                let old_cell_value = layer.cells[cell_index];
                drop(doc);
                if old_cell_value != value {
                    if !undo_pushed {
                        app.push_undo("Paint");
                        undo_pushed = true;
                    }
                    let mut doc = app.doc.borrow_mut();
                    doc.layer.cells[cell_index] = value;
                    app.dirty_mask.cells = true;
                }
                last_mouse_pos = mouse_pos;
            }
            _ => {}
        }
    }
}
