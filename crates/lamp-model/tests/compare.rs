//! P8: every addition must earn its place. Is the lamp better with its own clocks than with the light's state alone?
//! And P3: does the coherence gate reject noise?

// Numeric experiments over fixed layouts and series of known length.
#![allow(clippy::indexing_slicing)]

mod common;

use common::{clocks, experiment, push_at, DT};
use lamp_model::{Basis, Config, Model, DIMS};
use std::f64::consts::TAU;

const SEEDS: u64 = 12;

fn noisy(basis: Basis) -> Config {
    let mut c = Config::new(clocks());
    c.basis = basis;
    c.noise = [0.00577; DIMS]; // spread 0.1 against a driven amplitude of 1
    c.link = 1.0 / 300.0; // the second parameter follows the first: a second dimension for a rhythm to live in
    if basis == Basis::State {
        c.bound = f64::INFINITY;
    }
    c
}

fn summary(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(f64::total_cmp);
    (v[0], v[v.len() / 2], v[v.len() - 1])
}

#[test]
fn p8_clocks_against_the_light_alone() {
    let period = 1500.0;
    let mut table = Vec::new();
    for (name, cfg) in [
        ("clocks", noisy(Basis::Clocks)),
        ("state only", noisy(Basis::State)),
    ] {
        let (mut early, mut late) = (Vec::new(), Vec::new());
        for seed in 1..=SEEDS {
            let r = experiment(cfg.clone(), period, 40, 12, seed);
            early.push(r.free.get(2).copied().unwrap_or(0.0) / r.driven);
            late.push(r.free.get(11).copied().unwrap_or(0.0) / r.driven);
        }
        let (e, l) = (summary(&mut early), summary(&mut late));
        println!(
            "{name:<11} retained amplitude after 3 periods: min {:.2} median {:.2} max {:.2}; after 12 periods: min {:.2} median {:.2} max {:.2}",
            e.0, e.1, e.2, l.0, l.1, l.2
        );
        table.push((name, e, l));
    }
    // Claim: the clocks keep the rhythm steady; a rhythm held in the light's state alone is only marginally stable, so its
    // amplitude wanders far more from seed to seed.
    let range = |(lo, _, hi): (f64, f64, f64)| hi - lo;
    let (clocks_late, state_late) = (table[0].2, table[1].2);
    assert!(
        clocks_late.0 > 0.7 && clocks_late.2 < 1.3,
        "clocks hold the rhythm: {clocks_late:?}"
    );
    assert!(
        range(state_late) > 3.0 * range(clocks_late),
        "the light's state alone is far less steady: {state_late:?} against {clocks_late:?}"
    );
}

#[test]
fn p3_the_coherence_gate_against_noise() {
    let period = 1500.0;
    let rate = 1.0 / 600.0;
    let w = TAU / period;
    let size = (rate * rate + w * w).sqrt();
    let mut ratios = Vec::new();
    let mut rhythm = Vec::new();
    for gate in [false, true] {
        let mut cfg = Config::new(clocks());
        cfg.gate = gate;
        cfg.eta = 1.0e-4;
        let steps = (8.0 / cfg.eta) as usize;
        let mut s = Model::new(cfg.clone(), 1);
        let mut n = Model::new(cfg, 5);
        let mut rng = lamp_model::Rng::new(99);
        let (mut u, a) = (0.0, (-1.0f64 / 300.0).exp());
        for k in 0..steps {
            s.step(DT, Some(push_at(k as f64, period, rate)));
            u = a * u + (1.0 - a * a).sqrt() * rng.normal();
            n.step(DT, Some([size * u / 2f64.sqrt(), 0.0, 0.0]));
        }
        println!(
            "gate {gate:<5}: imprint {:.5} (rhythm) and {:.5} (random), ratio {:.3}",
            s.imprint_norm(),
            n.imprint_norm(),
            n.imprint_norm() / s.imprint_norm()
        );
        ratios.push(n.imprint_norm() / s.imprint_norm());
        rhythm.push(s.imprint_norm());
    }
    // Claim: the gate rejects noise and costs the real rhythm almost nothing.
    assert!(ratios[1] < 0.15, "gated noise leaves little: {ratios:?}");
    assert!(
        ratios[1] < 0.3 * ratios[0],
        "and far less than ungated: {ratios:?}"
    );
    assert!(
        rhythm[1] > 0.8 * rhythm[0],
        "the real rhythm is still learned: {rhythm:?}"
    );
}

#[test]
fn p5_still_holds_with_the_gate() {
    let mut cfg = Config::new(clocks());
    cfg.gate = true;
    let r = experiment(cfg, 1500.0, 40, 6, 1);
    println!("gated: driven {:.3}, free-running {:?}", r.driven, r.free);
    assert!((r.locked - 1.0).abs() < 0.02);
    assert!(
        r.free.get(2).is_some_and(|a| *a > 0.5 * r.driven),
        "the rhythm persists"
    );
}
