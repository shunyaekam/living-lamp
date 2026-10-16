//! The three colour conversions the screen analysis needs, copied from lampbus-render so this app has no code
//! dependency on the lampbus core.

/// RGB (each `0..=1`) to HSV (each `0..=1`).
pub fn rgb_to_hsv(r: f64, g: f64, b: f64) -> [f64; 3] {
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h / 6.0, if mx == 0.0 { 0.0 } else { d / mx }, mx]
}

/// sRGB-encoded value (`0..=1`) to linear light (`0..=1`).
pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light (`0..=1`) to sRGB-encoded value (`0..=1`).
pub fn linear_to_srgb(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
