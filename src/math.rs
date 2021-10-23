use glam::{ivec2, IVec2, Vec2};

pub fn critically_damped_spring(
    value: &mut f32,
    velocity: &mut f32,
    target: f32,
    dt: f32,
    ease_time: f32,
) {
    if ease_time == 0.0 {
        *value = target;
        *velocity = 0.0;
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

pub trait Rect {
    type Scalar;
    type Point;

    fn from_point(p: Self::Point) -> Self;
    fn invalid() -> Self;
    fn zero() -> Self;

    fn valid(&self) -> Option<Self>
    where
        Self: Sized;
    fn intersect(&self, o: Self) -> Option<Self>
    where
        Self: Sized;
    fn union(&self, o: Self) -> Self;
    fn inflate(&self, v: Self::Scalar) -> Self;
    fn contains(&self, o: Self) -> bool;
    fn contains_point(&self, p: Self::Point) -> bool;
    fn size(&self) -> Self::Point;
    fn to_array(&self) -> [Self::Scalar; 4];
}

impl Rect for [Vec2; 2] {
    type Scalar = f32;
    type Point = Vec2;

    fn from_point(p: Vec2) -> Self {
        [p, p]
    }

    fn invalid() -> Self {
        [Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)]
    }
    fn zero() -> Self {
        [Vec2::ZERO, Vec2::ZERO]
    }

    fn valid(&self) -> Option<Self> {
        if self[0].x <= self[1].x && self[0].y <= self[1].y {
            Some(*self)
        } else {
            None
        }
    }

    fn intersect(&self, o: Self) -> Option<Self> {
        [self[0].max(o[0]), self[1].min(o[1])].valid()
    }

    fn union(&self, o: Self) -> Self {
        [self[0].min(o[0]), self[1].max(o[1])]
    }

    fn inflate(&self, v: Self::Scalar) -> Self {
        [self[0] - Vec2::splat(v), self[1] + Vec2::splat(v)]
    }

    fn contains(&self, o: Self) -> bool {
        self.contains_point(self[0]) && self.contains_point(self[1])
    }
    fn contains_point(&self, p: Vec2) -> bool {
        self[0].x <= p.x && p.x <= self[1].x && self[0].y <= p.y && p.y <= self[1].y
    }
    fn size(&self) -> Self::Point {
        self[1] - self[0]
    }
    fn to_array(&self) -> [Self::Scalar; 4] {
        [self[0].x, self[0].y, self[1].x, self[1].y]
    }
}

impl Rect for [IVec2; 2] {
    type Scalar = i32;
    type Point = IVec2;

    fn from_point(p: IVec2) -> Self {
        [p, p + ivec2(1, 1)]
    }

    fn invalid() -> Self {
        [IVec2::splat(i32::MAX), IVec2::splat(i32::MIN)]
    }
    fn zero() -> Self {
        [IVec2::ZERO, IVec2::ZERO]
    }

    fn valid(&self) -> Option<Self> {
        if self[0].x <= self[1].x && self[0].y <= self[1].y {
            Some(*self)
        } else {
            None
        }
    }

    fn intersect(&self, o: Self) -> Option<Self> {
        [self[0].max(o[0]), self[1].min(o[1])].valid()
    }

    fn union(&self, o: Self) -> Self {
        [self[0].min(o[0]), self[1].max(o[1])]
    }

    fn inflate(&self, v: Self::Scalar) -> Self {
        [self[0] - IVec2::splat(v), self[1] + IVec2::splat(v)]
    }

    fn contains(&self, o: Self) -> bool {
        o[0].x >= self[0].x && o[1].x <= self[1].x && o[0].y >= self[0].y && o[1].y <= self[1].y
    }
    fn contains_point(&self, p: IVec2) -> bool {
        self[0].x <= p.x && p.x < self[1].x && self[0].y <= p.y && p.y < self[1].y
    }
    fn size(&self) -> Self::Point {
        self[1] - self[0]
    }
    fn to_array(&self) -> [Self::Scalar; 4] {
        [self[0].x, self[0].y, self[1].x, self[1].y]
    }
}

/*
impl Into<[Vec2; 2]> for Vec2 {
    fn into(self) -> [Vec2; 2] {
        [self, self]
    }
}

impl Into<[IVec2; 2]> for IVec2 {
    fn into(self) -> [IVec2; 2] {
        [self, self]
    }
}
*/
