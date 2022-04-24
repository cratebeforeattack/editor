use crate::document::View;
use cbmap::{
    MapMarkup, MarkupPoint, MarkupPointKind, MarkupRect, MarkupRectKind, MarkupSegment,
    MarkupSegmentKind,
};
use glam::vec2;
use glam::Vec2;
use realtime_drawing::{MiniquadBatch, VertexPos3UvColor};

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ZoneRef {
    Point(usize),
    Rect(usize),
    Segment(usize),
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum AnyZone {
    Point(MarkupPoint),
    Rect(MarkupRect),
    Segment(MarkupSegment),
}

impl ZoneRef {
    pub(crate) fn fetch(&self, markup: &MapMarkup) -> AnyZone {
        match self {
            ZoneRef::Point(i) => AnyZone::Point(markup.points[*i]),
            ZoneRef::Rect(i) => AnyZone::Rect(markup.rects[*i]),
            ZoneRef::Segment(i) => AnyZone::Segment(markup.segments[*i]),
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
            ZoneRef::Segment(i) => {
                markup.segments.remove(i);
            }
        };
    }
    pub(crate) fn update(&self, markup: &mut MapMarkup, mark: AnyZone) {
        match (self, &mark) {
            (ZoneRef::Point(i), AnyZone::Point(v)) => markup.points[*i] = *v,
            (ZoneRef::Rect(i), AnyZone::Rect(v)) => markup.rects[*i] = *v,
            (ZoneRef::Segment(i), AnyZone::Segment(v)) => markup.segments[*i] = *v,
            _ => {
                eprintln!("incompatible mark and ref types");
            }
        }
    }
    pub(crate) fn is_valid(&self, markup: &MapMarkup) -> bool {
        match *self {
            ZoneRef::Point(i) => i < markup.points.len(),
            ZoneRef::Rect(i) => i < markup.rects.len(),
            ZoneRef::Segment(i) => i < markup.segments.len(),
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
            AnyZone::Segment(s) => {
                s.start[0] += delta[0];
                s.start[1] += delta[1];
                s.end[0] += delta[0];
                s.end[1] += delta[1];
            }
        }
    }
}

impl EditorBounds for ZoneRef {
    fn bounds(&self, markup: &MapMarkup, view: &View) -> (Vec2, Vec2) {
        match *self {
            ZoneRef::Point(i) => markup.points[i].bounds(markup, view),
            ZoneRef::Rect(i) => markup.rects[i].bounds(markup, view),
            ZoneRef::Segment(i) => markup.segments[i].bounds(markup, view),
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

impl EditorBounds for MarkupSegment {
    fn bounds(&self, _markup: &MapMarkup, view: &View) -> (Vec2, Vec2) {
        let min_x = self.start[0].min(self.end[0]);
        let max_x = self.start[0].max(self.end[0]);
        let min_y = self.start[1].min(self.end[1]);
        let max_y = self.start[1].max(self.end[1]);
        (
            view.world_to_screen()
                .transform_point2(to_vec2([min_x, min_y]))
                .floor()
                - vec2(4.0, 4.0),
            view.world_to_screen()
                .transform_point2(to_vec2([max_x, max_y]))
                .floor()
                + vec2(4.0, 4.0),
        )
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
    pub(crate) fn hit_test_zone(
        markup: &MapMarkup,
        mouse_screen: Vec2,
        view: &View,
    ) -> Vec<ZoneRef> {
        let mut result = Vec::new();
        for r in ((0..markup.rects.len()).map(ZoneRef::Rect))
            .chain((0..markup.points.len()).map(ZoneRef::Point))
            .chain((0..markup.segments.len()).map(ZoneRef::Segment))
        {
            if point_inside(r.bounds(markup, view), mouse_screen) {
                result.push(r);
            }
        }
        result
    }

    pub(crate) fn get_corner(&self, corner: usize) -> [i32; 2] {
        match self {
            AnyZone::Segment(s) => match corner {
                0 => s.start,
                1 => s.end,
                _ => {
                    panic!("Invalid corner")
                }
            },
            AnyZone::Point(s) => match corner {
                0 => s.pos,
                _ => {
                    panic!("Invalid corner");
                }
            },
            AnyZone::Rect(s) => match corner {
                0 => s.start,
                1 => s.end,
                _ => {
                    panic!("Invalid corner");
                }
            },
        }
    }

    pub(crate) fn update_corner(&mut self, corner: usize, value: [i32; 2]) {
        match self {
            AnyZone::Segment(s) => match corner {
                0 => {
                    s.start = value;
                }
                1 => {
                    s.end = value;
                }
                _ => {
                    panic!("Invalid corner")
                }
            },
            AnyZone::Point(s) => match corner {
                0 => {
                    s.pos = value;
                }
                _ => {
                    panic!("Invalid corner");
                }
            },
            AnyZone::Rect(s) => match corner {
                0 => {
                    s.start = value;
                }
                1 => {
                    s.end = value;
                }
                _ => {
                    panic!("Invalid corner");
                }
            },
        }
    }

    pub(crate) fn hit_test_zone_corner(
        markup: &MapMarkup,
        mouse_screen: Vec2,
        view: &View,
    ) -> Option<(ZoneRef, u8)> {
        let hover = Self::hit_test_zone(markup, mouse_screen, view)
            .last()
            .copied();
        let hit_distance = 8.;
        match hover {
            Some(ZoneRef::Rect(i)) => {
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
            }
            Some(ZoneRef::Segment(i)) => {
                let start = view
                    .world_to_screen()
                    .transform_point2(to_vec2(markup.segments[i].start))
                    .floor();
                let end = view
                    .world_to_screen()
                    .transform_point2(to_vec2(markup.segments[i].end))
                    .floor();
                if (mouse_screen - start).length_squared() <= hit_distance * hit_distance {
                    Some((ZoneRef::Segment(i), 0))
                } else if (mouse_screen - end).length_squared() <= hit_distance * hit_distance {
                    Some((ZoneRef::Segment(i), 1))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn draw_zones(
        batch: &mut MiniquadBatch<VertexPos3UvColor>,
        markup: &MapMarkup,
        view: &View,
        selection: Option<ZoneRef>,
        mouse_screen: Vec2,
    ) {
        let selected_color = [
            TEAM_COLORS[0][0][0],
            TEAM_COLORS[0][0][1],
            TEAM_COLORS[0][0][2],
            255,
        ];
        let neutral_color = [160, 160, 160, 255];
        let hover_color = [
            TEAM_COLORS[0][1][0],
            TEAM_COLORS[0][1][1],
            TEAM_COLORS[0][1][2],
            255,
        ];

        let hover = Self::hit_test_zone(markup, mouse_screen, view)
            .last()
            .copied();
        let hover_corner = Self::hit_test_zone_corner(markup, mouse_screen, view).map(|r| r.1);

        let map_color = |r| {
            if Some(r) == selection {
                selected_color
            } else {
                neutral_color
            }
        };

        let map_outline_color = |r| {
            if Some(r) == hover && hover_corner.is_none() {
                hover_color
            } else {
                [0, 0, 0, 255]
            }
        };

        let world_to_screen = view.world_to_screen();
        for (i, &MarkupRect { kind, start, end }) in markup.rects.iter().enumerate() {
            let r = ZoneRef::Rect(i);
            let v = map_color(r);
            let vo = map_outline_color(r);
            let vh = hover_color;
            let start = world_to_screen.transform_point2(to_vec2(start));
            let end = world_to_screen.transform_point2(to_vec2(end));
            match kind {
                MarkupRectKind::RaceFinish => {
                    batch.geometry.stroke_rect(start, end, 4.0, vo);
                    batch.geometry.fill_circle_aa(
                        start,
                        6.0,
                        16,
                        if Some(r) == hover && hover_corner == Some(0) {
                            vh
                        } else {
                            vo
                        },
                    );
                    batch.geometry.fill_circle_aa(
                        end,
                        6.0,
                        16,
                        if Some(r) == hover && hover_corner == Some(1) {
                            vh
                        } else {
                            vo
                        },
                    );
                    batch.geometry.stroke_rect(start, end, 2.0, v);
                    batch.geometry.fill_circle_aa(start, 4.0, 16, v);
                    batch.geometry.fill_circle_aa(end, 4.0, 16, v);
                }
            }
        }
        for (i, &MarkupSegment { kind, start, end }) in markup.segments.iter().enumerate() {
            let r = ZoneRef::Segment(i);
            let v = map_color(r);
            let vo = map_outline_color(r);
            let vh = hover_color;
            let start = world_to_screen.transform_point2(to_vec2(start));
            let end = world_to_screen.transform_point2(to_vec2(end));
            match kind {
                MarkupSegmentKind::Boost | MarkupSegmentKind::Bounce => {
                    batch.geometry.stroke_line_aa(start, end, 4.0, vo);
                    batch.geometry.fill_circle_aa(
                        start,
                        6.0,
                        16,
                        if Some(r) == hover && hover_corner == Some(0) {
                            vh
                        } else {
                            vo
                        },
                    );
                    batch.geometry.fill_circle_aa(
                        end,
                        6.0,
                        16,
                        if Some(r) == hover && hover_corner == Some(1) {
                            vh
                        } else {
                            vo
                        },
                    );
                    batch.geometry.stroke_line_aa(start, end, 2.0, v);
                    batch.geometry.fill_circle_aa(start, 4.0, 16, v);
                    batch.geometry.fill_circle_aa(end, 4.0, 16, v);
                }
            }
        }
        for (i, &MarkupPoint { kind, pos }) in markup.points.iter().enumerate() {
            let r = ZoneRef::Point(i);
            let pos = world_to_screen.transform_point2(to_vec2(pos));
            let v = map_color(r);
            match kind {
                MarkupPointKind::Start => {
                    let apos = pos - vec2(0., 4.);
                    let arrow_points = [
                        apos,
                        apos + vec2(-24., -24.),
                        apos + vec2(-12., -24.),
                        apos + vec2(-12., -48.),
                        apos + vec2(12., -48.),
                        apos + vec2(12., -24.),
                        apos + vec2(24., -24.),
                    ];
                    batch.geometry.stroke_polyline_aa(
                        &arrow_points,
                        true,
                        4.0,
                        map_outline_color(r),
                    );
                    batch
                        .geometry
                        .stroke_polyline_aa(&arrow_points, true, 2.0, v);
                    batch.geometry.fill_circle_aa(pos, 4.0, 8, v);
                }
            }
        }
    }
}

#[rustfmt::skip]
const TEAM_COLORS: [[[u8; 3]; 4]; 8 + 1] = [
    // UI Text         Light            Middle           Dark
    [[0, 124, 224], [146, 173, 215], [101, 139, 199], [70, 117, 187]],
    [[104, 224, 0], [123, 199, 101], [88, 175, 63], [78, 154, 56]],
    [[224, 124, 0], [249, 192, 119], [241, 167, 75], [238, 150, 40]],
    [[182, 54, 255], [210, 145, 247], [190, 96, 244], [181, 72, 242]],
    [[237, 30, 30], [248, 153, 153], [246, 120, 120], [244, 92, 92]],
    [[255, 255, 73], [249, 247, 179], [240, 235, 108], [233, 225, 34]],
    [[0, 224, 199], [127, 246, 233], [16, 230, 206], [14, 194, 174]],
    [[229, 229, 229], [229, 229, 229], [191, 191, 191], [170, 170, 170]],
    [[60, 60, 60], [91, 91, 91], [70, 70, 70], [48, 48, 48]],
];
