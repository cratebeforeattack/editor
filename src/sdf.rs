use glam::{vec2, Vec2};

// Based on slightly improved version of a Trapezoid by Per Bloksgaard/2020 (MIT License)
#[inline]
pub fn sd_trapezoid(p: Vec2, a: Vec2, b: Vec2, ra: f32, rb: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let baba = ba.dot(ba);
    let x = pa.perp_dot(ba).abs() / baba.sqrt();
    let paba = pa.dot(ba) / baba;
    let rba = rb - ra;
    let cax = (x - (if paba < 0.5 { ra } else { rb })).max(0.0);
    let cay = (paba - 0.5).abs() - 0.5;
    let f = ((rba * (x - ra) + paba * baba) / (rba * rba + baba)).clamp(0., 1.);
    let cbx = x - ra - f * rba;
    let cby = paba - f;
    cbx.max(cay).signum()
        * (cax * cax + cay * cay * baba)
            .min(cbx * cbx + cby * cby * baba)
            .sqrt()
}

// Inigo Quilez, MIT License
#[inline]
pub fn sd_octogon(mut p: Vec2, r: f32) -> f32 {
    let k = [-0.9238795325, 0.3826834323, 0.4142135623];
    p = p.abs();
    p -= 2.0 * vec2(k[0], k[1]).dot(p).min(0.0) * vec2(k[0], k[1]);
    p -= 2.0 * vec2(-k[0], k[1]).dot(p).min(0.0) * vec2(-k[0], k[1]);
    p -= vec2(p.x.clamp(-k[2] * r, k[2] * r), r);
    return p.length() * p.y.signum();
}

#[inline]
pub fn sd_circle(p: Vec2, center: Vec2, r: f32) -> f32 {
    (p - center).length() - r
}

// Inigo Quilez, MIT License
#[inline]
pub fn sd_box(p: Vec2, b: Vec2) -> f32 {
    let d = p.abs() - b;
    return d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0);
}
