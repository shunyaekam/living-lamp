//! The readout from the model's state to what a lamp is told: a full-chroma colour and a separate level, the way the
//! lampbus UDP input takes them.

use crate::DIMS;
use std::f64::consts::TAU;

/// What to send to the lamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// Hue and saturation at full value.
    pub rgb: [u8; 3],
    /// Intensity percent (not rounded).
    pub level: f64,
}

/// The state `(a, b, v)` as a lamp colour: hue is the angle of `(a, b)`, saturation its length, and `v` the level.
pub fn to_light(x: [f64; DIMS]) -> Light {
    let hue = x[1].atan2(x[0]).rem_euclid(TAU) / TAU * 6.0;
    let sat = x[0].hypot(x[1]).clamp(0.0, 1.0);
    let sector = hue.floor();
    let f = hue - sector;
    let (p, q, t) = (1.0 - sat, 1.0 - sat * f, 1.0 - sat * (1.0 - f));
    let (r, g, b) = match sector as u8 % 6 {
        0 => (1.0, t, p),
        1 => (q, 1.0, p),
        2 => (p, 1.0, t),
        3 => (p, q, 1.0),
        4 => (t, p, 1.0),
        _ => (1.0, p, q),
    };
    Light {
        rgb: [r, g, b].map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8),
        level: x[2].clamp(0.0, 1.0) * 100.0,
    }
}
