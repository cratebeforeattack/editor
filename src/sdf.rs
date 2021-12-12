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
pub fn sd_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
    (pa - ba * h).length()
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

pub fn sd_outline(d: f32, half_thickness: f32) -> f32 {
    d.abs() - half_thickness
}

// Distance transform of a 1D grid
// See "Distance Transform of Sampled Functions"
// http://cs.brown.edu/people/pfelzens/dt/
pub fn distance_transform_1d(
    out: &mut [f32],
    input: &[f32],
    envelope: &mut Vec<i32>,
    boundaries: &mut Vec<f32>,
) {
    let len = input.len();
    assert_eq!(out.len(), len);
    envelope.clear();
    envelope.resize(len, 0i32);
    boundaries.clear();
    boundaries.resize(len + 1, 0.0f32);

    let mut rightmost = 0;
    boundaries[0] = f32::MIN;
    boundaries[1] = f32::MAX;
    for i in 1..len {
        let mut s;
        loop {
            let env_r = envelope[rightmost];
            s = ((input[i] + i as f32 * i as f32)
                - (input[env_r as usize] + env_r as f32 * env_r as f32))
                / (2.0 * i as f32 - 2.0 * env_r as f32);
            if s > boundaries[rightmost] {
                break;
            }
            rightmost -= 1;
        }
        rightmost += 1;
        envelope[rightmost] = i as i32;
        boundaries[rightmost] = s;
        boundaries[rightmost + 1] = f32::MAX;
    }

    rightmost = 0;
    for i in 0..len {
        while boundaries[rightmost + 1] < i as f32 {
            rightmost += 1;
        }
        out[i] = (i as i32 - envelope[rightmost]) as f32 * (i as i32 - envelope[rightmost]) as f32
            + input[envelope[rightmost] as usize];
    }
}

pub fn distance_transform(
    image: &[u8],
    w: u32,
    h: u32,
    value_test: impl Fn(u8) -> bool,
) -> Vec<f32> {
    let mut has_pixels = false;
    let mut image_f: Vec<f32> = image
        .iter()
        .cloned()
        .map(|v| {
            if value_test(v) {
                has_pixels = true;
                0.0
            } else {
                f32::MAX
            }
        })
        .collect();

    if !has_pixels {
        return image_f;
    }

    let mut old_row = Vec::new();
    old_row.resize(w as usize, 0.0);

    let mut envelope = Vec::new();
    let mut boundaries = Vec::new();

    // horizontal pass
    for row in image_f.chunks_mut(w as usize) {
        old_row.copy_from_slice(row);
        distance_transform_1d(row, &old_row, &mut envelope, &mut boundaries);
    }

    // vertical pass
    old_row.resize(h as usize, 0.0);
    let mut new_row = vec![0.0; h as usize];
    for x in 0..w {
        for y in 0..h {
            old_row[y as usize] = image_f[(y * w + x) as usize];
        }
        distance_transform_1d(&mut new_row, &old_row, &mut envelope, &mut boundaries);
        for y in 0..h {
            image_f[(y * w + x) as usize] = new_row[y as usize];
        }
    }

    // distance squares to distances
    for d in image_f.iter_mut() {
        *d = d.sqrt();
    }

    image_f
}
