use crate::app::App;
use rimui::UIEvent;
use glam::{vec2, Vec2};

impl App {
    fn screen_to_document(&self, screen_pos: Vec2)->Vec2 {
        screen_pos + self.view.target
    }
    fn document_to_screen(&self, screen_pos: Vec2)->Vec2 {
        screen_pos - self.view.target
    }
}

pub(crate) fn operation_pan(app: &App)->impl FnMut(&mut App, &UIEvent) {
    let start_mouse_pos: Vec2 = app.last_mouse_pos.into();
    let start_target = app.view.target;
    move |app, event| {
        match event {
            UIEvent::MouseMove{ pos } => {
                let delta = vec2(pos[0] as f32, pos[1] as f32) - start_mouse_pos;
                app.view.target = start_target + delta;
            }
            _ => {}
        }
    }
}

pub(crate) fn operation_stroke(app: &App)->impl FnMut(&mut App, &UIEvent) {
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
                let mut x = grid_pos.x.floor() as i32;
                let mut y = grid_pos.y.floor() as i32;
                let [mut w, mut h] = layer.size();
                if x < layer.bounds[0] || x >= layer.bounds[2] || y < layer.bounds[1] || y >= layer.bounds[3] {
                    println!("out of bounds: {}, {}", x, y);
                    // Drawing outside of the grid? Resize it.
                    layer.resize_to_include([x, y]);

                    let grid_pos = document_pos / Vec2::splat(layer.cell_size as f32);
                    x = grid_pos.x.floor() as i32;
                    y = grid_pos.y.floor() as i32;
                    w = layer.size()[0];
                    h = layer.size()[1];

                    assert!(x >= layer.bounds[0] && x < layer.bounds[2] && y >= layer.bounds[1] && y < layer.bounds[3]);
                }
                layer.cells[(y - layer.bounds[1]) as usize * w as usize + (x - layer.bounds[0])  as usize] = 1;
                last_mouse_pos = mouse_pos;
            }
            _ => {}
        }
    }
}
