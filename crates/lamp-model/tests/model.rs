//! The experiments behind `docs/MODEL.md`, section 5. Each test states the claim it checks.

use lamp_model::{geometric, Config, Model, DIMS};
use std::f64::consts::TAU;

const DT: f64 = 1.0;

fn clocks() -> Vec<f64> {
    geometric(100.0, 3200.0, 1.26)
}

/// A feed pushing the first parameter with a sine of the given period, scaled so the driven amplitude is about 1.
fn push_at(t: f64, period: f64, rate: f64) -> [f64; DIMS] {
    let w = TAU / period;
    let amp = (rate * rate + w * w).sqrt();
    [amp * (w * t).sin(), 0.0, 0.0]
}

/// Amplitude of the component of `x` at `period` (a lock-in over whole periods).
fn amplitude(x: &[f64], dt: f64, period: f64) -> f64 {
    let w = TAU / period;
    let (mut c, mut s) = (0.0, 0.0);
    for (n, v) in x.iter().enumerate() {
        let t = n as f64 * dt;
        c += v * (w * t).cos();
        s += v * (w * t).sin();
    }
    2.0 * c.hypot(s) / x.len() as f64
}

struct Run {
    driven: f64,
    free: Vec<f64>,
    locked: f64,
    norm: f64,
}

/// Drive for `periods_on` periods, then let go and measure the amplitude of each following period.
fn experiment(cfg: Config, period: f64, periods_on: usize, periods_off: usize, seed: u64) -> Run {
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

fn quiet() -> Config {
    let mut c = Config::new(clocks());
    c.noise = [0.0; DIMS];
    c
}

#[test]
fn p6_statistics_do_not_depend_on_the_frame_rate() {
    let mut cfg = Config::new(clocks());
    cfg.noise = [0.00577; DIMS];
    let mut results = Vec::new();
    for dt in [0.1f64, 1.0, 6.0] {
        let mut m = Model::new(cfg.clone(), 7);
        let steps = (2.0e6 / dt) as usize;
        let lag = (300.0 / dt).round() as usize;
        let mut xs = Vec::with_capacity(steps);
        for _ in 0..steps {
            m.step(dt, Some([0.0; DIMS]));
            xs.push(m.state()[1]);
        }
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        let var = xs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / xs.len() as f64;
        let cov = xs
            .iter()
            .zip(xs.iter().skip(lag))
            .map(|(a, b)| (a - mean) * (b - mean))
            .sum::<f64>()
            / (xs.len() - lag) as f64;
        results.push((dt, var, cov / var));
    }
    for (dt, var, rho) in &results {
        println!(
            "dt {dt:>4}: std {:.4}  autocorrelation at 300 s {rho:.4}",
            var.sqrt()
        );
        assert!(
            (var.sqrt() - 0.1).abs() < 0.006,
            "spread at dt {dt}: {}",
            var.sqrt()
        );
        assert!(
            (rho - (-300.0f64 / 600.0).exp()).abs() < 0.04,
            "autocorrelation at dt {dt}: {rho}"
        );
    }
}

#[test]
fn p2_alone_nothing_is_imprinted_and_the_clocks_keep_their_speeds() {
    let mut cfg = Config::new(clocks());
    cfg.noise = [0.00577; DIMS];
    let mut m = Model::new(cfg, 3);
    for _ in 0..2_000_000 {
        m.step(DT, None);
    }
    assert_eq!(m.imprint_norm(), 0.0, "nothing to learn from");
    assert!(m
        .omegas()
        .iter()
        .zip(m.omegas_at_birth())
        .all(|(a, b)| (a - b).abs() < 1e-15));
}

#[test]
fn p5_a_periodic_feed_is_imprinted_and_the_rhythm_persists() {
    let period = 1500.0; // between two clocks (1270 s and 1600 s)
    let r = experiment(quiet(), period, 30, 8, 1);
    println!(
        "driven amplitude {:.3}; free-running amplitude per period {:?}; nearest clock at {:.4} of the feed's speed; imprint {:.4}",
        r.driven,
        r.free.iter().map(|a| (a * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
        r.locked,
        r.norm
    );
    assert!(
        (r.locked - 1.0).abs() < 0.02,
        "a clock locked to the feed: {}",
        r.locked
    );
    assert!(
        r.free.get(2).is_some_and(|a| *a > 0.5 * r.driven),
        "the rhythm is still there three periods after the feed stopped"
    );

    // the control: the same lamp that cannot learn, and whose clocks cannot lock
    let mut control = quiet();
    control.eta = 0.0;
    control.entrain = false;
    let c = experiment(control, period, 30, 8, 1);
    println!("control: driven {:.3}, free-running {:?}", c.driven, c.free);
    assert!(
        c.free.get(2).is_some_and(|a| *a < 0.05 * c.driven),
        "without an imprint the rhythm dies away"
    );
}

#[test]
fn capture_range_is_reported_across_feed_periods() {
    println!("feed period / nearest clock period -> clock speed / feed speed, retained amplitude after 3 periods");
    let periods = clocks();
    for ratio in [0.85, 0.9, 0.95, 1.0, 1.06, 1.12, 1.18] {
        let near = periods.get(12).copied().unwrap_or(1600.0);
        let period = (near * ratio).round();
        let r = experiment(quiet(), period, 40, 4, 2);
        println!(
            "{ratio:>5.2} (T={period:>5}) -> {:.4}, retained {:.2}",
            r.locked,
            r.free.get(2).copied().unwrap_or(0.0) / r.driven
        );
    }
}

#[test]
fn p3_noise_leaves_a_mark_that_shrinks_as_learning_slows() {
    // Drive with a periodic feed and with a random feed of the same size, for the same number of learning time
    // constants, at several learning rates. A random feed cannot leave nothing: it leaves a trace of size about the
    // square root of the learning rate, while a real rhythm converges to a fixed size.
    let period = 1500.0;
    let rate = 1.0 / 600.0;
    let w = TAU / period;
    let size = (rate * rate + w * w).sqrt();
    let mut ratios = Vec::new();
    for eta in [4.0e-4, 1.0e-4, 2.5e-5, 6.0e-6] {
        let steps = (8.0 / eta) as usize;
        let mut cfg = quiet();
        cfg.eta = eta;
        let mut s = Model::new(cfg.clone(), 1);
        let mut n = Model::new(cfg, 5);
        let mut rng = lamp_model::Rng::new(99);
        let (mut u, a) = (0.0, (-1.0f64 / 300.0).exp());
        for k in 0..steps {
            s.step(DT, Some(push_at(k as f64, period, rate)));
            u = a * u + (1.0 - a * a).sqrt() * rng.normal();
            n.step(DT, Some([size * u / 2f64.sqrt(), 0.0, 0.0]));
        }
        let ratio = n.imprint_norm() / s.imprint_norm();
        println!(
            "eta {eta:.1e}: imprint {:.5} (rhythm) and {:.5} (random), ratio {ratio:.3}",
            s.imprint_norm(),
            n.imprint_norm()
        );
        ratios.push(ratio);
    }
    assert!(
        ratios.windows(2).all(|p| p[1] < p[0]),
        "slower learning leaves less noise: {ratios:?}"
    );
    // KNOWN GAP (docs/MODEL.md, section 7): plain least-squares cannot tell noise from a rhythm, so at these rates a random
    // feed still leaves a trace about a third the size of a real rhythm. Rejecting it needs a rule that learns only what
    // repeats (a coherence gate), not just a smaller rate.
    assert!(
        ratios.last().is_some_and(|r| *r < 0.5),
        "noise stays well below the rhythm: {ratios:?}"
    );
}

#[test]
fn p4_rehearsal_builds_the_trace_and_forgetting_fades_it() {
    let period = 1500.0;
    let mut cfg = quiet();
    cfg.eta = 6.0e-5;
    cfg.half_life = 300.0 * period;
    let mut m = Model::new(cfg.clone(), 1);
    let mut t = 0.0;
    let n = period as usize;
    let mut after = Vec::new();
    for _episode in 0..8 {
        for _ in 0..(3 * n) {
            m.step(DT, Some(push_at(t, period, cfg.rate[0])));
            t += DT;
        }
        after.push(m.imprint_norm());
        for _ in 0..(3 * n) {
            m.step(DT, None);
            t += DT;
        }
    }
    println!(
        "imprint after each episode: {:?}",
        after
            .iter()
            .map(|v| (v * 1e5).round() / 1e5)
            .collect::<Vec<_>>()
    );
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    assert!(
        mean(&after[after.len() - 3..]) > mean(&after[..2]),
        "repetition builds the trace: {after:?}"
    );
    assert!(after.last().copied().unwrap_or(0.0) > 2.0 * after.first().copied().unwrap_or(1.0));
    let peak = m.imprint_norm();
    for _ in 0..(300 * n) {
        m.step(DT, None);
    }
    let ratio = m.imprint_norm() / peak;
    println!("after one half-life of silence the imprint is {ratio:.3} of what it was");
    assert!((ratio - 0.5).abs() < 0.05);
}

#[test]
fn a_rhythm_outside_every_clock_is_not_followed() {
    // far slower than the slowest clock can reach (3200 s * 1.26 = about 4000 s)
    let r = experiment(quiet(), 9000.0, 12, 2, 4);
    println!(
        "feed period 9000 s: nearest clock at {:.3} of the feed's speed, retained {:.2} of {:.2}",
        r.locked,
        r.free.first().copied().unwrap_or(0.0),
        r.driven
    );
    assert!((r.locked - 1.0).abs() > 0.2, "no clock can reach it");
}
