//! Helpers shared by the experiments.
#![allow(dead_code, clippy::indexing_slicing)]

use lamp_model::{geometric, Config, Model, DIMS};
use std::f64::consts::TAU;

pub const DT: f64 = 1.0;

pub fn clocks() -> Vec<f64> {
    geometric(100.0, 3200.0, 1.26)
}

/// A feed pushing the first parameter with a sine of the given period, scaled so the driven amplitude is about 1.
pub fn push_at(t: f64, period: f64, rate: f64) -> [f64; DIMS] {
    let w = TAU / period;
    let amp = (rate * rate + w * w).sqrt();
    [amp * (w * t).sin(), 0.0, 0.0]
}

/// Amplitude of the component of `x` at `period` (a lock-in over whole periods).
pub fn amplitude(x: &[f64], dt: f64, period: f64) -> f64 {
    let w = TAU / period;
    let (mut c, mut s) = (0.0, 0.0);
    for (n, v) in x.iter().enumerate() {
        let t = n as f64 * dt;
        c += v * (w * t).cos();
        s += v * (w * t).sin();
    }
    2.0 * c.hypot(s) / x.len() as f64
}

pub struct Run {
    pub driven: f64,
    pub free: Vec<f64>,
    pub locked: f64,
    pub norm: f64,
}

/// Drive for `periods_on` periods, then let go and measure the amplitude of each following period.
pub fn experiment(
    cfg: Config,
    period: f64,
    periods_on: usize,
    periods_off: usize,
    seed: u64,
) -> Run {
    let rate = cfg.rate[0];
    let mut m = Model::new(cfg, seed);
    let n = period.round() as usize;
    let mut t = 0.0;
    let mut last = Vec::new();
    for p in 0..periods_on {
        let mut x = Vec::with_capacity(n);
        for _ in 0..n {
            m.step(DT, Some(push_at(t, period, rate)));
            x.push(m.state()[0]);
            t += DT;
        }
        if p + 1 == periods_on {
            last = x;
        }
    }
    let driven = amplitude(&last, DT, period);
    let norm = m.imprint_norm();
    let mut free = Vec::new();
    for _ in 0..periods_off {
        let mut x = Vec::with_capacity(n);
        for _ in 0..n {
            m.step(DT, None);
            x.push(m.state()[0]);
        }
        free.push(amplitude(&x, DT, period));
    }
    let target = TAU / period;
    let nearest = m
        .omegas()
        .iter()
        .copied()
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
        .unwrap_or(target);
    Run {
        driven,
        free,
        locked: nearest / target,
        norm,
    }
}

pub fn quiet() -> Config {
    let mut c = Config::new(clocks());
    c.noise = [0.0; DIMS];
    c
}
