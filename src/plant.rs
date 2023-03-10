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
    pub parent: PlantKey,
    pub bmin: IVec2,
    pub bmax: IVec2,
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
            segment_length: 4.0,
            branch_period: 32.0,
            thickness: 8.0,
            material: 2,
            layer: LayerKey::default(),
            max_length: 512.0,
        }
    }
    pub fn grow_plants(
        _field: &Field,
        plants: &SlotMap<PlantKey, Plant>,
        _segments: &mut SlotMap<PlantSegmentKey, PlantSegment>,
    ) {
        for (_plant_k, plant) in plants {
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
                while length < plant.max_length {
                    length += plant.segment_length;
                    pos += dir * length;
                    dir = bend_dir(dir, 0.25 * std::f32::consts::PI);
                    if length > last_branch_length + plant.branch_period {
                        branch_stack.push(Branch {
                            pos,
                            dir: bend_dir(dir, 0.25 * std::f32::consts::PI),
                            start_length: length,
                        });
                    }
                }
            }
        }
    }
}

fn bend_dir(d: Vec2, angle: f32) -> Vec2 {
    let mx = vec2(angle.cos(), angle.sin());
    let my = mx.perp();
    vec2(mx.dot(d), my.dot(d))
}
