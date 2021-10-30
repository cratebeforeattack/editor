#![allow(dead_code)]
mod font_manager;
mod text_editor;
pub use font_manager::*;

mod miniquad_render;
pub use miniquad_render::*;

use glam::{vec2, Vec2};
use std::borrow::Borrow;
use std::cmp::{max, min};
use std::sync::Arc;
// use superslice::Ext;
use log::info;

type Position = i32;

type Margins = [i16; 4];
const MARGINS_DEFAULT: Margins = [-32767, -32767, -32767, -32767];

pub type IndexType = u16;
pub trait VertexSlice<'a> {
    fn set_pos_uv(&mut self, i: IndexType, pos: [f32; 2], uv: [f32; 2]);
    fn set_index(&mut self, i: IndexType, value: IndexType);
    fn len_verts(&self) -> usize;
    fn len_indices(&self) -> usize;
    fn first_vertex(&self) -> IndexType;
}
pub trait Render {
    fn set_sprite(&mut self, sprite: Option<SpriteKey>);
    fn set_clip(&mut self, clip: Option<[i32; 4]>);
    fn add_vertices(
        &mut self,
        positions: &[[f32; 2]],
        uvs: &[[f32; 2]],
        indices: &[IndexType],
        color: [u8; 4],
    );
    fn draw_text(&mut self, font: FontKey, text: &str, pos: [f32; 2], color: [u8; 4], scale: f32);
    fn draw_rounded_rect(
        &mut self,
        rect: [f32; 4],
        radius: f32,
        thickness: f32,
        outline_color: [u8; 4],
        fill_color: [u8; 4],
    );
}

pub type SpriteKey = usize;
pub trait SpriteContext {
    fn sprite_size(&self, key: SpriteKey) -> [u32; 2];
    fn sprite_uv(&self, key: SpriteKey) -> [f32; 4];
}

pub type FontKey = usize;
pub trait FontContext {
    fn load_font(&mut self, name: &str) -> FontKey;
    fn measure_text(&self, font: FontKey, label: &str, scale: f32) -> [f32; 2];
    fn hit_character(&self, font: FontKey, label: &str, scale: f32, pos: f32) -> Option<u32>;
    fn font_height(&self, font: FontKey) -> f32;
    fn font_ascent(&self, font: FontKey) -> f32;
    fn font_descent(&self, font: FontKey) -> f32;
    fn wrap_text(
        &self,
        wrapped_lines: &mut Vec<(i32, i32, i32)>,
        font: FontKey,
        text: &str,
        width: i32,
    ) -> i32;
}

pub type Rect = [Position; 4];

#[derive(Copy, Clone)]
pub struct Frame {
    pub frame_type: FrameType,
    pub margins: Margins,
    pub offset: [Position; 2],
    pub color: [u8; 4],
    pub def: bool,
    pub expand: bool,
}

impl Frame {
    pub fn margins(self, margins: Margins) -> Self {
        Self { margins, ..self }
    }
    pub fn color(self, color: [u8; 4]) -> Self {
        Self { color, ..self }
    }
}
pub fn frame() -> Frame {
    Frame::default()
}

pub struct Center<'l> {
    pub id: &'l str,
    pub expand: bool,
    pub align: [i8; 2],
    pub min_size: [u16; 2],
    pub position: [Position; 2],
    pub scale: [f32; 2],
    pub offset: [Position; 2],
}

pub fn center<'l>(id: &'l str) -> Center<'l> {
    Center {
        id,
        ..Default::default()
    }
}

impl<'l> Center<'l> {
    pub fn position(self, position: [Position; 2]) -> Self {
        Self { position, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn align(self, align: [i8; 2]) -> Self {
        Self { align, ..self }
    }
}

impl<'l> Default for Center<'l> {
    fn default() -> Self {
        Self {
            id: "",
            expand: false,
            align: [0, 0],
            min_size: [0, 0],
            position: [0, 0],
            scale: [1.0, 1.0],
            offset: [0, 0],
        }
    }
}

#[derive(Default)]
pub struct Stack {
    pub expand: bool,
    pub min_size: [u16; 2],
    pub offset: [Position; 2],
}
pub fn stack() -> Stack {
    Stack::default()
}

impl Stack {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum LabelHeight {
    // using FreeType conventions
    LineSpace, // ascent + (-descent) + line_gap
    NoLineGap, // ascent + (-deccent)
    Ascent,    // ascent only
    Custom(f32),
}

#[derive(Copy, Clone)]
pub struct Label<'l> {
    pub label_id: &'l str,
    pub expand: bool,
    pub min_size: [u16; 2],
    pub scale: f32,
    pub font: Option<FontKey>,
    pub height_mode: LabelHeight,
    pub offset: [Position; 2],
    pub color: Option<[u8; 4]>,
    pub align: Align,
}

impl<'l> Label<'l> {
    pub fn label_id(self, label_id: &'l str) -> Self {
        Self { label_id, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }
    pub fn font(self, font: Option<FontKey>) -> Self {
        Self { font, ..self }
    }
    pub fn height_mode(self, height_mode: LabelHeight) -> Self {
        Self {
            height_mode,
            ..self
        }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn color(self, color: Option<[u8; 4]>) -> Self {
        Self { color, ..self }
    }
    pub fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
}
pub fn label<'l>(label_id: &'l str) -> Label<'l> {
    Label {
        label_id,
        ..Default::default()
    }
}

pub fn spacer() -> UIImage {
    UIImage {
        expand: true,
        ..Default::default()
    }
}

pub struct Edit<'i, 's> {
    pub id: &'i str,
    pub text: Option<&'s mut String>,
    pub expand: bool,
    pub min_size: [u16; 2],
    pub scale: f32,
    pub font: Option<FontKey>,
    pub height_mode: LabelHeight,
    pub offset: [Position; 2],
    pub color: Option<[u8; 4]>,
    pub align: Align,
    pub multiline: bool,
}

impl<'i, 's> Edit<'i, 's> {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }
    pub fn font(self, font: Option<FontKey>) -> Self {
        Self { font, ..self }
    }
    pub fn height_mode(self, height_mode: LabelHeight) -> Self {
        Self {
            height_mode,
            ..self
        }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn color(self, color: Option<[u8; 4]>) -> Self {
        Self { color, ..self }
    }
    pub fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }
    pub fn multiline(self, multiline: bool) -> Self {
        Self { multiline, ..self }
    }
}
pub fn edit<'i, 's>(id: &'i str, text: &'s mut String) -> Edit<'i, 's> {
    Edit {
        id,
        text: Some(text),
        ..Default::default()
    }
}

pub struct UIImage {
    pub sprite_id: Option<SpriteKey>,
    pub expand: bool,
    pub min_size: [u16; 2],
    pub scale: [f32; 2],
    pub offset: [Position; 2],
    pub color: [u8; 4],
}
impl UIImage {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn scale(self, scale: [f32; 2]) -> Self {
        Self { scale, ..self }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn color(self, color: [u8; 4]) -> Self {
        Self { color, ..self }
    }
}
pub fn image(sprite_id: SpriteKey) -> UIImage {
    UIImage {
        sprite_id: Some(sprite_id),
        ..Default::default()
    }
}

pub struct WrappedText<'l> {
    pub id: &'l str,
    pub text: &'l str,
    pub expand: bool,
    pub min_size: [u16; 2],
    // limits text width, but keeps smaller size if text fits
    pub max_width: u16,
    pub align: Align,
    pub color: Option<[u8; 4]>,
    pub font: Option<FontKey>,
    pub offset: [Position; 2],
    pub scale: f32,
}
impl<'l> WrappedText<'l> {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn max_width(self, max_width: u16) -> Self {
        Self { max_width, ..self }
    }
    pub fn font(self, font: Option<FontKey>) -> Self {
        Self { font, ..self }
    }
    pub fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }
    pub fn color(self, color: Option<[u8; 4]>) -> Self {
        Self { color, ..self }
    }
}
pub fn wrapped_text<'l>(id: &'l str, text: &'l str) -> WrappedText<'l> {
    WrappedText {
        id,
        text,
        ..Default::default()
    }
}

#[derive(Copy, Clone)]
pub enum BoxOrientation {
    Horizontal,
    Vertical,
}
pub use BoxOrientation::*;

#[derive(Copy, Clone)]
pub struct BoxLayout {
    pub orientation: BoxOrientation,
    pub expand: bool,
    pub min_size: [u16; 2],
    pub scale: [f32; 2],
    pub offset: [Position; 2],
    pub padding: i16,
    pub margins: Margins,
}

impl BoxLayout {
    pub fn orientation(self, orientation: BoxOrientation) -> Self {
        Self {
            orientation,
            ..self
        }
    }
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn scale(self, scale: [f32; 2]) -> Self {
        Self { scale, ..self }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn padding(self, padding: i16) -> Self {
        Self { padding, ..self }
    }
    pub fn margins(self, margins: Margins) -> Self {
        Self { margins, ..self }
    }
}
pub fn vbox() -> BoxLayout {
    BoxLayout {
        orientation: BoxOrientation::Vertical,
        ..Default::default()
    }
}
pub fn hbox() -> BoxLayout {
    BoxLayout {
        orientation: BoxOrientation::Horizontal,
        ..Default::default()
    }
}

#[derive(Copy, Clone)]
pub struct Separator {
    pub expand: bool,
    pub margins: Margins,
    pub color: [u8; 4],
    pub offset: [Position; 2],
    pub width: u16,
}

pub fn separator() -> Separator {
    Separator::default()
}

impl Default for Separator {
    fn default() -> Separator {
        Separator {
            expand: false,
            margins: [0, 0, 0, 0],
            color: [255, 255, 255, 255],
            offset: [0, 0],
            width: 2,
        }
    }
}
impl Separator {
    pub fn margins(self, margins: Margins) -> Self {
        Self { margins, ..self }
    }
}

#[derive(Copy, Clone)]
pub struct Button<'l> {
    pub label_id: &'l str,
    pub sprite_id: Option<SpriteKey>,
    pub expand: bool,
    pub margins: Margins,
    pub min_size: [u16; 2],
    pub offset: [Position; 2],
    pub color: Option<[u8; 4]>,
    pub content_color: Option<[u8; 4]>,
    pub font: Option<FontKey>,
    pub style: Option<StyleKey>,
    pub scale: [f32; 2],
    pub enabled: bool,
    pub down: bool,
    pub can_be_pushed: bool,
    pub for_area: bool,
    pub item: bool,
    pub align: Option<Align>,
}

impl<'l> Button<'l> {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn margins(self, margins: Margins) -> Self {
        Self { margins, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn scale(self, scale: [f32; 2]) -> Self {
        Self { scale, ..self }
    }
    pub fn font(self, font: Option<FontKey>) -> Self {
        Self { font, ..self }
    }
    pub fn offset(self, offset: [Position; 2]) -> Self {
        Self { offset, ..self }
    }
    pub fn color(self, color: Option<[u8; 4]>) -> Self {
        Self { color, ..self }
    }
    pub fn content_color(self, content_color: Option<[u8; 4]>) -> Self {
        Self {
            content_color,
            ..self
        }
    }
    pub fn down(self, down: bool) -> Self {
        Self { down, ..self }
    }
    pub fn item(self, item: bool) -> Self {
        Self { item, ..self }
    }
    pub fn enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }
    pub fn style(self, style: Option<StyleKey>) -> Self {
        Self { style, ..self }
    }
    pub fn align(self, align: Option<Align>) -> Self {
        Self { align, ..self }
    }
}
pub fn button<'l>(label_id: &'l str) -> Button<'l> {
    Button {
        label_id,
        ..Default::default()
    }
}
pub fn button_with_image<'l>(id: &'l str, sprite: SpriteKey) -> Button<'l> {
    Button {
        label_id: id,
        sprite_id: Some(sprite),
        ..Default::default()
    }
}
pub fn button_area<'l>(id: &'l str) -> Button<'l> {
    Button {
        label_id: id,
        for_area: true,
        ..Default::default()
    }
}

pub struct ButtonState {
    pub area: AreaRef,
    pub clicked: bool,
    pub down: bool,
    pub hovered: bool,
    pub text_color: [u8; 4],
}

#[derive(Copy, Clone)]
pub struct Progress {
    pub color: Option<[u8; 4]>,
    pub progress: f32,
    pub scale: f32,
    pub expand: bool,
    pub align: Align,
    pub min_size: [u16; 2],
}
pub fn progress() -> Progress {
    Default::default()
}
impl Progress {
    pub fn color(self, color: Option<[u8; 4]>) -> Self {
        Self { color, ..self }
    }
    pub fn progress(self, progress: f32) -> Self {
        Self { progress, ..self }
    }
    pub fn scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
}

pub struct ScrollArea<'i> {
    pub id: &'i str,
    pub expand: bool,
    pub min_size: [u16; 2],
    pub max_size: [u16; 2],
    pub margins: Margins,
    pub scale: f32,
    pub align: [i8; 2],
    pub enabled: bool,
}
impl<'i> ScrollArea<'i> {
    pub fn expand(self, expand: bool) -> Self {
        Self { expand, ..self }
    }
    pub fn min_size(self, min_size: [u16; 2]) -> Self {
        Self { min_size, ..self }
    }
    pub fn max_size(self, max_size: [u16; 2]) -> Self {
        Self { max_size, ..self }
    }
    pub fn margins(self, margins: Margins) -> Self {
        Self { margins, ..self }
    }
    pub fn scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }
    pub fn align(self, align: [i8; 2]) -> Self {
        Self { align, ..self }
    }
    pub fn enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }
}
pub fn scroll_area<'i>(id: &'i str) -> ScrollArea<'i> {
    ScrollArea {
        id,
        expand: false,
        min_size: [0, 0],
        max_size: [0, 0],
        margins: [0, 0, 0, 0],
        scale: 1.0,
        align: [0, -1],
        enabled: true,
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CustomRect {
    pub user_data: u32,
    pub expand: bool,
    pub min_size: [u16; 2],
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Tooltip {
    pub placement: TooltipPlacement,
    pub padding: Position,
    pub offset_along: Position,
}

pub struct EditState {
    id: ItemId,
    window: ItemId,
    state: text_editor::EditboxState,
    scroll: [Position; 2],
}

pub struct UI {
    pub font_context: Option<Arc<dyn FontContext>>,
    pub sprite_context: Option<Arc<dyn SpriteContext>>,
    pub styles: slotmap::SlotMap<StyleKey, UIStyle>,

    pub style: StyleKey,
    pub flat_button_style: StyleKey,

    // active popup
    shown_popup: Option<(ItemId, Rect, TooltipPlacement)>,

    // item pressed by mouse
    hit_item: ItemId,
    hovered_item: ItemId,
    released_item: ItemId,
    input_focus: Option<(ItemId, ItemId)>,
    edit_state: Option<EditState>,

    render_rect: [i32; 4],
    hseparator_sprite: Option<SpriteKey>,
    vseparator_sprite: Option<SpriteKey>,

    window_ids: Vec<u32>,
    window_order: Vec<usize>,
    windows: Vec<Window>,
    window_draw_items: Vec<Vec<DrawItem>>,
    window_draw_texts: Vec<String>,

    // populated during rendering
    pub custom_rects: Vec<(u32, Rect)>,

    new_named_areas: Vec<u32>,
    named_areas: Vec<NamedArea>,
    frame: u32,
    last_mouse_position: [Position; 2],
    frame_input: Vec<UIEvent>,

    debug_frame: u64,
}

struct ClipItem {
    parent: usize,
    element_index: ElementIndex,
    margins: Margins,
}

struct Window {
    id_str: String,
    id: ItemId,
    top_hash: ItemId,
    hash_stack: Vec<ItemId>,
    sort_key: i32,
    areas: Vec<Area>,
    update_frame: u32,
    placement: WindowPlacement,
    flags: WindowFlags,
    layout: Layout,
    computed_rect: [i32; 4],
    hit_items: Vec<HitItem>,
    clip_items: Vec<ClipItem>,

    image_ids: Vec<ItemId>,
    images: Vec<SpriteKey>,

    // wrapped text
    wrapped_text_elements: Vec<ElementIndex>,
    wrapped_texts: Vec<WrappedTextItem>,

    drag_items: Vec<DragItem>,
    drop_items: Vec<DropItem>,
    drag_item: usize,
    over_drop_item: usize,
    drag_start: [i32; 2],
    drag_offset: [i32; 2],
    drag_result: DragResult,
    drop_result: DragResult,

    // scroll state
    scroll_item: ItemId,
    scrolls: Vec<(ItemId, TouchScroll)>,
    scroll_elements: Vec<(ItemId, ElementIndex)>,
    scroll_animations: Vec<ScrollAnimation>,

    // temporary
    clip_item_rects: Vec<Rect>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KeyCode {
    Left,
    Up,
    Right,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Backspace,
    Delete,
    A,
    V,
    C,
    X,
    Z,
    Y,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
}

#[derive(Debug, Clone)]
pub enum UIEvent {
    MouseDown {
        pos: [i32; 2],
        button: i32,
        time: f64,
    },
    MouseUp {
        pos: [i32; 2],
        button: i32,
    },
    MouseMove {
        pos: [i32; 2],
    },
    MouseWheel {
        pos: [i32; 2],
        delta: f32,
    },
    TextInput {
        text: String,
    },
    KeyDown {
        key: KeyCode,
        control: bool,
        shift: bool,
        alt: bool,
    },
    TouchDown {
        finger: i32,
        pos: [i32; 2],
    },
    TouchMove {
        finger: i32,
        pos: [i32; 2],
    },
    TouchUp {
        finger: i32,
        pos: [i32; 2],
    },
}

pub type ItemId = u32;
const INVALID_ITEM_ID: ItemId = 0;
type ElementIndex = i32;
const INVALID_ELEMENT_INDEX: ElementIndex = -1;
type ChildrenListIndex = i16;
const INVALID_CHILDREN_LIST: ChildrenListIndex = -1;

pub type ExpandFlags = u32;
pub const EXPAND_RIGHT: ExpandFlags = 1 << 0;
pub const EXPAND_DOWN: ExpandFlags = 1 << 1;
pub const EXPAND_LEFT: ExpandFlags = 1 << 2;
pub const EXPAND_UP: ExpandFlags = 1 << 3;
pub const EXPAND_ALL: ExpandFlags = EXPAND_LEFT | EXPAND_RIGHT | EXPAND_UP | EXPAND_DOWN;

#[derive(Copy, Clone, Debug)]
pub enum TooltipPlacement {
    Beside,
    Below,
    BelowCentered,
}

impl Default for TooltipPlacement {
    fn default() -> Self {
        TooltipPlacement::Below
    }
}

pub enum WindowPlacement {
    Absolute {
        pos: [i32; 2],
        size: [i32; 2],
        expand: ExpandFlags,
    },
    Center {
        offset: [i32; 2],
        size: [i32; 2],
        expand: ExpandFlags,
    },
    Fullscreen,
    Tooltip {
        around_rect: [i32; 4],
        minimal_size: [i32; 2],
        placement: TooltipPlacement,
    },
}

pub type WindowFlags = u32;
pub const WINDOW_TRANSPARENT: WindowFlags = 1 << 0;

#[derive(Default)]
struct Area {
    scroll_to_time: f32,
    drag_id: usize,
    drop_id: usize,
    scroll_area_id: ItemId,
    scroll_area_element: i32,
    button_id: ItemId,
    button_offset: [Position; 2],
    last_element: i32,
    element_index: ElementIndex,
    clip_item_index: usize,
    can_be_pushed: bool,
}

struct NamedArea {
    id: ItemId,
    window_id: ItemId,
    element_index: i32,
    last_rect: [i32; 4],
}

#[derive(Debug)]
struct HitItem {
    item_id: ItemId,
    element_index: ElementIndex,
    style: StyleKey,
    frame_type: Option<FrameType>,
    clip_item_index: usize,
    is_scroll: bool,
    consumes_keys: bool,
    consumes_chars: bool,
}

#[derive(Debug)]
struct DrawItem {
    element_index: ElementIndex,
    clip: usize,
    dragged: bool,
    offset: [i32; 2],
    color: [u8; 4],
    command: DrawCommand,
}

pub struct CustomDrawFunc(pub Box<dyn Fn(&mut dyn Render, Rect)>);
impl std::fmt::Debug for CustomDrawFunc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomDrawFunc").finish()
    }
}

#[derive(Debug)]
enum DrawCommand {
    None,
    Rect,
    Image {
        sprite: SpriteKey,
        scale: [f32; 2],
    },
    Separator {
        style: StyleKey,
        frame_type: FrameType,
    },
    Frame {
        style: StyleKey,
        frame_type: FrameType,
    },
    Progress {
        style: StyleKey,
        align: Align,
        progress: f32,
    },
    Text {
        text: (u32, u32),
        font: FontKey,
        scale: f32,
        alignment: Align,
        height_mode: LabelHeight,
        caret: Option<u32>,
        selection: Option<(u32, u32)>,
    },
    WrappedText {
        index: u32,
    },
    CustomRect {
        user_data: u32,
    },
}

#[derive(Copy, Clone, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}
pub use Align::*;

struct WrappedTextItem {
    text: (u32, u32),
    font: FontKey,
    alignment: Align,
    max_width: u16,
    lines: Vec<(i32, i32, i32)>,
}

struct DragItem {
    id: usize,
    element_index: ElementIndex,
    clip_item_index: usize,
}

struct DropItem {
    id: usize,
    element_index: ElementIndex,
    clip_item_index: usize,
}

#[derive(Copy, Clone)]
struct DragResult {
    drag: usize,
    drop: usize,
}

struct Scroll {
    offset: [i32; 2],
    velocity: [f32; 2],
    remainder: [f32; 2],
    range: [i32; 4],
}

struct ScrollAnimation {
    velocity: Vec2,
    position: Vec2,
    target: Vec2,
    ease_time: f32,
    initialized: bool,
    scroll_area_id: ItemId,
    target_element: ElementIndex,
    duration: f32,
}

#[derive(Copy, Clone)]
pub struct FrameStyle {
    pub look: FrameLook,
    pub frame_thickness: Margins,
    pub margins: Margins,
    pub inset: Margins,
    pub clip: Margins,
    pub offset: [Position; 2],
    pub color: [u8; 4],
    pub content_offset: [Position; 2],
}

#[derive(Default, Clone, Copy)]
pub struct UIStyle {
    pub font: FontKey,
    pub tooltip_font: FontKey,
    pub text_color: [u8; 4],
    pub window_frame: FrameStyle,
    pub button_normal: ButtonStyle,
    pub button_hovered: ButtonStyle,
    pub button_pressed: ButtonStyle,
    pub button_disabled: ButtonStyle,
    pub hseparator: FrameStyle,
    pub vseparator: FrameStyle,
    pub progress_inner: FrameStyle,
    pub progress_outer: FrameStyle,
}
slotmap::new_key_type! {
    pub struct StyleKey;
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FrameType {
    Window,
    ButtonNormal,
    ButtonHovered,
    ButtonPressed,
    ButtonDisabled,
    HSeparator,
    VSeparator,
    ProgressInner,
    ProgressOuter,
}

#[derive(Copy, Clone)]
pub struct ButtonStyle {
    pub frame: FrameStyle,
    pub text_color: [u8; 4],
    pub content_offset: [Position; 2],
}

#[derive(Debug)]
struct Layout {
    elements: Vec<LayoutElement>,
    item_ids: Vec<ItemId>,
    rectangles: Vec<Rect>,
    minimal_sizes: [Vec<u16>; 2],
    children_lists: Vec<Vec<ElementIndex>>,
    next_children_list: ChildrenListIndex,
}

#[derive(Debug)]
struct LayoutElement {
    typ: ElementType,
    parent: ElementIndex,
    min_size: [u16; 2],
    expanding: bool,
    focus_flags: FocusFlags,
    scroll: [i16; 2],
    margins: Margins,
}

impl ElementType {
    fn children_list(&self) -> i16 {
        match &self {
            ElementType::Horizontal { children_list, .. } => *children_list,
            ElementType::Vertical { children_list, .. } => *children_list,
            ElementType::Stack { children_list, .. } => *children_list,
            ElementType::Align { children_list, .. } => *children_list,
            ElementType::Scroll { children_list, .. } => *children_list,
            ElementType::Frame { children_list, .. } => *children_list,
            _ => INVALID_CHILDREN_LIST,
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
enum ElementType {
    FixedSize,
    Align {
        children_list: i16,
        align: [i8; 2],
    },
    Scroll {
        children_list: i16,
        max_size: [u16; 2],
        align: [i8; 2],
    },
    Frame {
        children_list: i16,
    },
    HeightByWidth,
    Horizontal {
        children_list: i16,
        padding: i16,
    },
    Vertical {
        children_list: i16,
        padding: i16,
    },
    Stack {
        children_list: i16,
    },
}

#[derive(Debug)]
#[repr(u8)]
enum FocusFlags {
    NotFocusable = 0,
    Focusable = 1 << 0,
    ForwardsFocus = 1 << 1,
}

#[derive(Copy, Clone)]
struct TouchFinger {
    start_position: [i32; 2],
    last_position: [i32; 2],
    last_motion_time: f32,
}

struct TouchScroll {
    scrolling: bool,
    scroll_move_threshold: Position,
    size: [Position; 2],
    offset: Vec2,

    range: [f32; 4],

    velocity_sample_period: f32,
    minimal_velocity_threshold: f32,
    width: i32,
    height: i32,

    pressed: bool,

    velocity_samples: std::collections::VecDeque<(Vec2, f32)>,
    velocity_samples_duration: f32,

    offset_remainder: Vec2,
    velocity: Vec2,

    fingers: [TouchFinger; MAX_FINGERS as usize + 1],
    scroll_fingers_down: i32,
}

#[derive(Copy, Clone)]
pub struct AreaRef {
    window_index: u32,
    area_index: u32,
}

pub fn rect_width(rect: [i32; 4]) -> i32 {
    rect[2] - rect[0]
}
pub fn rect_size(rect: [i32; 4]) -> [i32; 2] {
    [rect[2] - rect[0], rect[3] - rect[1]]
}
pub fn rect_center(rect: [i32; 4]) -> [i32; 2] {
    [(rect[2] + rect[0]) / 2, (rect[3] + rect[1]) / 2]
}
pub fn rect_adjusted(r: [i32; 4], a: [i32; 4]) -> [i32; 4] {
    [r[0] + a[0], r[1] + a[1], r[2] + a[2], r[3] + a[3]]
}
pub fn rect_add_margins(r: [i32; 4], m: Margins) -> [i32; 4] {
    [
        r[0] - m[0] as i32,
        r[1] - m[1] as i32,
        r[2] + m[2] as i32,
        r[3] + m[3] as i32,
    ]
}
pub fn rect_translated(r: [i32; 4], t: [i32; 2]) -> [i32; 4] {
    [r[0] + t[0], r[1] + t[1], r[2] + t[0], r[3] + t[1]]
}
pub fn rect_axis(rect: [i32; 4], axis: usize) -> i32 {
    rect[axis + 2] - rect[axis]
}
pub fn rect_contains_point(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0] && point[0] < rect[2] && point[1] >= rect[1] && point[1] < rect[3]
}
pub fn rect_intersect(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    [
        max(a[0], b[0]),
        max(a[1], b[1]),
        min(a[2], b[2]),
        min(a[3], b[3]),
    ]
}
fn uv_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let s = [a[2] - a[0], a[3] - a[1]];
    [
        a[0] + s[0] * b[0],
        a[1] + s[1] * b[1],
        a[0] + s[0] * b[2],
        a[1] + s[1] * b[3],
    ]
}
pub fn rect_round(r: [f32; 4]) -> [i32; 4] {
    [
        r[0].round() as i32,
        r[1].round() as i32,
        r[2].round() as i32,
        r[3].round() as i32,
    ]
}

fn hash_id_label(id_label: &str, salt: ItemId) -> ItemId {
    let mut h = salt;
    let id = id_from_label(&id_label);
    for ch in id.bytes() {
        h = h.wrapping_mul(101 as ItemId).wrapping_add(ch as ItemId);
    }
    h
}

struct SimpleHasher {
    pub seed: ItemId,
}

impl std::hash::Hasher for SimpleHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.seed;
        for &ch in bytes {
            h = h.wrapping_mul(101 as ItemId).wrapping_add(ch as ItemId);
        }
        self.seed = h;
    }
    fn finish(&self) -> u64 {
        self.seed as u64
    }
}

fn id_from_label(id_label: &str) -> &str {
    let sep = id_label.bytes().position(|s| s == '#' as u8);
    match sep {
        None => &id_label,
        Some(index) => &id_label[..index],
    }
}

fn label_from_id(id_label: &str) -> &str {
    let sep = id_label.bytes().position(|s| s == '#' as u8);
    match sep {
        None => &id_label,
        Some(index) => &id_label[index + 1..],
    }
}

fn margins_add(a: Margins, b: Margins) -> Margins {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

pub type AddDrawItemFlags = u32;
const ADD_DRAW_ITEM_DISABLED: AddDrawItemFlags = 1 << 0;
const ADD_DRAW_ITEM_PRESSED: AddDrawItemFlags = 1 << 1;

fn add_suffix(filename: &str, suffix: &str) -> String {
    let pos = filename.bytes().position(|c| c == ('.' as u8));
    let mut result;
    match pos {
        Some(pos) => {
            result = filename[..pos].to_owned();
            result += suffix;
            result += &filename[pos..]
        }
        None => {
            result = filename.to_owned();
            result += suffix;
        }
    }
    result
}

#[derive(Copy, Clone)]
pub enum FrameLook {
    Hollow,
    /// An image is segmented into 9 areas. Corners retain fixed size.
    /// Center area is being stretch in both horizontal and vertical direction.
    /// Remaining regions are stretched either in vertical or horizontal direction.
    ///
    /// ```
    ///       |           |
    /// fixed |  <----->  | fixed
    /// -------------------------
    ///   ^   |     ^     |   ^
    ///   |   |     |     |   |
    ///   |   |  <--+-->  |   |
    ///   |   |     |     |   |
    ///   v   |     v     |   v
    /// -------------------------
    /// fixed |  <----->  | fixed
    ///       |           |
    /// ```
    SegmentedImage {
        image: SpriteKey,
        cut: Margins,
    },
    /// A rectangle with rounded corners
    RoundRectangle {
        corner_radius: f32,
        thickness: f32,
        outline_color: [u8; 4],
        cut: Margins,
    },
}

impl Default for FrameLook {
    fn default() -> FrameLook {
        FrameLook::RoundRectangle {
            corner_radius: 2.0,
            thickness: 1.0,
            outline_color: [128, 128, 128, 255],
            cut: [2, 2, 2, 2],
        }
    }
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            look: Default::default(),
            frame_thickness: Default::default(),
            margins: Default::default(),
            inset: Default::default(),
            clip: Default::default(),
            offset: Default::default(),
            color: [255, 255, 255, 255],
            content_offset: [0, 0],
        }
    }
}

impl<'l> Default for Button<'l> {
    fn default() -> Self {
        Self {
            label_id: "",
            sprite_id: None,
            style: None,
            min_size: [0, 0],
            expand: false,
            scale: [1.0, 1.0],
            margins: MARGINS_DEFAULT,
            offset: [0, 0],
            color: None,
            content_color: None,
            font: None,
            enabled: true,
            down: false,
            item: false,
            can_be_pushed: true,
            for_area: false,
            align: None,
        }
    }
}

impl<'l> Default for Label<'l> {
    fn default() -> Self {
        Self {
            label_id: "",
            min_size: [0, 0],
            expand: false,
            scale: 1.0,
            font: None,
            height_mode: LabelHeight::NoLineGap,
            offset: [0, 0],
            align: Align::Left,
            color: None,
        }
    }
}

impl<'i, 's> Default for Edit<'i, 's> {
    fn default() -> Self {
        Self {
            id: "",
            text: None,
            min_size: [120, 0],
            expand: false,
            scale: 1.0,
            font: None,
            height_mode: LabelHeight::NoLineGap,
            offset: [0, 0],
            align: Align::Left,
            color: None,
            multiline: false,
        }
    }
}

impl Default for UIImage {
    fn default() -> Self {
        Self {
            sprite_id: None,
            min_size: [0, 0],
            expand: false,
            scale: [1.0, 1.0],
            offset: [0, 0],
            color: [255, 255, 255, 255],
        }
    }
}

impl<'l> Default for WrappedText<'l> {
    fn default() -> Self {
        Self {
            id: "",
            text: "",
            min_size: [0, 0],
            max_width: 0,
            expand: false,
            font: None,
            offset: [0, 0],
            align: Align::Center,
            color: None,
            scale: 1.0,
        }
    }
}
impl Default for BoxLayout {
    fn default() -> Self {
        Self {
            orientation: Vertical,
            expand: Default::default(),
            min_size: Default::default(),
            margins: Default::default(),
            scale: [1.0, 1.0],
            offset: Default::default(),
            padding: 0,
        }
    }
}

impl Default for DrawCommand {
    fn default() -> Self {
        DrawCommand::None
    }
}

pub trait UIElement {
    type AddResult;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult;
}

impl<'l> UIElement for Label<'l> {
    type AddResult = ();
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let label = label_from_id(&self.label_id);
        let font = self
            .font
            .unwrap_or_else(|| ui.styles.get(ui.style).expect("default style").font);
        let fonts = ui.font_context.as_ref().unwrap();
        let size = [
            fonts.measure_text(font, &label, self.scale)[0],
            match self.height_mode {
                LabelHeight::LineSpace => fonts.font_height(font),
                LabelHeight::NoLineGap => fonts.font_ascent(font) - fonts.font_descent(font),
                LabelHeight::Ascent => fonts.font_ascent(font),
                LabelHeight::Custom(height) => height as f32,
            },
        ];
        let min_size = [
            max(size[0].round() as u16, self.min_size[0]),
            max(size[1].round() as u16, self.min_size[1]),
        ];

        let (r, window) = ui.add_area(parent);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let clip = window.areas[r.area_index as usize].clip_item_index;

        let element_index = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::FixedSize,
                parent: parent_area_element,
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );
        window.areas[parent.area_index as usize].last_element = element_index;

        let i = parent.window_index as usize;
        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index,
                clip,
                color: self
                    .color
                    .unwrap_or(ui.styles.get(ui.style).unwrap().text_color),
                dragged: false,
                offset: self.offset,
                command: DrawCommand::Text {
                    text: UI::add_draw_text(&mut ui.window_draw_texts[i], &label),
                    alignment: self.align,
                    font,
                    height_mode: self.height_mode,
                    scale: self.scale,
                    selection: None,
                    caret: None,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
    }
}

impl<'i, 's> UIElement for Edit<'i, 's> {
    type AddResult = bool;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let mut empty_text = String::new();
        let text = self.text.unwrap_or(&mut empty_text);
        let font = self
            .font
            .unwrap_or_else(|| ui.styles.get(ui.style).expect("default style").font);
        let fonts = ui.font_context.as_ref().unwrap();
        let height = match self.height_mode {
            LabelHeight::LineSpace => fonts.font_height(font),
            LabelHeight::NoLineGap => fonts.font_ascent(font) - fonts.font_descent(font),
            LabelHeight::Ascent => fonts.font_ascent(font),
            LabelHeight::Custom(height) => height as f32,
        };
        let min_size = [
            self.min_size[0],
            max(height.round() as u16, self.min_size[1]),
        ];

        let (r, window) = ui.add_area(parent);
        let item_id = hash_id_label(self.id, window.top_hash);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let parent_clip_item_index = window.areas[r.area_index as usize].clip_item_index;
        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                typ: ElementType::FixedSize,
                parent: parent_area_element,
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );
        window.areas[parent.area_index as usize].last_element = element_index;

        let clip_item_index = window.clip_items.len();
        window.clip_items.push(ClipItem {
            parent: parent_clip_item_index,
            element_index,
            margins: [0, 0, 0, 0],
        });

        // handle input
        let mut selection = None;
        let mut caret = None;
        let mut changed = false;
        let window = &mut ui.windows[parent.window_index as usize];
        let window_id = window.id;
        let mut draw_item_scroll = [0, 0];
        if Some(item_id) == ui.input_focus.map(|i| i.0) {
            let EditState {
                mut state,
                mut scroll,
                ..
            } = ui
                .edit_state
                .take()
                .filter(|edit| edit.id == item_id)
                .unwrap_or_else(|| {
                    let mut state = text_editor::EditboxState::default();
                    state.click_state = text_editor::ClickState::None;
                    state.selection = Some((0, text.len() as u32));
                    EditState {
                        id: item_id,
                        window: window_id,
                        state,
                        scroll: [0, 0],
                    }
                });
            // clamp selection to text length
            if let Some((start, end)) = state.selection.as_mut() {
                *start = (*start).min(text.len() as u32);
                while !text.is_char_boundary(*start as usize) {
                    *start -= 1;
                }
                *end = (*end).min(text.len() as u32);
                while !text.is_char_boundary(*end as usize) {
                    *end -= 1;
                }
            }
            state.cursor = state.cursor.min(text.len() as u32);
            while !text.is_char_boundary(state.cursor as usize) {
                state.cursor -= 1;
            }

            let old_text = text.clone();
            for event in &ui.frame_input {
                match event {
                    &UIEvent::MouseDown {
                        button, pos, time, ..
                    } => {
                        if button == 1 {
                            // test against layout of previous frame
                            if ui.hovered_item == item_id {
                                if let Some(r) = window.hit_rectangle(
                                    element_index,
                                    clip_item_index,
                                    None, /*Some(item_id)*/
                                ) {
                                    let local_x = pos[0] as f32 - r[0] as f32 - scroll[0] as f32;
                                    let fonts = ui.font_context.as_ref().unwrap();
                                    if let Some(offset) =
                                        fonts.hit_character(font, text, self.scale, local_x)
                                    {
                                        state.click_down(time as f32, text, offset);
                                    }
                                }
                            }
                        }
                    }
                    &UIEvent::MouseMove { pos, .. } => {
                        if ui.hit_item == item_id {
                            if let Some(r) = window.hit_rectangle(
                                element_index,
                                clip_item_index,
                                None, /*Some(item_id)*/
                            ) {
                                let local_x = pos[0] as f32 - r[0] as f32 - scroll[0] as f32;
                                let fonts = ui.font_context.as_ref().unwrap();
                                if let Some(offset) =
                                    fonts.hit_character(font, text, self.scale, local_x)
                                {
                                    state.click_move(text, offset);
                                }
                            }
                        }
                    }
                    &UIEvent::MouseUp { button, .. } => {
                        if button == 1 {
                            if ui.hit_item == item_id {
                                state.click_up(text);
                            }
                        }
                    }
                    &UIEvent::KeyDown {
                        key,
                        control,
                        shift,
                        alt: _,
                    } => {
                        match (key, control, shift) {
                            (KeyCode::Z, true, false) => {
                                state.undo(text);
                            }
                            (KeyCode::Y, true, false) => {
                                state.redo(text);
                            }
                            (KeyCode::X, true, false) => {
                                state.delete_selected(text);
                            }
                            (KeyCode::V, true, false) => {
                                /*
                                if let Some(clipboard) = clipboard.get() {
                                if clipboard.len() != 0 {
                                if state.selection.is_some() {
                                state.delete_selected(text);
                                }

                                state.insert_string(text, clipboard);
                                }
                                }
                                */
                            }
                            (KeyCode::A, true, false) => {
                                state.select_all(text);
                            }
                            (KeyCode::Enter, _, _) => {
                                if self.multiline {
                                    state.insert_character(text, '\n');
                                }
                            }
                            (KeyCode::Backspace, false, _) => {
                                if state.selection.is_none() {
                                    state.delete_current_character(text);
                                } else {
                                    state.delete_selected(text);
                                }
                            }
                            (KeyCode::Delete, false, _) => {
                                if state.selection.is_none() {
                                    state.delete_next_character(text);
                                } else {
                                    state.delete_selected(text);
                                }
                            }
                            (KeyCode::Right, control, shift) => {
                                if control {
                                    state.move_cursor_next_word(text, shift);
                                } else {
                                    state.move_cursor(text, 1, shift);
                                }
                            }
                            (KeyCode::Left, control, shift) => {
                                if control {
                                    state.move_cursor_prev_word(text, shift);
                                } else {
                                    state.move_cursor(text, -1, shift);
                                }
                            }
                            (KeyCode::Home, _, shift) => {
                                let to_line_begin = state.find_line_begin(&text) as i32;
                                state.move_cursor(text, -to_line_begin, shift);
                            }
                            (KeyCode::End, _, shift) => {
                                let to_line_end = state.find_line_end(&text) as i32;
                                state.move_cursor(text, to_line_end, shift);
                            }
                            (KeyCode::Up, _, shift) => {
                                let to_line_begin = state.find_line_begin(&text) as i32;
                                state.move_cursor(text, -to_line_begin, shift);
                                if state.cursor != 0 {
                                    state.move_cursor(text, -1, shift);
                                    let new_to_line_begin = state.find_line_begin(&text) as i32;
                                    let offset =
                                        to_line_begin.min(new_to_line_begin) - new_to_line_begin;
                                    state.move_cursor(text, offset, shift);
                                }
                            }
                            (KeyCode::Down, _, shift) => {
                                let to_line_begin = state.find_line_begin(&text) as i32;
                                let to_line_end = state.find_line_end(&text) as i32;

                                state.move_cursor(text, to_line_end, shift);
                                if text.len() != 0 && state.cursor < text.len() as u32 - 1 {
                                    state.move_cursor(text, 1, shift);
                                    state.move_cursor_within_line(text, to_line_begin, shift);
                                }
                            }
                            _ => {}
                        }
                    }
                    UIEvent::TextInput { text: new_chars } => {
                        if state.selection.is_some() {
                            state.delete_selected(text);
                        }
                        for c in new_chars.chars() {
                            if c != 13 as char && c != 10 as char {
                                state.insert_character(text, c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            changed = *text != old_text;
            selection = state.selection;
            caret = Some(state.cursor);

            // update scroll
            let window = &mut ui.windows[parent.window_index as usize];
            /*if window.layout.item_ids.get(element_index as usize).copied() == Some(item_id)*/
            {
                if let Some(r) = window.layout.rectangles.get(element_index as usize) {
                    let caret_offset_x = ui.font_context.as_ref().unwrap().measure_text(
                        font,
                        &text[0..state.cursor as usize],
                        self.scale,
                    )[0] as i32;
                    let text_width = ui
                        .font_context
                        .as_ref()
                        .unwrap()
                        .measure_text(font, &text, self.scale)[0]
                        as i32;
                    let w = r[2] - r[0];
                    scroll[0] = scroll[0].max(w - text_width).min(0);
                    if caret_offset_x + scroll[0] > w {
                        scroll[0] = -(caret_offset_x - w);
                    } else if caret_offset_x + scroll[0] < 0 {
                        scroll[0] = -caret_offset_x;
                    }
                    //if caret_offset_x +
                };
            }

            draw_item_scroll = scroll;
            ui.edit_state = Some(EditState {
                id: item_id,
                window: window_id,
                state,
                scroll,
            });
        }

        let window = &mut ui.windows[parent.window_index as usize];
        window.areas[parent.area_index as usize].last_element = element_index;
        let frame_type = FrameType::ButtonNormal;
        //let style = ui.styles.get(style_key).expect("missing ui style");
        let hit = HitItem {
            item_id,
            element_index,
            clip_item_index,
            style: ui.style,
            frame_type: Some(frame_type),
            is_scroll: false,
            consumes_keys: true,
            consumes_chars: true,
        };
        window.hit_items.push(hit);

        let i = parent.window_index as usize;
        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index,
                clip: clip_item_index,
                color: self
                    .color
                    .unwrap_or(ui.styles.get(ui.style).unwrap().text_color),
                dragged: false,
                offset: [
                    self.offset[0] + draw_item_scroll[0],
                    self.offset[1] + draw_item_scroll[1],
                ],
                command: DrawCommand::Text {
                    text: UI::add_draw_text(&mut ui.window_draw_texts[i], &text),
                    alignment: self.align,
                    font,
                    height_mode: self.height_mode,
                    scale: self.scale,
                    selection,
                    caret,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
        changed
    }
}

impl UIElement for CustomRect {
    type AddResult = ();
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let min_size = self.min_size;

        let (r, window) = ui.add_area(parent);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let clip = window.areas[r.area_index as usize].clip_item_index;

        let element_index = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::FixedSize,
                parent: parent_area_element,
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );

        let i = parent.window_index as usize;
        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index,
                clip,
                dragged: false,
                offset: [0, 0],
                color: [255, 255, 255, 255],
                command: DrawCommand::CustomRect {
                    user_data: self.user_data,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
    }
}

impl<'l> UIElement for Button<'l> {
    type AddResult = ButtonState;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let id_label = self.label_id;
        let sprite_id = self.sprite_id;
        let element_type = if self.for_area {
            ElementType::Stack {
                children_list: INVALID_CHILDREN_LIST,
            }
        } else {
            ElementType::FixedSize
        };
        let (r, _) = ui.add_area(parent);
        let window = &mut ui.windows[parent.window_index as usize];
        let item_id = hash_id_label(id_label, window.top_hash);
        let down = self.down || (ui.hovered_item == item_id && ui.hit_item == item_id);
        let frame_type = if self.enabled {
            if down {
                FrameType::ButtonPressed
            } else if ui.hovered_item == item_id && self.can_be_pushed {
                FrameType::ButtonHovered
            } else {
                FrameType::ButtonNormal
            }
        } else {
            FrameType::ButtonDisabled
        };
        let style_key = self.style.unwrap_or_else(|| {
            if self.item {
                ui.flat_button_style
            } else {
                ui.style
            }
        });
        let margins = UI::calculate_margins(&ui.styles, self.margins, style_key, frame_type);
        let style = ui.styles.get(style_key).expect("missing ui style");
        let frame = style.get_frame(frame_type);
        let content_offset = [
            frame.content_offset[0] + self.offset[0],
            frame.content_offset[1] + self.offset[1],
        ];
        let label = label_from_id(&id_label);

        let font = self
            .font
            .unwrap_or_else(|| ui.styles.get(style_key).expect("default style").font);
        let min_size: [Position; 2] = if let Some(sprite_id) = sprite_id {
            let size = ui.sprite_context.as_ref().unwrap().sprite_size(sprite_id);
            [
                (size[0] as f32 * self.scale[0]) as Position,
                (size[1] as f32 * self.scale[1]) as Position,
            ]
        } else if element_type == ElementType::FixedSize {
            let size = ui
                .font_context
                .as_ref()
                .unwrap()
                .measure_text(font, &label, self.scale[0]);
            [size[0].round() as Position, size[1].round() as Position]
        } else {
            [0, 0]
        };
        let min_size = [
            max(min_size[0] as u16, self.min_size[0] as u16),
            max(min_size[1] as u16, self.min_size[1] as u16),
        ];
        let window = &mut ui.windows[parent.window_index as usize];
        let parent_area = &window.areas[parent.area_index as usize];
        let parent_area_element = parent_area.element_index;
        let clip = parent_area.clip_item_index;

        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                parent: parent_area_element,
                expanding: self.expand,
                min_size,
                margins,
                typ: element_type,
                focus_flags: FocusFlags::NotFocusable,
                ..LayoutElement::new()
            },
        );
        if element_type != ElementType::FixedSize {
            window.areas[r.area_index as usize].element_index = element_index;
        }
        let old_can_be_pushed = {
            let area = &mut window.areas[r.area_index as usize];
            let old_can_be_pushed = area.can_be_pushed;
            area.can_be_pushed = self.can_be_pushed;
            old_can_be_pushed
        };

        let clicked = if self.enabled && ui.released_item == item_id {
            ui.released_item = INVALID_ITEM_ID;
            true
        } else {
            false
        };
        let hovered = self.enabled && ui.hovered_item == item_id;

        let frame = style.get_frame(frame_type);
        let color = self.color.unwrap_or(frame.color);

        let frame = DrawItem {
            element_index,
            clip,
            color,
            dragged: false,
            offset: self.offset,
            command: DrawCommand::Frame {
                style: style_key,
                frame_type,
            },
        };

        let area = &window.areas[r.area_index as usize];
        let old_button_id = area.button_id;
        let old_button_offset = area.button_offset;
        let i = r.window_index as usize;
        let area_can_be_pushed = area.can_be_pushed;
        if area_can_be_pushed {
            window.areas[r.area_index as usize].button_id = item_id;
            let mut flags = 0;
            if !self.enabled {
                flags |= ADD_DRAW_ITEM_DISABLED;
            }
            if self.down {
                flags |= ADD_DRAW_ITEM_PRESSED;
            }
            // add_draw_item may change frame_type here
            UI::add_draw_item(
                &mut ui.window_draw_items[i],
                &ui.windows[i],
                r.area_index,
                frame,
                flags,
                INVALID_ELEMENT_INDEX,
                ui.hit_item,
                ui.hovered_item,
            );
            let window = &mut ui.windows[i];
            let style = ui.styles.get(style_key).expect("default style");
            let frame = style.get_frame(frame_type);
            let button_offset = frame.content_offset;
            window.areas[r.area_index as usize].button_offset = button_offset;
        }

        let text_color = self.content_color.unwrap_or_else(|| {
            if !self.enabled {
                style.button_disabled.text_color
            } else if down {
                style.button_pressed.text_color
            } else if hovered {
                style.button_hovered.text_color
            } else {
                style.button_normal.text_color
            }
        });

        if let Some(sprite_id) = sprite_id {
            let image_item = DrawItem {
                dragged: false,
                element_index,
                clip,
                offset: content_offset,
                color: self.content_color.unwrap_or([255, 255, 255, 255]),
                command: DrawCommand::Image {
                    sprite: sprite_id,
                    scale: self.scale,
                },
            };
            UI::add_draw_item(
                &mut ui.window_draw_items[i],
                &ui.windows[i],
                r.area_index,
                image_item,
                0,
                INVALID_ELEMENT_INDEX,
                ui.hit_item,
                ui.hovered_item,
            );
        } else if element_type == ElementType::FixedSize && self.can_be_pushed {
            let text_item = DrawItem {
                element_index,
                offset: content_offset,
                color: text_color,
                clip,
                dragged: false,
                command: DrawCommand::Text {
                    text: UI::add_draw_text(&mut ui.window_draw_texts[i], &label),
                    font,
                    alignment: self.align.unwrap_or(if self.item {
                        Align::Left
                    } else {
                        Align::Center
                    }),
                    height_mode: LabelHeight::NoLineGap,
                    scale: self.scale[0],
                    selection: None,
                    caret: None,
                },
            };
            UI::add_draw_item(
                &mut ui.window_draw_items[i],
                &ui.windows[i],
                r.area_index,
                text_item,
                0,
                INVALID_ELEMENT_INDEX,
                ui.hit_item,
                ui.hovered_item,
            );
        }

        if self.enabled {
            let window = &mut ui.windows[r.window_index as usize];
            let hit = HitItem {
                item_id,
                element_index,
                clip_item_index: window.areas[r.area_index as usize].clip_item_index,
                style: style_key,
                frame_type: Some(frame_type),
                is_scroll: false,
                consumes_keys: false,
                consumes_chars: false,
            };
            window.hit_items.push(hit);
        }
        let mut window = &mut ui.windows[i];
        UI::add_common_items(&mut window, r.area_index, element_index);
        let area = &mut window.areas[r.area_index as usize];
        let is_button_area = element_type != ElementType::FixedSize;
        if !is_button_area {
            area.button_id = old_button_id;
            area.button_offset = old_button_offset;
            area.can_be_pushed = old_can_be_pushed;
        }
        window.areas[parent.area_index as usize].last_element = element_index;
        ButtonState {
            area: r,
            hovered,
            down,
            clicked,
            text_color,
        }
    }
}

impl UIElement for Separator {
    type AddResult = ();
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let window = &ui.windows[parent.window_index as usize];
        let parent_area = &window.areas[parent.area_index as usize];
        let clip = parent_area.clip_item_index;
        let parent_element_index = parent_area.element_index;
        let style = &ui.styles[ui.style];
        let (is_horizontal, style_margins) =
            match window.layout.elements[parent_area.element_index as usize].typ {
                ElementType::Horizontal { .. } => (true, style.hseparator.margins),
                _ => (false, style.vseparator.margins),
            };

        let margins = margins_add(self.margins, style_margins);

        let frame_type = if is_horizontal {
            FrameType::HSeparator
        } else {
            FrameType::VSeparator
        };

        let element_index = ui.windows[parent.window_index as usize].layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::FixedSize,
                expanding: self.expand,
                parent: parent_element_index,
                min_size: [self.width, self.width],
                focus_flags: FocusFlags::NotFocusable,
                margins,
                ..LayoutElement::new()
            },
        );
        let i = parent.window_index as usize;
        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            parent.area_index,
            DrawItem {
                clip,
                element_index,
                dragged: false,
                color: self.color,
                offset: self.offset,
                command: DrawCommand::Separator {
                    style: ui.style,
                    frame_type,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
    }
}

impl BoxLayout {
    pub fn new(orientation: BoxOrientation) -> Self {
        Self {
            orientation,
            ..Self::default()
        }
    }
}

impl UIElement for BoxLayout {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let (r, window) = ui.add_area(parent);
        let min_size = [
            (self.min_size[0] as f32 * self.scale[0]).round() as u16,
            (self.min_size[1] as f32 * self.scale[1]).round() as u16,
        ];
        let parent_element_index = window.areas[parent.area_index as usize].element_index;
        let element_index = window.layout.add_element(
            0,
            LayoutElement {
                parent: parent_element_index,
                typ: match self.orientation {
                    Horizontal => ElementType::Horizontal {
                        children_list: INVALID_CHILDREN_LIST,
                        padding: self.padding,
                    },
                    Vertical => ElementType::Vertical {
                        children_list: INVALID_CHILDREN_LIST,
                        padding: self.padding,
                    },
                },
                expanding: self.expand,
                min_size,
                margins: self.margins,
                ..LayoutElement::new()
            },
        );
        let parent_area = &mut window.areas[parent.area_index as usize];
        parent_area.last_element = element_index;
        window.areas[r.area_index as usize].element_index = element_index;
        r
    }
}

impl<'l> UIElement for Center<'l> {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let (r, window) = ui.add_area(parent);
        let item_id = hash_id_label(self.id, window.top_hash);
        let mut min_size = self.min_size;
        min_size[0] = (min_size[0] as f32 * self.scale[0]).round() as u16;
        min_size[1] = (min_size[1] as f32 * self.scale[1]).round() as u16;
        let parent_element_index = window.areas[parent.area_index as usize].element_index;
        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                parent: parent_element_index,
                typ: ElementType::Align {
                    align: self.align,
                    children_list: INVALID_CHILDREN_LIST,
                },
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                scroll: [-self.position[0] as i16, -self.position[1] as i16],
                ..LayoutElement::new()
            },
        );
        window.areas[r.area_index as usize].element_index = element_index;
        r
    }
}

impl UIElement for Stack {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let (r, window) = ui.add_area(parent);
        let item_id = 0;
        let mut min_size = self.min_size;
        min_size[0] = min_size[0] as u16;
        min_size[1] = min_size[1] as u16;
        let parent_element_index = window.areas[parent.area_index as usize].element_index;
        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                parent: parent_element_index,
                typ: ElementType::Stack {
                    children_list: INVALID_CHILDREN_LIST,
                },
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );
        window.areas[r.area_index as usize].element_index = element_index;
        window.areas[parent.area_index as usize].last_element = element_index;
        r
    }
}

impl UIElement for Frame {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let frame_type = FrameType::Window;
        let margins = UI::calculate_margins(&ui.styles, self.margins, ui.style, frame_type);
        let (r, window) = ui.add_area(parent);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let element_index = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::Frame {
                    children_list: INVALID_CHILDREN_LIST,
                },
                parent: parent_area_element,
                expanding: self.expand,
                min_size: [0, 0],
                focus_flags: FocusFlags::NotFocusable,
                margins,
                ..LayoutElement::new()
            },
        );
        window.areas[r.area_index as usize].element_index = element_index;
        let area = &window.areas[r.area_index as usize];
        let clip = area.clip_item_index;
        let style = ui.styles.get(ui.style).expect("default style");
        let frame = style.get_frame(frame_type);
        let i = parent.window_index as usize;
        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index,
                clip,
                color: frame.color,
                dragged: false,
                offset: self.offset,
                command: DrawCommand::Frame {
                    style: ui.style,
                    frame_type,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
        r
    }
}

impl UIElement for UIImage {
    type AddResult = ();
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let size = match self.sprite_id {
            Some(id) => ui.sprite_context.as_ref().unwrap().sprite_size(id),
            _ => [0, 0],
        };
        let min_size = [
            max((size[0] as f32 * self.scale[0]) as u16, self.min_size[0]),
            max((size[1] as f32 * self.scale[1]) as u16, self.min_size[1]),
        ];

        let (r, window) = ui.add_area(parent);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let clip = window.areas[r.area_index as usize].clip_item_index;

        let element_index = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::FixedSize,
                parent: parent_area_element,
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );
        window.areas[parent.area_index as usize].last_element = element_index;

        let i = parent.window_index as usize;
        if self.sprite_id.is_some() {
            UI::add_draw_item(
                &mut ui.window_draw_items[i],
                &ui.windows[i],
                r.area_index,
                DrawItem {
                    element_index,
                    clip,
                    color: self.color,
                    dragged: false,
                    offset: self.offset,
                    command: DrawCommand::Image {
                        sprite: self.sprite_id.unwrap(),
                        scale: self.scale,
                    },
                },
                0,
                INVALID_ELEMENT_INDEX,
                ui.hit_item,
                ui.hovered_item,
            );
        }
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
    }
}

impl UIElement for Progress {
    type AddResult = ();
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let style: &UIStyle = ui.styles.get(ui.style).expect("missing ui style");
        let margins = UI::calculate_margins(
            &ui.styles,
            style.progress_outer.margins,
            ui.style,
            FrameType::ProgressOuter,
        );
        let ui_style = ui.style;
        let style_progress_outer_color = style.progress_outer.color;
        let outer_frame = style.progress_outer;
        let outer_size = [
            ((outer_frame.margins[0] + outer_frame.margins[2]).max(0) as u16).max(self.min_size[0]),
            ((outer_frame.margins[1] + outer_frame.margins[3]).max(0) as u16).max(self.min_size[1]),
        ];

        let (r, window) = ui.add_area(parent);
        let parent_area_element = window.areas[parent.area_index as usize].element_index;
        let clip = window.areas[r.area_index as usize].clip_item_index;
        let i = parent.window_index as usize;

        // outer frame
        let container = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::Vertical {
                    padding: 0,
                    children_list: INVALID_CHILDREN_LIST,
                },
                parent: parent_area_element,
                expanding: self.expand,
                min_size: outer_size,
                focus_flags: FocusFlags::NotFocusable,
                margins,
                ..LayoutElement::new()
            },
        );

        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index: container,
                clip,
                color: style_progress_outer_color,
                dragged: false,
                offset: [0, 0],
                command: DrawCommand::Frame {
                    style: ui_style,
                    frame_type: FrameType::ProgressOuter,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );

        // inner frame
        let style: &UIStyle = ui.styles.get(ui.style).expect("missing ui style");
        let inner_frame = &style.progress_inner;
        let inner_size = [
            (inner_frame.margins[0] + inner_frame.margins[2]).max(0) as u16,
            (inner_frame.margins[1] + inner_frame.margins[3]).max(0) as u16,
        ];

        let window = &mut ui.windows[i];
        let element_index = window.layout.add_element(
            INVALID_ITEM_ID,
            LayoutElement {
                typ: ElementType::FixedSize,
                parent: container,
                expanding: true,
                min_size: inner_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: inner_frame.margins,
                ..LayoutElement::new()
            },
        );
        window.areas[parent.area_index as usize].last_element = element_index;

        let clamped_progress = self.progress.max(0.0).min(1.0);
        if clamped_progress != 0.0 {
            UI::add_draw_item(
                &mut ui.window_draw_items[i],
                &ui.windows[i],
                r.area_index,
                DrawItem {
                    element_index,
                    clip,
                    color: self.color.unwrap_or(style.progress_inner.color),
                    dragged: false,
                    offset: [0, 0],
                    command: DrawCommand::Progress {
                        style: ui.style,
                        progress: clamped_progress,
                        align: self.align,
                    },
                },
                0,
                INVALID_ELEMENT_INDEX,
                ui.hit_item,
                ui.hovered_item,
            );
        }
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
    }
}

impl<'l> UIElement for WrappedText<'l> {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let font = self.font.unwrap_or_else(|| ui.styles[ui.style].font);
        let color = self.color.unwrap_or_else(|| ui.styles[ui.style].text_color);

        let (r, window) = ui.add_area(parent);
        let min_size = [
            ((self.min_size[0] as f32) * self.scale) as u16,
            ((self.min_size[1] as f32) * self.scale) as u16,
        ];

        let parent_area = &window.areas[parent.area_index as usize];
        let parent_area_element = parent_area.element_index;
        let clip = parent_area.clip_item_index;
        let item_id = hash_id_label(self.id, window.top_hash);
        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                parent: parent_area_element,
                typ: ElementType::HeightByWidth,
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins: [0, 0, 0, 0],
                ..LayoutElement::new()
            },
        );
        window.areas[parent.area_index as usize].last_element = element_index;
        let wrapped_text_index = window.wrapped_text_elements.len() as u32;
        window.wrapped_text_elements.push(element_index);
        let i = r.window_index as usize;
        let text = UI::add_draw_text(&mut ui.window_draw_texts[i], self.text);
        let window = &mut ui.windows[r.window_index as usize];
        window.wrapped_texts.push(WrappedTextItem {
            text,
            font,
            alignment: self.align,
            lines: Vec::new(),
            max_width: self.max_width,
        });

        UI::add_draw_item(
            &mut ui.window_draw_items[i],
            &ui.windows[i],
            r.area_index,
            DrawItem {
                element_index,
                clip,
                color,
                offset: self.offset,
                dragged: false,
                command: DrawCommand::WrappedText {
                    index: wrapped_text_index,
                },
            },
            0,
            INVALID_ELEMENT_INDEX,
            ui.hit_item,
            ui.hovered_item,
        );
        UI::add_common_items(&mut ui.windows[i], r.area_index, element_index);
        r
    }
}

impl<'i> UIElement for ScrollArea<'i> {
    type AddResult = AreaRef;
    fn add_to_ui(self, ui: &mut UI, parent: AreaRef) -> Self::AddResult {
        let ui_style = ui.style;
        let frame_type = FrameType::Window;
        let margins = UI::calculate_margins(&ui.styles, self.margins, ui_style, frame_type);
        let (r, window) = ui.add_area(parent);
        let parent_area = &window.areas[parent.area_index as usize];
        let parent_area_element = parent_area.element_index;
        let parent_clip_item_index = parent_area.clip_item_index;
        let width = (self.min_size[0] as f32 * self.scale + 0.5) as u16;
        let height = (self.min_size[1] as f32 * self.scale + 0.5) as u16;
        let min_size = [width, height];
        let item_id = hash_id_label(self.id, window.top_hash);
        let scroll = Window::scroll_by_id(&mut window.scrolls, item_id).0.offset;
        let element_index = window.layout.add_element(
            item_id,
            LayoutElement {
                parent: parent_area_element,
                typ: ElementType::Scroll {
                    align: self.align,
                    max_size: self.max_size,
                    children_list: INVALID_CHILDREN_LIST,
                },
                expanding: self.expand,
                min_size,
                focus_flags: FocusFlags::NotFocusable,
                margins,
                scroll: [scroll[0] as i16, scroll[1] as i16],
                ..LayoutElement::new()
            },
        );
        window.scroll_elements.push((item_id, element_index));
        let clip_item_index = window.clip_items.len();
        window.clip_items.push(ClipItem {
            parent: parent_clip_item_index,
            element_index,
            margins: [0, 0, 0, 0],
        });

        let area_index = r.area_index;
        let area = &mut window.areas[area_index as usize];
        let parent_clip_item_index = area.clip_item_index;
        area.scroll_area_id = item_id;
        area.element_index = element_index;
        area.scroll_area_element = element_index;
        area.clip_item_index = clip_item_index;

        if self.enabled {
            window.hit_items.push(HitItem {
                item_id,
                element_index,
                clip_item_index: parent_clip_item_index,
                style: ui_style,
                frame_type: Some(frame_type),
                is_scroll: true,
                consumes_keys: false,
                consumes_chars: false,
            });
        }
        r
    }
}

impl UI {
    pub fn new() -> Self {
        let mut styles = slotmap::SlotMap::with_capacity_and_key(1);
        let style = styles.insert(UIStyle::default());
        Self {
            render_rect: [0, 0, 100, 100],
            last_mouse_position: [-1, -1],
            font_context: None,
            sprite_context: None,
            styles,
            style,
            flat_button_style: style,
            hseparator_sprite: None,
            vseparator_sprite: None,
            window_order: Vec::new(),
            window_ids: Vec::new(),
            windows: Vec::new(),
            window_draw_items: Vec::new(),
            window_draw_texts: Vec::new(),
            new_named_areas: Vec::new(),
            named_areas: Vec::new(),
            custom_rects: Vec::new(),
            frame: 0,
            hit_item: 0,
            hovered_item: 0,
            released_item: 0,
            shown_popup: None,
            input_focus: None,
            edit_state: None,
            frame_input: Vec::new(),
            debug_frame: 0,
        }
    }

    pub fn set_context(
        &mut self,
        font_context: Option<Arc<dyn FontContext>>,
        sprite_context: Option<Arc<dyn SpriteContext>>,
    ) {
        self.sprite_context = sprite_context;
        self.font_context = font_context;
    }

    pub fn add<T: UIElement>(&mut self, parent: AreaRef, element: T) -> T::AddResult {
        element.add_to_ui(self, parent)
    }

    pub fn push_id<I: std::hash::Hash>(&mut self, a: AreaRef, id: I) {
        let window = &mut self.windows[a.window_index as usize];
        window.hash_stack.push(window.top_hash);
        let mut hasher = SimpleHasher {
            seed: window.top_hash,
        };
        id.hash(&mut hasher);
        window.top_hash = hasher.seed;
    }

    pub fn pop_id(&mut self, a: AreaRef) {
        let window = &mut self.windows[a.window_index as usize];
        window.top_hash = window.hash_stack.pop().unwrap();
    }

    pub fn hash_id_label(&self, a: AreaRef, id_label: &str) -> ItemId {
        let window = &self.windows[a.window_index as usize];
        hash_id_label(id_label, window.top_hash)
    }

    fn add_area(&mut self, parent: AreaRef) -> (AreaRef, &mut Window) {
        let window = &mut self.windows[parent.window_index as usize];
        let clip_item_index = window.areas[parent.area_index as usize].clip_item_index;
        let scroll_to_time = window.areas[parent.area_index as usize].scroll_to_time;
        let scroll_area_id = window.areas[parent.area_index as usize].scroll_area_id;
        window.areas.push(Area {
            scroll_to_time,
            scroll_area_id,
            clip_item_index,
            ..Area::new()
        });
        (
            AreaRef {
                window_index: parent.window_index,
                area_index: window.areas.len() as u32 - 1,
            },
            &mut self.windows[parent.window_index as usize],
        )
    }

    fn calculate_margins(
        styles: &slotmap::SlotMap<StyleKey, UIStyle>,
        own_margins: Margins,
        style: StyleKey,
        frame_type: FrameType,
    ) -> Margins {
        let style = styles.get(style).expect("missing ui style");
        let frame = style.get_frame(frame_type);

        let mut m: Margins;
        if own_margins != MARGINS_DEFAULT {
            m = own_margins;
        } else {
            // margins are added with frame thickness
            m = frame.frame_thickness;
            if MARGINS_DEFAULT == own_margins {
                m = margins_add(m, frame.margins);
            } else {
                m = margins_add(m, own_margins);
            }
        }
        m
    }

    fn add_draw_text(draw_texts: &mut String, text: &str) -> (u32, u32) {
        let start = draw_texts.len();
        *draw_texts += text;
        (start as u32, text.len() as u32)
    }

    fn add_draw_item(
        draw_items: &mut Vec<DrawItem>,
        window: &Window,
        area_index: u32,
        mut item: DrawItem,
        flags: AddDrawItemFlags,
        after_element_index: ElementIndex,
        hit_item: ItemId,
        hovered_item: ItemId,
    ) {
        assert_ne!(item.element_index, INVALID_ELEMENT_INDEX);
        let area = &window.areas[area_index as usize];
        item.dragged = area.drag_id != 0 && window.drag_item == area.drag_id;
        let button_id = area.button_id;
        if button_id != INVALID_ITEM_ID {
            match &mut item.command {
                DrawCommand::Frame { frame_type, .. } => {
                    if (*frame_type == FrameType::ButtonNormal
                        || *frame_type == FrameType::ButtonHovered)
                        && area.can_be_pushed
                    {
                        if (hit_item == button_id && hovered_item == button_id)
                            || (flags & ADD_DRAW_ITEM_PRESSED) == ADD_DRAW_ITEM_PRESSED
                        {
                            *frame_type = FrameType::ButtonPressed;
                        } else if hovered_item == button_id {
                            *frame_type = FrameType::ButtonHovered;
                        }
                    }
                }
                DrawCommand::Text { .. }
                | DrawCommand::Image { .. }
                | DrawCommand::CustomRect { .. } => {
                    if (flags & ADD_DRAW_ITEM_DISABLED) != ADD_DRAW_ITEM_DISABLED {
                        item.offset[0] += area.button_offset[0];
                        item.offset[1] += area.button_offset[1];
                    } else {
                        item.color[3] /= 3;
                    }
                }
                _ => {}
            }
        }
        let insert_after_index = if after_element_index != INVALID_ELEMENT_INDEX {
            draw_items
                .iter()
                .position(|i| i.element_index == after_element_index)
        } else {
            None
        };
        match insert_after_index {
            Some(index) => draw_items.insert(index + 1, item),
            None => draw_items.push(item),
        }
    }

    fn add_common_items(window: &mut Window, area_index: u32, element_index: ElementIndex) {
        let area = &window.areas[area_index as usize];
        if area.drag_id != 0 {
            window.drag_items.push(DragItem {
                element_index,
                id: area.drag_id,
                clip_item_index: area.clip_item_index,
            });
        }
        if area.drop_id != 0 {
            window.drop_items.push(DropItem {
                element_index,
                id: area.drag_id,
                clip_item_index: area.clip_item_index,
            });
        }
        if area.scroll_to_time >= 0.0 {
            let scroll_area_id = area.scroll_area_id;
            let scroll_to_time = area.scroll_to_time;
            window.add_scroll_animation(scroll_area_id, element_index, scroll_to_time);
        }
    }

    pub fn load_default_resources(
        &mut self,
        _sprite_loader: impl Fn(&str) -> SpriteKey,
        font: FontKey,
        tooltip_font: FontKey,
    ) {
        let default_frame = FrameStyle {
            look: FrameLook::RoundRectangle {
                corner_radius: 10.0,
                thickness: 2.0,
                outline_color: [64, 64, 64, 255],
                cut: [2, 2, 2, 2],
            },
            frame_thickness: [2, 2, 2, 2],
            margins: [0, 2, 0, 0],
            inset: [-1, -1, -1, -1],
            clip: [0, 0, 0, 0],
            offset: [0, 0],
            color: [0, 0, 0, 192],
            content_offset: [0, 0],
        };
        let corner_radius = 7.0;
        let thickness = 1.41;
        let button_frame = FrameStyle {
            look: FrameLook::RoundRectangle {
                corner_radius,
                thickness,
                outline_color: [220, 220, 220, 255],
                cut: [4, 4, 4, 4],
            },
            inset: [1, 1, 1, 1],
            color: [0, 0, 0, 128],
            frame_thickness: [4, 4, 4, 4],
            content_offset: [0, -1],
            margins: [2, 1, 2, 0],
            ..default_frame
        };
        let button_style = ButtonStyle {
            frame: button_frame,
            text_color: [200, 200, 200, 255],
            content_offset: [0, 0],
        };
        let progress_frame = FrameStyle {
            look: FrameLook::RoundRectangle {
                corner_radius,
                thickness,
                outline_color: [64, 64, 64, 255],
                cut: [0, 0, 0, 0],
            },
            frame_thickness: [0, 0, 0, 0],
            margins: [3, 3, 3, 3],
            inset: [1, 1, 1, 1],
            clip: [0, 0, 0, 0],
            offset: [0, 0],
            color: [0, 0, 0, 255],
            ..default_frame
        };
        let separator = FrameStyle {
            look: FrameLook::Hollow,
            frame_thickness: [0, 0, 0, 0],
            margins: [0, 0, 0, 0],
            inset: [0, 0, 0, 0],
            clip: [0, 0, 0, 0],
            offset: [0, 0],
            color: [64, 64, 64, 255],
            content_offset: [0, 0],
        };
        let default_style = UIStyle {
            font,
            tooltip_font,
            text_color: [160, 160, 160, 255],
            window_frame: default_frame,
            button_normal: button_style,
            button_hovered: ButtonStyle {
                text_color: [255, 255, 255, 255],
                content_offset: [-1, -2],
                frame: button_frame,
                ..button_style
            },
            button_pressed: ButtonStyle {
                text_color: [200, 200, 200, 255],
                content_offset: [2, 4],
                frame: FrameStyle {
                    color: [32, 32, 32, 255],
                    ..button_frame
                },
                ..button_style
            },
            button_disabled: ButtonStyle {
                text_color: [64, 64, 64, 255],
                frame: FrameStyle {
                    look: FrameLook::RoundRectangle {
                        corner_radius,
                        thickness,
                        outline_color: [64, 64, 64, 255],
                        cut: [2, 2, 2, 2],
                    },
                    color: [0, 0, 0, 128],
                    ..button_frame
                },
                ..button_style
            },
            hseparator: FrameStyle {
                look: FrameLook::RoundRectangle {
                    corner_radius: 1.0,
                    thickness: 1.0,
                    outline_color: [32, 32, 32, 255],
                    cut: [0, 3, 0, 3],
                },
                margins: [5, 0, 5, 0],
                ..separator
            },
            vseparator: FrameStyle {
                look: FrameLook::RoundRectangle {
                    corner_radius: 1.0,
                    thickness: 1.0,
                    outline_color: [32, 32, 32, 255],
                    cut: [3, 0, 3, 0],
                },
                margins: [0, 5, 0, 5],
                ..separator
            },
            progress_inner: FrameStyle {
                margins: [0, 0, 0, 0],
                look: FrameLook::RoundRectangle {
                    corner_radius: 1.0,
                    thickness,
                    outline_color: [64, 64, 64, 255],
                    cut: [0, 0, 0, 0],
                },
                color: [32, 32, 32, 255],
                ..progress_frame
            },
            progress_outer: progress_frame,
        };
        self.style = self.styles.insert(default_style.clone());
        self.flat_button_style = self.styles.insert(UIStyle {
            button_normal: ButtonStyle {
                frame: FrameStyle {
                    look: FrameLook::Hollow,
                    ..default_style.button_normal.frame
                },
                ..default_style.button_normal
            },
            button_disabled: ButtonStyle {
                frame: FrameStyle {
                    look: FrameLook::Hollow,
                    ..default_style.button_disabled.frame
                },
                ..default_style.button_disabled
            },
            ..default_style
        });
    }

    pub fn default_style(&self) -> &UIStyle {
        self.styles.get(self.style).as_ref().unwrap()
    }

    pub fn render_debug(&mut self, batch: &mut dyn Render) {
        let windows = &self.windows;
        for window in self.window_order.iter().map(|&i| &windows[i]) {
            for (rect, e) in window
                .layout
                .rectangles
                .iter()
                .zip(window.layout.elements.iter())
            {
                let positions = [
                    [rect[0] as f32, rect[1] as f32],
                    [rect[2] as f32, rect[1] as f32],
                    [rect[2] as f32, rect[3] as f32],
                    [rect[0] as f32, rect[3] as f32],
                ];
                let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                let indices = [0, 1, 2, 0, 2, 3];
                let a = 64;
                let color = match e.typ {
                    ElementType::FixedSize { .. } => [128, 128, 128, a],
                    ElementType::Vertical { .. } => [255, 0, 0, a],
                    ElementType::Frame { .. } => [0, 0, 255, a],
                    ElementType::HeightByWidth { .. } => [255, 0, 0, a],
                    ElementType::Horizontal { .. } => [255, 255, 0, a],
                    ElementType::Scroll { .. } => [0, 255, 255, a],
                    ElementType::Align { .. } => [0, 128, 255, a],
                    ElementType::Stack { .. } => [0, 255, 0, a],
                };
                batch.add_vertices(&positions, &uvs, &indices, color);
            }
        }
    }

    pub fn layout_ui(
        &mut self,
        dt: f32,
        render_rect: [i32; 4],
        sort_key_range: Option<(i32, i32, bool)>,
    ) {
        let new_frame = sort_key_range.is_none() || sort_key_range.unwrap().2;

        self.render_rect = render_rect;
        if new_frame {
            self.frame_input.clear();
        }
        self.custom_rects.clear();

        for &i in self.window_order.iter() {
            let window = &mut self.windows[i];
            let draw_items = &mut self.window_draw_items[i];
            let draw_texts = &self.window_draw_texts[i];

            if let Some((range_begin, range_end, _)) = sort_key_range {
                if window.sort_key < range_begin || window.sort_key >= range_end {
                    continue;
                }
            }

            if window.update_frame != self.frame {
                // window was not updated this frame
                window.clear();
                draw_items.clear();
            }
            window.update_layout(
                self.render_rect,
                self.font_context.as_ref().unwrap().borrow(),
                draw_texts,
            );
            window.update_animation(dt, self.debug_frame);
            if window.update_scroll() {
                window.update_layout(
                    self.render_rect,
                    self.font_context.as_ref().unwrap().borrow(),
                    draw_texts,
                );
            }
            let mut removed_indices = Vec::new();
            let window_id = window.id;
            let named_areas = &mut self.named_areas;
            for (n, area_index) in self.new_named_areas.iter().enumerate() {
                let area = &mut named_areas[*area_index as usize];
                if area.window_id != window_id {
                    continue;
                }
                if area.element_index >= 0
                    && (area.element_index as isize) < (window.layout.rectangles.len()) as isize
                {
                    area.last_rect = window.layout.rectangles[area.element_index as usize];
                    removed_indices.push(n);
                }
            }
            for i in removed_indices.iter().rev() {
                self.new_named_areas.remove(*i as usize);
            }
            // calculate clipping rectangles
            window.clip_item_rects.clear();
            for i in 0..window.clip_items.len() {
                let clip_item = &window.clip_items[i];
                let parent_rect = window
                    .clip_item_rects
                    .get(clip_item.parent)
                    .copied()
                    .unwrap_or(render_rect);
                let r = window.layout.rectangles[clip_item.element_index as usize];
                let clip_element = &window.layout.elements[clip_item.element_index as usize];
                let r = [
                    r[0] - clip_element.margins[0] as Position + clip_item.margins[0] as Position,
                    r[1] - clip_element.margins[1] as Position + clip_item.margins[1] as Position,
                    r[2] + clip_element.margins[2] as Position + clip_item.margins[2] as Position,
                    r[3] + clip_element.margins[3] as Position + clip_item.margins[3] as Position,
                ];
                let rect = rect_intersect(parent_rect, r);
                window.clip_item_rects.push(rect);
            }
        }

        if new_frame {
            self.frame += 1;
        }
    }

    pub fn render_ui(&mut self, batch: &mut dyn Render, sort_key_range: Option<(i32, i32, bool)>) {
        for dragged in &[false, true] {
            for &i in self.window_order.iter() {
                let window = &mut self.windows[i];
                let draw_items = &mut self.window_draw_items[i];
                let draw_texts = &self.window_draw_texts[i];

                if let Some((range_begin, range_end, _)) = sort_key_range {
                    if window.sort_key < range_begin || window.sort_key >= range_end {
                        continue;
                    }
                }

                for item in draw_items
                    .iter()
                    //.inspect(|d| console_log(&format!(" d: {:?}", &d)))
                    .filter(|d| d.dragged == *dragged && d.element_index != INVALID_ELEMENT_INDEX)
                {
                    let e = &window.layout.elements[item.element_index as usize];
                    let clip_rect = window.clip_item_rects.get(item.clip).copied();

                    let mut rect = window.layout.rectangles[item.element_index as usize];

                    batch.set_clip(clip_rect);

                    match &item.command {
                        DrawCommand::Image { sprite, scale, .. } => {
                            let sprite = *sprite;
                            let center = rect_center(rect);
                            let center = [center[0] + item.offset[0], center[1] + item.offset[1]];
                            let (x, y) = if item.dragged {
                                (
                                    center[0] + window.drag_offset[0],
                                    center[1] + window.drag_offset[1],
                                )
                            } else {
                                (center[0], center[1])
                            };
                            batch.set_sprite(Some(sprite));
                            let [w, h] = self.sprite_context.as_ref().unwrap().sprite_size(sprite);
                            let w = (w as f32 * scale[0]) as i32;
                            let h = (h as f32 * scale[1]) as i32;
                            let rect = [(x - w / 2), (y - h / 2), (x + w - w / 2), (y + h - h / 2)];
                            let uv = [0.0, 0.0, 1.0, 1.0];
                            let positions = [
                                [rect[0] as f32, rect[1] as f32],
                                [rect[2] as f32, rect[1] as f32],
                                [rect[2] as f32, rect[3] as f32],
                                [rect[0] as f32, rect[3] as f32],
                            ];
                            let uvs = [
                                [uv[0], uv[1]],
                                [uv[2], uv[1]],
                                [uv[2], uv[3]],
                                [uv[0], uv[3]],
                            ];
                            let indices = [0, 1, 2, 0, 2, 3];
                            batch.add_vertices(&positions, &uvs, &indices, item.color);
                        }
                        DrawCommand::Rect { .. } => {
                            draw_rect(batch, rect, item.color);
                        }
                        DrawCommand::Frame { style, .. } | DrawCommand::Progress { style, .. } => {
                            let frame_type = match item.command {
                                DrawCommand::Frame { frame_type, .. } => frame_type,
                                DrawCommand::Progress { .. } => FrameType::ProgressInner,
                                _ => continue,
                            };
                            let mut delta = [
                                -(e.margins[0] as Position),
                                -(e.margins[1] as Position),
                                e.margins[2] as Position,
                                e.margins[3] as Position,
                            ];
                            let style = self.styles.get(*style).expect("missing style");
                            let frame = style.get_frame(frame_type);
                            match frame.look {
                                FrameLook::Hollow { .. } => {}
                                FrameLook::SegmentedImage { cut, .. }
                                | FrameLook::RoundRectangle { cut, .. } => {
                                    delta[0] += cut[0] as Position;
                                    delta[1] += cut[1] as Position;
                                    delta[2] -= cut[2] as Position;
                                    delta[3] -= cut[3] as Position;
                                }
                            }
                            rect[0] += frame.inset[0] as Position;
                            rect[1] += frame.inset[1] as Position;
                            rect[2] -= frame.inset[2] as Position;
                            rect[3] -= frame.inset[3] as Position;

                            let mut inner_rect =
                                rect_translated(rect_adjusted(rect, delta), item.offset);
                            let w = (inner_rect[2] - inner_rect[0]) as f32;
                            match item.command {
                                DrawCommand::Progress {
                                    progress, align, ..
                                } => match align {
                                    Align::Left => {
                                        inner_rect[2] = inner_rect[0] + (w * progress) as i32;
                                    }
                                    Align::Center => {
                                        let center = (inner_rect[2] + inner_rect[0]) / 2;
                                        inner_rect[0] = center - (w * progress * 0.5) as i32;
                                        inner_rect[2] = center + (w * progress * 0.5) as i32;
                                    }
                                    Align::Right => {
                                        inner_rect[0] = inner_rect[2] - (w * progress) as i32;
                                    }
                                },
                                _ => {}
                            }

                            match frame.look {
                                FrameLook::Hollow { .. } => {}
                                FrameLook::SegmentedImage { image, cut } => {
                                    draw_frame_rect(
                                        batch,
                                        image,
                                        cut,
                                        inner_rect,
                                        frame.offset,
                                        item.color,
                                        self.sprite_context.as_ref().unwrap().borrow(),
                                    );
                                }
                                FrameLook::RoundRectangle {
                                    outline_color,
                                    thickness,
                                    corner_radius,
                                    cut,
                                } => {
                                    let rect = [
                                        inner_rect[0] as f32 - 0.5 - cut[0] as f32
                                            + frame.offset[0] as f32,
                                        inner_rect[1] as f32 - 0.5 - cut[1] as f32
                                            + frame.offset[1] as f32,
                                        inner_rect[2] as f32 - 0.5
                                            + cut[2] as f32
                                            + frame.offset[0] as f32,
                                        inner_rect[3] as f32 - 0.5
                                            + cut[3] as f32
                                            + frame.offset[1] as f32,
                                    ];
                                    batch.set_sprite(None);
                                    batch.draw_rounded_rect(
                                        rect,
                                        corner_radius,
                                        thickness,
                                        outline_color,
                                        frame.color,
                                    );
                                }
                            }
                        }
                        &DrawCommand::Separator {
                            style, frame_type, ..
                        } => {
                            let inner_rect = rect_translated(rect, item.offset);
                            let style = self.styles.get(style).expect("missing ui style");
                            let frame = style.get_frame(frame_type);
                            match frame.look {
                                FrameLook::Hollow { .. } => {}
                                FrameLook::SegmentedImage { image, cut } => {
                                    draw_frame_rect(
                                        batch,
                                        image,
                                        cut,
                                        inner_rect,
                                        frame.offset,
                                        item.color,
                                        self.sprite_context.as_ref().unwrap().borrow(),
                                    );
                                }
                                FrameLook::RoundRectangle {
                                    outline_color,
                                    thickness,
                                    corner_radius,
                                    cut,
                                } => {
                                    let rect = [
                                        rect[0] as f32 - 0.5 - cut[0] as f32
                                            + frame.offset[0] as f32,
                                        rect[1] as f32 - 0.5 - cut[1] as f32
                                            + frame.offset[1] as f32,
                                        rect[2] as f32 - 0.5
                                            + cut[2] as f32
                                            + frame.offset[0] as f32,
                                        rect[3] as f32 - 0.5
                                            + cut[3] as f32
                                            + frame.offset[1] as f32,
                                    ];
                                    batch.set_sprite(None);
                                    batch.draw_rounded_rect(
                                        rect,
                                        corner_radius,
                                        thickness,
                                        outline_color,
                                        frame.color,
                                    );
                                }
                            }
                        }
                        DrawCommand::Text {
                            font,
                            text,
                            scale,
                            height_mode,
                            alignment,
                            selection,
                            caret,
                            ..
                        } => {
                            let item_font = *font;
                            let fonts = self.font_context.as_ref().unwrap();
                            let text = &draw_texts[text.0 as usize..(text.0 + text.1) as usize];
                            let text_w = fonts.measure_text(item_font, text, *scale)[0];
                            let font_ascent = fonts.font_ascent(item_font);
                            let font_descent = fonts.font_descent(item_font);
                            let text_h = match height_mode {
                                LabelHeight::LineSpace => fonts.font_height(item_font),
                                LabelHeight::NoLineGap => {
                                    font_ascent - fonts.font_descent(item_font)
                                }
                                LabelHeight::Ascent => font_ascent,
                                LabelHeight::Custom(height) => *height,
                            };
                            let align_x: f32 = match alignment {
                                Align::Left => 0.0,
                                Align::Center => 0.5,
                                Align::Right => 1.0,
                            };
                            let x = rect[0] as f32 * (1.0 - align_x)
                                + (rect[2] as f32 - text_w) * align_x
                                + item.offset[0] as f32;
                            let rect_h = (rect[3] - rect[1]) as f32;
                            let center_offset = (rect_h - font_ascent.min(text_h) * scale) * 0.5;
                            let y = rect[3] as f32 - center_offset + item.offset[1] as f32;
                            if let Some((start, end)) = selection {
                                let start = (*start as usize).min(text.len());
                                let end = (*end as usize).min(text.len());
                                let start_offset =
                                    fonts.measure_text(item_font, &text[0..start], *scale)[0];
                                let end_offset =
                                    fonts.measure_text(item_font, &text[0..end], *scale)[0];
                                let selection_color = [
                                    item.color[0],
                                    item.color[1],
                                    item.color[2],
                                    item.color[3] / 3,
                                ];
                                draw_rect(
                                    batch,
                                    [
                                        (x + start_offset - 1.0) as i32,
                                        (y - text_h) as i32,
                                        (x + end_offset - 1.0) as i32,
                                        (y - font_descent) as i32,
                                    ],
                                    selection_color,
                                );
                            }
                            batch.draw_text(item_font, &text, [x, y], item.color, *scale);
                            if let Some(caret) = caret {
                                let offset = fonts.measure_text(
                                    item_font,
                                    &text[0..(*caret as usize).min(text.len())],
                                    *scale,
                                )[0];
                                draw_rect(
                                    batch,
                                    [
                                        (x + offset) as i32 - 1,
                                        (y - text_h) as i32,
                                        (x + offset) as i32 + 1,
                                        (y - font_descent) as i32,
                                    ],
                                    item.color,
                                );
                            }
                        }
                        DrawCommand::WrappedText { index } => {
                            let wrapped = &window.wrapped_texts[*index as usize];
                            let font = wrapped.font;
                            let font_height = self.font_context.as_ref().unwrap().font_height(font);
                            let font_ascent = self.font_context.as_ref().unwrap().font_ascent(font);

                            let mut current_y = rect[1] as f32 + font_ascent.round();
                            let x = rect[0] as f32;
                            let text = &draw_texts[wrapped.text.0 as usize
                                ..(wrapped.text.0 + wrapped.text.1) as usize];
                            for line_range in &wrapped.lines {
                                let line = &text[line_range.0 as usize..line_range.1 as usize];
                                let align_x = match wrapped.alignment {
                                    Align::Right => rect[2] - rect[0] - line_range.2 as Position,
                                    Align::Center => {
                                        (rect[2] - rect[0] - line_range.2 as Position) / 2
                                    }
                                    Align::Left => 0,
                                } as f32;
                                batch.draw_text(
                                    font,
                                    line,
                                    [x + align_x, current_y],
                                    item.color,
                                    1.0,
                                );
                                current_y += font_height;
                            }
                        }
                        DrawCommand::CustomRect { user_data } => {
                            self.custom_rects.push((*user_data, rect));
                        }
                        DrawCommand::None => {}
                    }
                }
            }
        }
        batch.set_clip(None);

        let new_frame = sort_key_range.is_none() || sort_key_range.unwrap().2;
        if new_frame {
            self.debug_frame += 1;
        }
    }

    fn find_or_add_window(&mut self, id_label: &str, sort_key: i32) -> (u32, &mut Window) {
        let id = hash_id_label(id_label, 0);
        match self.window_ids.iter().position(|x| *x == id) {
            Some(index) => (index as u32, &mut self.windows[index]),
            None => {
                let windows = &self.windows;
                let pos = self
                    .window_order
                    .partition_point(|i| windows[*i].sort_key < sort_key);
                let index = self.windows.len();
                self.window_order.insert(pos, index);
                self.window_ids.push(id);
                self.window_draw_items.push(Vec::new());
                self.window_draw_texts.push(String::new());
                self.windows.push(Window {
                    id_str: id_from_label(id_label).to_owned(),
                    id,
                    top_hash: id,
                    sort_key,
                    ..Window::new()
                });
                (index as u32, &mut self.windows[index])
            }
        }
    }
    pub fn window(
        &mut self,
        id_label: &str,
        placement: WindowPlacement,
        flags: u32,
        sort_key: i32,
    ) -> AreaRef {
        let frame = self.frame;
        let (window_index, area_index) = {
            let (window_index, window) = self.find_or_add_window(id_label, sort_key);

            window.clear();
            window.update_frame = frame;
            window.placement = placement;
            window.flags = flags;

            let area_index = window.areas.len() as u32;
            window.areas.push(Area::new());
            let area = &mut window.areas[area_index as usize];
            let root = window.layout.elements.len() - 1;
            area.element_index = root as ElementIndex;

            self.window_draw_items[window_index as usize].clear();
            self.window_draw_texts[window_index as usize].clear();

            (window_index, area_index)
        };

        AreaRef {
            window_index,
            area_index,
        }
    }

    pub fn is_window_hovered(&self, a: AreaRef) -> bool {
        for i in (0..self.windows.len()).rev() {
            if rect_contains_point(self.windows[i].computed_rect, self.last_mouse_position) {
                return i == a.window_index as usize;
            }
        }
        false
    }

    pub fn hovered_window(&self) -> Option<ItemId> {
        for i in (0..self.windows.len()).rev() {
            if rect_contains_point(self.windows[i].computed_rect, self.last_mouse_position) {
                return Some(self.windows[i].id);
            }
        }
        None
    }

    pub fn window_rect(&self, id_str: &str) -> Option<Rect> {
        let window_id = hash_id_label(id_str, 0);
        if let Some(window_index) = self.window_ids.iter().position(|id| *id == window_id) {
            Some(self.windows[window_index].computed_rect)
        } else {
            None
        }
    }

    pub fn is_last_hovered(&self, a: AreaRef) -> bool {
        let window = &self.windows[a.window_index as usize];
        let area = &window.areas[a.area_index as usize];
        let last_element = area.last_element;
        if let Some(mut rect) = window.layout.rectangles.get(last_element as usize).copied() {
            if let Some(e) = window.layout.elements.get(last_element as usize) {
                rect = rect_add_margins(rect, e.margins);
                // inflate elements of boxes so the parent does not shine through the
                // padding and margins
                if !matches!(
                    e.typ,
                    ElementType::Vertical { .. } | ElementType::Horizontal { .. }
                ) {
                    if let Some(p) = window.layout.elements.get(area.element_index as usize) {
                        match p.typ {
                            ElementType::Vertical { padding, .. } => {
                                rect = rect_add_margins(
                                    rect,
                                    [
                                        p.margins[0],
                                        p.margins[1].max(padding),
                                        p.margins[2],
                                        p.margins[3].max(padding),
                                    ],
                                );
                            }
                            ElementType::Horizontal { padding, .. } => {
                                rect = rect_add_margins(
                                    rect,
                                    [
                                        p.margins[0].max(padding),
                                        p.margins[1],
                                        p.margins[2].max(padding),
                                        p.margins[3],
                                    ],
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            if !rect_contains_point(rect, self.last_mouse_position) {
                return false;
            }
            if !self.is_window_hovered(a) {
                return false;
            }
            true
        } else {
            false
        }
    }

    pub fn last_tooltip(&mut self, a: AreaRef, init: Tooltip) -> Option<AreaRef> {
        if !self.is_last_hovered(a) {
            return None;
        }
        let window = &self.windows[a.window_index as usize];
        let area = &window.areas[a.area_index as usize];
        let last_element = area.last_element as usize;
        let margins = if let Some(e) = window.layout.elements.get(last_element) {
            e.margins
        } else {
            [0, 0, 0, 0]
        };
        let rect = if let Some(rect) = window.layout.rectangles.get(last_element) {
            rect_add_margins(*rect, margins)
        } else {
            [0, 0, 0, 0]
        };
        Some(self.tooltip_at_rect(rect, init))
    }

    pub fn tooltip_at_rect(&mut self, mut rect: [i32; 4], init: Tooltip) -> AreaRef {
        match init.placement {
            TooltipPlacement::Beside => {
                rect[0] -= init.padding;
                rect[2] += init.padding;
            }
            TooltipPlacement::Below | TooltipPlacement::BelowCentered => {
                rect[1] -= init.padding;
                rect[3] += init.padding;
            }
        }
        self.window(
            "tooltip",
            WindowPlacement::Tooltip {
                around_rect: rect,
                placement: init.placement,
                minimal_size: [0, 0],
            },
            WINDOW_TRANSPARENT,
            i32::MAX - 1,
        )
    }

    pub fn last_item(&self, a: AreaRef) -> Option<ItemId> {
        let window = &self.windows[a.window_index as usize];
        let area = &window.areas[a.area_index as usize];
        let last_element = area.last_element as usize;
        window.layout.item_ids.get(last_element).copied()
    }

    pub fn reset_input_focus(&mut self) {
        self.input_focus = None;
    }

    pub fn set_input_focus(&mut self, window: AreaRef, item: Option<ItemId>) {
        if let Some(item) = item {
            if item != INVALID_ITEM_ID {
                self.input_focus = Some((item, self.window_ids[window.window_index as usize]));
            } else {
                self.input_focus = None;
            }
        } else {
            self.input_focus = None;
        }
    }

    pub fn input_focus(&self) -> Option<ItemId> {
        self.input_focus.map(|i| i.0)
    }

    pub fn edit_insert(&mut self, text: &mut String, text_to_insert: &str) {
        if self.edit_state.is_none() {
            if let Some((id, window)) = self.input_focus {
                self.edit_state = Some(EditState {
                    id,
                    window,
                    state: Default::default(),
                    scroll: [0, 0],
                });
            }
        }
        if let Some(EditState { state, .. }) = &mut self.edit_state {
            state.insert_string(text, text_to_insert);
            state.selection = None;
            state.cursor = (state.cursor as usize + text_to_insert.len()).min(text.len()) as u32;
        }
    }

    pub fn key_pressed(&self, key_code: KeyCode) -> bool {
        self.frame_input.iter().any(|e| match e {
            UIEvent::KeyDown { key, .. } => *key == key_code,
            _ => false,
        })
    }

    pub fn mouse_position(&self) -> [i32; 2] {
        self.last_mouse_position
    }

    pub fn is_mouse_clicked(&self, clicked_button: i32) -> bool {
        self.frame_input.iter().any(|e| match e {
            UIEvent::MouseDown { button, .. } => *button == clicked_button,
            _ => false,
        })
    }

    pub fn is_mouse_released(&self, clicked_button: i32) -> bool {
        self.frame_input.iter().any(|e| match e {
            UIEvent::MouseUp { button, .. } => *button == clicked_button,
            _ => false,
        })
    }

    pub fn scroll_to(&mut self, a: AreaRef, duration: f32) {
        let window = &mut self.windows[a.window_index as usize];
        window.interrupt_scroll_animation();
        let area = &mut window.areas[a.area_index as usize];
        area.scroll_to_time = duration;
    }

    fn show_popup_at_element(
        shown_popup: &mut Option<(ItemId, Rect, TooltipPlacement)>,
        window: &Window,
        element_index: usize,
        name: &str,
        beside: bool,
    ) {
        let margins = if let Some(e) = window.layout.elements.get(element_index) {
            e.margins
        } else {
            [0, 0, 0, 0]
        };
        let rect = if let Some(rect) = window.layout.rectangles.get(element_index) {
            rect_add_margins(*rect, margins)
        } else {
            [0, 0, 0, 0]
        };
        let id = hash_id_label(name, window.top_hash);
        *shown_popup = Some((
            id,
            rect,
            if beside {
                TooltipPlacement::Beside
            } else {
                TooltipPlacement::Below
            },
        ));
    }

    pub fn show_popup_at(&mut self, a: AreaRef, name: &str, beside: bool) {
        let window = &self.windows[a.window_index as usize];
        let area = &window.areas[a.area_index as usize];
        let element = area.element_index as usize;
        Self::show_popup_at_element(&mut self.shown_popup, window, element, name, beside);
    }
    pub fn show_popup_at_last(&mut self, a: AreaRef, name: &str) {
        let window = &self.windows[a.window_index as usize];
        let area = &window.areas[a.area_index as usize];
        let last_element = area.last_element as usize;
        Self::show_popup_at_element(&mut self.shown_popup, window, last_element, name, false);
    }

    pub fn show_popup_at_cursor(&mut self, a: AreaRef, name: &str) {
        let window = &self.windows[a.window_index as usize];
        let rect = [
            self.last_mouse_position[0],
            self.last_mouse_position[1],
            self.last_mouse_position[0],
            self.last_mouse_position[1],
        ];
        let id = hash_id_label(name, window.top_hash);
        self.shown_popup = Some((id, rect, TooltipPlacement::Below));
    }

    fn hide_popup_internal(
        shown_popup: &mut Option<(ItemId, Rect, TooltipPlacement)>,
        input_focus: &mut Option<(ItemId, ItemId)>,
        edit_state: &mut Option<EditState>,
    ) {
        let popup_id = hash_id_label("popup", 0);
        if Some(popup_id) == input_focus.map(|i| i.1) {
            *input_focus = None;
        }
        if Some(popup_id) == edit_state.as_ref().map(|i| i.window) {
            *edit_state = None;
        }
        *shown_popup = None;
    }

    pub fn hide_popup(&mut self) {
        Self::hide_popup_internal(
            &mut self.shown_popup,
            &mut self.input_focus,
            &mut self.edit_state,
        );
    }

    pub fn is_popup_shown(&mut self, window: AreaRef, name: &str) -> Option<AreaRef> {
        if let Some((shown_id, rect, placement)) = self.shown_popup {
            let id = hash_id_label(name, self.windows[window.window_index as usize].top_hash);
            if shown_id == id {
                let a = self.window(
                    "popup",
                    WindowPlacement::Tooltip {
                        around_rect: rect,
                        placement,
                        minimal_size: [0, 0],
                    },
                    0x70000000,
                    i32::MAX - 2,
                );
                let a = self.add(
                    a,
                    Frame {
                        margins: [6, 6, 6, 6],
                        ..Default::default()
                    },
                );
                let a = self.add(a, BoxLayout::default());
                return Some(a);
            }
        }
        None
    }

    pub fn handle_event(
        &mut self,
        event: &UIEvent,
        render_rect: [Position; 4],
        event_time: f32,
    ) -> bool {
        let mut handled_scroll = false;
        let mut handled = false;

        // continue previous scrolling if needed
        match event {
            UIEvent::MouseMove { .. }
            | UIEvent::MouseDown { .. }
            | UIEvent::MouseUp { .. }
            | UIEvent::MouseWheel { .. } => {
                for window in &mut self.windows {
                    let mut interrupt_scroll = false;
                    for (_, scroll) in &mut window.scrolls {
                        let was_scrolling = scroll.scrolling;
                        if scroll.handle_subsequent_input_event(&event, event_time) {
                            handled_scroll = true;
                        }
                        let scrolling = scroll.scrolling;
                        if scrolling && !was_scrolling {
                            interrupt_scroll = true;
                        }
                    }
                    if interrupt_scroll {
                        window.interrupt_scroll_animation();
                        self.hit_item = INVALID_ITEM_ID;
                        self.input_focus = None;
                    }
                }
            }
            _ => {}
        }

        self.frame_input.push(event.clone());

        match event {
            &UIEvent::MouseMove { pos } => {
                self.last_mouse_position = pos;
                let mut hover_handled = false;
                self.hovered_item = INVALID_ITEM_ID;
                for &i in self.window_order.iter().rev() {
                    let window = &mut self.windows[i];
                    window.over_drop_item = 0;
                    if window.drag_item != 0 {
                        window.drag_offset =
                            [pos[0] - window.drag_start[0], pos[1] - window.drag_start[1]];
                    }
                    if !rect_contains_point(window.computed_rect, pos) {
                        continue;
                    }
                    if window.is_empty() {
                        continue;
                    }
                    if hover_handled {
                        continue;
                    }
                    for hit_item in &window.hit_items {
                        if let Some(r) = window.hit_rectangle(
                            hit_item.element_index,
                            hit_item.clip_item_index,
                            None,
                        ) {
                            if rect_contains_point(r, pos) {
                                self.hovered_item = hit_item.item_id;
                            }
                        }
                    }
                    for drop_item in &window.drop_items {
                        if let Some(r) = window.hit_rectangle(
                            drop_item.element_index,
                            drop_item.clip_item_index,
                            None,
                        ) {
                            if rect_contains_point(r, pos) {
                                window.over_drop_item = drop_item.id;
                            }
                        }
                    }
                    if self.hit_item != INVALID_ITEM_ID {
                        handled = true;
                    }
                    hover_handled = true;
                    if (window.flags & WINDOW_TRANSPARENT) == 0 {
                        // stop handling event
                        handled = true;
                    }
                }
            }
            &UIEvent::MouseDown { pos, .. } | &UIEvent::MouseWheel { pos, .. } => {
                let popup_id = hash_id_label("popup", 0);
                for &i in self.window_order.iter().rev() {
                    let window = &mut self.windows[i];
                    if !rect_contains_point(window.computed_rect, pos) {
                        if window.id == popup_id {
                            Self::hide_popup_internal(
                                &mut self.shown_popup,
                                &mut self.input_focus,
                                &mut self.edit_state,
                            );
                        }
                        continue;
                    }
                    if window.is_empty() {
                        continue;
                    }
                    let mut non_scroll_hit_items = false;
                    for hit_item in window.hit_items.iter().rev() {
                        if let Some(r) = window.hit_rectangle(
                            hit_item.element_index,
                            hit_item.clip_item_index,
                            None,
                        ) {
                            if rect_contains_point(r, pos) && !hit_item.is_scroll {
                                non_scroll_hit_items = true;
                                break;
                            }
                        }
                    }
                    // could had been handled by an overlapping window
                    let mut interrupt_scroll = false;
                    for hit_item in window.hit_items.iter().rev() {
                        let r = match window.hit_rectangle(
                            hit_item.element_index,
                            hit_item.clip_item_index,
                            None,
                        ) {
                            Some(r) => r,
                            None => continue,
                        };
                        if !rect_contains_point(r, pos) {
                            continue;
                        }
                        if hit_item.is_scroll {
                            if !handled_scroll {
                                let (scroll, _) =
                                    Window::scroll_by_id(&mut window.scrolls, hit_item.item_id);
                                if non_scroll_hit_items {
                                    scroll.scroll_move_threshold = 10;
                                } else {
                                    scroll.scroll_move_threshold = 0;
                                    if !matches!(event, UIEvent::MouseWheel { .. }) {
                                        window.scroll_item = hit_item.item_id;
                                    }
                                }
                                scroll.size = rect_size(render_rect);
                                scroll.handle_start_input_event(&event, event_time);
                                interrupt_scroll = true;
                            }
                        } else {
                            match event {
                                UIEvent::MouseWheel { .. } => {}
                                _ => {
                                    if !handled {
                                        self.hit_item = hit_item.item_id;
                                        if hit_item.consumes_keys || hit_item.consumes_chars {
                                            self.input_focus = Some((hit_item.item_id, window.id));
                                        }
                                        handled = true;
                                    }
                                }
                            }
                        }
                    }
                    if interrupt_scroll {
                        window.interrupt_scroll_animation();
                    }
                    if handled && handled_scroll {
                        break;
                    }
                    for drag_item in window.drag_items.iter().rev() {
                        if let Some(r) = window.hit_rectangle(
                            drag_item.element_index,
                            drag_item.clip_item_index,
                            None,
                        ) {
                            if rect_contains_point(r, pos) {
                                window.drag_item = drag_item.id;
                                window.drag_start = pos;
                                window.drag_offset = [0, 0];
                                handled = true;
                                break;
                            }
                        }
                    }
                    if (window.flags & WINDOW_TRANSPARENT) == 0 {
                        handled = true;
                    }
                }
            }
            &UIEvent::MouseUp { pos, .. } => {
                for &i in self.window_order.iter().rev() {
                    let window = &mut self.windows[i];
                    if window.is_empty() {
                        continue;
                    }
                    if window.drag_item != 0 {
                        if window.over_drop_item != 0 {
                            let result = DragResult {
                                drag: window.drag_item,
                                drop: window.over_drop_item,
                            };
                            // duplicate as they get cleared separately
                            window.drag_result = result;
                            window.drop_result = result;
                            window.over_drop_item = 0;
                        }
                        window.drag_item = 0;
                        window.drag_offset = [0, 0];
                    }
                    if rect_contains_point(window.computed_rect, pos) {
                        let hit_item_id = self.hit_item;
                        let hit_item = window.hit_items.iter().find(|i| i.item_id == hit_item_id);
                        if let Some(hit_item) = hit_item {
                            if let Some(r) = window.hit_rectangle(
                                hit_item.element_index,
                                hit_item.clip_item_index,
                                None,
                            ) {
                                if rect_contains_point(r, pos) {
                                    self.released_item = hit_item.item_id;
                                    handled = true;
                                }
                            }
                        }
                    }
                    if rect_contains_point(window.computed_rect, pos) {}
                    window.scroll_item = 0;
                    /*
                    match event {
                        FingerUp{ .. } => {
                            window.hovered_item = INVALID_ITEM_ID;
                        }
                    }
                    */
                }
                if Some(self.hit_item) != self.input_focus.map(|i| i.0) {
                    self.input_focus = None;
                }
                self.hit_item = INVALID_ITEM_ID;
            }
            UIEvent::TextInput { .. } | UIEvent::KeyDown { .. } => {
                if self.input_focus.is_some() {
                    handled = true;
                }
            }
            _ => {}
        }
        handled
    }

    pub fn consumes_key_down(&self) -> bool {
        self.input_focus.is_some()
    }
    pub fn hovered_item(&self) -> Option<ItemId> {
        if self.hovered_item != INVALID_ITEM_ID {
            Some(self.hovered_item)
        } else {
            None
        }
    }
    pub fn hit_item(&self) -> Option<ItemId> {
        if self.hit_item != INVALID_ITEM_ID {
            Some(self.hit_item)
        } else {
            None
        }
    }
}

fn draw_rect(render: &mut dyn Render, r: [i32; 4], color: [u8; 4]) {
    render.set_sprite(None);
    let positions = [
        [r[0] as f32, r[1] as f32],
        [r[2] as f32, r[1] as f32],
        [r[2] as f32, r[3] as f32],
        [r[0] as f32, r[3] as f32],
    ];
    let uvs = [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]];
    let indices = [0, 1, 2, 2, 3, 0];
    render.add_vertices(&positions, &uvs, &indices, color);
}

fn draw_frame_rect(
    batch: &mut dyn Render,
    sprite: SpriteKey,
    cut: Margins,
    rect: [i32; 4],
    offset: [Position; 2],
    color: [u8; 4],
    sprite_context: &dyn SpriteContext,
) {
    batch.set_sprite(Some(sprite));
    let [width, height] = sprite_context.sprite_size(sprite);
    let sprite_uv = sprite_context.sprite_uv(sprite);
    let m = cut;

    let mut uv_x = [
        0f32,
        m[0] as f32 / width as f32,
        1.0 - m[2] as f32 / width as f32,
        1.0,
    ];
    let mut uv_y = [
        0f32,
        m[1] as f32 / height as f32,
        1.0 - m[3] as f32 / height as f32,
        1.0,
    ];
    for u in &mut uv_x {
        *u = *u * (sprite_uv[2] - sprite_uv[0]) + sprite_uv[0];
    }
    for v in &mut uv_y {
        *v = *v * (sprite_uv[3] - sprite_uv[1]) + sprite_uv[1];
    }
    let mut pos_x = [
        rect[0] - m[0] as i32,
        rect[0],
        rect[2],
        rect[2] + m[2] as i32,
    ];
    let mut pos_y = [
        rect[1] - m[1] as i32,
        rect[1],
        rect[3],
        rect[3] + m[3] as i32,
    ];
    for x in &mut pos_x {
        *x = *x + offset[0];
    }
    for y in &mut pos_y {
        *y = *y + offset[1];
    }

    let mut positions: [[f32; 2]; 4 * 4] = Default::default();
    let mut uvs: [[f32; 2]; 4 * 4] = Default::default();
    for (index, (pos, uv)) in positions.iter_mut().zip(uvs.iter_mut()).enumerate() {
        let y = index / 4;
        let x = index % 4;
        *pos = [pos_x[x] as f32, pos_y[y] as f32];
        *uv = [uv_x[x], uv_y[y]];
    }

    let quad_indices = [0, 1, 2, 0, 2, 3];
    let mut indices: [IndexType; 3 * 3 * 6] = [0; 3 * 3 * 6];
    for (index, v) in indices.iter_mut().enumerate() {
        let quad_index = index / 6;
        let y = quad_index / 3;
        let x = quad_index % 3;
        let corner_index = quad_indices[index % 6];
        let i = match corner_index {
            0 => y * 4 + x,
            1 => y * 4 + x + 1,
            2 => (y + 1) * 4 + x + 1,
            _ => (y + 1) * 4 + x,
        } as IndexType;
        *v = i;
    }
    batch.add_vertices(&positions, &uvs, &indices, color);
}

const AXIS_HORIZONTAL: usize = 0;
const AXIS_VERTICAL: usize = 1;

fn debug_trace_animation() -> bool {
    false
}

impl Window {
    fn new() -> Self {
        Self {
            id_str: String::new(),
            id: 0,
            sort_key: 0,
            top_hash: 0,
            hash_stack: Vec::new(),
            areas: Vec::new(),
            update_frame: 0,
            placement: WindowPlacement::Fullscreen,
            flags: WINDOW_TRANSPARENT,
            layout: Layout::new(),
            computed_rect: [0, 0, 0, 0],
            hit_items: Vec::new(),
            clip_items: Vec::new(),

            image_ids: Vec::new(),
            images: Vec::new(),

            // wrapped text
            wrapped_text_elements: Vec::new(),
            wrapped_texts: Vec::new(),

            drag_items: Vec::new(),
            drop_items: Vec::new(),
            drag_item: 0,
            over_drop_item: 0,
            drag_start: [0, 0],
            drag_offset: [0, 0],
            drag_result: DragResult { drag: 0, drop: 0 },
            drop_result: DragResult { drag: 0, drop: 0 },

            // scroll state
            scroll_item: 0,
            scrolls: Vec::new(),
            scroll_elements: Vec::new(),
            scroll_animations: Vec::new(),

            // temporary
            clip_item_rects: Vec::new(),
        }
    }
    fn clear(&mut self) {
        self.areas.clear();
        self.flags = WINDOW_TRANSPARENT;
        self.hit_items.clear();
        self.clip_items.clear();
        self.drag_items.clear();
        self.drop_items.clear();
        self.scroll_elements.clear();
        self.layout.clear();
        self.layout.add_element(
            self.id,
            LayoutElement {
                parent: -1,
                typ: ElementType::Vertical {
                    children_list: -1,
                    padding: 0,
                },
                expanding: true,
                min_size: [0, 0],
                focus_flags: FocusFlags::NotFocusable,
                ..LayoutElement::new()
            },
        );
        self.wrapped_text_elements.clear();
        self.wrapped_texts.clear();
    }
    fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }

    fn find_parent(layout: &Layout, child: ElementIndex) -> ElementIndex {
        // brute force search, would be better to have parent reference in the layout element
        // but it should be fine for now as we do this only once in a while
        let mut children_list = -1;
        for i in 0..layout.children_lists.len() as i16 {
            let children = &layout.children_lists[i as usize];
            if children.contains(&child) {
                children_list = i;
                break;
            }
        }
        if children_list == -1 {
            // nothing found
            assert!(false);
            return -1;
        }
        for i in 0..layout.elements.len() {
            if layout.elements[i].typ.children_list() == children_list {
                return i as ElementIndex;
            }
        }
        assert!(false);
        return -1;
    }

    fn find_delta_to_fit_element_on_screen(layout: &Layout, target: ElementIndex) -> [i32; 2] {
        let target_rect = layout.rectangles[target as usize];

        let mut current = target as usize;
        loop {
            let parent = Self::find_parent(layout, current as ElementIndex);
            if parent == -1 {
                // failed to get parent
                assert!(false);
                return [0, 0];
            }
            assert!(layout.elements.len() == layout.item_ids.len());
            let e = &layout.elements[current as usize];
            match e.typ {
                ElementType::Scroll { .. } => {
                    let r = layout.rectangles[current];
                    let mut x_delta = (target_rect[2] - r[2]).max(0);
                    if target_rect[0] < r[0] {
                        x_delta = target_rect[0] - r[0];
                    }
                    let mut y_delta = (target_rect[3] - r[3]).max(0);
                    if target_rect[1] < r[1] {
                        y_delta = target_rect[1] - r[1];
                    }
                    return [x_delta, y_delta];
                }
                _ => {}
            }
            current = parent as usize;
        }
    }

    fn update_animation(&mut self, dt: f32, debug_frame: u64) -> bool {
        let mut result = false;
        for (_, scroll) in &mut self.scrolls {
            scroll.update(dt);
        }
        let mut removed_anims = Vec::new();
        for i in 0..self.scroll_animations.len() {
            let anim = &mut self.scroll_animations[i];
            if !anim.initialized {
                let start_scroll = Self::scroll_by_id(&mut self.scrolls, anim.scroll_area_id)
                    .0
                    .offset;
                let delta =
                    Self::find_delta_to_fit_element_on_screen(&self.layout, anim.target_element);
                let target = start_scroll + vec2(delta[0] as f32, delta[1] as f32);
                anim.position = start_scroll.into();
                anim.target = target;
                anim.initialized = true;
                if anim.duration == 0.0 {
                    let value = &mut Self::scroll_by_id(&mut self.scrolls, anim.scroll_area_id)
                        .0
                        .offset;
                    let old_value = *value;
                    *value = target.into();
                    result = true;
                    let scroll_area_id = anim.scroll_area_id;
                    removed_anims.push(i);
                    if debug_trace_animation() {
                        info!(
                            "{}: instant scroll 0x{:x}: {:?} -> {:?}",
                            debug_frame, scroll_area_id, old_value, value
                        );
                    }
                    continue;
                } else {
                    if debug_trace_animation() {
                        info!("initialized scroll 0x{:x}", anim.scroll_area_id);
                    }
                }
            }

            if self.scroll_item == anim.scroll_area_id {
                // being dragged currently, wait before user releases it
                continue;
            }
            let old_value = anim.position;
            smooth_cd(
                &mut anim.position,
                &mut anim.velocity,
                anim.target,
                dt,
                anim.ease_time,
            );
            let new_value = anim.position;
            let value = &mut Self::scroll_by_id(&mut self.scrolls, anim.scroll_area_id)
                .0
                .offset;
            if debug_trace_animation() && value.round() != new_value.round() {
                info!(
                    "{}, animated scroll 0x{:x}: {:?} -> {:?} in {} s",
                    debug_frame, anim.scroll_area_id, old_value, anim.position, dt
                );
            }
            *value = new_value;
            result = true;
            if anim.position.round() == anim.target.round() {
                if debug_trace_animation() {
                    info!(
                        "{}: animated scroll 0x{:x} finished.",
                        debug_frame, anim.scroll_area_id
                    );
                }
                removed_anims.push(i);
            }
        }
        for index in removed_anims.into_iter().rev() {
            self.scroll_animations.remove(index);
        }
        result
    }

    fn update_scroll(&mut self) -> bool {
        let mut result = false;
        for j in 0..self.scroll_elements.len() {
            for i in 0..self.scrolls.len() {
                if self.scroll_elements[j].0 == self.scrolls[i].0 {
                    let scroll = &mut self.scrolls[i].1;
                    let element_index = self.scroll_elements[j].1 as usize;
                    let e = &mut self.layout.elements[element_index];
                    // read scroll range from element
                    let r = self.layout.rectangles[element_index];
                    let mut child_rect = r;
                    if e.typ.children_list() >= 0 {
                        let children = &self.layout.children_lists[e.typ.children_list() as usize];
                        if !children.is_empty() {
                            child_rect = self.layout.rectangles[children[0] as usize];
                        }
                    }
                    let mut size_delta = [
                        (child_rect[2] - child_rect[0]) - (r[2] - r[0]),
                        (child_rect[3] - child_rect[1]) - (r[3] - r[1]),
                    ];
                    if size_delta[0] < 0 {
                        size_delta[0] = 0;
                    }
                    if size_delta[1] < 0 {
                        size_delta[1] = 0;
                    }
                    scroll.range[0] = 0.0;
                    scroll.range[1] = 0.0;
                    scroll.range[2] = size_delta[0] as f32;
                    scroll.range[3] = size_delta[1] as f32;

                    // clamp scroll to the limits
                    let clamped_scroll = [
                        scroll.offset.x.max(scroll.range[0]).min(scroll.range[2]),
                        scroll.offset.y.max(scroll.range[1]).min(scroll.range[3]),
                    ];
                    scroll.offset = vec2(clamped_scroll[0], clamped_scroll[1]);

                    // write scroll to element
                    if e.scroll[0] as f32 != scroll.offset.x
                        || e.scroll[1] as f32 != scroll.offset.y
                    {
                        e.scroll[0] = scroll.offset.x as _;
                        e.scroll[1] = scroll.offset.y as _;
                        result = true;
                    }
                }
            }
        }
        result
    }

    fn update_layout(
        &mut self,
        render_rect: [i32; 4],
        font_context: &dyn FontContext,
        draw_texts: &str,
    ) {
        let wrapped_texts = &mut self.wrapped_texts;
        let wrapped_text_elements = &self.wrapped_text_elements;
        for (wrapped, element) in wrapped_texts
            .iter_mut()
            .zip(wrapped_text_elements.iter().copied())
        {
            if wrapped.max_width != 0 {
                wrapped.lines.clear();
                let text = &draw_texts
                    [wrapped.text.0 as usize..(wrapped.text.0 + wrapped.text.1) as usize];
                let longest_line = font_context.wrap_text(
                    &mut wrapped.lines,
                    wrapped.font,
                    text,
                    wrapped.max_width as i32,
                );
                let e = &mut self.layout.elements[element as usize];
                e.min_size[0] = e.min_size[0].max(longest_line as u16);
            } else {
                // computed in height_by_width
            }
        }

        let num_elements = self.layout.elements.len();
        self.layout.minimal_sizes[0].resize(num_elements, 0);
        self.layout.minimal_sizes[1].resize(num_elements, 0);
        self.layout.rectangles.resize(num_elements, [0, 0, -1, -1]);

        let lroot: ElementIndex = 0;
        Layout::calculate_minimal_sizes_r(
            &mut self.layout.minimal_sizes,
            &self.layout.elements,
            &self.layout.children_lists,
            AXIS_HORIZONTAL,
            lroot,
            &mut |_| -> Position { 0 },
        );

        let root_min_width = self.layout.minimal_sizes[AXIS_HORIZONTAL][lroot as usize] as i32;

        let expand: ExpandFlags = match self.placement {
            WindowPlacement::Fullscreen => 0,
            WindowPlacement::Absolute { expand, .. } => expand,
            WindowPlacement::Center { expand, .. } => expand,
            WindowPlacement::Tooltip { .. } => 0,
        };

        let calculate_placement = |placement: &WindowPlacement,
                                   axis: usize,
                                   root_min_size: i32|
         -> (i32, i32) {
            match *placement {
                WindowPlacement::Fullscreen => (render_rect[axis], render_rect[axis + 2]),
                WindowPlacement::Absolute { pos, size, .. } => (pos[axis], pos[axis] + size[axis]),
                WindowPlacement::Center { offset, size, .. } => (
                    (render_rect[axis] + render_rect[axis + 2]) / 2 + offset[axis] - size[axis] / 2,
                    (render_rect[axis] + render_rect[axis + 2]) / 2 + offset[axis] - size[axis] / 2
                        + size[axis],
                ),
                WindowPlacement::Tooltip {
                    minimal_size,
                    around_rect,
                    placement: t_placement,
                } => {
                    let min_size = max(root_min_size, minimal_size[axis]);

                    match (t_placement, axis) {
                        (TooltipPlacement::Beside, 0)
                        | (TooltipPlacement::Below, 1)
                        | (TooltipPlacement::BelowCentered, 1) => {
                            let upper_side_loss = max(
                                0,
                                around_rect[axis + 2] + min_size - rect_axis(render_rect, axis),
                            );
                            let lower_side_loss = max(0, min_size - around_rect[axis]);
                            if upper_side_loss <= lower_side_loss {
                                (around_rect[axis + 2], around_rect[axis + 2] + min_size)
                            } else {
                                (around_rect[axis] - min_size, around_rect[axis])
                            }
                        }
                        (TooltipPlacement::Beside, 1) | (TooltipPlacement::Below, 0) => {
                            let mut lower = max(around_rect[axis], 0);
                            let mut higher = lower + min_size;
                            if higher > rect_axis(render_rect, axis) {
                                if min_size < rect_axis(render_rect, axis) {
                                    lower = rect_axis(render_rect, axis) - min_size;
                                    higher = rect_axis(render_rect, axis);
                                } else {
                                    lower = 0;
                                    higher = min_size;
                                }
                            }
                            (lower, higher)
                        }
                        (TooltipPlacement::BelowCentered, 0) => {
                            let lower =
                                (around_rect[axis] + around_rect[axis + 2]) / 2 - min_size / 2;
                            let higher = lower + min_size;
                            (lower, higher)
                        }
                        (_, _) => panic!(""),
                    }
                }
            }
        };

        let (mut root_left, mut root_right) =
            calculate_placement(&self.placement, AXIS_HORIZONTAL, root_min_width);

        let expand_position = |lower: &mut i32,
                               higher: &mut i32,
                               min_value: i32,
                               flag_lower: bool,
                               flag_higher: bool| {
            let delta = min_value - (*higher - *lower);
            let (lower_offset, higher_offset) = match (flag_lower, flag_higher) {
                (true, true) => (-delta / 2, delta - delta / 2),
                (true, false) => (-delta, 0),
                (false, true) => (0, delta),
                _ => (0, 0),
            };
            *lower += lower_offset;
            *higher += higher_offset;
        };

        expand_position(
            &mut root_left,
            &mut root_right,
            root_min_width,
            (expand & EXPAND_LEFT) != 0,
            (expand & EXPAND_RIGHT) != 0,
        );

        Layout::calculate_rectangles_r(
            &mut self.layout.rectangles,
            &self.layout.elements,
            &self.layout.children_lists,
            &self.layout.minimal_sizes,
            AXIS_HORIZONTAL,
            lroot,
            root_left,
            root_right - root_left,
        );

        let rectangles = &self.layout.rectangles;
        let mut height_by_width = |element: ElementIndex| -> Position {
            let width = rect_width(rectangles[element as usize]) as u16;
            let wrapped_text_index = match wrapped_text_elements.iter().position(|e| *e == element)
            {
                Some(v) => v,
                None => return 1,
            };
            let wrapped = &mut wrapped_texts[wrapped_text_index as usize];
            if wrapped.max_width == 0 {
                wrapped.lines.clear();
                let text = &draw_texts
                    [wrapped.text.0 as usize..(wrapped.text.0 + wrapped.text.1) as usize];
                font_context.wrap_text(&mut wrapped.lines, wrapped.font, &text, width as i32);
            } else {
                // computed in the beginning of layout
            }
            (wrapped.lines.len() as i32) * (font_context.font_height(wrapped.font).ceil() as i32)
        };
        Layout::calculate_minimal_sizes_r(
            &mut self.layout.minimal_sizes,
            &self.layout.elements,
            &self.layout.children_lists,
            AXIS_VERTICAL,
            lroot,
            &mut height_by_width,
        );

        let root_min_height = self.layout.minimal_sizes[AXIS_VERTICAL][lroot as usize] as i32;

        let (mut root_top, mut root_bottom) =
            calculate_placement(&self.placement, AXIS_VERTICAL, root_min_height);

        expand_position(
            &mut root_top,
            &mut root_bottom,
            root_min_height,
            (expand & EXPAND_UP) != 0,
            (expand & EXPAND_DOWN) != 0,
        );

        Layout::calculate_rectangles_r(
            &mut self.layout.rectangles,
            &self.layout.elements,
            &self.layout.children_lists,
            &self.layout.minimal_sizes,
            AXIS_VERTICAL,
            lroot,
            root_top,
            root_bottom - root_top,
        );

        self.computed_rect = [root_left, root_top, root_right, root_bottom];
        // console_log(&format!("layout {:#?}", &self.layout));
    }

    fn add_scroll_animation(
        &mut self,
        scroll_area_id: ItemId,
        target_element: ElementIndex,
        duration: f32,
    ) {
        if debug_trace_animation() {
            info!(
                "add_scroll_animation area {:x} {}",
                scroll_area_id, duration
            );
        }
        if scroll_area_id == 0 {
            assert!(false);
            return;
        }

        let anim = ScrollAnimation {
            scroll_area_id,
            target_element,
            initialized: false,
            duration,
            velocity: vec2(0.0, 0.0),
            target: vec2(0.0, 0.0),
            position: vec2(0.0, 0.0),
            ease_time: duration,
        };
        if debug_trace_animation() {
            info!(
                "added scroll animation 0x{:x}: element {} duration {}",
                scroll_area_id, anim.target_element, anim.duration
            );
        }
        let mut animation_index = -1;
        for i in 0..self.scroll_animations.len() {
            if self.scroll_animations[i].scroll_area_id == scroll_area_id {
                animation_index = i as i32;
                break;
            }
        }
        if animation_index == -1 {
            self.scroll_animations.push(anim)
        } else {
            self.scroll_animations[animation_index as usize] = anim
        }
    }

    fn interrupt_scroll_animation(&mut self) {
        // interrupt scroll animations
        let scroll_item = self.scroll_item;
        self.scroll_animations.retain(|anim| {
            if anim.scroll_area_id == scroll_item {
                if debug_trace_animation() {
                    info!("interrupted scroll animation 0x{:x}", anim.scroll_area_id);
                }
                false
            } else {
                true
            }
        });
    }

    fn hit_rectangle(
        &self,
        element_index: ElementIndex,
        clip_item_index: usize,
        item_id: Option<ItemId>,
    ) -> Option<[Position; 4]> {
        let e = match self.layout.elements.get(element_index as usize) {
            Some(e) => e,
            None => return None,
        };
        let r = match self.layout.rectangles.get(element_index as usize) {
            Some(r) => *r,
            None => return None,
        };
        if item_id.is_some() {
            if self.layout.item_ids.get(element_index as usize).copied() != item_id {
                return None;
            }
        }
        let mut r = rect_add_margins(r, e.margins);
        if let Some(clip_rect) = self.clip_item_rects.get(clip_item_index).copied() {
            r = rect_intersect(r, clip_rect);
        }
        Some(r)
    }

    fn scroll_by_id(
        scrolls: &mut Vec<(ItemId, TouchScroll)>,
        scroll_id: ItemId,
    ) -> (&mut TouchScroll, bool /*created*/) {
        let find_result = scrolls.iter_mut().position(|(id, _)| *id == scroll_id);
        let mut added = false;
        let pos = match find_result {
            Some(pos) => pos,
            None => {
                added = true;
                scrolls.push((scroll_id, TouchScroll::new()));
                scrolls.len() - 1
            }
        };
        (&mut scrolls[pos].1, added)
    }
}

impl Layout {
    fn new() -> Self {
        Self {
            elements: Vec::new(),
            item_ids: Vec::new(),
            rectangles: Vec::new(),
            minimal_sizes: [Vec::new(), Vec::new()],
            children_lists: Vec::new(),
            next_children_list: 0,
        }
    }

    fn clear(&mut self) {
        self.elements.clear();
        self.item_ids.clear();
        for list in &mut self.children_lists {
            list.clear();
        }
        self.next_children_list = 0;
        self.minimal_sizes[0].clear();
        self.minimal_sizes[1].clear();
    }

    fn add_element(&mut self, item_id: ItemId, e: LayoutElement) -> ElementIndex {
        assert!(self.item_ids.len() == self.elements.len());
        self.item_ids.push(item_id);
        let index = self.elements.len() as ElementIndex;
        let parent = e.parent;
        self.elements.push(e);
        if parent != -1 {
            self.add_to_children_list(parent, index);
        }
        index
    }

    fn add_to_children_list(&mut self, parent: ElementIndex, child: ElementIndex) {
        let e = &mut self.elements[parent as usize];
        if e.typ.children_list() == INVALID_CHILDREN_LIST {
            let dest = match &mut e.typ {
                ElementType::Horizontal { children_list, .. } => children_list,
                ElementType::Vertical { children_list, .. } => children_list,
                ElementType::Stack { children_list, .. } => children_list,
                ElementType::Scroll { children_list, .. } => children_list,
                ElementType::Align { children_list, .. } => children_list,
                ElementType::Frame { children_list, .. } => children_list,
                _ => panic!("parenting to invalid element type {:?}", e.typ),
            };
            *dest = self.next_children_list;
            if self.next_children_list as isize >= self.children_lists.len() as isize {
                self.children_lists.push(Vec::<ElementIndex>::new());
            }
            self.next_children_list += 1;
        }

        self.children_lists[e.typ.children_list() as usize].push(child);
    }

    fn calculate_minimal_sizes_r(
        minimal_sizes: &mut [Vec<u16>; 2],
        elements: &[LayoutElement],
        children_lists: &[Vec<ElementIndex>],
        axis: usize,
        element: ElementIndex,
        height_by_width: &mut dyn FnMut(ElementIndex) -> Position,
    ) -> Position {
        let e = &elements[element as usize];
        let min_size: Position = match e.typ {
            ElementType::FixedSize | ElementType::HeightByWidth => {
                if axis == AXIS_VERTICAL && e.typ == ElementType::HeightByWidth {
                    height_by_width(element)
                } else {
                    (e.min_size[axis] + e.margins[axis] as u16 + e.margins[axis + 2] as u16)
                        as Position
                }
            }
            ElementType::Align { children_list, .. } => {
                let result = e.min_size[axis] as Position;
                if children_list != INVALID_CHILDREN_LIST {
                    let children = &children_lists[children_list as usize];
                    for child in children {
                        Layout::calculate_minimal_sizes_r(
                            minimal_sizes,
                            &elements,
                            &children_lists,
                            axis,
                            *child,
                            height_by_width,
                        );
                    }
                }
                result
            }
            ElementType::Scroll {
                children_list,
                max_size,
                ..
            } => {
                let mut result = e.min_size[axis] as Position;
                if children_list != INVALID_CHILDREN_LIST {
                    let children = &children_lists[children_list as usize];
                    let mut s = 0;
                    for child in children {
                        let children_size = Layout::calculate_minimal_sizes_r(
                            minimal_sizes,
                            &elements,
                            &children_lists,
                            axis,
                            *child,
                            height_by_width,
                        );
                        if max_size[axis] != 0 {
                            s = max(s, children_size);
                        }
                    }
                    result = max(result, min(s, max_size[axis] as i32));
                }
                result
            }
            ElementType::Stack { children_list, .. } | ElementType::Frame { children_list, .. } => {
                let s = if children_list != INVALID_CHILDREN_LIST {
                    let mut s = 0;
                    let children = &children_lists[children_list as usize];
                    for child in children {
                        let children_size = Layout::calculate_minimal_sizes_r(
                            minimal_sizes,
                            &elements,
                            &children_lists,
                            axis,
                            *child,
                            height_by_width,
                        );
                        s = max(s, children_size);
                    }
                    s
                } else {
                    0
                };
                let margin_along = e.margins[axis] as Position + e.margins[axis + 2] as Position;
                max(s, e.min_size[axis] as Position) + margin_along
            }
            ElementType::Horizontal {
                padding,
                children_list,
            }
            | ElementType::Vertical {
                padding,
                children_list,
            } => {
                let margin_along = e.margins[axis] as Position + e.margins[axis + 2] as Position;
                let children_size = if children_list != INVALID_CHILDREN_LIST {
                    let children = &children_lists[children_list as usize];
                    let type_axis = if matches!(e.typ, ElementType::Horizontal { .. }) {
                        AXIS_HORIZONTAL
                    } else {
                        AXIS_VERTICAL
                    };
                    if type_axis == axis {
                        let mut s = margin_along as Position;
                        for child in children {
                            let children_size = Layout::calculate_minimal_sizes_r(
                                minimal_sizes,
                                &elements,
                                &children_lists,
                                axis,
                                *child,
                                height_by_width,
                            );
                            s += children_size + padding as Position;
                        }
                        if !children.is_empty() {
                            s -= padding as Position;
                        }
                        s
                    } else {
                        let mut s = 0 as Position;
                        for child in children {
                            let children_size = Layout::calculate_minimal_sizes_r(
                                minimal_sizes,
                                &elements,
                                &children_lists,
                                axis,
                                *child,
                                height_by_width,
                            );
                            s = max(s, children_size + margin_along);
                        }
                        s
                    }
                } else {
                    0
                };
                max(children_size, e.min_size[axis] as Position + margin_along)
            }
        };
        minimal_sizes[axis][element as usize] = min_size as u16;
        min_size
    }

    fn calculate_rectangles_r(
        mut rectangles: &mut [[i32; 4]],
        elements: &[LayoutElement],
        children_lists: &[Vec<ElementIndex>],
        minimal_sizes: &[Vec<u16>],
        axis: usize,
        element: ElementIndex,
        offset: i32,
        length: i32,
    ) {
        assert_eq!(elements.len(), rectangles.len());
        assert_eq!(elements.len(), minimal_sizes[0].len());
        assert_eq!(elements.len(), minimal_sizes[1].len());

        let e = &elements[element as usize];
        let out = &mut rectangles[element as usize];
        out[axis] = offset + e.margins[axis] as i32;
        out[axis + 2] = offset + length - e.margins[axis + 2] as i32;

        let children = match e.typ.children_list() {
            INVALID_CHILDREN_LIST => None,
            index => Some(&children_lists[index as usize]),
        };

        match (e.typ, children) {
            (ElementType::Align { align, .. }, Some(children)) => {
                let scroll = -e.scroll[axis];
                for child in children {
                    let child_fixed_length = minimal_sizes[axis][*child as usize] as i32;
                    let align = align[axis];
                    let aligned_offset: i32 = match align {
                        -1 => -child_fixed_length / 2,
                        0 => (length - child_fixed_length) / 2,
                        1 => length - child_fixed_length / 2,
                        _ => panic!(""),
                    };
                    let child_offset = scroll as i32 + offset as i32 + aligned_offset;
                    Layout::calculate_rectangles_r(
                        &mut rectangles,
                        &elements,
                        &children_lists,
                        &minimal_sizes,
                        axis,
                        *child,
                        child_offset,
                        child_fixed_length,
                    );
                }
                return;
            }
            (ElementType::Scroll { align, .. }, Some(children)) => {
                let scroll = -e.scroll[axis];
                for child in children {
                    let child_fixed_length = minimal_sizes[axis][*child as usize] as i32;
                    let align = align[axis];
                    let aligned_offset: i32 = match align {
                        -1 => 0,
                        0 => (length - child_fixed_length) / 2,
                        1 => length - child_fixed_length,
                        _ => panic!(""),
                    };
                    let child_offset = scroll as i32 + offset as i32 + aligned_offset;
                    Layout::calculate_rectangles_r(
                        &mut rectangles,
                        &elements,
                        &children_lists,
                        &minimal_sizes,
                        axis,
                        *child,
                        child_offset,
                        child_fixed_length,
                    );
                }
                return;
            }
            (ElementType::Frame { .. }, _) => {
                let child_offset = offset + e.margins[axis] as i32;
                let child_length = length - e.margins[axis] as i32 - e.margins[axis + 2] as i32;
                if let Some(children) = children {
                    for child in children {
                        Layout::calculate_rectangles_r(
                            &mut rectangles,
                            &elements,
                            &children_lists,
                            &minimal_sizes,
                            axis,
                            *child,
                            child_offset,
                            child_length,
                        );
                    }
                }
                return;
            }
            (ElementType::Stack { .. }, Some(children)) => {
                let child_offset = offset + e.margins[axis] as i32;
                let child_length = length - e.margins[axis] as i32 - e.margins[axis + 2] as i32;
                for child in children {
                    Layout::calculate_rectangles_r(
                        &mut rectangles,
                        &elements,
                        &children_lists,
                        &minimal_sizes,
                        axis,
                        *child,
                        child_offset,
                        child_length,
                    );
                }
                return;
            }
            (ElementType::Horizontal { padding, .. }, Some(children))
            | (ElementType::Vertical { padding, .. }, Some(children)) => {
                let element_axis = match e.typ {
                    ElementType::Horizontal { .. } => AXIS_HORIZONTAL,
                    _ => AXIS_VERTICAL,
                };
                if element_axis == axis {
                    let available_length =
                        length - (e.margins[axis] as i32 + e.margins[axis + 2] as i32);
                    let mut expanding_count = 0;
                    let mut total_fixed_length: Position = 0;
                    let is_root = element == 0;
                    for &child_index in children {
                        let child = &elements[child_index as usize];
                        if child.expanding || is_root {
                            expanding_count += 1;
                        }
                        let child_fixed_length = minimal_sizes[axis][child_index as usize];
                        total_fixed_length += child_fixed_length as Position;
                        total_fixed_length += padding as Position;
                    }
                    if !children.is_empty() {
                        total_fixed_length -= padding as Position;
                    }

                    let fixed_length = total_fixed_length;
                    let mut free_space_left = max(0, available_length - fixed_length);

                    let mut expanding_left = expanding_count;
                    let mut position = e.margins[axis] as Position;
                    for &child_index in children {
                        let child = &elements[child_index as usize];
                        let child_fixed_length =
                            minimal_sizes[axis][child_index as usize] as Position;
                        let mut child_length = child_fixed_length;
                        if child.expanding || is_root {
                            let free_delta = if expanding_left != 0 {
                                free_space_left / expanding_left
                            } else {
                                0
                            };
                            child_length += free_delta;
                            free_space_left -= free_delta;
                            expanding_left -= 1;
                        }
                        Layout::calculate_rectangles_r(
                            &mut rectangles,
                            &elements,
                            &children_lists,
                            &minimal_sizes,
                            axis,
                            child_index,
                            offset + position,
                            child_length,
                        );
                        position += child_length;
                        position += padding as i32;
                    }
                } else {
                    for &child_index in children {
                        let child_offset = offset + e.margins[axis] as i32;
                        let child_length =
                            length - e.margins[axis] as i32 - e.margins[axis + 2] as i32;
                        Layout::calculate_rectangles_r(
                            &mut rectangles,
                            &elements,
                            &children_lists,
                            &minimal_sizes,
                            axis,
                            child_index,
                            child_offset,
                            child_length,
                        );
                    }
                }
            }
            (_, _) => {}
        }
    }
}

impl LayoutElement {
    fn new() -> Self {
        Self {
            parent: INVALID_ELEMENT_INDEX,
            min_size: [0, 0],
            typ: ElementType::FixedSize,
            expanding: false,
            focus_flags: FocusFlags::NotFocusable,
            scroll: [0, 0],
            margins: [1, 1, 1, 1],
        }
    }
}

impl Default for DrawItem {
    fn default() -> Self {
        Self {
            offset: [0, 0],
            element_index: INVALID_ELEMENT_INDEX,
            clip: usize::MAX,
            dragged: false,
            color: [255, 255, 255, 255],
            command: DrawCommand::None,
        }
    }
}

impl Area {
    fn new() -> Self {
        Self {
            scroll_to_time: -1.0,
            scroll_area_element: INVALID_ELEMENT_INDEX,
            clip_item_index: usize::MAX,
            ..Default::default()
        }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            margins: Default::default(),
            color: [255, 255, 255, 255],
            def: false,
            frame_type: FrameType::Window,
            offset: [0, 0],
            expand: false,
        }
    }
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            frame: Default::default(),
            text_color: [255, 255, 255, 255],
            content_offset: Default::default(),
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            min_size: [0, 0],
            expand: false,
            color: None,
            progress: 0.5,
            scale: 1.0,
            align: Left,
        }
    }
}

impl<'l> Label<'l> {
    pub fn new(label_id: &'l str) -> Self {
        Self {
            label_id,
            ..Default::default()
        }
    }
}

impl<'l> Button<'l> {
    pub fn new(label_id: &'l str) -> Self {
        Self {
            label_id,
            ..Default::default()
        }
    }

    pub fn with_image(sprite_id: SpriteKey) -> Self {
        Self {
            sprite_id: Some(sprite_id),
            ..Default::default()
        }
    }

    pub fn with_area(id: &'l str) -> Self {
        Self {
            label_id: id,
            for_area: true,
            ..Default::default()
        }
    }
}

impl UIStyle {
    fn get_frame(&self, frame: FrameType) -> &FrameStyle {
        match frame {
            FrameType::Window => &self.window_frame,
            FrameType::ButtonNormal => &self.button_normal.frame,
            FrameType::ButtonHovered => &self.button_hovered.frame,
            FrameType::ButtonPressed => &self.button_pressed.frame,
            FrameType::ButtonDisabled => &self.button_disabled.frame,
            FrameType::HSeparator => &self.hseparator,
            FrameType::VSeparator => &self.vseparator,
            FrameType::ProgressInner => &self.progress_inner,
            FrameType::ProgressOuter => &self.progress_outer,
        }
    }
}

const MAX_FINGERS: usize = 10;
const NUM_VELOCITY_SAMPLES: usize = 60;

impl TouchScroll {
    pub fn new() -> Self {
        Self {
            scrolling: false,
            size: [0, 0],
            offset: vec2(0., 0.),

            range: [0., 0., 100., 100.],

            velocity_sample_period: 0.048,
            minimal_velocity_threshold: 50.,
            scroll_move_threshold: 4,
            width: 1,
            height: 1,

            pressed: false,

            velocity_samples: std::collections::VecDeque::new(),
            velocity_samples_duration: 0.0,

            offset_remainder: vec2(0., 0.),
            velocity: vec2(0., 0.),

            fingers: [TouchFinger {
                start_position: [0, 0],
                last_position: [0, 0],
                last_motion_time: 0.0,
            }; MAX_FINGERS + 1],
            scroll_fingers_down: 0,
        }
    }
    pub fn handle_start_input_event(&mut self, ev: &UIEvent, event_time: f32) -> bool {
        let is_mouse_button = match ev {
            &UIEvent::MouseDown { button, .. } => button == 2 || button == 3,
            _ => false,
        };

        let is_touch = matches!(ev, UIEvent::TouchDown { .. });
        let finger_index = match ev {
            &UIEvent::TouchDown { finger, .. } => finger,
            &UIEvent::TouchMove { finger, .. } => finger,
            &UIEvent::TouchUp { finger, .. } => finger,
            _ => MAX_FINGERS as i32,
        } as usize;
        match ev {
            UIEvent::TouchDown { finger, .. } => {
                self.scroll_fingers_down |= 1 << finger;
            }
            _ => {}
        }
        let pos = match ev {
            &UIEvent::TouchDown { pos, .. } => Some(pos),
            &UIEvent::MouseDown { pos, button, .. } => {
                if button == 2 || button == 3 {
                    Some(pos)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(pos) = pos {
            self.fingers[finger_index].start_position = pos;
            self.fingers[finger_index].last_position = pos;
            self.fingers[finger_index].last_motion_time = event_time;
        }

        if is_mouse_button || is_touch {
            if self.scroll_move_threshold > 0 {
                self.pressed = true;
            } else {
                self.scrolling = true;
            }
            self.velocity = vec2(0., 0.);
            self.velocity_samples.clear();
            return self.scrolling;
        }

        match ev {
            UIEvent::MouseWheel { delta, .. } => {
                let mut v = vec2(0.0, *delta * self.height as f32 / 16.0);
                if self.range[3] - self.range[1] == 0.0 {
                    v[0] = v[1];
                    v[1] = 0.0;
                }
                if self.velocity.dot(v) < 0.0 {
                    self.velocity = vec2(0., 0.);
                }
                self.velocity += vec2(v[0] as f32, v[1] as f32).normalize_or_zero() * 300.0;
                return true;
            }
            _ => {}
        }
        return false;
    }

    pub fn handle_subsequent_input_event(&mut self, ev: &UIEvent, event_time: f32) -> bool {
        if !self.scrolling && !self.pressed {
            return false;
        }

        let finger_index = match ev {
            &UIEvent::TouchDown { finger, .. } => finger,
            &UIEvent::TouchMove { finger, .. } => finger,
            &UIEvent::TouchUp { finger, .. } => finger,
            _ => MAX_FINGERS as i32,
        } as usize;

        match ev {
            &UIEvent::TouchDown { pos, .. } => {
                self.fingers[finger_index].start_position = pos;
                self.fingers[finger_index].last_position = pos;
                self.fingers[finger_index].last_motion_time = event_time;
            }
            _ => {}
        };

        let mut result = false;

        match ev {
            UIEvent::MouseMove { pos: event_pos, .. }
            | UIEvent::TouchMove { pos: event_pos, .. } => {
                if self.fingers[finger_index].last_motion_time != 0.0 {
                    let delta = vec2(
                        (event_pos[0] - self.fingers[finger_index].last_position[0]) as f32,
                        (event_pos[1] - self.fingers[finger_index].last_position[1]) as f32,
                    );

                    if !self.scrolling {
                        let start_delta = vec2(
                            (event_pos[0] - self.fingers[finger_index].start_position[0]) as f32,
                            (event_pos[1] - self.fingers[finger_index].start_position[1]) as f32,
                        );
                        if start_delta.length() >= self.scroll_move_threshold as f32 {
                            self.scrolling = true;
                            self.offset -= start_delta;
                        }
                    } else {
                        // update camera position
                        self.offset -= delta;
                    }

                    // sample velocity for swipe
                    let dt = event_time - self.fingers[finger_index].last_motion_time;
                    self.velocity_samples_duration += dt;
                    self.velocity_samples.push_back((delta, dt));
                    while self.velocity_samples_duration > self.velocity_sample_period {
                        if let Some((_, dt)) = self.velocity_samples.pop_front() {
                            self.velocity_samples_duration -= dt
                        } else {
                            break;
                        }
                    }
                    let (path, time) = self
                        .velocity_samples
                        .iter()
                        .fold((vec2(0., 0.), 0.0), |(path, time), (dl, dt)| {
                            (path + *dl, time + *dt)
                        });
                    self.velocity = if time > 0.0 {
                        path / time
                    } else {
                        vec2(0., 0.)
                    };
                    // minimal velocity theshold
                    if self.velocity.length() < self.minimal_velocity_threshold * 0.001 {
                        self.velocity = vec2(0., 0.);
                    }
                }
                result = true;
            }
            &UIEvent::MouseUp { button, .. } if button == 2 || button == 3 => {
                // test not emulated(ev.type == SDL_MOUSEBUTTONUP && ev.button.which != SDL_TOUCH_MOUSEID &&
                self.scrolling = false;
                self.pressed = false;
                result = true;
            }
            &UIEvent::TouchUp { finger, .. } => {
                self.scroll_fingers_down &= !(1 << finger);
                if self.scroll_fingers_down == 0 {
                    self.scrolling = false;
                    self.pressed = false;
                }
                result = true;
            }
            _ => {}
        }

        let pos = match ev {
            &UIEvent::TouchMove { pos, .. } => Some(pos),
            &UIEvent::TouchUp { pos, .. } => Some(pos),
            &UIEvent::TouchDown { pos, .. } => Some(pos),
            &UIEvent::MouseDown { pos, .. } => Some(pos),
            &UIEvent::MouseUp { pos, .. } => Some(pos),
            &UIEvent::MouseMove { pos, .. } => Some(pos),
            _ => None,
        };

        if let Some(pos) = pos {
            self.fingers[finger_index].last_position = pos;
            self.fingers[finger_index].last_motion_time = event_time;
        }
        result && self.scrolling
    }

    fn update(&mut self, dt: f32) {
        if !self.scrolling {
            self.offset += -self.velocity * dt;
            let clamped_offset = vec2(
                self.offset.x.max(self.range[0]).min(self.range[2]),
                self.offset.y.max(self.range[1]).min(self.range[3]),
            );
            if clamped_offset.x != self.offset.x {
                self.offset.x = clamped_offset.x;
                self.velocity.x = 0.0;
            }
            if clamped_offset.y != self.offset.y {
                self.offset.y = clamped_offset.y;
                self.velocity.y = 0.0;
            }
            self.velocity = self.velocity * 0.5f32.powf(dt / 0.75);
        }
    }
}

fn smooth_cd(value: &mut Vec2, velocity: &mut Vec2, target: Vec2, dt: f32, ease_time: f32) {
    if ease_time == 0.0 {
        *value = target;
        *velocity = vec2(0.0, 0.0);
        return;
    }
    let omega = 2.0 / ease_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = *value - target;
    let temp = (*velocity + change * omega) * dt;
    *velocity = (*velocity - temp * omega) * exp;
    *value = target + (change + temp) * exp;
}
