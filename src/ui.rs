use crate::app::App;
use crate::document::{ChangeMask, Document};
use crate::tool::Tool;
use crate::zone::{EditorBounds, ZoneRef};
use anyhow::Context;
use cbmap::{MapMarkup, MarkupPoint, MarkupPointKind, MarkupRect, MarkupRectKind};
use glam::vec2;
use rimui::*;
use std::borrow::Borrow;
use std::mem::discriminant;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};

impl App {
    pub fn ui(&mut self, context: &mut miniquad::Context, _time: f32, dt: f32) {
        self.ui_toolbar(context);

        self.ui_sidebar(context);
        if matches!(self.tool, Tool::Zone) {
            self.ui_zone_list(context);
        }

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
                size: [0, self.window_size[1] as i32 - 8],
                expand: EXPAND_LEFT,
            },
            0,
            0,
        );

        let frame = self.ui.add(window, Frame::default());
        let rows = self
            .ui
            .add(frame, vbox().padding(2).min_size([sidebar_width as u16, 0]));
        self.ui.add(rows, label("Materials"));

        for (index, material) in self.doc.borrow().materials.iter().enumerate().skip(1) {
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

        self.ui.add(rows, label("Layers"));
        self.ui
            .add(rows, button("1. Grid").down(true).align(Some(Align::Left)));

        self.ui.add(rows, label("Reference"));
        if self.doc.borrow().reference_path.is_some() {
            let show_reference = self.doc.borrow().show_reference;
            if self
                .ui
                .add(rows, button("Show Reference").down(show_reference))
                .clicked
            {
                self.doc.borrow_mut().show_reference = !show_reference;
            }

            let hbar = self.ui.add(rows, hbox());
            self.ui.add(hbar, label("Scale:"));
            if self
                .ui
                .add(
                    hbar,
                    button("1x").down(self.doc.borrow().reference_scale == 1),
                )
                .clicked
            {
                self.doc.borrow_mut().reference_scale = 1;
            }
            if self
                .ui
                .add(
                    hbar,
                    button("2x").down(self.doc.borrow().reference_scale == 2),
                )
                .clicked
            {
                self.doc.borrow_mut().reference_scale = 2;
            }
        }

        let mut doc = self.doc.borrow_mut();
        let mut buffer = String::new();
        let reference_text = doc
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

        if self.ui.add(rows, button(reference_text)).clicked {
            let new_reference_path = self.report_error({
                let path = doc.reference_path.as_ref().map(PathBuf::from);
                nfd2::open_file_dialog(Some("png"), path.as_ref().map(|p| p.as_path()))
                    .context("Opening dialog")
            });

            if let Some(nfd2::Response::Okay(new_reference_path)) = new_reference_path {
                doc.reference_path = Some(new_reference_path.to_string_lossy().to_string());
                self.graphics.borrow_mut().generate(
                    &doc,
                    ChangeMask {
                        reference_path: true,
                        ..ChangeMask::default()
                    },
                    Some(context),
                );
            }
        }
        if let Some(path) = &doc.reference_path {
            last_tooltip(&mut self.ui, rows, path);
        }

        drop(doc);
    }

    fn ui_zone_list(&mut self, context: &mut miniquad::Context) {
        let sidebar_width = 280;
        let zone_window = self.ui.window(
            "Zones",
            WindowPlacement::Absolute {
                pos: [self.window_size[0] as i32 - 20 - sidebar_width, 8],
                size: [0, 0],
                expand: EXPAND_LEFT | EXPAND_DOWN,
            },
            0,
            0,
        );

        let frame = self.ui.add(zone_window, Frame::default());
        let rows = self
            .ui
            .add(frame, vbox().padding(2).min_size([sidebar_width as u16, 0]));

        let row = self.ui.add(rows, hbox());
        self.ui.add(row, label("Zones").expand(true));

        let mut dirty = false;
        let doc = self.doc.borrow();
        let selection = doc.zone_selection;
        let mut new_selection = None;
        let font = Some(0);
        let font_chat = 0;

        let can_add_start = !doc
            .markup
            .points
            .iter()
            .any(|p| p.kind == MarkupPointKind::Start);
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

                    let mut doc = self.doc.borrow_mut();
                    new_selection = Some(ZoneRef::Point(doc.markup.points.len()));
                    doc.markup.points.push(MarkupPoint {
                        kind: MarkupPointKind::Start,
                        pos: center,
                    });
                    dirty = true;
                }
                tooltip(&mut self.ui, p, MarkupPointKind::Start.tooltip());
            }

            if can_add_finish {
                if self.ui.add(p, button("Race Finish").item(true)).clicked {
                    self.ui.hide_popup();
                    self.push_undo("Add Race Finish");

                    let mut doc = self.doc.borrow_mut();
                    new_selection = Some(ZoneRef::Rect(doc.markup.rects.len()));
                    doc.markup.rects.push(MarkupRect {
                        kind: MarkupRectKind::RaceFinish,
                        start: [center[0] - 100, center[1] - 100],
                        end: [center[0] + 100, center[1] + 100],
                    });
                    dirty = true;
                }
                tooltip(&mut self.ui, p, MarkupRectKind::RaceFinish.tooltip());
            }
        }

        let doc = self.doc.borrow();
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
            let mut doc = self.doc.borrow_mut();
            doc.markup = MapMarkup::new();
            doc.zone_selection = None;
            dirty = true;
        }
        if self
            .ui
            .add(h, button("Delete").enabled(selection.is_some()))
            .clicked
        {
            if let Some(selection) = selection {
                self.push_undo("Delete Zone");
                let mut doc = self.doc.borrow_mut();
                selection.remove_zone(&mut doc.markup);
                if !selection.is_valid(&doc.markup) {
                    doc.zone_selection = None;
                }
            }
        }

        if let Some(new_selection) = new_selection {
            let mut doc = self.doc.borrow_mut();
            if doc.zone_selection != Some(new_selection) {
                doc.zone_selection = Some(new_selection);
            } else {
                let (start, end) = new_selection.bounds(&doc.markup, &self.view);
                let center = (start + end) * 0.5;
                self.view.target = self.view.screen_to_world().transform_point2(center).floor();
            }
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
        {
            let frame = self.ui.add(toolbar, Frame::default());
            let cols = self.ui.add(frame, hbox());
            self.ui.add(cols, label("Map"));
            if self.ui.add(cols, button("Open")).clicked {
                let response =
                    self.report_error(nfd2::open_file_dialog(None, None).context("Opening dialog"));
                if let Some(nfd2::Response::Okay(path)) = response {
                    let doc = self.report_error(App::load_doc(&path));
                    if let Some(doc) = doc {
                        self.doc.replace(doc);
                        self.doc_path = Some(path);
                        let state_res = self.save_app_state();
                        self.report_error(state_res);
                    }
                };
            }
            let mut save_as = false;
            if self.ui.add(cols, button("Save")).clicked {
                if let Some(path) = &self.doc_path {
                    self.doc.borrow_mut().pre_save_cleanup();
                    self.report_error(App::save_doc(
                        path,
                        &self.doc.borrow(),
                        &self.graphics.borrow(),
                        self.white_texture.clone(),
                        self.pipeline.clone(),
                        &self.view,
                        context,
                        self.active_material,
                    ));
                    let state_res = self.save_app_state();
                    self.report_error(state_res);
                } else {
                    save_as = true;
                }
            }
            if self.ui.add(cols, button("Save As...")).clicked {
                save_as = true;
            }

            if save_as {
                let path = self.report_error(
                    nfd2::open_save_dialog(Some("js"), None).context("Opening dialog"),
                );

                if let Some(nfd2::Response::Okay(path)) = path {
                    self.doc.borrow_mut().pre_save_cleanup();
                    self.report_error(App::save_doc(
                        Path::new(&path),
                        &self.doc.borrow(),
                        &self.graphics.borrow(),
                        self.white_texture.clone(),
                        self.pipeline.clone(),
                        &self.view,
                        context,
                        self.active_material,
                    ));
                    let state_res = self.save_app_state();
                    self.report_error(state_res);
                }
            }

            self.ui.add(cols, label("Edit"));
            if (self.ui.add(cols, button("Undo").enabled(!self.undo.is_empty())).clicked ||
                //self.ui.key_pressed_with_modifiers(KeyCode::Z, true, false, false) {
                self.ui.key_pressed(KeyCode::Z))
                && !self.undo.is_empty()
            {
                let mut doc_ref = self.doc.borrow_mut();
                let doc: &mut Document = &mut doc_ref;
                let err = self.undo.apply(doc, &mut self.redo);
                self.report_error(err);
                self.dirty_mask = ChangeMask {
                    cells: true,
                    reference_path: false,
                };
            }
            if (self.ui.add(cols, button("Redo").enabled(!self.redo.is_empty())).clicked ||
                //self.ui.key_pressed_with_modifiers(KeyCode::Z, true, true, false)
                self.ui.key_pressed(KeyCode::Y))
                && !self.redo.is_empty()
            {
                let mut doc_ref = self.doc.borrow_mut();
                let doc: &mut Document = &mut doc_ref;
                let err = self.redo.apply(doc, &mut self.undo);
                self.report_error(err);
                self.dirty_mask = ChangeMask {
                    cells: true,
                    reference_path: false,
                };
            }

            self.ui.add(cols, label("Tool"));

            let tools = [
                (Tool::Pan, "Pan"),
                (Tool::Paint, "Paint"),
                (Tool::Fill, "Fill"),
                (Tool::Rectangle, "Rectangle"),
                (Tool::Zone, "Zone"),
            ];

            let old_tool = self.tool.clone();

            for (tool, title) in tools.iter() {
                let is_selected = discriminant(&old_tool) == discriminant(&tool);
                if self.ui.add(cols, button(title).down(is_selected)).clicked {
                    self.tool = *tool;
                }
            }
        }
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
}

fn last_tooltip(ui: &mut UI, parent: AreaRef, tooltip_text: &str) {
    use rimui::*;
    if let Some(t) = ui.last_tooltip(
        parent,
        Tooltip {
            placement: TooltipPlacement::Beside,
            ..Tooltip::default()
        },
    ) {
        let frame = ui.add(t, Frame::default());
        let rows = ui.add(frame, vbox());
        ui.add(rows, label(tooltip_text));
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
