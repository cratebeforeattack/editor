use crate::app::App;
use rimui::UIEvent;
use glam::{vec2, Vec2};

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
