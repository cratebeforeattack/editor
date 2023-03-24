use glam::{vec2, IVec2, Vec2};
use rimui::{KeyCode, UIEvent};

use crate::app::{App, MODIFIER_ALT, MODIFIER_CONTROL, MODIFIER_SHIFT};
use crate::document::{Document, LayerKey, SelectRef, Vec2Ord};
use crate::graph::{GraphEdge, GraphNode, GraphNodeKey, SplitPos};
use crate::grid::Grid;
use crate::grid_segment_iterator::GridSegmentIterator;
use crate::math::Rect;
use crate::mouse_operation::MouseOperation;
use crate::plant::{Plant, PlantKey};
use crate::tool::Tool;
use crate::zone::{AnyZone, EditorTranslate, ZoneRef};
use core::iter::once;
use miniquad::Context;
use std::collections::BTreeSet;
use std::mem::replace;

impl App {
    pub(crate) fn screen_to_document(&self, screen_pos: Vec2) -> Vec2 {
        self.view.screen_to_world().transform_point2(screen_pos)
    }
    pub(crate) fn document_to_screen(&self, world_pos: Vec2) -> Vec2 {
        self.view.world_to_screen().transform_point2(world_pos)
    }

    pub fn handle_event(&mut self, event: UIEvent, context: &mut Context) -> bool {
        // handle zoom
        match event {
            UIEvent::MouseWheel { pos: _, delta } => {
                let mult = if delta < 0.0 { 0.5 } else { 2.0 };
                self.view.zoom_target = (self.view.zoom_target * mult).clamp(0.125, 16.0);
            }
            _ => {}
        }

        // handle current mouse operation
        if self.invoke_operation(&event, context) {
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
                    self.operation.start(op, 3, context);
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
                        self.operation.start(op, button, context)
                    }
                    Tool::Paint => {
                        if button == 1 || button == 2 {
                            let op = operation_stroke(
                                self,
                                if button == 1 { self.active_material } else { 0 },
                            );
                            self.operation.start(op, button, context);
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
                            self.operation.start(op, button, context);
                        }
                    }
                    Tool::Zone => {
                        if button == 1 {
                            let hit_result = AnyZone::hit_test_zone_corner(
                                &self.doc.markup,
                                pos.as_vec2(),
                                &self.view,
                            );
                            match hit_result {
                                Some((
                                    reference @ (ZoneRef::Rect(_) | ZoneRef::Segment(_)),
                                    corner,
                                )) => {
                                    self.doc.zone_selection = Some(reference);
                                    let start_rect = reference.fetch(&self.doc.markup);
                                    let op = operation_move_zone_corner(
                                        start_rect,
                                        reference,
                                        corner,
                                        mouse_world,
                                    );
                                    self.operation.start(op, button, context);
                                }
                                _ => {
                                    let new_selection = AnyZone::hit_test_zone(
                                        &self.doc.markup,
                                        pos.as_vec2(),
                                        &self.view,
                                    )
                                    .last()
                                    .copied();
                                    self.doc.zone_selection = new_selection;

                                    if let Some(selection) = self.doc.zone_selection {
                                        let start_value = selection.fetch(&self.doc.markup);
                                        let op = operation_move_zone(
                                            start_value,
                                            selection,
                                            mouse_world,
                                        );
                                        self.operation.start(op, button, context);
                                    }
                                }
                            }
                        }
                    }
                    Tool::Select { .. } => {
                        self.handle_select_mouse_down(button, pos, mouse_world, &event, context);
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
                    if (material_index as usize) < self.doc.materials.len() {
                        self.active_material = material_index;
                    }
                }

                match key {
                    KeyCode::Delete => match self.tool {
                        Tool::Select => {
                            action_delete_selection(self);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }

        // make sure operation is called with invoking event
        self.invoke_operation(&event, context);

        false
    }

    fn invoke_operation(&mut self, event: &UIEvent, context: &mut Context) -> bool {
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
            if self.operation.operation.is_none() {
                if !released {
                    self.operation = MouseOperation {
                        operation: Some(operation),
                        button: start_button,
                    };
                } else {
                    self.locked_hover.take();
                    context.set_cursor_grab(false);
                }
            }
            return true;
        }
        return false;
    }

    fn handle_select_mouse_down(
        &mut self,
        button: i32,
        pos: IVec2,
        mouse_world: Vec2,
        _event: &UIEvent,
        context: &mut Context,
    ) {
        if button != 1 {
            return;
        }

        let default_node = match self.doc.selected.last() {
            Some(SelectRef::NodeRadius(key) | SelectRef::Node(key)) => {
                self.doc.nodes.get(*key).map(|n| n.clone())
            }
            _ => None,
        };
        let mut hover = self.doc.hit_test(pos.as_vec2(), &self.view);

        let mut push_undo = true;

        match hover {
            None => {
                if self.modifier_down[MODIFIER_CONTROL] {
                    push_undo = false;
                    let current_layer = self.doc.current_layer;
                    hover = Some(SelectRef::Node(action_add_graph_node(
                        self,
                        current_layer,
                        default_node,
                        mouse_world,
                    )))
                }
            }
            Some(hover) => match hover {
                SelectRef::Node { .. } | SelectRef::Plant { .. } => {
                    // expand/toggle selection
                    if self.modifier_down[MODIFIER_SHIFT] || self.modifier_down[MODIFIER_CONTROL] {
                        if !self.doc.selected.contains(&hover) {
                            self.doc.selected.push(hover);
                        } else {
                            self.doc.selected.retain(|e| *e != hover);
                        }
                    } else {
                        if !self.doc.selected.contains(&hover) {
                            self.doc.selected = once(hover).collect();
                        } else {
                            // start moving nodes below
                        }
                    }
                }
                SelectRef::NodeRadius(node_key) => {
                    if self.doc.selected.iter().all(|s| match *s {
                        SelectRef::Node(node) | SelectRef::NodeRadius(node) => node != node_key,
                        SelectRef::Edge(_) => true,
                        SelectRef::EdgePoint(_, _) => true,
                        SelectRef::Plant(_) | SelectRef::PlantDirection(_) => true,
                        SelectRef::Point(_) => true,
                    }) {
                        // change selection if we are trying to resize node that is not being selected
                        self.doc.selected = once(hover).collect();
                    }
                }
                _ => {}
            },
        }

        let select_hovered = {
            move |app: &mut App| {
                if !app.modifier_down[MODIFIER_CONTROL] && !app.modifier_down[MODIFIER_SHIFT] {
                    app.doc.selected = hover.iter().cloned().collect();
                }
            }
        };

        match hover {
            Some(SelectRef::Node { .. } | SelectRef::Plant { .. }) => {
                if self.modifier_down[MODIFIER_ALT] {
                    let op = operation_paint_selection(self, SelectOperation::Substract);
                    self.operation.start(op, button, context);
                } else if self.modifier_down[MODIFIER_SHIFT] {
                    let op = operation_paint_selection(self, SelectOperation::Extend);
                    self.operation.start(op, button, context);
                } else {
                    let op = operation_move_selection(self, mouse_world, push_undo, select_hovered);
                    self.operation.start(op, button, context);
                }
            }
            Some(SelectRef::NodeRadius(key)) => {
                let op = operation_move_graph_node_radius(self, key);
                self.operation.start(op, button, context);
            }
            Some(SelectRef::PlantDirection(key)) => {
                let op = operation_move_plant_direction(self, key);
                self.operation.start(op, button, context);
            }
            Some(hover @ SelectRef::EdgePoint { .. }) => {
                self.doc.selected = once(hover).collect();
                let op = operation_move_selection(self, mouse_world, true, |_| {});
                self.operation.start(op, button, context);
            }
            _ => {
                // start rectangle selection
                let op = operation_rectangle_selection(
                    self,
                    if self.modifier_down[MODIFIER_ALT] {
                        SelectOperation::Substract
                    } else if self.modifier_down[MODIFIER_SHIFT] {
                        SelectOperation::Extend
                    } else {
                        SelectOperation::Replace
                    },
                );
                self.operation.start(op, button, context);
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

pub(crate) fn operation_stroke(app: &mut App, value: u8) -> impl FnMut(&mut App, &UIEvent) {
    let mut undo_pushed = false;
    let mut last_document_pos = app.screen_to_document(app.last_mouse_pos);
    move |app, _event| {
        let mouse_pos = app.last_mouse_pos;
        let document_pos = app.screen_to_document(mouse_pos);
        let current_layer = app.doc.current_layer;
        let cell_size = app.doc.cell_size;

        let grid_key = Document::get_or_add_layer_grid(
            &mut app.doc.layers,
            app.doc.current_layer,
            &mut app.doc.grids,
        );

        let grid_pos_outside = app
            .doc
            .grids
            .get(grid_key)
            .map(|grid| grid.world_to_grid_pos(document_pos, cell_size).err())
            .flatten();

        // resize, do not forget undo
        if let Some(grid_pos_outside) = grid_pos_outside {
            if !undo_pushed {
                app.push_undo("Paint");
                undo_pushed = true;
            }

            // Drawing outside of the grid? Resize it.
            let Some(grid) = app.doc.grids.get_mut(grid_key) else { return };
            grid.resize_to_include_amortized(Rect::from_point(grid_pos_outside));
            assert!(grid.bounds.contains_point(grid_pos_outside));
        }

        let doc = &app.doc;
        let cell_index = if let Some(grid) = doc.grids.get(grid_key) {
            let pos = grid.world_to_grid_pos(document_pos, cell_size).unwrap();
            let w = grid.size().x;

            let cell_index = (pos.y - grid.bounds[0].y) as usize * w as usize
                + (pos.x - grid.bounds[0].x) as usize;
            Some(cell_index)
        } else {
            None
        };

        if cell_index.is_some() {
            if !undo_pushed {
                app.push_undo("Paint");
                undo_pushed = true;
            }
            let doc = &mut app.doc;
            if let Some(layer) = doc.grids.get_mut(grid_key) {
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
                            app.dirty_mask.mark_dirty_layer(current_layer)
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

    let current_layer = app.doc.current_layer;
    let cell_size = app.doc.cell_size;
    let grid_key = Document::get_or_add_layer_grid(
        &mut app.doc.layers,
        app.doc.current_layer,
        &mut app.doc.grids,
    );
    let (grid_pos, serialized_layer) = if let Some(grid) = app.doc.grids.get_mut(grid_key) {
        let grid_pos = grid
            .world_to_grid_pos(start_pos, cell_size)
            .unwrap_or_else(|e| e);
        grid.resize_to_include_amortized(Rect::from_point(grid_pos));
        (grid_pos, bincode::serialize(&grid).unwrap())
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

        let doc = &mut app.doc;
        if let Some(grid) = doc.grids.get_mut(grid_key) {
            let grid_pos = grid
                .world_to_grid_pos(document_pos, cell_size)
                .unwrap_or_else(|e| e);
            if grid_pos == last_pos {
                return;
            }
            *grid = bincode::deserialize(&serialized_layer).unwrap();
            grid.resize_to_include_amortized(Rect::from_point(grid_pos));
            grid.rectangle_outline(start_pos.union(Rect::from_point(grid_pos)), value);
            app.dirty_mask.mark_dirty_layer(current_layer);
            last_pos = grid_pos;
        }
    }
}

pub(crate) fn action_flood_fill(app: &mut App, mouse_pos: IVec2, value: u8) {
    app.push_undo("Fill");
    let world_pos = app.screen_to_document(mouse_pos.as_vec2());
    let doc = &mut app.doc;

    let current_layer = doc.current_layer;
    let cell_size = doc.cell_size;
    let grid_key =
        Document::get_or_add_layer_grid(&mut doc.layers, doc.current_layer, &mut doc.grids);
    if let Some(grid) = doc.grids.get_mut(grid_key) {
        if let Ok(pos) = grid.world_to_grid_pos(world_pos, cell_size) {
            Grid::flood_fill(&mut grid.cells, grid.bounds, pos, value, 0);
            app.dirty_mask.mark_dirty_layer(current_layer);
        }
    }
}

fn operation_move_zone_corner(
    start_rect: AnyZone,
    reference: ZoneRef,
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
        let old_value = new_value.get_corner(corner as usize);
        new_value.update_corner(
            corner as usize,
            [old_value[0] + delta.x as i32, old_value[1] + delta.y as i32],
        );
        if matches!(reference, ZoneRef::Rect(_)) {
            let min_x = new_value.get_corner(0)[0].min(new_value.get_corner(1)[0]);
            let max_x = new_value.get_corner(0)[0].max(new_value.get_corner(1)[0]);
            let min_y = new_value.get_corner(0)[1].min(new_value.get_corner(1)[1]);
            let max_y = new_value.get_corner(0)[1].max(new_value.get_corner(1)[1]);
            new_value.update_corner(0, [min_x, min_y]);
            new_value.update_corner(1, [max_x, max_y]);
        }
        if first_change {
            app.push_undo("Move Zone Corner");
            first_change = false;
        }
        reference.update(&mut app.doc.markup, new_value);
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
        reference.update(&mut app.doc.markup, new_value);
    }
}

pub fn action_add_graph_node(
    app: &mut App,
    layer_key: LayerKey,
    mut default_node: Option<GraphNode>,
    world_pos: Vec2,
) -> GraphNodeKey {
    app.push_undo("Add Graph Node");
    let cell_size = app.doc.cell_size as f32;

    let doc = &mut app.doc;

    let prev_node = match doc.selected.last().cloned() {
        Some(SelectRef::Node(key) | SelectRef::NodeRadius(key)) => Some(key),
        Some(SelectRef::EdgePoint(key, pos)) => {
            let split_node =
                GraphNode::split_edge_node(&doc.nodes, &doc.edges, key, SplitPos::Fraction(*pos));
            let node_key = doc.nodes.insert(split_node);
            let split_node_key = GraphEdge::split_edge(&mut doc.edges, key, node_key);
            default_node = Some(doc.nodes[split_node_key].clone());
            Some(split_node_key)
        }
        _ => None,
    };
    let pos = ((world_pos / cell_size).floor() * cell_size).as_ivec2();
    let key = doc.nodes.insert(GraphNode {
        pos,
        layer: layer_key,
        ..default_node.unwrap_or(GraphNode::new())
    });

    if let Some(prev_node) = prev_node {
        // connect with previously selection node
        doc.edges.insert(GraphEdge {
            start: prev_node,
            end: key,
        });
    }
    doc.selected = vec![SelectRef::Node(key)];

    app.dirty_mask.mark_dirty_layer(layer_key);
    key
}

pub fn action_add_plant(app: &mut App, layer_key: LayerKey, world_pos: Vec2) -> PlantKey {
    app.push_undo("Add Plant");
    let cell_size = app.doc.cell_size as f32;

    let doc = &mut app.doc;
    let pos = ((world_pos / cell_size).floor() * cell_size).as_ivec2();
    let key = doc.plants.insert(Plant {
        pos,
        layer: layer_key,
        ..Plant::new()
    });

    doc.selected = vec![SelectRef::Plant(key)];

    app.dirty_mask.mark_dirty_layer(layer_key);
    key
}

fn action_delete_selection(app: &mut App) {
    let can_delete = {
        app.doc.selected.iter().any(|n| match n {
            SelectRef::Node { .. } | SelectRef::NodeRadius { .. } => true,
            SelectRef::Plant { .. } => true,
            _ => false,
        })
    };

    if can_delete {
        app.push_undo("Delete Selection");
        let mut removed_nodes = Vec::new();
        let mut removed_edges = Vec::new();
        let mut removed_plants = Vec::new();
        for selection in &app.doc.selected {
            match *selection {
                SelectRef::Node(key) => {
                    removed_nodes.push(key);
                }
                SelectRef::Edge(key) => {
                    removed_edges.push(key);
                }
                SelectRef::Plant(key) => {
                    removed_plants.push(key);
                }
                _ => {}
            }
        }

        // mark edges of removed nodes
        for (key, edge) in &app.doc.edges {
            if removed_nodes.contains(&edge.start) || removed_nodes.contains(&edge.end) {
                removed_edges.push(key);
            }
        }

        let mut affected_layers = BTreeSet::new();
        for &key in &removed_nodes {
            let Some(node) = app.doc.nodes.get(key) else { continue };
            affected_layers.insert(node.layer);
        }
        for &key in &removed_plants {
            let Some(plant) = app.doc.plants.get(key) else { continue };
            affected_layers.insert(plant.layer);
        }

        app.doc.selected.retain(|s| match s {
            SelectRef::NodeRadius(key) | SelectRef::Node(key) => !removed_nodes.contains(&key),
            SelectRef::Edge(key) | SelectRef::EdgePoint(key, _) => !removed_edges.contains(key),
            SelectRef::Plant(key) | SelectRef::PlantDirection(key) => !removed_plants.contains(key),
            SelectRef::Point(_) => false,
        });
        if !removed_edges.is_empty() {
            app.doc.edges.retain(|key, _| !removed_edges.contains(&key));
        }
        if !removed_nodes.is_empty() {
            app.doc.nodes.retain(|key, _| !removed_nodes.contains(&key))
        }
        if !removed_plants.is_empty() {
            app.doc
                .plants
                .retain(|key, _| !removed_plants.contains(&key))
        }
        for layer in affected_layers {
            app.dirty_mask.mark_dirty_layer(layer);
        }
    }
}

fn operation_move_selection(
    app: &App,
    start_pos_world: Vec2,
    push_undo: bool,
    mut click_action: impl FnMut(&mut App),
) -> impl FnMut(&mut App, &UIEvent) {
    let doc = &app.doc;

    let start_selected = app.doc.selected.clone();
    let start_nodes = app.doc.nodes.clone();
    let start_edges = app.doc.edges.clone();

    let mut selected_plants = Vec::new();
    let mut selected_nodes = Vec::new();
    for &s in &doc.selected {
        match s {
            SelectRef::Node(key) => selected_nodes.push(key),
            SelectRef::Plant(key) => {
                if let Some(plant) = doc.plants.get(key) {
                    selected_plants.push((key, plant.clone()));
                }
            }
            _ => {}
        }
    }

    drop(doc);
    let mut changed = false;
    let mut last_delta = IVec2::ZERO;
    move |app, event| {
        if start_nodes.is_empty() {
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

        let doc = &app.doc;
        let current_layer = doc.current_layer;
        let cell_size = doc.cell_size;
        drop(doc);

        let delta = Document::snap_to_grid(pos_world - start_pos_world, cell_size).as_ivec2();
        //let delta = (pos_world - start_pos_world).as_ivec2();

        if delta != IVec2::ZERO || changed {
            if !changed {
                if push_undo {
                    app.push_undo("Move Graph Node");
                }
            }
            changed = true;
        } else {
            return;
        }

        let doc = &mut app.doc;
        {
            // insert nodes if we are trying to move edge points
            doc.nodes = start_nodes.clone();
            doc.edges = start_edges.clone();
            doc.selected = start_selected.clone();

            for (sel, _start_node) in doc.selected.iter_mut().zip(start_nodes.iter()) {
                if let SelectRef::EdgePoint(key, pos) = *sel {
                    let mut node = GraphNode::split_edge_node(
                        &doc.nodes,
                        &doc.edges,
                        key,
                        SplitPos::Fraction(*pos),
                    );
                    node.pos = Document::snap_to_grid(node.pos.as_vec2(), doc.cell_size).as_ivec2();
                    let node_key = doc.nodes.insert(node);
                    *sel = SelectRef::Node(GraphEdge::split_edge(&mut doc.edges, key, node_key));
                    changed = true;
                }
            }

            if delta != IVec2::ZERO || changed {
                changed = true;
                // Actual move happens here:
                for &key in &selected_nodes {
                    let Some(node) = doc.nodes.get_mut(key) else { continue };
                    node.pos += delta;
                }

                for (key, old_plant) in selected_plants.iter().cloned() {
                    let Some(plant) = doc.plants.get_mut(key) else { continue };
                    *plant = old_plant;
                    plant.pos += delta;
                }

                // Merge graph nodes that are moved together
                let merged_pairs =
                    GraphNode::merge_nodes(&selected_nodes, &doc.nodes, cell_size as f32);

                let replace_node = |key: GraphNodeKey| -> Option<GraphNodeKey> {
                    if let Ok(i) = merged_pairs.binary_search_by_key(&key, |(f, _t)| *f) {
                        Some(merged_pairs[i].1)
                    } else {
                        None
                    }
                };

                for &(from, to) in &merged_pairs {
                    if let Some([from, to]) = doc.nodes.get_disjoint_mut([from, to]) {
                        if from.radius < to.radius {
                            *from = to.clone();
                        } else {
                            *to = from.clone();
                        }
                        changed = true;
                    }
                }
                doc.nodes.retain(|k, _node| {
                    merged_pairs.binary_search_by_key(&k, |(f, _t)| *f).is_err()
                });

                doc.edges.retain(|_k, edge| {
                    if let Some(start) = replace_node(edge.start) {
                        edge.start = start;
                    }
                    if let Some(end) = replace_node(edge.end) {
                        edge.end = end;
                    }
                    edge.start != edge.end
                });
                // update selected nodes
                for sel in &mut doc.selected {
                    match sel {
                        SelectRef::Node(ref mut key) | SelectRef::NodeRadius(ref mut key) => {
                            if let Some(new_key) = replace_node(*key) {
                                *key = new_key;
                            }
                        }
                        SelectRef::Edge { .. } | SelectRef::EdgePoint { .. } => {}
                        SelectRef::Plant { .. } | SelectRef::PlantDirection { .. } => {}
                        SelectRef::Point { .. } => {}
                    }
                }

                // update selected edges
                doc.selected.retain(|sel| match *sel {
                    SelectRef::Edge(key) | SelectRef::EdgePoint(key, _) => {
                        doc.edges.contains_key(key)
                    }
                    _ => true,
                });

                // deduplicate selection, preserving order
                let mut uniques = BTreeSet::new();
                doc.selected.retain(|sel| uniques.insert(*sel));
            }
        }
        drop(doc);
        if delta != last_delta {
            app.dirty_mask.mark_dirty_layer(current_layer);
            last_delta = delta;
        }
    }
}

fn operation_move_graph_node_radius(
    app: &mut App,
    edited_key: GraphNodeKey,
) -> impl FnMut(&mut App, &UIEvent) {
    let mut push_undo = true;
    app.locked_hover = Some(SelectRef::NodeRadius(edited_key));
    move |app, _event| {
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);

        if push_undo {
            app.push_undo("Resize Graph Node");
            push_undo = false;
        }

        let doc = &mut app.doc;
        let current_layer = doc.current_layer;
        let cell_size = doc.cell_size;
        let edited_pos = match doc.nodes.get(edited_key) {
            Some(n) => n.pos,
            _ => return,
        };
        for selection in &doc.selected {
            match *selection {
                SelectRef::Node(key) | SelectRef::NodeRadius(key) => {
                    if let Some(node) = doc.nodes.get_mut(key) {
                        let mut new_radius = (pos_world - edited_pos.as_vec2()).length();

                        let snap_step = cell_size as f32;
                        new_radius = (new_radius / snap_step).round() * (snap_step);
                        node.radius = new_radius as usize;
                    }
                }
                _ => {}
            }
        }

        drop(doc);
        app.dirty_mask.mark_dirty_layer(current_layer);
    }
}

fn operation_move_plant_direction(
    app: &mut App,
    edited_key: PlantKey,
) -> impl FnMut(&mut App, &UIEvent) {
    let mut push_undo = true;
    app.locked_hover = Some(SelectRef::PlantDirection(edited_key));
    move |app, _event| {
        let pos_world = app
            .view
            .screen_to_world()
            .transform_point2(app.last_mouse_pos);

        if push_undo {
            app.push_undo("Change Plant Direction");
            push_undo = false;
        }

        let doc = &mut app.doc;
        let current_layer = doc.current_layer;
        let Some(edited_pos) = doc.plants.get(edited_key).map(|p| p.pos) else { return };
        for &selection in doc
            .selected
            .iter()
            .chain(once(&SelectRef::Plant(edited_key)))
        {
            match selection {
                SelectRef::Plant(key) | SelectRef::PlantDirection(key) => {
                    let Some(plant) = doc.plants.get_mut(key) else { continue };
                    if let Some(new_dir) = (pos_world - edited_pos.as_vec2()).try_normalize() {
                        plant.dir = new_dir;
                    }
                }
                _ => {}
            }
        }

        drop(doc);
        app.dirty_mask.mark_dirty_layer(current_layer);
    }
}

enum SelectOperation {
    Replace,
    Extend,
    Substract,
}

fn operation_rectangle_selection(
    app: &mut App,
    operation: SelectOperation,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos: [Vec2; 2] = Rect::from_point(app.last_mouse_pos);

    let start_selection = match operation {
        SelectOperation::Replace => vec![],
        SelectOperation::Extend | SelectOperation::Substract => app.doc.selected.clone(),
    };

    let mut changed = false;
    move |app, event| match event {
        UIEvent::MouseMove { .. } => {
            if app.last_mouse_pos.distance(start_pos[0]) > 2.0 && !changed {
                app.push_undo("Select Nodes");
                changed = true;
            }
            let rect = start_pos.union(Rect::from_point(app.last_mouse_pos));

            let mut new_selection = start_selection.clone();
            let mut test_and_add = |bounds: [Vec2; 2], sel_ref| {
                if bounds.intersect(rect).is_some() {
                    match operation {
                        SelectOperation::Substract => new_selection.retain(|e| *e != sel_ref),
                        SelectOperation::Extend | SelectOperation::Replace => {
                            if !new_selection.contains(&sel_ref) {
                                new_selection.push(sel_ref);
                            }
                        }
                    }
                }
            };
            for (node_key, node) in &app.doc.nodes {
                let [min, max] = node.bounds();
                let bounds = [
                    app.view.world_to_screen().transform_point2(min),
                    app.view.world_to_screen().transform_point2(max),
                ];

                test_and_add(bounds, SelectRef::Node(node_key));
            }

            for (plant_key, plant) in &app.doc.plants {
                let pos_screen = app
                    .view
                    .world_to_screen()
                    .transform_point2(plant.pos.as_vec2());
                test_and_add(
                    [pos_screen - vec2(8.0, 8.0), pos_screen + vec2(8.0, 8.0)],
                    SelectRef::Plant(plant_key),
                );
            }

            if app.doc.selected != new_selection {
                app.doc.selected = new_selection;
            }

            app.operation_batch.set_image(app.white_texture);

            app.operation_batch
                .geometry
                .fill_rect(rect[0], rect[1], [255, 255, 255, 32]);
            app.operation_batch
                .geometry
                .stroke_rect(rect[0], rect[1], 1.0, [255, 255, 255, 128]);
        }
        UIEvent::MouseUp { .. } => {
            if !changed {
                let sel_ref = SelectRef::Point(Vec2Ord(
                    app.view
                        .screen_to_world()
                        .transform_point2(app.last_mouse_pos),
                ));
                match operation {
                    SelectOperation::Substract => app.doc.selected.retain(|e| *e != sel_ref),
                    SelectOperation::Extend => {
                        if !app.doc.selected.contains(&sel_ref) {
                            app.doc.selected.push(sel_ref);
                        }
                    }
                    SelectOperation::Replace => {
                        app.doc.selected.clear();
                        if !app.doc.selected.contains(&sel_ref) {
                            app.doc.selected.push(sel_ref);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn operation_paint_selection(
    app: &mut App,
    operation: SelectOperation,
) -> impl FnMut(&mut App, &UIEvent) {
    let start_pos = app.last_mouse_pos;

    let mut changed = false;
    move |app, _event| {
        if app.last_mouse_pos != start_pos && !changed {
            app.push_undo("Select Nodes");
            changed = true;
        }
        let mut new_selection = app.doc.selected.clone();
        let mut test_and_add = |bounds: [Vec2; 2], sel_ref| {
            if bounds.contains_point(app.last_mouse_pos) {
                match operation {
                    SelectOperation::Substract => new_selection.retain(|e| *e != sel_ref),
                    SelectOperation::Extend | SelectOperation::Replace => {
                        if !new_selection.contains(&sel_ref) {
                            new_selection.push(sel_ref);
                        }
                    }
                }
            }
        };
        for (node_key, node) in &app.doc.nodes {
            let [min, max] = node.bounds();
            let bounds = [
                app.view.world_to_screen().transform_point2(min),
                app.view.world_to_screen().transform_point2(max),
            ];

            test_and_add(bounds, SelectRef::Node(node_key));
        }
        for (plant_key, plant) in &app.doc.plants {
            let pos_screen = app
                .view
                .world_to_screen()
                .transform_point2(plant.pos.as_vec2());
            test_and_add(
                [pos_screen - vec2(8.0, 8.0), pos_screen + vec2(8.0, 8.0)],
                SelectRef::Plant(plant_key),
            );
        }
        if new_selection != app.doc.selected {
            app.doc.selected = new_selection;
        }
    }
}
