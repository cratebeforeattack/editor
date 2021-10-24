use glam::{ivec2, vec2, IVec2, Vec2};
use rimui::{KeyCode, UIEvent};

use cbmap::MarkupRect;

use crate::app::{App, MODIFIER_ALT, MODIFIER_CONTROL, MODIFIER_SHIFT};
use crate::document::Layer;
use crate::graph::{GraphEdge, GraphNode, GraphNodeKey, GraphNodeShape, GraphRef};
use crate::grid::Grid;
use crate::grid_segment_iterator::GridSegmentIterator;
use crate::math::Rect;
use crate::mouse_operation::MouseOperation;
use crate::tool::Tool;
use crate::zone::{AnyZone, EditorTranslate, ZoneRef};
use core::iter::once;
use std::mem::replace;

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
        if self.invoke_operation(&event) {
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
                    self.operation.start(op, 3);
                }
            }
        }
        match event {
            UIEvent::MouseDown { button, pos, .. } => {
                let pos = IVec2::from(pos);
                let mouse_world = self.view.screen_to_world().transform_point2(pos.as_vec2());
                // start new operations
                match self.tool {
                    Tool::Pan => {
                        let op = operation_pan(self);
                        self.operation.start(op, 3)
                    }
                    Tool::Paint => {
                        if button == 1 || button == 2 {
                            let op = operation_stroke(
                                self,
                                if button == 1 { self.active_material } else { 0 },
                            );
                            self.operation.start(op, button);
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
                            self.operation.start(op, button);
                        }
                    }
                    Tool::Zone => {
                        if button == 1 {
                            let hit_result = AnyZone::hit_test_zone_corner(
                                &self.doc.borrow().markup,
                                pos.as_vec2(),
                                &self.view,
                            );
                            match hit_result {
                                Some((ZoneRef::Rect(i), corner)) => {
                                    self.doc.borrow_mut().zone_selection = Some(ZoneRef::Rect(i));
                                    let start_rect = self.doc.borrow().markup.rects[i];
                                    let op = operation_move_zone_corner(
                                        start_rect,
                                        i,
                                        corner,
                                        mouse_world,
                                    );
                                    self.operation.start(op, button);
                                }
                                _ => {
                                    let new_selection = AnyZone::hit_test_zone(
                                        &self.doc.borrow().markup,
                                        pos.as_vec2(),
                                        &self.view,
                                    )
                                    .last()
                                    .copied();
                                    self.doc.borrow_mut().zone_selection = new_selection;

                                    if let Some(selection) = self.doc.borrow().zone_selection {
                                        let start_value =
                                            selection.fetch(&self.doc.borrow().markup);
                                        let op = operation_move_zone(
                                            start_value,
                                            selection,
                                            mouse_world,
                                        );
                                        self.operation.start(op, button);
                                    }
                                }
                            }
                        }
                    }
                    Tool::Graph { .. } => {
                        self.handle_graph_mouse_down(button, pos, mouse_world, &event);
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

                match key {
                    KeyCode::Delete => match self.tool {
                        Tool::Graph => {
                            action_remove_graph_node(self);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }

        // make sure operation is called with invoking event
        self.invoke_operation(&event);

        false
    }

    fn invoke_operation(&mut self, event: &UIEvent) -> bool {
        self.operation_batch.clear();
        if let MouseOperation {
            operation: Some(mut operation),
            button: start_button,
        } = replace(&mut self.operation, MouseOperation::new())
        {
            operation(self, &event);
            let released = match *event {
                UIEvent::MouseUp { button, .. } => button == start_button,
                _ => false,
            };
            if self.operation.operation.is_none() && !released {
                self.operation = MouseOperation {
                    operation: Some(operation),
                    button: start_button,
                };
            }
            return true;
        }
        return false;
    }

    fn handle_graph_mouse_down(
        &mut self,
        button: i32,
        pos: IVec2,
        mouse_world: Vec2,
        _event: &UIEvent,
    ) {
        if button != 1 {
            return;
        }

        let active_layer = self.doc.borrow().active_layer;

        let (mut hover, default_node) =
            if let Some(Layer::Graph(graph)) = self.doc.borrow().layers.get(active_layer) {
                let default_node = match graph.selected.last() {
                    Some(GraphRef::NodeRadius(key) | GraphRef::Node(key)) => {
                        graph.nodes.get(*key).map(|n| n.clone())
                    }
                    _ => None,
                };
                (graph.hit_test(pos.as_vec2(), &self.view), default_node)
            } else {
                (None, None)
            };

        let mut push_undo = true;

        match hover {
            None => {
                if self.modifier_down[MODIFIER_CONTROL] {
                    push_undo = false;
                    hover = action_add_graph_node(self, active_layer, default_node, mouse_world)
                        .map(GraphRef::Node);
                }
            }
            Some(hover) => {
                if matches!(hover, GraphRef::Node { .. }) {
                    // expand/toggle selection
                    let active_layer = active_layer;
                    if let Some(Layer::Graph(graph)) =
                        self.doc.borrow_mut().layers.get_mut(active_layer)
                    {
                        if self.modifier_down[MODIFIER_SHIFT]
                            || self.modifier_down[MODIFIER_CONTROL]
                        {
                            if !graph.selected.contains(&hover) {
                                graph.selected.push(hover);
                            } else {
                                graph.selected.retain(|e| *e != hover);
                            }
                        } else {
                            if !graph.selected.contains(&hover) {
                                graph.selected = once(hover).collect();
                            } else {
                                // start moving nodes below
                            }
                        }
                    }
                }
            }
        }

        let select_hovered = {
            let active_layer = active_layer;
            move |app: &mut App| {
                if !app.modifier_down[MODIFIER_CONTROL] && !app.modifier_down[MODIFIER_SHIFT] {
                    if let Some(Layer::Graph(graph)) =
                        app.doc.borrow_mut().layers.get_mut(active_layer)
                    {
                        graph.selected = hover.iter().cloned().collect();
                    }
                }
            }
        };

        match hover {
            Some(hover @ GraphRef::Node { .. }) => {
                if self.modifier_down[MODIFIER_ALT] {
                    let op = operation_graph_paint_selection(self, SelectOperation::Substract);
                    self.operation.start(op, button);
                } else if self.modifier_down[MODIFIER_SHIFT] {
                    let op = operation_graph_paint_selection(self, SelectOperation::Extend);
                    self.operation.start(op, button);
                } else {
                    let op =
                        operation_move_graph_node(self, mouse_world, push_undo, select_hovered);
                    self.operation.start(op, button);
                }
            }
            Some(GraphRef::NodeRadius(key)) => {
                let op = operation_move_graph_node_radius(self, key);
                self.operation.start(op, button);
            }
            _ => {
                // start rectangle selection
                let op = operation_graph_rectangle_selection(
                    self,
                    if self.modifier_down[MODIFIER_ALT] {
                        SelectOperation::Substract
                    } else if self.modifier_down[MODIFIER_SHIFT] {
                        SelectOperation::Extend
                    } else {
                        SelectOperation::Replace
                    },
                );
                self.operation.start(op, button);
            }
        }
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
    let start_pos: [IVec2; 2] = Rect::from_point(grid_pos);
    let mut last_pos = grid_pos;

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
        *selection = bincode::deserialize(&serialized_selection).unwrap();
        selection.resize_to_include_amortized(Rect::from_point(grid_pos));
        let rect = start_pos.union(Rect::from_point(grid_pos));
        doc.selection.rectangle_fill(rect, 1);
        app.dirty_mask.mark_dirty_layer(doc.active_layer);
        last_pos = grid_pos;
    }
}

pub(crate) fn operation_stroke(app: &mut App, value: u8) -> impl FnMut(&mut App, &UIEvent) {
    let mut undo_pushed = false;
    let mut last_document_pos = app.screen_to_document(app.last_mouse_pos);
    move |app, _event| {
        let mouse_pos = app.last_mouse_pos;
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
            let layer = match doc.layers.get_mut(active_layer) {
                Some(Layer::Grid(grid)) => grid,
                _ => return,
            };
            layer.resize_to_include_amortized(Rect::from_point(grid_pos_outside));
            assert!(layer.bounds.contains_point(grid_pos_outside));
        }

        let cell_index = if let Some(Layer::Grid(layer)) = app.doc.borrow().layers.get(active_layer)
        {
            let pos = layer.world_to_grid_pos(document_pos, cell_size).unwrap();
            let w = layer.size().x;

            let cell_index = (pos.y - layer.bounds[0].y) as usize * w as usize
                + (pos.x - layer.bounds[0].x) as usize;
            Some(cell_index)
        } else {
            None
        };

        if cell_index.is_some() {
            if !undo_pushed {
                app.push_undo("Paint");
                undo_pushed = true;
            }
            let mut doc = app.doc.borrow_mut();
            if let Some(Layer::Grid(layer)) = doc.layers.get_mut(active_layer) {
                for pos in GridSegmentIterator::new(
                    last_document_pos,
                    document_pos,
                    Vec2::ZERO,
                    Vec2::splat(cell_size as f32),
                    1024,
                ) {
                    if layer.bounds.contains_point(pos) {
                        let cell_index = layer.grid_pos_index(pos.x, pos.y);
                        if layer.cells[cell_index] != value {
                            layer.cells[cell_index] = value;
                            app.dirty_mask.mark_dirty_layer(active_layer)
                        }
                    }
                }
            }
        }
        last_document_pos = document_pos;
    }
}

pub(crate) fn operation_rectangle(
    app: &mut App,
    mouse_pos: IVec2,
    value: u8,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos = app.screen_to_document(mouse_pos.as_vec2());
    app.push_undo("Rectangle");

    let active_layer = app.doc.borrow().active_layer;
    let cell_size = app.doc.borrow().cell_size;
    let (grid_pos, serialized_layer) =
        if let Some(Layer::Grid(layer)) = app.doc.borrow_mut().layers.get_mut(active_layer) {
            let grid_pos = layer
                .world_to_grid_pos(start_pos, cell_size)
                .unwrap_or_else(|e| e);
            layer.resize_to_include_amortized(Rect::from_point(grid_pos));
            (grid_pos, bincode::serialize(&layer).unwrap())
        } else {
            (IVec2::ZERO, Vec::new())
        };

    let start_pos: [IVec2; 2] = Rect::from_point(grid_pos);
    let mut last_pos = grid_pos;

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
            *layer = bincode::deserialize(&serialized_layer).unwrap();
            layer.resize_to_include_amortized(Rect::from_point(grid_pos));
            layer.rectangle_outline(start_pos.union(Rect::from_point(grid_pos)), value);
            app.dirty_mask.mark_dirty_layer(active_layer);
            last_pos = grid_pos;
        }
    }
}

pub(crate) fn action_flood_fill(app: &mut App, mouse_pos: IVec2, value: u8) {
    app.push_undo("Fill");
    let world_pos = app.screen_to_document(mouse_pos.as_vec2());
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
    move |app, _event| {
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
    move |app, _event| {
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

fn action_add_graph_node(
    app: &mut App,
    layer: usize,
    default_node: Option<GraphNode>,
    world_pos: Vec2,
) -> Option<GraphNodeKey> {
    app.push_undo("Add Graph Node");
    let cell_size = app.doc.borrow().cell_size as f32;
    let result = if let Some(Layer::Graph(graph)) = app.doc.borrow_mut().layers.get_mut(layer) {
        let prev_node = match graph.selected.last() {
            Some(GraphRef::Node(key) | GraphRef::NodeRadius(key)) => Some(*key),
            _ => None,
        };
        let pos = ((world_pos / cell_size).floor() * cell_size).as_ivec2();
        let key = graph.nodes.insert(GraphNode {
            pos,
            ..default_node.unwrap_or(GraphNode {
                pos: IVec2::ZERO,
                radius: 192,
                shape: GraphNodeShape::Octogon,
                no_outline: false,
            })
        });

        if let Some(prev_node) = prev_node {
            // connect with previously selection node
            graph.edges.insert(GraphEdge {
                start: prev_node,
                end: key,
            });
        }
        graph.selected = vec![GraphRef::Node(key)];
        Some(key)
    } else {
        None
    };

    app.dirty_mask.mark_dirty_layer(layer);
    result
}

fn action_remove_graph_node(app: &mut App) {
    let active_layer = app.doc.borrow().active_layer;

    let can_delete = if let Some(Layer::Graph(graph)) = app.doc.borrow().layers.get(active_layer) {
        graph.selected.iter().any(|n| match n {
            GraphRef::Node { .. } | GraphRef::NodeRadius { .. } => true,
            _ => false,
        })
    } else {
        false
    };

    if can_delete {
        app.push_undo("Remove Graph Element");
        if let Some(Layer::Graph(graph)) = app.doc.borrow_mut().layers.get_mut(active_layer) {
            let mut removed_nodes = Vec::new();
            let mut removed_edges = Vec::new();
            for selection in &graph.selected {
                match *selection {
                    GraphRef::Node(key) => {
                        removed_nodes.push(key);
                    }
                    GraphRef::Edge(key) => {
                        removed_edges.push(key);
                    }
                    _ => {}
                }
            }

            // mark edges of removed nodes
            for (key, edge) in &graph.edges {
                if removed_nodes.contains(&edge.start) || removed_nodes.contains(&edge.end) {
                    removed_edges.push(key);
                }
            }

            graph.selected.retain(|s| match s {
                GraphRef::NodeRadius(key) | GraphRef::Node(key) => !removed_nodes.contains(&key),
                GraphRef::Edge(key) => !removed_edges.contains(key),
            });
            if !removed_edges.is_empty() {
                graph.edges.retain(|key, _| !removed_edges.contains(&key));
            }
            if !removed_nodes.is_empty() {
                graph.nodes.retain(|key, _| !removed_nodes.contains(&key))
            }
        }
        app.dirty_mask.mark_dirty_layer(active_layer);
    }
}

fn operation_move_graph_node(
    app: &App,
    start_pos_world: Vec2,
    push_undo: bool,
    mut click_action: impl FnMut(&mut App),
) -> impl FnMut(&mut App, &UIEvent) {
    let doc = app.doc.borrow();

    let start_positions: Vec<_> =
        if let Some(Layer::Graph(graph)) = doc.layers.get(doc.active_layer) {
            graph
                .selected
                .iter()
                .filter_map(|s| match *s {
                    GraphRef::Node(key) => graph.nodes.get(key).map(|n| n.pos),
                    _ => None,
                })
                .collect()
        } else {
            vec![]
        };
    drop(doc);
    let mut changed = false;
    move |app, event| {
        if start_positions.is_empty() {
            click_action(app);
            return;
        }
        match event {
            UIEvent::MouseUp { .. } => {
                if !changed {
                    click_action(app);
                    return;
                }
            }
            _ => {}
        }
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);

        let delta = pos_world - start_pos_world;

        if delta != Vec2::ZERO && !changed {
            if push_undo {
                app.push_undo("Move Graph Node");
            }

            changed = true;
        }

        let mut doc = app.doc.borrow_mut();
        let active_layer = doc.active_layer;
        let cell_size = doc.cell_size;
        if let Some(Layer::Graph(graph)) = doc.layers.get_mut(active_layer) {
            let selected_nodes = graph.selected.iter().filter_map(|s| match *s {
                GraphRef::Node(key) => Some(key),
                _ => None,
            });

            for (node_key, start_pos) in selected_nodes.zip(start_positions.iter().cloned()) {
                let node = &mut graph.nodes[node_key];
                let mut new_pos = start_pos.as_vec2() + delta;

                // snap to grid
                let snap_step = cell_size as f32;
                new_pos = (new_pos / snap_step).round() * snap_step;

                node.pos = new_pos.floor().as_ivec2();
            }
        }
        drop(doc);
        app.dirty_mask.mark_dirty_layer(active_layer);
    }
}

fn operation_move_graph_node_radius(
    app: &App,
    edited_key: GraphNodeKey,
) -> impl FnMut(&mut App, &UIEvent) {
    let mut push_undo = true;
    move |app, _event| {
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);

        if push_undo {
            app.push_undo("Resize Graph Node");
            push_undo = false;
        }

        let mut doc = app.doc.borrow_mut();
        let active_layer = doc.active_layer;
        let cell_size = doc.cell_size;
        if let Some(Layer::Graph(graph)) = doc.layers.get_mut(active_layer) {
            let edited_pos = match graph.nodes.get(edited_key) {
                Some(n) => n.pos,
                _ => return,
            };
            for selection in &graph.selected {
                match *selection {
                    GraphRef::Node(key) | GraphRef::NodeRadius(key) => {
                        if let Some(node) = graph.nodes.get_mut(key) {
                            let mut new_radius = (pos_world - edited_pos.as_vec2()).length();

                            let snap_step = cell_size as f32;
                            new_radius = (new_radius / snap_step).round() * (snap_step);
                            node.radius = new_radius as usize;
                        }
                    }
                    _ => {}
                }
            }
        }
        drop(doc);
        app.dirty_mask.mark_dirty_layer(active_layer);
    }
}

enum SelectOperation {
    Replace,
    Extend,
    Substract,
}

fn operation_graph_rectangle_selection(
    app: &mut App,
    operation: SelectOperation,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos: [Vec2; 2] = Rect::from_point(app.last_mouse_pos);

    let active_layer = app.doc.borrow().active_layer;
    let start_selection = match operation {
        SelectOperation::Replace => vec![],
        SelectOperation::Extend | SelectOperation::Substract => {
            if let Some(Layer::Graph(graph)) = app.doc.borrow().layers.get(active_layer) {
                graph.selected.clone()
            } else {
                vec![]
            }
        }
    };

    let mut changed = false;
    move |app, event| {
        if app.last_mouse_pos != start_pos[0] && !changed {
            app.push_undo("Select Nodes");
            changed = true;
        }
        let rect = start_pos.union(Rect::from_point(app.last_mouse_pos));

        let mut doc = app.doc.borrow_mut();
        let mut new_selection = start_selection.clone();
        let active_layer = doc.active_layer;
        if let Some(Layer::Graph(graph)) = doc.layers.get_mut(active_layer) {
            for (node_key, node) in &graph.nodes {
                let [min, max] = node.bounds();
                let bounds = [
                    app.view.world_to_screen().transform_point2(min),
                    app.view.world_to_screen().transform_point2(max),
                ];

                if bounds.intersect(rect).is_some() {
                    match operation {
                        SelectOperation::Substract => {
                            new_selection.retain(|e| *e != GraphRef::Node(node_key))
                        }
                        SelectOperation::Extend | SelectOperation::Replace => {
                            if !new_selection.contains(&GraphRef::Node(node_key)) {
                                new_selection.push(GraphRef::Node(node_key));
                            }
                        }
                    }
                }
            }
            if graph.selected != new_selection {
                graph.selected = new_selection;
            }
        }
        drop(doc);

        app.operation_batch.set_image(app.white_texture);

        app.operation_batch
            .geometry
            .fill_rect(rect[0], rect[1], [255, 255, 255, 32]);
        app.operation_batch
            .geometry
            .stroke_rect(rect[0], rect[1], 1.0, [255, 255, 255, 128]);
    }
}

fn operation_graph_paint_selection(
    app: &mut App,
    operation: SelectOperation,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos = app.last_mouse_pos;

    let mut changed = false;
    move |app, event| {
        if app.last_mouse_pos != start_pos && !changed {
            app.push_undo("Select Nodes");
            changed = true;
        }
        let mut doc = app.doc.borrow_mut();
        let active_layer = doc.active_layer;
        if let Some(Layer::Graph(graph)) = doc.layers.get_mut(active_layer) {
            let mut new_selection = graph.selected.clone();
            for (node_key, node) in &graph.nodes {
                let [min, max] = node.bounds();
                let bounds = [
                    app.view.world_to_screen().transform_point2(min),
                    app.view.world_to_screen().transform_point2(max),
                ];

                if bounds.contains_point(app.last_mouse_pos) {
                    match operation {
                        SelectOperation::Substract => {
                            new_selection.retain(|e| *e != GraphRef::Node(node_key))
                        }
                        SelectOperation::Extend | SelectOperation::Replace => {
                            if !new_selection.contains(&GraphRef::Node(node_key)) {
                                new_selection.push(GraphRef::Node(node_key));
                            }
                        }
                    }
                }
            }
            if new_selection != graph.selected {
                graph.selected = new_selection;
            }
        }
    }
}
