use crate::app::App;
use rimui::UIEvent;
use glam::{vec2, Vec2};
use crate::document::ChangeMask;

impl App {
    pub(crate) fn screen_to_document(&self, screen_pos: Vec2)->Vec2 {
        self.view.screen_to_world().transform_point2(screen_pos)
    }
    pub(crate) fn document_to_screen(&self, world_pos: Vec2)->Vec2 {
        self.view.world_to_screen().transform_point2(world_pos)

    }
}

pub(crate) fn operation_pan(app: &App)->impl FnMut(&mut App, &UIEvent) {
    let start_mouse_pos: Vec2 = app.last_mouse_pos.into();
    let start_target = app.view.target;
    move |app, event| {
        match event {
            UIEvent::MouseMove{ pos } => {
                let delta = vec2(pos[0] as f32, pos[1] as f32) - start_mouse_pos;
                app.view.target = start_target - delta / app.view.zoom;
            }
            _ => {}
        }
    }
}

pub(crate) fn operation_stroke(app: &App, value: u8)->impl FnMut(&mut App, &UIEvent) {
    let mut last_mouse_pos: Vec2 = app.last_mouse_pos.into();
    let start_target = app.view.target;
    move |app, event| {
        match event {
            UIEvent::MouseMove{ pos } => {
                let mouse_pos = Vec2::new(pos[0] as f32, pos[1] as f32);
                let document_pos = app.screen_to_document(mouse_pos);
                let mut doc = app.doc.borrow_mut();
                let layer = &mut doc.layer;
                let grid_pos = document_pos / Vec2::splat(layer.cell_size as f32);
                let x = grid_pos.x.floor() as i32;
                let y = grid_pos.y.floor() as i32;
                if x < layer.bounds[0] || x >= layer.bounds[2] || y < layer.bounds[1] || y >= layer.bounds[3] {
                    println!("out of bounds: {}, {}", x, y);
                    // Drawing outside of the grid? Resize it.
                    layer.resize_to_include([x, y]);

                    assert!(x >= layer.bounds[0] && x < layer.bounds[2] && y >= layer.bounds[1] && y < layer.bounds[3]);
                }
                let [w, _] = layer.size();

                layer.cells[(y - layer.bounds[1]) as usize * w as usize + (x - layer.bounds[0]) as usize] = value;
                drop(doc);
                app.dirty_mask.cells = true;
                last_mouse_pos = mouse_pos;
            }
            _ => {}
        }
    }
}
