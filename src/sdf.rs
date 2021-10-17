use glam::Vec2;

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
