use std::mem::discriminant;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use glam::vec2;
use rimui::*;

use cbmap::{MapMarkup, MarkupPoint, MarkupPointKind, MarkupRect, MarkupRectKind, MaterialSlot};

use crate::app::{App, PlayState};
use crate::document::{ChangeMask, Document, Layer, LayerContent};
use crate::field::Field;
use crate::graph::{Graph, GraphNodeShape, GraphRef};
use crate::grid::Grid;
use crate::net_client_connection::{ClientConnection, ConnectionState};
use crate::some_or::some_or;
use crate::tool::{Tool, ToolGroup, ToolGroupState};
use crate::zone::{EditorBounds, ZoneRef};
use bincode::Options;
use editor_protocol::{Blob, EditorClientMessage, EDITOR_PROTOCOL_VERSION};
use log::info;
use std::hash::Hasher;
use std::sync::Arc;

impl App {
    pub fn ui(&mut self, context: &mut miniquad::Context, _time: f32, dt: f32) {
        self.ui_toolbar(context);

        self.ui_play_bar(context);

        self.ui_sidebar(context);
        match self.tool {
            Tool::Zone => {
                self.ui_zone_list(context);
            }
            Tool::Graph => {
                self.ui_graph_panel(context);
            }
            _ => {}
        }

        self.ui_status_bar(context);

        self.ui_confirm_unsaved_changes(context);

        self.ui_error_message(context);

        self.ui.layout_ui(
            dt,
            [0, 0, self.window_size[0] as i32, self.window_size[1] as i32],
            None,
        );
    }

    fn ui_sidebar(&mut self, context: &mut miniquad::Context) {
        let sidebar_width = 280i32;
        let window = self.ui.window(
            "Test",
            WindowPlacement::Absolute {
                pos: [self.window_size[0] as i32 - 8, 8],
                size: [0, self.window_size[1] as i32 - 16],
                expand: EXPAND_LEFT,
            },
            0,
            0,
        );

        let frame = self.ui.add(window, Frame::default());
        let rows = self.ui.add(
            frame,
            vbox()
                .padding(2)
                .margins([2, 2, 2, 4])
                .min_size([sidebar_width as u16, 0]),
        );
        self.ui.add(rows, label("Materials"));

        for (index, material) in self.doc.materials.iter().enumerate().skip(1) {
            if self
                .ui
                .add(
                    rows,
                    button(&format!("{}. {}", index, material.label()))
                        .item(true)
                        .down(index == self.active_material as usize),
                )
                .clicked
            {
                self.active_material = index as u8;
            }
        }

        self.ui.add(rows, separator());

        self.ui_layer_list(rows);

        self.ui.add(rows, separator());

        self.ui.add(rows, label("Reference"));
        if self.doc.reference_path.is_some() {
            let show_reference = self.doc.show_reference;
            if self
                .ui
                .add(rows, button("Show Reference").down(show_reference))
                .clicked
            {
                self.doc.show_reference = !show_reference;
            }

            let hbar = self.ui.add(rows, hbox());
            self.ui.add(hbar, label("Scale:"));
            if self
                .ui
                .add(hbar, button("1x").down(self.doc.reference_scale == 1))
                .clicked
            {
                self.doc.reference_scale = 1;
            }
            if self
                .ui
                .add(hbar, button("2x").down(self.doc.reference_scale == 2))
                .clicked
            {
                self.doc.reference_scale = 2;
            }
        }

        let mut buffer = String::new();
        let reference_text = self
            .doc
            .reference_path
            .as_ref()
            .map(|s| {
                if let Some((_, name)) = s.rsplit_once('/') {
                    buffer = format!(".../{}", name);
                    &buffer
                } else {
                    s.as_str()
                }
            })
            .unwrap_or("Load...");

        let h = self.ui.add(rows, hbox());
        let mut new_reference_path = None;
        if self.ui.add(h, button(reference_text).expand(true)).clicked {
            let selected_reference_path = self.report_error({
                let path = self.doc.reference_path.as_ref().map(PathBuf::from);
                nfd2::open_file_dialog(Some("png"), path.as_ref().map(|p| p.as_path()))
                    .context("Opening dialog")
            });

            if let Some(nfd2::Response::Okay(selected_reference_path)) = selected_reference_path {
                new_reference_path =
                    Some(Some(selected_reference_path.to_string_lossy().to_string()));
            }
        }
        if self.ui.add(h, button("X").min_size([16, 0])).clicked {
            new_reference_path = Some(None);
        }
        if let Some(path) = &self.doc.reference_path {
            if !path.is_empty() {
                last_tooltip(&mut self.ui, rows, path, self.font_tiny, false);
            }
        }
        if let Some(new_reference_path) = new_reference_path {
            self.doc.reference_path = new_reference_path;
            self.generation_profiler.begin_frame();
            self.graphics.borrow_mut().generate(
                &self.doc,
                ChangeMask {
                    reference_path: true,
                    ..ChangeMask::default()
                },
                true,
                Some(context),
                &mut self.generation_profiler,
            );
        }
    }

    fn ui_layer_list(&mut self, rows: AreaRef) {
        let h = self.ui.add(rows, hbox());
        self.ui.add(h, label("Layers").expand(true));
        if button_drop_down(&mut self.ui, h, "Add", None, Align::Left, true, false, 0).clicked {
            self.ui.show_popup_at_last(h, "layer_add");
        }

        let can_remove = self.doc.active_layer < self.doc.layers.len();
        if self.ui.add(h, button("Delete").enabled(can_remove)).clicked && can_remove {
            self.push_undo("Remove Layer");
            let mut doc = &mut self.doc;
            let active_layer = doc.active_layer;
            let removed = doc.layers.remove(active_layer);
            match removed.content {
                LayerContent::Graph(key) => {
                    doc.graphs.remove(key);
                }
                LayerContent::Grid(key) => {
                    doc.grids.remove(key);
                }
                LayerContent::Field(key) => {
                    doc.fields.remove(key);
                }
            };
            drop(doc);
            self.dirty_mask.cell_layers = u64::MAX;
        }

        if let Some(p) = self.ui.is_popup_shown(h, "layer_add") {
            let mut new_layer = None;
            if self.ui.add(p, button("Graph").item(true)).clicked {
                let key = self.doc.graphs.insert(Graph::new());
                new_layer = Some(Layer {
                    content: LayerContent::Graph(key),
                    hidden: false,
                });
            }
            if self.ui.add(p, button("Grid").item(true)).clicked {
                let key = self.doc.grids.insert(Grid::new(0));
                new_layer = Some(Layer {
                    content: LayerContent::Grid(key),
                    hidden: false,
                });
            }
            if self.ui.add(p, button("Field").item(true)).clicked {
                let key = self.doc.fields.insert(Field::new());
                new_layer = Some(Layer {
                    content: LayerContent::Field(key),
                    hidden: false,
                });
            }

            if let Some(new_layer) = new_layer {
                self.ui.hide_popup();

                self.push_undo("Add Layer");
                let mut doc = &mut self.doc;
                let new_layer_index = doc.layers.len();

                Document::set_active_layer(
                    &mut doc.active_layer,
                    &mut self.tool,
                    &mut self.tool_groups,
                    new_layer_index,
                    &new_layer.content,
                );

                doc.layers.push(new_layer);
            }
        }

        let mut doc = &mut self.doc;
        for (i, layer) in doc.layers.iter_mut().enumerate() {
            let h = self.ui.add(rows, hbox());
            if self
                .ui
                .add(
                    h,
                    button(&format!(
                        "{}_vis#{}",
                        i,
                        if layer.hidden { "X" } else { " " }
                    ))
                    .item(true)
                    .down(layer.hidden)
                    .align(Some(Align::Center))
                    .min_size([16, 0]),
                )
                .clicked
            {
                layer.hidden = !layer.hidden;
                self.dirty_mask.cell_layers = u64::MAX;
            }
            tooltip(
                &mut self.ui,
                h,
                if !layer.hidden {
                    "Hide Layer"
                } else {
                    "Show Layer"
                },
            );
            if self
                .ui
                .add(
                    h,
                    button(&format!("{}. {}", i + 1, layer.label()))
                        .down(i == doc.active_layer)
                        .align(Some(Align::Left))
                        .expand(true),
                )
                .clicked
            {
                Document::set_active_layer(
                    &mut doc.active_layer,
                    &mut self.tool,
                    &mut self.tool_groups,
                    i,
                    &layer.content,
                )
            }
        }

        let h = self.ui.add(rows, hbox());
        self.ui.add(h, spacer());
        let is_layer_selected = doc.active_layer < doc.layers.len();

        let mut swap_index = None;
        let active_layer = doc.active_layer;

        self.ui.add(h, label("Move"));
        if self
            .ui
            .add(
                h,
                button("Up").enabled(is_layer_selected && doc.active_layer > 0),
            )
            .clicked
        {
            swap_index = Some(active_layer - 1);
        }
        if self
            .ui
            .add(
                h,
                button("Down")
                    .enabled(is_layer_selected && doc.active_layer + 1 < doc.layers.len()),
            )
            .clicked
        {
            swap_index = Some(active_layer + 1);
        }

        if let Some(swap_index) = swap_index {
            doc.layers.swap(active_layer, swap_index);
            let replace_layers = |tool_groups: &mut [ToolGroupState],
                                  mapping: &[(usize, usize)]| {
                for group in tool_groups {
                    group.layer = mapping
                        .iter()
                        .cloned()
                        .find(|(from, to)| Some(*from) == group.layer)
                        .map(|(_, to)| Some(to))
                        .unwrap_or(group.layer);
                }
            };
            replace_layers(
                &mut self.tool_groups,
                &[(active_layer, swap_index), (swap_index, active_layer)],
            );
            doc.active_layer = swap_index;
            self.dirty_mask.cell_layers = u64::MAX;
        }
    }

    fn ui_zone_list(&mut self, _context: &mut miniquad::Context) {
        let sidebar_width = 280;
        let zone_window = self.ui.window(
            "Zones",
            WindowPlacement::Absolute {
                pos: [self.window_size[0] as i32 - 24 - sidebar_width, 8],
                size: [0, 0],
                expand: EXPAND_LEFT | EXPAND_DOWN,
            },
            0,
            0,
        );

        let frame = self.ui.add(zone_window, Frame::default());
        let rows = self.ui.add(
            frame,
            vbox()
                .padding(2)
                .margins([2, 2, 2, 4])
                .min_size([sidebar_width as u16, 0]),
        );

        let row = self.ui.add(rows, hbox());
        self.ui.add(row, label("Zones").expand(true));

        let doc = &self.doc;
        let selection = doc.zone_selection;
        let mut new_selection = None;
        let font = Some(0);
        let font_chat = 0;
        let is_race = false;
        let can_add_start = !is_race
            || doc
                .markup
                .points
                .iter()
                .filter(|p| p.kind == MarkupPointKind::Start)
                .count()
                < 1;
        let can_add_finish = !doc
            .markup
            .rects
            .iter()
            .any(|r| r.kind == MarkupRectKind::RaceFinish);
        drop(doc);

        if button_drop_down(
            &mut self.ui,
            row,
            "Add",
            None,
            Left,
            can_add_start || can_add_finish,
            false,
            0, // sprites.ui_drop_down_arrow,
        )
        .down
        {
            self.ui.show_popup_at_last(row, "markup_add");
        }

        if let Some(p) = self.ui.is_popup_shown(row, "markup_add") {
            let center = self
                .view
                .screen_to_world()
                .transform_point2(
                    vec2(
                        self.view.screen_width_px as f32,
                        self.view.screen_height_px as f32,
                    ) * 0.5,
                )
                .ceil();
            let center = [center.x as i32, center.y as i32];

            if can_add_start {
                if self.ui.add(p, button("Start Point").item(true)).clicked {
                    self.ui.hide_popup();
                    self.push_undo("Add Start Point");

                    new_selection = Some(ZoneRef::Point(self.doc.markup.points.len()));
                    self.doc.markup.points.push(MarkupPoint {
                        kind: MarkupPointKind::Start,
                        pos: center,
                    });
                }
                tooltip(&mut self.ui, p, MarkupPointKind::Start.tooltip());
            }

            if can_add_finish {
                if self.ui.add(p, button("Race Finish").item(true)).clicked {
                    self.ui.hide_popup();
                    self.push_undo("Add Race Finish");

                    new_selection = Some(ZoneRef::Rect(self.doc.markup.rects.len()));
                    self.doc.markup.rects.push(MarkupRect {
                        kind: MarkupRectKind::RaceFinish,
                        start: [center[0] - 100, center[1] - 100],
                        end: [center[0] + 100, center[1] + 100],
                    });
                }
                tooltip(&mut self.ui, p, MarkupRectKind::RaceFinish.tooltip());
            }
        }

        let doc = &self.doc;
        for (i, MarkupPoint { kind, pos }) in doc.markup.points.iter().enumerate() {
            let b = self.ui.add(
                rows,
                button_area(&format!("pb{}#", i))
                    .down(selection == Some(ZoneRef::Point(i)))
                    .item(true),
            );
            let h = self.ui.add(b.area, hbox());
            self.ui.add(
                h,
                label(match kind {
                    MarkupPointKind::Start => "Start Point",
                })
                .expand(true)
                .font(font),
            );
            self.ui.add(
                h,
                label(&format!("{}, {}", pos[0], pos[1])).font(Some(font_chat)),
            );
            if b.clicked {
                new_selection = Some(ZoneRef::Point(i));
            }
            tooltip(&mut self.ui, rows, kind.tooltip());
        }

        for (i, MarkupRect { kind, start, end }) in doc.markup.rects.iter().enumerate() {
            let b = self.ui.add(
                rows,
                button_area(&format!("rb{}#", i))
                    .down(selection == Some(ZoneRef::Rect(i)))
                    .item(true),
            );
            let h = self.ui.add(b.area, hbox());
            self.ui.add(
                h,
                label(match kind {
                    MarkupRectKind::RaceFinish => "Race Finish",
                })
                .expand(true)
                .font(font),
            );
            self.ui.add(
                h,
                label(&format!(
                    "{}, {} : {}, {}",
                    start[0], start[1], end[0], end[1]
                ))
                .font(Some(font_chat)),
            );
            if b.clicked {
                new_selection = Some(ZoneRef::Rect(i));
            }
            tooltip(&mut self.ui, rows, kind.tooltip());
        }
        drop(doc);

        let h = self.ui.add(rows, hbox());
        self.ui.add(h, rimui::spacer());
        if self.ui.add(h, button("Clear All")).clicked {
            self.push_undo("Delete All Zones");
            self.doc.markup = MapMarkup::new();
            self.doc.zone_selection = None;
        }
        if self
            .ui
            .add(h, button("Delete").enabled(selection.is_some()))
            .clicked
        {
            if let Some(selection) = selection {
                self.push_undo("Delete Zone");
                selection.remove_zone(&mut self.doc.markup);
                if !selection.is_valid(&self.doc.markup) {
                    self.doc.zone_selection = None;
                }
            }
        }

        if let Some(new_selection) = new_selection {
            if self.doc.zone_selection != Some(new_selection) {
                self.doc.zone_selection = Some(new_selection);
            } else {
                let (start, end) = new_selection.bounds(&self.doc.markup, &self.view);
                let center = (start + end) * 0.5;
                self.view.target = self.view.screen_to_world().transform_point2(center).floor();
            }
        }
    }

    fn ui_graph_panel(&mut self, _context: &mut miniquad::Context) {
        let sidebar_width = 280;
        let zone_window = self.ui.window(
            "Graph",
            WindowPlacement::Absolute {
                pos: [self.window_size[0] as i32 - 24 - sidebar_width, 8],
                size: [0, 0],
                expand: EXPAND_LEFT | EXPAND_DOWN,
            },
            0,
            0,
        );

        let frame = self.ui.add(zone_window, Frame::default());
        let rows = self.ui.add(
            frame,
            vbox()
                .padding(2)
                .margins([2, 2, 2, 4])
                .min_size([sidebar_width as u16, 0]),
        );

        let row = self.ui.add(rows, hbox());
        self.ui.add(row, label("Graph").expand(true));

        let mut doc = &mut self.doc;
        let layer = doc.active_layer;
        let cell_size = doc.cell_size;

        let mut change = Option::<Box<dyn FnMut(&mut App)>>::None;
        let graph_key = Document::layer_graph(&doc.layers, doc.active_layer);
        if let Some(graph) = doc.graphs.get_mut(graph_key) {
            // graph settings
            let h = self.ui.add(rows, hbox());
            self.ui.add(h, label("Thickness").expand(true));
            for i in 0..=4 {
                let t = i * cell_size as i32;
                if self
                    .ui
                    .add(
                        h,
                        button(&format!("{}", t)).down(t == graph.outline_width as i32),
                    )
                    .clicked
                {
                    change = Some(Box::new({
                        let t = t;
                        move |app: &mut App| {
                            app.push_undo("Graph: Outline Width");
                            let graph_key =
                                Document::layer_graph(&app.doc.layers, app.doc.active_layer);
                            if let Some(graph) = app.doc.graphs.get_mut(graph_key) {
                                graph.outline_width = t as usize;
                            }
                        }
                    }));
                }
            }

            self.ui.add(rows, separator());

            self.ui.add(rows, label("Node").expand(true));
            let selected_nodes = || {
                graph.selected.iter().filter_map(|n| match *n {
                    GraphRef::Node(key) | GraphRef::NodeRadius(key) => Some(key),
                    _ => None,
                })
            };

            let first_key = selected_nodes().next();

            if let Some(first_key) = first_key {
                let h = self.ui.add(rows, hbox());
                self.ui.add(h, label("Shape").expand(true));
                let shapes = [
                    ("Square", GraphNodeShape::Square),
                    ("Octogon", GraphNodeShape::Octogon),
                    ("Circle", GraphNodeShape::Circle),
                ];
                let first_node = graph.nodes.get(first_key).clone();
                for (label, shape) in shapes {
                    if self
                        .ui
                        .add(
                            h,
                            button(label).down(selected_nodes().any(|k| {
                                graph.nodes.get(k).map(|n| discriminant(&n.shape))
                                    == Some(discriminant(&shape))
                            })),
                        )
                        .clicked
                    {
                        let selected_nodes: Vec<_> = selected_nodes().collect();
                        change = Some(Box::new(move |app: &mut App| {
                            app.push_undo("Node Shape");
                            if let Some(graph) = app.doc.graphs.get_mut(graph_key) {
                                for &key in &selected_nodes {
                                    let node = &mut graph.nodes[key];
                                    node.shape = shape;
                                }
                            }
                        }));
                    }
                }

                let h = self.ui.add(rows, hbox());
                self.ui.add(h, label("Material").expand(true));
                let mut material = first_node.map(|n| n.material).unwrap_or(0);
                if material_drop_down(&mut self.ui, h, &mut material, &doc.materials) {
                    let selected_nodes: Vec<_> = selected_nodes().collect();
                    change = Some(Box::new(move |app| {
                        app.push_undo("Node: Material");
                        for &key in &selected_nodes {
                            if let Some(graph) = app.doc.graphs.get_mut(graph_key) {
                                let node = &mut graph.nodes[key];
                                node.material = material;
                            }
                        }
                    }))
                }

                let no_outline = first_node.map(|n| n.no_outline).unwrap_or(false);
                if self
                    .ui
                    .add(rows, button("No Outline").down(no_outline))
                    .clicked
                {
                    let selected_nodes: Vec<_> = selected_nodes().collect();
                    change = Some(Box::new(move |app| {
                        app.push_undo("Node: No Outline");
                        for &key in &selected_nodes {
                            if let Some(graph) = app.doc.graphs.get_mut(graph_key) {
                                let node = &mut graph.nodes[key];
                                node.no_outline = !no_outline;
                            }
                        }
                    }));
                }
            }
        }
        drop(doc);

        if let Some(mut change) = change {
            change(self);
            self.dirty_mask.mark_dirty_layer(layer);
        }
    }

    pub fn ui_toolbar(&mut self, context: &mut miniquad::Context) {
        let toolbar = self.ui.window(
            "Map",
            WindowPlacement::Absolute {
                pos: [8, 8],
                size: [0, 32],
                expand: EXPAND_RIGHT,
            },
            0,
            0,
        );

        let frame = self.ui.add(toolbar, Frame::default());
        let cols = self.ui.add(frame, hbox().margins([0, 0, 0, 2]));
        self.ui.add(cols, label("Map"));
        if self.ui.add(cols, button("New")).clicked {
            self.on_map_new(context);
        }
        if self.ui.add(cols, button("Open")).clicked {
            self.on_map_open(context);
        }
        if self.ui.add(cols, button("Save")).clicked {
            if matches!(self.play_state, PlayState::Connected { .. }) {
                self.on_map_play(context);
            } else {
                self.on_map_save(context);
            }
        }
        if self.ui.add(cols, button("Save As...")).clicked {
            self.on_map_save_as(context);
        }

        self.ui.add(cols, label("Edit"));
        if (self.ui.add(cols, button("Undo").enabled(!self.undo.borrow().is_empty())).clicked ||
            //self.ui.key_pressed_with_modifiers(KeyCode::Z, true, false, false) {
            self.ui.key_pressed(KeyCode::Z))
            && !self.undo.borrow().is_empty()
        {
            let doc: &mut Document = &mut self.doc;
            let err = self
                .undo
                .borrow_mut()
                .apply(doc, &mut self.redo.borrow_mut());
            self.report_error(err);
            self.dirty_mask = ChangeMask {
                cell_layers: u64::MAX,
                reference_path: false,
            };
        }
        if (self.ui.add(cols, button("Redo").enabled(!self.redo.borrow().is_empty())).clicked ||
            //self.ui.key_pressed_with_modifiers(KeyCode::Z, true, true, false)
            self.ui.key_pressed(KeyCode::Y))
            && !self.redo.borrow().is_empty()
        {
            let doc: &mut Document = &mut self.doc;
            let err = self
                .redo
                .borrow_mut()
                .apply(doc, &mut self.undo.borrow_mut());
            self.report_error(err);
            self.dirty_mask = ChangeMask {
                cell_layers: u64::MAX,
                reference_path: false,
            };
        }

        self.ui.add(cols, label("Tool"));

        let tools = [
            (Tool::Pan, "Pan"),
            (Tool::Paint, "Paint"),
            (Tool::Fill, "Fill"),
            (Tool::Rectangle, "Rectangle"),
            (Tool::Graph, "Graph"),
            (Tool::Zone, "Zone"),
        ];

        let old_tool = self.tool.clone();

        for (tool, title) in tools.iter() {
            let is_selected = discriminant(&old_tool) == discriminant(&tool);
            if self.ui.add(cols, button(title).down(is_selected)).clicked {
                if let Some(tool_group) = ToolGroup::from_tool(*tool) {
                    self.tool_groups[tool_group as usize].tool = *tool;
                    if let Some(layer) = self.tool_groups[tool_group as usize].layer.or_else(|| {
                        self.doc.layers.iter().position(|l| {
                            discriminant(&l.content) == tool_group.layer_content_discriminant()
                        })
                    }) {
                        self.doc.active_layer = layer;
                    }
                }
                self.tool = *tool;
            }
        }
    }

    fn on_map_open(&mut self, _context: &mut miniquad::Context) {
        if self.ask_to_save_changes(|app, context| app.on_map_open(context)) {
            return;
        }
        let response =
            self.report_error(nfd2::open_file_dialog(None, None).context("Opening dialog"));
        if let Some(nfd2::Response::Okay(path)) = response {
            let doc = self.report_error(App::load_doc(&path));
            if let Some(doc) = doc {
                self.doc = doc;
                self.doc_path = Some(path);
                self.undo.borrow_mut().clear();
                self.undo_saved_position.replace(0);
                self.redo.borrow_mut().clear();
                self.confirm_unsaved_changes = None;
                let state_res = self.save_app_state();
                self.report_error(state_res);
            }
            self.dirty_mask.cell_layers = u64::MAX;
        };
    }

    fn ui_error_message(&mut self, _context: &mut miniquad::Context) {
        let error_message_borrow = self.error_message.borrow();
        if let Some(error_message) = error_message_borrow.as_ref() {
            let window = self.ui.window(
                "ErrorMessage",
                WindowPlacement::Center {
                    size: [0, 0],
                    offset: [0, 0],
                    expand: EXPAND_ALL,
                },
                0,
                0,
            );

            let frame = self.ui.add(window, Frame::default());
            let rows = self.ui.add(
                frame,
                vbox().padding(2).min_size([200, 0]).margins([8, 8, 8, 8]),
            );
            self.ui.add(
                rows,
                wrapped_text("message", &error_message)
                    .min_size([300, 0])
                    .max_width(500),
            );
            let columns = self.ui.add(rows, hbox());

            self.ui.add(columns, spacer());
            drop(error_message_borrow);
            if self
                .ui
                .add(columns, button("OK").min_size([120, 0]))
                .clicked
            {
                self.error_message.replace(None);
            }
            self.ui.add(columns, spacer());
        }
    }

    fn ui_confirm_unsaved_changes(&mut self, context: &mut miniquad::Context) {
        if let Some(mut post_action) = self.confirm_unsaved_changes.take() {
            if *self.undo_saved_position.borrow() == self.undo.borrow().records.len() {
                return;
            }
            let window = self.ui.window(
                "ConfirmChanges",
                WindowPlacement::Center {
                    size: [0, 0],
                    offset: [0, 0],
                    expand: EXPAND_ALL,
                },
                0,
                0,
            );

            let frame = self.ui.add(window, Frame::default());
            let rows = self.ui.add(
                frame,
                vbox().padding(2).min_size([200, 0]).margins([8, 8, 8, 8]),
            );
            self.ui.add(
                rows,
                wrapped_text(
                    "message",
                    &"The map contains unsaved changes.\n\nWould you live to save changes first?",
                )
                .min_size([300, 0])
                .max_width(500),
            );
            let columns = self.ui.add(rows, hbox());
            let button_width = 130;

            self.ui.add(columns, spacer());

            if self
                .ui
                .add(columns, button("Save").min_size([button_width, 0]))
                .clicked
            {
                if self.on_map_save(context) {
                    post_action(self, context);
                }
            }

            if self
                .ui
                .add(columns, button("Don't Save").min_size([button_width, 0]))
                .clicked
            {
                self.undo_saved_position
                    .replace(self.undo.borrow().records.len());
                post_action(self, context);
            }

            self.confirm_unsaved_changes = Some(post_action);

            if self
                .ui
                .add(columns, button("Cancel").min_size([button_width, 0]))
                .clicked
            {
                self.confirm_unsaved_changes = None;
            }

            self.ui.add(columns, spacer());
        }
    }

    fn on_map_new(&mut self, context: &mut miniquad::Context) {
        if self.ask_to_save_changes(|app, context| {
            app.on_map_new(context);
        }) {
            return;
        }

        self.doc = Document::new();
        self.undo.borrow_mut().clear();
        self.redo.borrow_mut().clear();
        self.undo_saved_position.replace(0);
        self.dirty_mask = ChangeMask {
            cell_layers: u64::MAX,
            reference_path: true,
        }
    }

    pub(crate) fn ask_to_save_changes<T>(&mut self, post_action: T) -> bool
    where
        T: for<'a> FnMut(&mut App, &mut miniquad::Context) + 'static,
    {
        if *self.undo_saved_position.borrow() != self.undo.borrow().records.len() {
            self.confirm_unsaved_changes = Some(Box::new(post_action));
            return true;
        }
        false
    }

    fn on_map_save(&mut self, context: &mut miniquad::Context) -> bool {
        if let Some(path) = &self.doc_path {
            self.doc.pre_save_cleanup();
            self.generation_profiler.begin_frame();
            self.graphics.borrow_mut().generate(
                &self.doc,
                ChangeMask {
                    cell_layers: u64::MAX,
                    reference_path: false,
                },
                true,
                Some(context),
                &mut self.generation_profiler,
            );
            let save_res = App::save_doc(
                path,
                &self.doc,
                &self.graphics.borrow(),
                self.white_texture.clone(),
                self.finish_texture.clone(),
                self.pipeline_sdf.clone(),
                &self.view,
                context,
                self.active_material,
            );
            let result = save_res.is_ok();
            if save_res.is_ok() {
                self.undo_saved_position
                    .replace(self.undo.borrow().records.len());
                self.confirm_unsaved_changes = None;
            } else {
                self.report_error(save_res);
            }

            let state_res = self.save_app_state();
            self.report_error(state_res);
            result
        } else {
            self.on_map_save_as(context)
        }
    }

    fn on_map_save_as(&mut self, context: &mut miniquad::Context) -> bool {
        let path = self
            .report_error(nfd2::open_save_dialog(Some("cbmap"), None).context("Opening dialog"));

        if let Some(nfd2::Response::Okay(path)) = path {
            self.doc.pre_save_cleanup();
            let save_res = App::save_doc(
                Path::new(&path),
                &self.doc,
                &self.graphics.borrow(),
                self.white_texture.clone(),
                self.finish_texture.clone(),
                self.pipeline_sdf.clone(),
                &self.view,
                context,
                self.active_material,
            );
            let result = save_res.is_ok();
            if save_res.is_ok() {
                self.undo_saved_position
                    .replace(self.undo.borrow().records.len());
                self.confirm_unsaved_changes = None;
            } else {
                self.report_error(save_res);
            }
            let state_res = self.save_app_state();
            if state_res.is_ok() {
                self.doc_path = Some(path.into());
            }
            self.report_error(state_res);
            result
        } else {
            false
        }
    }

    fn ui_status_bar(&mut self, context: &mut miniquad::Context) {
        let height = 32;
        let statusbar = self.ui.window(
            "StatusBar",
            WindowPlacement::Absolute {
                pos: [8, self.window_size[1] as i32 - 8 - height],
                size: [0, height],
                expand: EXPAND_RIGHT | EXPAND_UP,
            },
            0,
            0,
        );

        let frame = self.ui.add(statusbar, Frame::default());
        let rows = self.ui.add(frame, vbox().margins([2, 2, 2, 2]));

        if self.generation_profiler_show {
            self.generation_profiler.ui_profiler(
                &mut self.ui,
                rows,
                "Generation",
                Some(self.font_tiny),
            );
        }

        if let Some(last_generation_time) = self.generation_profiler.total_duration() {
            let h = self.ui.add(rows, hbox().padding(2));
            if self
                .ui
                .add(
                    h,
                    button(if self.generation_profiler_show {
                        "-"
                    } else {
                        "+"
                    })
                    .min_size([14, 0]),
                )
                .clicked
            {
                self.generation_profiler_show = !self.generation_profiler_show;
            }
            if !self.generation_profiler_show {
                self.ui.add(
                    h,
                    label(&format!(
                        "Generated in {:.1} ms",
                        last_generation_time * 1000.0
                    )),
                );
            }
        }
    }

    fn ui_play_bar(&mut self, context: &mut miniquad::Context) {
        let play_bar = self.ui.window(
            "Play",
            WindowPlacement::Absolute {
                pos: [8, 48],
                size: [0, 32],
                expand: EXPAND_RIGHT,
            },
            0,
            0,
        );

        let frame = self.ui.add(play_bar, Frame::default());
        let cols = self.ui.add(frame, hbox().margins([0, 0, 0, 2]));

        let button_text = match self.play_state {
            PlayState::Offline => "Play",
            PlayState::Connecting => "Connecting...",
            PlayState::Connected { .. } => "Connected",
        };

        if self
            .ui
            .add(cols, button(button_text).style(Some(self.green_style)))
            .clicked
        {
            match self.play_state {
                PlayState::Connected { .. } => {
                    self.ui.show_popup_at_last(cols, "connection");
                }
                PlayState::Offline { .. } => {
                    self.on_map_play(context);
                }
                _ => {}
            }
        }

        match &self.play_state {
            PlayState::Connected { url } => {
                if let Some(popup) = self.ui.is_popup_shown(cols, "connection") {
                    if self.ui.add(popup, button("Open Link").item(true)).clicked {
                        self.report_error(open::that(&url).context("Opening URL"));
                        self.ui.hide_popup();
                    }
                    if self.ui.add(popup, button("Copy Link").item(true)).clicked {
                        let result = self.clipboard.set_text(url.clone()).context("Copying");
                        self.report_error(result);
                        self.ui.hide_popup();
                    }
                    self.ui.add(popup, separator());
                    if self.ui.add(popup, button("Disconnect").item(true)).clicked {
                        self.connection.disconnect();
                        self.ui.hide_popup();
                    }
                } else {
                    last_tooltip(
                        &mut self.ui,
                        cols,
                        "You are connected to an online match.\n\nSave your map to update it in game.",
                        self.font_tiny,
                        true,
                    );
                }
            }
            PlayState::Offline => {
                last_tooltip(
                    &mut self.ui,
                    cols,
                    "Test your map in Crate Before Attack.\n\nUploads the map to the server and opens a private link to the match.",
                    self.font_tiny,
                    true,
                );
            }
            _ => {}
        }
    }

    fn on_map_play(&mut self, context: &mut miniquad::Context) {
        self.on_map_save(context);
        let content = std::fs::read(self.doc_path.as_ref().unwrap()).unwrap();

        if !matches!(self.connection.state, ConnectionState::Connected) {
            self.connection.connect("ws://localhost:8099/editor");
            self.play_state = PlayState::Connecting;
        }
        self.network_operation = Some(Box::new(upload_map_operation(content)));
    }
}

fn material_drop_down(
    ui: &mut UI,
    area: rimui::AreaRef,
    value: &mut u8,
    materials: &[MaterialSlot],
) -> bool {
    use rimui::*;
    let text = materials
        .get(*value as usize)
        .map(|m| m.label())
        .unwrap_or("None");
    if button_drop_down(ui, area, text, None, Align::Left, true, false, 0).clicked {
        ui.show_popup_at(area, "material_drop_down", false);
    }
    let mut result = false;
    if let Some(popup) = ui.is_popup_shown(area, "material_drop_down") {
        for (i, mat) in materials.iter().enumerate() {
            if ui
                .add(
                    popup,
                    button(mat.label()).item(true).down(i == *value as usize),
                )
                .clicked
            {
                *value = i as u8;
                result = true;
            }
        }
    }
    result
}

fn upload_map_operation(mut content: Vec<u8>) -> impl FnMut(&mut App) -> bool {
    let content = Arc::new(Blob(content));

    let mut hasher = twox_hash::XxHash64::with_seed(0);
    hasher.write(content.0.as_slice());
    let map_hash = hasher.finish();

    move |app| {
        match app.connection.state {
            ConnectionState::Connecting => {
                return false;
            }
            ConnectionState::Offline => {
                app.report_error(Result::<()>::Err(anyhow!("Failed to connect to server")));
                return true;
            }
            ConnectionState::Connected => {}
        }

        let result = (|| {
            send_message(
                &mut app.connection,
                EditorClientMessage::Introduction {
                    protocol_version: EDITOR_PROTOCOL_VERSION,
                    build: env!("GIT_HASH").to_owned(),
                },
            )?;

            send_message(
                &mut app.connection,
                EditorClientMessage::Upload {
                    map_hash,
                    content: content.clone(),
                },
            )?;

            Ok(())
        })();
        if let Err(err) = result {
            app.report_error(Result::<()>::Err(err));
        }
        true
    }
}

fn send_message(connection: &mut ClientConnection, message: EditorClientMessage) -> Result<()> {
    let bytes = bincode::options()
        .serialize(&message)
        .context("Serializing message")?;
    connection.send(bytes);
    info!("sent {:?}", &message);
    Ok(())
}

fn last_tooltip(ui: &mut UI, parent: AreaRef, tooltip_text: &str, font: FontKey, below: bool) {
    use rimui::*;
    if let Some(t) = ui.last_tooltip(
        parent,
        Tooltip {
            placement: if below {
                TooltipPlacement::Below
            } else {
                TooltipPlacement::Beside
            },
            ..Tooltip::default()
        },
    ) {
        let frame = ui.add(t, Frame::default().margins([6, 6, 6, 3]));
        let rows = ui.add(frame, vbox());
        ui.add(
            rows,
            wrapped_text("text", tooltip_text)
                .max_width(400)
                .align(Align::Left)
                .font(Some(font)),
        );
    }
}

pub fn button_drop_down(
    ui: &mut rimui::UI,
    area: rimui::AreaRef,
    text: &str,
    font: Option<FontKey>,
    align: rimui::Align,
    enabled: bool,
    expand: bool,
    sprite: SpriteKey,
) -> rimui::ButtonState {
    use rimui::*;
    let state = ui.add(area, button_area(text).enabled(enabled).expand(expand));
    let h = if matches!(align, Center) {
        let st = ui.add(state.area, stack());
        ui.add(
            st,
            label(text)
                .font(font)
                .color(Some(state.text_color))
                .offset([0, -2])
                .align(align)
                .expand(expand)
                .height_mode(LabelHeight::Custom(23.0)),
        );
        let h = ui.add(st, hbox().padding(2).margins([0, 0, 0, 0]));
        ui.add(h, spacer().expand(true));
        h
    } else {
        let h = ui.add(state.area, hbox().padding(2).margins([0, 0, 0, 0]));
        ui.add(
            h,
            label(text)
                .font(font)
                .color(Some(state.text_color))
                .offset([0, -2])
                .align(align)
                .expand(expand),
        );
        h
    };
    ui.add(h, image(sprite).color(state.text_color).offset([0, -1]));
    state
}

fn tooltip_impl(
    ui: &mut rimui::UI,
    parent: rimui::AreaRef,
    beside: bool,
    text: &str,
    shortcut: Option<&str>,
    shortcut_key_sprite: SpriteKey,
) {
    use rimui::*;
    if let Some(t) = ui.last_tooltip(
        parent,
        Tooltip {
            placement: if beside {
                TooltipPlacement::Beside
            } else {
                TooltipPlacement::Below
            },
            ..Default::default()
        },
    ) {
        let frame = ui.add(
            t,
            Frame {
                margins: [6, 6, 6, 3],
                ..Default::default()
            },
        );
        let rows = ui.add(frame, vbox());
        let tooltip_font = Some(ui.default_style().tooltip_font);
        ui.add(
            rows,
            WrappedText {
                text,
                font: tooltip_font,
                max_width: 400,
                align: Left,
                ..Default::default()
            },
        );
        if let Some(shortcut) = shortcut {
            let h = ui.add(rows, hbox().padding(1));
            ui.add(h, label("Shortcut:").font(tooltip_font).offset([0, -2]));
            ui.add(h, label("").min_size([4, 0]));
            for (index, key) in shortcut.split('+').enumerate() {
                if index != 0 {
                    ui.add(h, label("+").font(tooltip_font));
                }
                ui_key_str(ui, h, shortcut_key_sprite, key, tooltip_font);
            }
        }
    }
}

pub fn tooltip(ui: &mut rimui::UI, parent: rimui::AreaRef, text: &str) {
    tooltip_impl(ui, parent, true, text, None, SpriteKey::default())
}

pub fn ui_key_str(
    ui: &mut rimui::UI,
    p: rimui::AreaRef,
    key_sprite: SpriteKey,
    text: &str,
    font: Option<FontKey>,
) {
    use rimui::*;
    let st = ui.add(p, stack());
    ui.add(st, image(key_sprite));
    ui.add(
        st,
        label(text)
            .offset([0, -3])
            .font(font)
            .align(Center)
            .color(Some([160, 160, 160, 255])),
    );
}

pub trait Tooltip {
    fn tooltip(&self) -> &'static str;
}
impl Tooltip for MarkupPointKind {
    fn tooltip(&self) -> &'static str {
        match self {
            MarkupPointKind::Start => {
                "A point where frog will spawn. Overides default random placement."
            }
        }
    }
}

impl Tooltip for MarkupRectKind {
    fn tooltip(&self) -> &'static str {
        match self {
            MarkupRectKind::RaceFinish => "Finish area for Race rules.",
        }
    }
}
