use crate::document::View;
use cbmap::{MapMarkup, MarkupPoint, MarkupRect};
use glam::vec2;
use glam::Vec2;
use rimui::UI;
use std::convert::TryInto;

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ZoneRef {
    Point(usize),
    Rect(usize),
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum AnyZone {
    Point(MarkupPoint),
    Rect(MarkupRect),
}

impl ZoneRef {
    fn fetch(&self, markup: &MapMarkup) -> AnyZone {
        match self {
            ZoneRef::Point(i) => AnyZone::Point(markup.points[*i]),
            ZoneRef::Rect(i) => AnyZone::Rect(markup.rects[*i]),
        }
    }
    pub fn remove_zone(&self, markup: &mut MapMarkup) {
        match *self {
            ZoneRef::Point(i) => {
                markup.points.remove(i);
            }
            ZoneRef::Rect(i) => {
                markup.rects.remove(i);
            }
        };
    }
    fn update(&self, markup: &mut MapMarkup, mark: AnyZone) {
        match (self, &mark) {
            (ZoneRef::Point(i), AnyZone::Point(v)) => markup.points[*i] = *v,
            (ZoneRef::Rect(i), AnyZone::Rect(v)) => markup.rects[*i] = *v,
            _ => {
                eprintln!("incompatible mark and ref types");
            }
        }
    }
    pub(crate) fn is_valid(&self, markup: &MapMarkup) -> bool {
        match *self {
            ZoneRef::Point(i) => i < markup.points.len(),
            ZoneRef::Rect(i) => i < markup.rects.len(),
        }
    }
}

pub trait EditorBounds {
    fn bounds(&self, markup: &MapMarkup, view: &View) -> (Vec2, Vec2);
}
pub trait EditorTranslate {
    fn translate(&mut self, delta: [i32; 2]);
}

impl EditorTranslate for AnyZone {
    fn translate(&mut self, delta: [i32; 2]) {
        match self {
            AnyZone::Point(p) => {
                p.pos[0] += delta[0];
                p.pos[1] += delta[1];
            }
            AnyZone::Rect(r) => {
                r.start[0] += delta[0];
                r.start[1] += delta[1];
                r.end[0] += delta[0];
                r.end[1] += delta[1];
            }
        }
    }
}

impl EditorBounds for ZoneRef {
    fn bounds(&self, markup: &MapMarkup, view: &View) -> (Vec2, Vec2) {
        match *self {
            ZoneRef::Point(i) => markup.points[i].bounds(markup, view),
            ZoneRef::Rect(i) => markup.rects[i].bounds(markup, view),
        }
    }
}

impl EditorBounds for MarkupPoint {
    fn bounds(&self, _markup: &MapMarkup, view: &View) -> (Vec2, Vec2) {
        let pos = view
            .world_to_screen()
            .transform_point2(to_vec2(self.pos))
            .floor();
        (pos + vec2(-24., -48.), pos + vec2(24., 0.))
    }
}

impl EditorBounds for MarkupRect {
    fn bounds(&self, _markup: &MapMarkup, view: &View) -> (Vec2, Vec2) {
        (
            view.world_to_screen()
                .transform_point2(to_vec2(self.start))
                .floor()
                - vec2(4.0, 4.0),
            view.world_to_screen()
                .transform_point2(to_vec2(self.end))
                .floor()
                + vec2(4.0, 4.0),
        )
    }
}

fn to_vec2([x, y]: [i32; 2]) -> Vec2 {
    vec2(x as f32, y as f32)
}

fn point_inside(rect: (Vec2, Vec2), point: Vec2) -> bool {
    point.x >= rect.0.x && point.x <= rect.1.x && point.y >= rect.0.y && point.y <= rect.1.y
}

impl AnyZone {
    fn hit_test_zone(markup: &MapMarkup, screen_pos: Vec2, view: &View) -> Vec<ZoneRef> {
        let mut result = Vec::new();
        for r in ((0..markup.rects.len()).map(ZoneRef::Rect))
            .chain((0..markup.points.len()).map(ZoneRef::Point))
        {
            if point_inside(r.bounds(markup, view), screen_pos) {
                result.push(r);
            }
        }
        result
    }

    fn hit_test_zone_corner(
        markup: &MapMarkup,
        screen_pos: Vec2,
        mouse_screen: Vec2,
        view: &View,
    ) -> Option<(ZoneRef, u8)> {
        let hover = Self::hit_test_zone(markup, screen_pos, view)
            .last()
            .copied();
        let hit_distance = 8.;
        if let Some(ZoneRef::Rect(i)) = hover {
            let start = view
                .world_to_screen()
                .transform_point2(to_vec2(markup.rects[i].start))
                .floor();
            let end = view
                .world_to_screen()
                .transform_point2(to_vec2(markup.rects[i].end))
                .floor();
            if (mouse_screen - start).length_squared() <= hit_distance * hit_distance {
                Some((ZoneRef::Rect(i), 0))
            } else if (mouse_screen - end).length_squared() <= hit_distance * hit_distance {
                Some((ZoneRef::Rect(i), 1))
            } else {
                None
            }
        } else {
            None
        }
    }
}
