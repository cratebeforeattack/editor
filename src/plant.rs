use crate::math::Rect;
use glam::{ivec2, vec2, IVec2, Vec2};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};

use crate::{document::LayerKey, field::Field};

new_key_type! {
    pub struct PlantKey;
    pub struct PlantSegmentKey;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Plant {
    pub pos: IVec2,
    pub dir: Vec2,
    pub material: u8,
    pub layer: LayerKey,

    // design
    pub thickness: f32,
    pub segment_length: f32,
    pub branch_period: f32,
    pub max_length: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PlantSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub start_thickness: f32,
    pub end_thickness: f32,
    pub plant: PlantKey,
}

struct Branch {
    pos: Vec2,
    dir: Vec2,
    start_length: f32,
}

impl Plant {
    pub fn new() -> Self {
        Self {
            pos: ivec2(0, 0),
            dir: vec2(0.0, -1.0),
            segment_length: 8.0,
            branch_period: 128.0,
            thickness: 8.0,
            material: 3,
            layer: LayerKey::default(),
            max_length: 1024.0,
        }
    }

    pub fn grow_plants(
        _field: &Field,
        plants: &SlotMap<PlantKey, Plant>,
        segments: &mut SlotMap<PlantSegmentKey, PlantSegment>,
    ) {
        segments.clear();
        println!("Growing plants...");

        for (plant_k, plant) in plants {
            let mut branch_stack = Vec::new();
            branch_stack.push(Branch {
                pos: plant.pos.as_vec2(),
                dir: plant.dir,
                start_length: 0.0,
            });
            while let Some(Branch {
                mut pos,
                mut dir,
                start_length: mut length,
            }) = branch_stack.pop()
            {
                let last_branch_length = length;
                let mut last_pos = pos;
                let mut dir = dir.normalize();
                while length < plant.max_length {
                    let next_branch = last_branch_length + plant.branch_period;
                    let last_length = length;
                    length += plant.segment_length;
                    pos += dir * plant.segment_length;
                    dir = bend_dir(dir, 0.0025 * std::f32::consts::PI);
                    println!("l {length} {dir}");
                    if last_length < next_branch && length >= next_branch {
                        branch_stack.push(Branch {
                            pos,
                            dir: bend_dir(dir, 0.1 * std::f32::consts::PI),
                            start_length: length,
                        });
                        branch_stack.push(Branch {
                            pos,
                            dir: bend_dir(dir, -0.1 * std::f32::consts::PI),
                            start_length: length,
                        });
                    }
                    segments.insert(PlantSegment {
                        start: last_pos,
                        end: pos,
                        start_thickness: plant.thickness,
                        end_thickness: plant.thickness,
                        plant: plant_k,
                    });
                    last_pos = pos;
                }
            }
        }

        println!("Grown {} segments", segments.len());
    }
}

impl PlantSegment {
    pub(crate) fn bounds(&self) -> [Vec2; 2] {
        [
            self.start - Vec2::splat(self.start_thickness),
            self.start + Vec2::splat(self.start_thickness),
        ]
        .union([
            self.end - Vec2::splat(self.end_thickness),
            self.end + Vec2::splat(self.end_thickness),
        ])
    }
}

fn bend_dir(d: Vec2, angle: f32) -> Vec2 {
    let mx = vec2(angle.cos(), angle.sin());
    let my = mx.perp();
    vec2(mx.dot(d), my.dot(d)).normalize()
}
