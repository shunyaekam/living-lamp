//! A deterministic simulator of the lamp's inner model (`docs/MODEL.md`): the spontaneous process (L1), alignment (L2),
//! imprint (L3) and the lamp's own clocks. It has no lamp, no network and no wall clock, so it can be replayed far faster
//! than real time and every run with the same seed is identical.
//!
//! Time appears only as the step length `dt`. A rhythm in the world reaches the model only as a push on the state.

// Numeric code over fixed-size arrays and vectors of equal length; every index is bounded by construction.
#![allow(clippy::indexing_slicing, clippy::needless_range_loop)]

use std::f64::consts::TAU;

/// The number of independent light parameters: colour as a point on a disc, and level.
pub const DIMS: usize = 3;

/// A small deterministic random source (SplitMix64 with Box-Muller normals).
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    spare: Option<f64>,
}

impl Rng {
    /// A source for `seed`.
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: seed,
            spare: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal.
    pub fn normal(&mut self) -> f64 {
        if let Some(s) = self.spare.take() {
            return s;
        }
        let r = (-2.0 * (1.0 - self.uniform()).ln()).sqrt();
        let a = TAU * self.uniform();
        self.spare = Some(r * a.sin());
        r * a.cos()
    }
}

/// Clock periods in a geometric ladder from `lo` to at least `hi`, each `ratio` times the last.
pub fn geometric(lo: f64, hi: f64, ratio: f64) -> Vec<f64> {
    let mut v = vec![lo];
    while v.last().is_some_and(|p| *p < hi) {
        let next = v.last().map_or(lo, |p| p * ratio);
        v.push(next);
    }
    v
}

/// Everything that is set once, not learned. See the ledger in `docs/MODEL.md`.
#[derive(Clone, Debug)]
pub struct Config {
    /// The clocks' starting periods in seconds.
    pub periods: Vec<f64>,
    /// How fast each parameter returns to its mean (per second).
    pub rate: [f64; DIMS],
    /// The mean each parameter returns to.
    pub mean: [f64; DIMS],
    /// The strength of the noise that keeps the lamp alive (per root second).
    pub noise: [f64; DIMS],
    /// How fast the imprint learns (per second).
    pub eta: f64,
    /// Half-life of forgetting, in seconds.
    pub half_life: f64,
    /// The largest the imprint may grow (Frobenius norm).
    pub bound: f64,
    /// Whether the clocks are pulled by what drives the lamp.
    pub entrain: bool,
    /// The demodulator's low-pass rate, as a fraction of the clock's own angular frequency.
    pub bandwidth: f64,
    /// The phase-locked loop's proportional gain, as a fraction of the clock's angular frequency.
    pub loop_gain: f64,
    /// The loop's damping ratio.
    pub damping: f64,
    /// The amplitude below which the loop ignores the state.
    pub floor: f64,
}

impl Config {
    /// Defaults for clocks with the given starting periods.
    pub fn new(periods: Vec<f64>) -> Config {
        Config {
            periods,
            rate: [1.0 / 600.0; DIMS],
            mean: [0.0; DIMS],
            noise: [0.0; DIMS],
            eta: 4.0e-4,
            half_life: 1.0e9,
            bound: 1.0,
            entrain: true,
            bandwidth: 0.15,
            loop_gain: 0.15,
            damping: 0.7,
            floor: 0.02,
        }
    }
}

/// The lamp's inner state: its light, its clocks, and what it has imprinted.
#[derive(Clone, Debug)]
pub struct Model {
    cfg: Config,
    x: [f64; DIMS],
    phase: Vec<f64>,
    omega: Vec<f64>,
    omega0: Vec<f64>,
    ratio: f64,
    /// Demodulated state per clock and parameter: (in-phase, quadrature).
    demod: Vec<[(f64, f64); DIMS]>,
    /// The imprint: for each parameter, a weight per clock feature (cos, sin).
    theta: [Vec<f64>; DIMS],
    rng: Rng,
}

impl Model {
    /// A newborn lamp: its state at the mean, its clocks ticking at their starting speeds, nothing imprinted.
    pub fn new(cfg: Config, seed: u64) -> Model {
        let mut rng = Rng::new(seed);
        let omega0: Vec<f64> = cfg.periods.iter().map(|p| TAU / p).collect();
        let phase = omega0.iter().map(|_| TAU * rng.uniform()).collect();
        let n = cfg.periods.len();
        let ratio = match (cfg.periods.first(), cfg.periods.get(1)) {
            (Some(a), Some(b)) if *a > 0.0 => (b / a).max(1.01),
            _ => 1.5,
        };
        Model {
            x: cfg.mean,
            omega: omega0.clone(),
            omega0,
            phase,
            ratio,
            demod: vec![[(0.0, 0.0); DIMS]; n],
            theta: [vec![0.0; 2 * n], vec![0.0; 2 * n], vec![0.0; 2 * n]],
            cfg,
            rng,
        }
    }

    /// The light parameters now.
    pub fn state(&self) -> [f64; DIMS] {
        self.x
    }

    /// The clocks' current speeds (radians per second).
    pub fn omegas(&self) -> &[f64] {
        &self.omega
    }

    /// The clocks' starting speeds.
    pub fn omegas_at_birth(&self) -> &[f64] {
        &self.omega0
    }

    /// The size of the imprint (Frobenius norm).
    pub fn imprint_norm(&self) -> f64 {
        self.theta
            .iter()
            .flat_map(|t| t.iter())
            .map(|w| w * w)
            .sum::<f64>()
            .sqrt()
    }

    /// What the imprint would push the state by, from the clocks' present phases.
    pub fn replay(&self) -> [f64; DIMS] {
        let mut out = [0.0; DIMS];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.imprint_for(i);
        }
        out
    }

    fn imprint_for(&self, i: usize) -> f64 {
        self.phase
            .iter()
            .enumerate()
            .map(|(k, p)| self.theta[i][2 * k] * p.cos() + self.theta[i][2 * k + 1] * p.sin())
            .sum()
    }

    /// Advance by `dt` seconds. `push` is what the world is doing to the light right now (the sum over feeds of their
    /// direction times their value), or `None` when no feed is present.
    ///
    /// While a feed is present the world's push drives the state and the imprint learns it; otherwise the imprint is
    /// replayed. The light always returns toward its mean, and the noise keeps it moving.
    pub fn step(&mut self, dt: f64, push: Option<[f64; DIMS]>) {
        let before = self.x;
        let g = self.replay();
        let drive = push.unwrap_or(g);
        for i in 0..DIMS {
            let k = self.cfg.rate[i].max(1e-12);
            let e = (-k * dt).exp();
            let gain = (1.0 - e) / k;
            let sd = self.cfg.noise[i] * ((1.0 - e * e) / (2.0 * k)).sqrt();
            self.x[i] = self.cfg.mean[i]
                + e * (before[i] - self.cfg.mean[i])
                + gain * drive[i]
                + sd * self.rng.normal();
        }
        // demodulate what the light is doing against each clock
        for k in 0..self.phase.len() {
            let beta = self.cfg.bandwidth * self.omega[k];
            let (c, s) = (self.phase[k].cos(), self.phase[k].sin());
            for i in 0..DIMS {
                let v = self.x[i] - self.cfg.mean[i];
                let (re, im) = self.demod[k][i];
                self.demod[k][i] = (
                    re + beta * dt * (v * c - re),
                    im + beta * dt * (-v * s - im),
                );
            }
        }
        if let Some(p) = push {
            self.learn(dt, p);
            if self.cfg.entrain {
                self.entrain(dt);
            }
        }
        // forgetting
        let keep = (-std::f64::consts::LN_2 * dt / self.cfg.half_life).exp();
        for t in &mut self.theta {
            for w in t.iter_mut() {
                *w *= keep;
            }
        }
        let norm = self.imprint_norm();
        if norm > self.cfg.bound {
            let scale = self.cfg.bound / norm;
            for t in &mut self.theta {
                for w in t.iter_mut() {
                    *w *= scale;
                }
            }
        }
        for k in 0..self.phase.len() {
            self.phase[k] = (self.phase[k] + self.omega[k] * dt).rem_euclid(TAU);
        }
    }

    /// L3: move the imprint toward the push, as a function of the clocks' phases (least mean squares).
    fn learn(&mut self, dt: f64, push: [f64; DIMS]) {
        for i in 0..DIMS {
            let err = push[i] - self.imprint_for(i);
            for k in 0..self.phase.len() {
                let (c, s) = (self.phase[k].cos(), self.phase[k].sin());
                self.theta[i][2 * k] += self.cfg.eta * err * c * dt;
                self.theta[i][2 * k + 1] += self.cfg.eta * err * s * dt;
            }
        }
    }

    /// L2: a second-order phase-locked loop on each clock, driven by the strongest oscillation of the state near it.
    fn entrain(&mut self, dt: f64) {
        for k in 0..self.phase.len() {
            let (mut best, mut amp) = (0, 0.0);
            for i in 0..DIMS {
                let (re, im) = self.demod[k][i];
                let a = re.hypot(im);
                if a > amp {
                    (best, amp) = (i, a);
                }
            }
            let (_, im) = self.demod[k][best];
            let err = im / (amp + self.cfg.floor);
            let kp = self.cfg.loop_gain * self.omega0[k];
            let ki = (kp / (2.0 * self.cfg.damping)).powi(2);
            self.omega[k] = (self.omega[k] + ki * err * dt)
                .clamp(self.omega0[k] / self.ratio, self.omega0[k] * self.ratio);
            self.phase[k] += kp * err * dt;
        }
    }
}
