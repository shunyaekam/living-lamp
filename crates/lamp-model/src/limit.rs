//! L0, the autonomic layer: fixed laws applied to whatever the lower layers produce. Nothing here is learned (principle P1).
//!
//! Output passes through a three-stage smoother (so a violently changing input gives a gently changing light), a rate limit,
//! the brightness ceiling and the colour disc, and a reversal governor, and finally a breathing wave. Smoothing alone cannot
//! bound how often a light changes direction, so the governor does: a parameter may reverse only after a minimum gap, which
//! bounds reversals by construction.

use crate::{Rng, DIMS};
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
    /// The shaped light before breathing and flicker: what the slew limit works from.
    shaped: [f64; DIMS],
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
            shaped: [0.0; DIMS],
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
            self.shaped = want;
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
        let mut next = self.shaped;
        for i in 0..DIMS {
            next[i] += (input[i] - next[i]).clamp(-step, step);
        }
        self.shaped = self.clamp(next);
        self.shaped
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

/// The limiter plus the lamp's two signs of life on the level: a breathing wave and a quick, small flicker.
#[derive(Clone, Debug)]
pub struct Autonomic {
    limit: Limit,
    depth: f64,
    period: f64,
    phase: f64,
    t: f64,
    flicker_depth: f64,
    flicker: [f64; 3],
    /// Mean seconds between spontaneous stirs (0 turns stirring off), and how far a stir lifts the level.
    stir_every: f64,
    stir_boost: f64,
    next_stir: f64,
    excitement: f64,
    stirred: f64,
    rng: Rng,
}

/// Time constant of each of the flicker's three smoothing stages, in seconds: about two shimmers a second.
const FLICKER_TAU: f64 = 0.25;

/// The variance of three cascaded one-pole filters driven by white noise of unit density is 0.1875 / tau; this scales the
/// noise so the flicker has a standard deviation of 1 (checked by a test at two frame rates).
const FLICKER_NORM: f64 = 0.1875;

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
            flicker_depth: 0.0,
            flicker: [0.0; 3],
            stir_every: 0.0,
            stir_boost: 0.0,
            next_stir: f64::INFINITY,
            excitement: 0.0,
            stirred: 0.0,
            rng: Rng::new(0xf11c_c0de),
        }
    }

    /// Set the depth of the breathing and of the flicker, as fractions of the level. Both fade out as the world takes over.
    pub fn set_life(&mut self, breath: f64, flicker: f64) {
        self.depth = breath.max(0.0);
        self.flicker_depth = flicker.max(0.0);
    }

    /// Turn spontaneous stirring on or off: about every `every` seconds the lamp brightens by `boost` (in level units) and
    /// warms a little, then settles, as an ember does when it is poked by the air. `every` of 0 turns it off. Stirs fade out
    /// as the world takes over, like breathing.
    pub fn set_stir(&mut self, every: f64, boost: f64) {
        self.stir_every = every.max(0.0);
        self.stir_boost = boost.max(0.0);
        if self.stir_every == 0.0 {
            self.next_stir = f64::INFINITY;
        } else if !self.next_stir.is_finite() {
            self.next_stir = self.t + self.stir_every * self.rng.uniform().max(0.1);
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
        // the flicker is white noise through three smoothing stages, scaled to a standard deviation of one
        let alpha = 1.0 - (-dt / FLICKER_TAU).exp();
        let mut input = self.rng.normal() * (FLICKER_TAU / (FLICKER_NORM * dt.max(1e-9))).sqrt();
        for stage in &mut self.flicker {
            *stage += alpha * (input - *stage);
            input = *stage;
        }
        let calm = 1.0 - arousal.clamp(0.0, 1.0);
        x[2] *= 1.0 + self.flicker_depth * calm * self.flicker[2];
        // stirring: a random kick of excitement that decays over a few seconds and is felt through a soft attack
        if self.t >= self.next_stir && self.stir_every > 0.0 {
            self.excitement = (self.excitement + 1.0).min(1.6);
            let wait = -(1.0 - self.rng.uniform()).ln() * self.stir_every;
            self.next_stir = self.t + wait.max(8.0);
        }
        self.excitement *= (-dt / 4.0).exp();
        self.stirred += (self.excitement - self.stirred) * (1.0 - (-dt / 0.8).exp());
        let lift = self.stir_boost * calm * self.stirred;
        x[2] += lift;
        // and the colour warms a little toward yellow while it is stirred
        let turn = 0.14 * calm * self.stirred;
        let (s, c) = turn.sin_cos();
        let (a, b) = (x[0] * c - x[1] * s, x[0] * s + x[1] * c);
        x[0] = a;
        x[1] = b;
        // breathing is the lamp at rest: it fades out as the world takes over, so a commanded level is shown faithfully
        x[2] *= 1.0 + self.depth * (1.0 - arousal.clamp(0.0, 1.0)) * self.phase.sin();
        // the governor comes last, so the bound on reversals covers everything that is shown, breathing included
        self.limit.commit(x, arousal)
    }
}
