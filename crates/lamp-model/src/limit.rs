//! L0, the autonomic layer: fixed laws applied to whatever the lower layers produce. Nothing here is learned (principle P1).
//!
//! Output passes through a three-stage smoother (so a violently changing input gives a gently changing light), a rate limit,
//! the brightness ceiling and the colour disc, and a reversal governor, and finally a breathing wave. Smoothing alone cannot
//! bound how often a light changes direction, so the governor does: a parameter may reverse only after a minimum gap, which
//! bounds reversals by construction.

use crate::DIMS;
use std::f64::consts::TAU;

/// Limits, set once.
#[derive(Clone, Copy, Debug)]
pub struct LimitConfig {
    /// Time constant of each of the three smoothing stages at rest, in seconds.
    pub tau: f64,
    /// The same when the world is driving the lamp hard (arousal 1): quicker, like an alert animal.
    pub tau_fast: f64,
    /// The highest level the lamp may show at rest, `0..=1`: the organism's own light stays gentle.
    pub ceiling: f64,
    /// The highest level when the world is driving hard (arousal 1): a person who asks for full brightness gets it.
    pub ceiling_fast: f64,
    /// The fastest the light may move, in state units per second.
    pub max_rate: f64,
    /// The least time between two changes of direction of one parameter at rest, in seconds.
    pub min_reversal_gap: f64,
    /// The same when the world is driving the lamp hard (arousal 1).
    pub gap_fast: f64,
}

/// The smoother, rate limit and clamps.
#[derive(Clone, Debug)]
pub struct Limit {
    cfg: LimitConfig,
    stages: [[f64; DIMS]; 3],
    out: [f64; DIMS],
    good: [f64; DIMS],
    started: bool,
    t: f64,
    ceiling_now: f64,
    direction: [f64; DIMS],
    last_flip: [f64; DIMS],
}

impl Limit {
    /// A limiter with the given settings.
    pub fn new(cfg: LimitConfig) -> Limit {
        Limit {
            cfg,
            stages: [[0.0; DIMS]; 3],
            out: [0.0; DIMS],
            good: [0.0; DIMS],
            started: false,
            t: 0.0,
            ceiling_now: cfg.ceiling,
            direction: [0.0; DIMS],
            last_flip: [f64::NEG_INFINITY; DIMS],
        }
    }

    fn clamp(&self, x: [f64; DIMS]) -> [f64; DIMS] {
        let (mut a, mut b) = (x[0].clamp(-1.0, 1.0), x[1].clamp(-1.0, 1.0));
        let r = a.hypot(b);
        if r > 1.0 {
            (a, b) = (a / r, b / r);
        }
        [a, b, x[2].clamp(0.0, self.ceiling_now)]
    }

    /// The light for what is wanted, after `dt` seconds. Non-finite and huge inputs are tamed, never passed on.
    pub fn apply(&mut self, dt: f64, desired: [f64; DIMS]) -> [f64; DIMS] {
        self.apply_aroused(dt, desired, 0.0)
    }

    /// Like [`Limit::apply`], with the limits loosened by `arousal` (0 at rest, 1 when the world is driving hard).
    pub fn apply_aroused(&mut self, dt: f64, desired: [f64; DIMS], arousal: f64) -> [f64; DIMS] {
        let shaped = self.shape(dt, desired, arousal);
        self.commit(shaped, arousal)
    }

    /// Smoothing, rate limit and clamps: the light the lamp would move to, before the reversal governor.
    fn shape(&mut self, dt: f64, desired: [f64; DIMS], arousal: f64) -> [f64; DIMS] {
        let mut want = desired;
        for (w, g) in want.iter_mut().zip(self.good) {
            if !w.is_finite() {
                *w = g;
            }
        }
        let want = self.clamp(want);
        self.good = want;
        if !self.started {
            self.stages = [want; 3];
            self.out = want;
            self.started = true;
        }
        let a = arousal.clamp(0.0, 1.0);
        self.ceiling_now = self.cfg.ceiling * (1.0 - a) + self.cfg.ceiling_fast * a;
        let tau = self.cfg.tau * (1.0 - a) + self.cfg.tau_fast * a;
        let alpha = 1.0 - (-dt / tau.max(1e-9)).exp();
        let mut input = want;
        for stage in &mut self.stages {
            for i in 0..DIMS {
                stage[i] += alpha * (input[i] - stage[i]);
            }
            input = *stage;
        }
        self.t += dt;
        let step = self.cfg.max_rate * dt;
        let mut next = self.out;
        for i in 0..DIMS {
            next[i] += (input[i] - next[i]).clamp(-step, step);
        }
        self.clamp(next)
    }

    /// The reversal governor: a parameter turns round only after the minimum gap since it last did; until then it holds.
    fn commit(&mut self, next: [f64; DIMS], arousal: f64) -> [f64; DIMS] {
        let next = self.clamp(next);
        let a = arousal.clamp(0.0, 1.0);
        let gap = self.cfg.min_reversal_gap * (1.0 - a) + self.cfg.gap_fast * a;
        for i in 0..DIMS {
            let delta = next[i] - self.out[i];
            if delta.abs() <= 1e-12 {
                continue;
            }
            let sign = delta.signum();
            if self.direction[i] != 0.0 && sign != self.direction[i] {
                if self.t - self.last_flip[i] < gap {
                    continue; // too soon to turn round: hold
                }
                self.last_flip[i] = self.t;
            } else if self.direction[i] == 0.0 {
                self.last_flip[i] = self.t;
            }
            self.direction[i] = sign;
            self.out[i] = next[i];
        }
        self.out
    }
}

/// The limiter plus a fixed breathing wave on the level.
#[derive(Clone, Debug)]
pub struct Autonomic {
    limit: Limit,
    depth: f64,
    period: f64,
    phase: f64,
    t: f64,
}

impl Autonomic {
    /// `depth` is the fraction of the level the breathing swings by; `period` its mean length in seconds. The period
    /// wanders slowly so the breath never repeats exactly.
    pub fn new(cfg: LimitConfig, depth: f64, period: f64) -> Autonomic {
        Autonomic {
            limit: Limit::new(cfg),
            depth,
            period,
            phase: 0.0,
            t: 0.0,
        }
    }

    /// The light to show for what the lamp wants, after `dt` seconds.
    pub fn apply(&mut self, dt: f64, desired: [f64; DIMS]) -> [f64; DIMS] {
        self.apply_aroused(dt, desired, 0.0)
    }

    /// Like [`Autonomic::apply`], with the limits loosened by `arousal` (0 at rest, 1 when the world is driving hard).
    pub fn apply_aroused(&mut self, dt: f64, desired: [f64; DIMS], arousal: f64) -> [f64; DIMS] {
        let mut x = self.limit.shape(dt, desired, arousal);
        self.t += dt;
        let wander = 1.0 + 0.15 * (TAU * self.t / (self.period * 7.3)).sin();
        self.phase = (self.phase + TAU * dt / (self.period * wander)).rem_euclid(TAU);
        // breathing is the lamp at rest: it fades out as the world takes over, so a commanded level is shown faithfully
        x[2] *= 1.0 + self.depth * (1.0 - arousal.clamp(0.0, 1.0)) * self.phase.sin();
        // the governor comes last, so the bound on reversals covers everything that is shown, breathing included
        self.limit.commit(x, arousal)
    }
}
