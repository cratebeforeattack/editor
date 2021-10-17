use glam::{vec2, Vec2};
use rimui::{KeyCode, UIEvent};

use cbmap::MarkupRect;

use crate::app::App;
use crate::document::Layer;
use crate::grid::Grid;
use crate::tool::Tool;
use crate::zone::{AnyZone, EditorTranslate, ZoneRef};

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

        // pan operation
        match self.tool {
            _ => {
                if matches!(event, UIEvent::MouseDown { button: 3, .. }) {
                    let op = operation_pan(self);
                    self.operation = Some((Box::new(op), 3));
                }
            }
        }
        match event {
            UIEvent::MouseDown { button, pos, .. } => {
                // start new operations
                match self.tool {
                    Tool::Pan => {
                        let op = operation_pan(self);
                        self.operation = Some((Box::new(op), button));
                    }
                    Tool::Paint => {
                        if button == 1 || button == 2 {
                            let op = operation_stroke(
                                self,
                                if button == 1 { self.active_material } else { 0 },
                            );
                            self.operation = Some((Box::new(op), button));
                        }
                    }
                    Tool::Fill => {
                        if button == 1 || button == 2 {
                            action_flood_fill(
                                self,
                                pos,
                                if button == 1 { self.active_material } else { 0 },
                            );
                        }
                    }
                    Tool::Rectangle => {
                        if button == 1 || button == 2 {
                            let op = operation_rectangle(
                                self,
                                pos,
                                if button == 1 { self.active_material } else { 0 },
                            );
                            self.operation = Some((Box::new(op), button));
                        }
                    }
                    Tool::Zone => {
                        if button == 1 {
                            let pos = vec2(pos[0] as f32, pos[1] as f32);
                            let mouse_world = self.view.screen_to_world().transform_point2(pos);
                            let hit_result = AnyZone::hit_test_zone_corner(
                                &self.doc.borrow().markup,
                                pos,
                                &self.view,
                            );
                            match hit_result {
                                Some((ZoneRef::Rect(i), corner)) => {
                                    self.doc.borrow_mut().zone_selection = Some(ZoneRef::Rect(i));
                                    let start_rect = self.doc.borrow().markup.rects[i];
                                    let operation = operation_move_zone_corner(
                                        start_rect,
                                        i,
                                        corner,
                                        mouse_world,
                                    );
                                    self.operation = Some((Box::new(operation), button));
                                }
                                _ => {
                                    let new_selection = AnyZone::hit_test_zone(
                                        &self.doc.borrow().markup,
                                        pos,
                                        &self.view,
                                    )
                                    .last()
                                    .copied();
                                    self.doc.borrow_mut().zone_selection = new_selection;

                                    if let Some(selection) = self.doc.borrow().zone_selection {
                                        let start_value =
                                            selection.fetch(&self.doc.borrow().markup);
                                        let operation = operation_move_zone(
                                            start_value,
                                            selection,
                                            mouse_world,
                                        );
                                        self.operation = Some((Box::new(operation), button));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            UIEvent::KeyDown { key, .. } => {
                let material_index = match key {
                    KeyCode::Key1 => Some(1),
                    KeyCode::Key2 => Some(2),
                    KeyCode::Key3 => Some(3),
                    KeyCode::Key4 => Some(4),
                    KeyCode::Key5 => Some(5),
                    KeyCode::Key6 => Some(6),
                    KeyCode::Key7 => Some(7),
                    KeyCode::Key8 => Some(8),
                    KeyCode::Key9 => Some(9),
                    _ => None,
                };
                if let Some(material_index) = material_index {
                    if (material_index as usize) < self.doc.borrow().materials.len() {
                        self.active_material = material_index;
                    }
                }
            }
            _ => {}
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

pub(crate) fn operation_select(
    app: &mut App,
    mouse_pos: [i32; 2],
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos = app.screen_to_document(vec2(mouse_pos[0] as f32, mouse_pos[1] as f32));
    app.push_undo("Select");
    let doc = app.doc.borrow();
    let grid_pos = doc
        .selection
        .world_to_grid_pos(start_pos, doc.cell_size)
        .unwrap_or_else(|e| e);
    drop(doc);
    let [start_x, start_y] = grid_pos;
    let mut last_pos = [start_x, start_y];

    let serialized_selection = bincode::serialize(&app.doc.borrow().selection).unwrap();

    move |app, event| {
        let pos = match event {
            UIEvent::MouseDown { pos, .. } => pos,
            UIEvent::MouseMove { pos } => pos,
            _ => return,
        };
        let mouse_pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        let document_pos = app.screen_to_document(mouse_pos);

        let mut doc = app.doc.borrow_mut();
        let cell_size = doc.cell_size;
        let selection = &mut doc.selection;
        let grid_pos = selection
            .world_to_grid_pos(document_pos, cell_size)
            .unwrap_or_else(|e| e);
        if grid_pos == last_pos {
            return;
        }
        let [x, y] = grid_pos;
        *selection = bincode::deserialize(&serialized_selection).unwrap();
        selection.resize_to_include([grid_pos[0], grid_pos[1], grid_pos[0], grid_pos[1]]);
        doc.selection.rectangle_fill(
            [
                start_x.min(x),
                start_y.min(y),
                x.max(start_x),
                y.max(start_y),
            ],
            1,
        );
        app.dirty_mask.mark_dirty_layer(doc.active_layer);
        last_pos = grid_pos;
    }
}

pub(crate) fn operation_stroke(_app: &App, value: u8) -> impl FnMut(&mut App, &UIEvent) {
    let mut undo_pushed = false;
    move |app, event| {
        match event {
            UIEvent::MouseMove { pos } => {
                let mouse_pos = Vec2::new(pos[0] as f32, pos[1] as f32);
                let document_pos = app.screen_to_document(mouse_pos);
                let active_layer = app.doc.borrow().active_layer;
                let cell_size = app.doc.borrow().cell_size;

                let grid_pos_outside =
                    if let Some(Layer::Grid(layer)) = app.doc.borrow().layers.get(active_layer) {
                        layer.world_to_grid_pos(document_pos, cell_size).err()
                    } else {
                        None
                    };

                // resize, do not forget undo
                if let Some(grid_pos_outside) = grid_pos_outside {
                    if !undo_pushed {
                        app.push_undo("Paint");
                        undo_pushed = true;
                    }

                    // Drawing outside of the grid? Resize it.
                    let mut doc = app.doc.borrow_mut();
                    let mut layer = match doc.layers.get_mut(active_layer) {
                        Some(Layer::Grid(grid)) => grid,
                        _ => return,
                    };
                    let [x, y] = grid_pos_outside;
                    layer.resize_to_include([x, y, x, y]);
                    assert!(
                        x >= layer.bounds[0]
                            && x < layer.bounds[2]
                            && y >= layer.bounds[1]
                            && y < layer.bounds[3]
                    );
                }

                let old_cell_value_index =
                    if let Some(Layer::Grid(layer)) = app.doc.borrow().layers.get(active_layer) {
                        let [x, y] = layer.world_to_grid_pos(document_pos, cell_size).unwrap();
                        let [w, _] = layer.size();

                        let cell_index = (y - layer.bounds[1]) as usize * w as usize
                            + (x - layer.bounds[0]) as usize;
                        Some((layer.cells[cell_index], cell_index))
                    } else {
                        None
                    };

                if let Some((old_cell_value, cell_index)) = old_cell_value_index {
                    if old_cell_value != value {
                        if !undo_pushed {
                            app.push_undo("Paint");
                            undo_pushed = true;
                        }
                        let mut doc = app.doc.borrow_mut();
                        if let Some(Layer::Grid(layer)) = doc.layers.get_mut(active_layer) {
                            layer.cells[cell_index] = value;
                            app.dirty_mask.mark_dirty_layer(doc.active_layer)
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn operation_rectangle(
    app: &mut App,
    mouse_pos: [i32; 2],
    value: u8,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos = app.screen_to_document(vec2(mouse_pos[0] as f32, mouse_pos[1] as f32));
    app.push_undo("Rectangle");

    let active_layer = app.doc.borrow().active_layer;
    let cell_size = app.doc.borrow().cell_size;
    let (grid_pos, serialized_layer) =
        if let Some(Layer::Grid(layer)) = app.doc.borrow_mut().layers.get_mut(active_layer) {
            let grid_pos = layer
                .world_to_grid_pos(start_pos, cell_size)
                .unwrap_or_else(|e| e);
            layer.resize_to_include([grid_pos[0], grid_pos[1], grid_pos[0], grid_pos[1]]);
            (grid_pos, bincode::serialize(&layer).unwrap())
        } else {
            ([0, 0], Vec::new())
        };

    let [start_x, start_y] = grid_pos;
    let mut last_pos = [start_x, start_y];

    move |app, event| {
        let pos = match event {
            UIEvent::MouseDown { pos, .. } => pos,
            UIEvent::MouseMove { pos } => pos,
            _ => return,
        };
        let mouse_pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        let document_pos = app.screen_to_document(mouse_pos);

        let mut doc = app.doc.borrow_mut();
        if let Some(Layer::Grid(layer)) = doc.layers.get_mut(active_layer) {
            let grid_pos = layer
                .world_to_grid_pos(document_pos, cell_size)
                .unwrap_or_else(|e| e);
            if grid_pos == last_pos {
                return;
            }
            let [x, y] = grid_pos;
            *layer = bincode::deserialize(&serialized_layer).unwrap();
            layer.resize_to_include([x, y, x, y]);
            layer.rectangle_outline(
                [
                    start_x.min(x),
                    start_y.min(y),
                    x.max(start_x),
                    y.max(start_y),
                ],
                value,
            );
            app.dirty_mask.mark_dirty_layer(active_layer);
            last_pos = grid_pos;
        }
    }
}

pub(crate) fn action_flood_fill(app: &mut App, mouse_pos: [i32; 2], value: u8) {
    app.push_undo("Fill");
    let world_pos = app.screen_to_document(vec2(mouse_pos[0] as f32, mouse_pos[1] as f32));
    let mut doc = app.doc.borrow_mut();

    let active_layer = doc.active_layer;
    let cell_size = doc.cell_size;
    if let Some(Layer::Grid(layer)) = doc.layers.get_mut(active_layer) {
        if let Ok(pos) = layer.world_to_grid_pos(world_pos, cell_size) {
            Grid::flood_fill(&mut layer.cells, layer.bounds, pos, value);
            app.dirty_mask.mark_dirty_layer(active_layer);
        }
    }
}

fn operation_move_zone_corner(
    start_rect: MarkupRect,
    rect_index: usize,
    corner: u8,
    start_mouse_world: Vec2,
) -> impl FnMut(&mut App, &UIEvent) {
    let mut first_change = true;
    move |app, event| {
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);
        let delta = pos_world - start_mouse_world;
        let mut new_value = start_rect.clone();
        if corner == 0 {
            new_value.start[0] = new_value.start[0] + delta.x as i32;
            new_value.start[1] = new_value.start[1] + delta.y as i32;
        } else {
            new_value.end[0] = new_value.end[0] + delta.x as i32;
            new_value.end[1] = new_value.end[1] + delta.y as i32;
        }
        let min_x = new_value.start[0].min(new_value.end[0]);
        let max_x = new_value.start[0].max(new_value.end[0]);
        let min_y = new_value.start[1].min(new_value.end[1]);
        let max_y = new_value.start[1].max(new_value.end[1]);
        new_value.start[0] = min_x;
        new_value.start[1] = min_y;
        new_value.end[0] = max_x;
        new_value.end[1] = max_y;
        if first_change {
            app.push_undo("Move Zone Corner");
            first_change = false;
        }
        app.doc.borrow_mut().markup.rects[rect_index] = new_value;
    }
}

fn operation_move_zone(
    start_value: AnyZone,
    reference: ZoneRef,
    start_mouse_world: Vec2,
) -> impl FnMut(&mut App, &UIEvent) {
    let mut first_move = true;
    move |app, event| {
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);
        let delta = pos_world - start_mouse_world;
        let mut new_value = start_value.clone();
        if first_move {
            app.push_undo("Move Zone");
            first_move = false;
        }
        new_value.translate([delta.x as i32, delta.y as i32]);
        reference.update(&mut app.doc.borrow_mut().markup, new_value);
    }
}
