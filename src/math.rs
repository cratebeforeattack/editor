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
