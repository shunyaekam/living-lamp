//! The purer organism: a person's command and the screen are feeds like any other (setpoint feeds), and the organism is the
//! only thing that drives the lamp. These tests run the whole stack: feeds, model, limits.

mod common;

use common::clocks;
use lamp_model::feeds::{FeedSpec, Feeds};
use lamp_model::limit::{Autonomic, LimitConfig};
use lamp_model::{Config, Model, DIMS};

// Numeric experiments over fixed layouts and series of known length.
#[allow(clippy::indexing_slicing)]
mod run {
    use super::*;

    pub const LIMITS: LimitConfig = LimitConfig {
        tau: 0.4,
        tau_fast: 0.03,
        ceiling: 0.4,
        ceiling_fast: 0.4,
        max_rate: 4.0,
        min_reversal_gap: 0.5,
        gap_fast: 0.1,
    };

    pub struct Stack {
        pub model: Model,
        pub feeds: Feeds,
        pub limits: Autonomic,
        pub t: f64,
        pub adaptive: bool,
    }

    impl Stack {
        pub fn new(seed: u64, adaptive: bool) -> Stack {
            let mut cfg = Config::new(clocks());
            cfg.mean = [0.0, 0.0, 0.2];
            cfg.noise = [0.004; DIMS];
            Stack {
                model: Model::new(cfg, seed),
                feeds: Feeds::new(0.5, 1.0e-6),
                limits: Autonomic::new(LIMITS, 0.0, 6.0),
                t: 0.0,
                adaptive,
            }
        }

        /// Advance one step and return the light shown.
        pub fn step(&mut self, dt: f64) -> [f64; DIMS] {
            let drive = self.feeds.drive(self.t);
            self.model.step_drive(dt, &drive);
            let arousal = if self.adaptive { drive.weight } else { 0.0 };
            self.t += dt;
            self.limits.apply_aroused(dt, self.model.state(), arousal)
        }
    }
}

use run::Stack;

fn person() -> FeedSpec {
    FeedSpec {
        dir: [0.0; DIMS],
        stiffness: 5.0,
        priority: 1.0e6,
        memory: 300.0,
    }
}

#[test]
fn a_persons_command_takes_the_lamp_holds_it_and_hands_it_back() {
    let dt = 0.05;
    let mut s = Stack::new(1, true);
    let id = s.feeds.add(person());
    let target = [0.8, 0.0, 0.3];
    let mut shown = Vec::new();
    for _ in 0..(120.0 / dt) as usize {
        shown.push(s.step(dt));
    }
    s.feeds.set_target(id, target, s.t);
    let at = |s: &Stack| s.t;
    let start = at(&s);
    let mut near = None;
    let mut still = Vec::new();
    let mut later = Vec::new();
    while s.t < start + 3.0 * 3600.0 {
        let x = s.step(dt);
        let since = s.t - start;
        let close = (x[0] - target[0]).abs() < 0.05 && (x[2] - target[2]).abs() < 0.05;
        if close && near.is_none() {
            near = Some(since);
        }
        if (1800.0..2400.0).contains(&since) {
            still.push(close);
        }
        if since > 3.0 * 3600.0 - 600.0 {
            later.push(x[0]);
        }
    }
    let reached = near.unwrap_or(f64::INFINITY);
    let held = still.iter().filter(|c| **c).count() as f64 / still.len() as f64;
    let back = later.iter().sum::<f64>() / later.len() as f64;
    println!("the command reached the target after {reached:.2} s; 30 to 40 minutes on it was held {held:.2} of the time; after 3 hours the colour has gone back to {back:.3} (the target was {})", target[0]);
    assert!(reached < 2.0, "the lamp goes where it is told, quickly");
    assert!(held > 0.99, "and stays there");
    assert!(
        back.abs() < 0.2,
        "and the organism has its own light back, far from the target: {back}"
    );
}

#[test]
fn the_screen_as_a_feed_tracks_quickly_when_the_limits_loosen() {
    let dt = 1.0 / 60.0;
    let screen = FeedSpec {
        dir: [0.0; DIMS],
        stiffness: 20.0,
        priority: 1.0e3,
        memory: 1.0,
    };
    let (red, cyan) = ([1.0, 0.0, 0.4], [-1.0, 0.0, 0.4]);
    let mut results = Vec::new();
    for adaptive in [true, false] {
        let mut s = Stack::new(2, adaptive);
        let id = s.feeds.add(screen);
        let mut trace: Vec<(f64, f64)> = Vec::new();
        let mut next_frame = 0.0;
        let mut switch_at = f64::INFINITY;
        while s.t < 40.0 {
            if s.t >= next_frame {
                let colour = if (s.t / 3.0) as usize % 2 == 0 {
                    red
                } else {
                    cyan
                };
                s.feeds.set_target(id, colour, s.t);
                next_frame += 1.0 / 15.0;
            }
            let x = s.step(dt);
            trace.push((s.t, x[0]));
            if s.t >= 21.0 && switch_at.is_infinite() {
                switch_at = 21.0;
            }
        }
        // the colour switches from red to cyan at 21 s: time for the red component to fall from +0.8 to -0.8
        let first = |level: f64| {
            trace
                .iter()
                .find(|(t, a)| *t >= switch_at && *a <= level)
                .map(|(t, _)| t - switch_at)
        };
        let (t90, tfull) = (
            first(-0.8).unwrap_or(f64::INFINITY),
            first(0.0).unwrap_or(f64::INFINITY),
        );
        println!(
            "limits {}: halfway after {tfull:.2} s, 90% of the way after {t90:.2} s",
            if adaptive {
                "loosen with arousal"
            } else {
                "fixed at rest"
            }
        );
        results.push((tfull, t90));
    }
    assert!(
        results[0].1 < 0.5,
        "tracking is quick enough for ambient mirroring: {results:?}"
    );
    assert!(
        results[1].1 > 2.0 * results[0].1,
        "fixed rest limits would be far too slow: {results:?}"
    );
}

#[test]
fn even_under_a_violently_flickering_screen_reversals_stay_bounded() {
    use lamp_model::Rng;
    let dt = 1.0 / 60.0;
    let screen = FeedSpec {
        dir: [0.0; DIMS],
        stiffness: 20.0,
        priority: 1.0e3,
        memory: 1.0,
    };
    let mut s = Stack::new(3, true);
    let id = s.feeds.add(screen);
    let mut rng = Rng::new(5);
    let mut next_frame = 0.0;
    let mut series: [Vec<f64>; DIMS] = [Vec::new(), Vec::new(), Vec::new()];
    while s.t < 60.0 {
        if s.t >= next_frame {
            let target = [
                rng.uniform() * 2.0 - 1.0,
                rng.uniform() * 2.0 - 1.0,
                rng.uniform() * 0.4,
            ];
            s.feeds.set_target(id, target, s.t);
            next_frame += 1.0 / 15.0;
        }
        let x = s.step(dt);
        for (v, o) in series.iter_mut().zip(x) {
            v.push(o);
        }
    }
    let bound = (1.0 / run::LIMITS.gap_fast) as usize;
    for (i, v) in series.iter().enumerate() {
        let worst = worst_reversals_per_second(v, dt);
        println!("parameter {i}: at most {worst} reversals in any second under random screen frames (bound {bound})");
        assert!(worst <= bound);
    }
}

#[allow(clippy::indexing_slicing)]
fn worst_reversals_per_second(series: &[f64], dt: f64) -> usize {
    let mut flips = Vec::new();
    let mut last = 0.0f64;
    for (n, w) in series.windows(2).enumerate() {
        let d = w[1] - w[0];
        if d.abs() > 1e-9 {
            if last != 0.0 && d.signum() != last.signum() {
                flips.push(n as f64 * dt);
            }
            last = d;
        }
    }
    let mut worst = 0;
    for (i, t) in flips.iter().enumerate() {
        worst = worst.max(flips[i..].iter().take_while(|u| **u < t + 1.0).count());
    }
    worst
}
