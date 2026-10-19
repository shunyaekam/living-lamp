//! L0 (limits), the feed layer, and the readout to the lamp: principles P1, P2 and P9.

// Numeric experiments over fixed layouts and series of known length.
#![allow(clippy::indexing_slicing)]

mod common;

use common::{clocks, DT};
use lamp_model::feeds::{FeedSpec, Feeds};
use lamp_model::light::to_light;
use lamp_model::limit::{Autonomic, Limit, LimitConfig};
use lamp_model::{Config, Model, Rng, DIMS};
use std::f64::consts::TAU;

const LIMITS: LimitConfig = LimitConfig {
    tau: 0.4,
    tau_fast: 0.03,
    ceiling: 0.4,
    ceiling_fast: 0.4,
    max_rate: 2.0,
    min_reversal_gap: 0.5,
    gap_fast: 0.1,
};

/// The most direction reversals of `series` inside any window of one second.
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

#[test]
fn p1_limits_hold_whatever_arrives() {
    let dt = 0.05;
    let mut rng = Rng::new(11);
    let mut l = Autonomic::new(LIMITS, 0.1, 6.0);
    let mut outs: [Vec<f64>; DIMS] = [Vec::new(), Vec::new(), Vec::new()];
    for n in 0..40_000 {
        let mut want = [0.0; DIMS];
        for w in &mut want {
            *w = (rng.uniform() - 0.5) * 2.0e6;
        }
        match n % 9 {
            0 => want[0] = f64::NAN,
            3 => want[1] = f64::INFINITY,
            5 => want[2] = f64::NEG_INFINITY,
            _ => {}
        }
        let x = l.apply(dt, want);
        assert!(x.iter().all(|v| v.is_finite()), "finite at step {n}: {x:?}");
        assert!(
            x[0].hypot(x[1]) <= 1.0 + 1e-12,
            "inside the colour disc at step {n}: {x:?}"
        );
        assert!(
            (0.0..=LIMITS.ceiling + 1e-12).contains(&x[2]),
            "level within the ceiling at step {n}: {x:?}"
        );
        for (o, v) in outs.iter_mut().zip(x) {
            o.push(v);
        }
    }
    for (i, o) in outs.iter().enumerate() {
        let r = worst_reversals_per_second(o, dt);
        println!("parameter {i}: at most {r} reversals in any second, under violent random input");
        assert!(r <= 3, "parameter {i} reversed {r} times in a second");
    }
}

#[test]
fn p1_a_sudden_change_always_glides() {
    let mut l = Limit::new(LIMITS);
    l.apply(0.05, [0.0, 0.0, 0.0]);
    let mut prev = 0.0;
    let mut path = Vec::new();
    for _ in 0..200 {
        let y = l.apply(0.05, [0.0, 0.0, 0.4])[2];
        assert!(
            y - prev <= LIMITS.max_rate * 0.05 + 1e-12,
            "no faster than the rate limit"
        );
        path.push(y);
        prev = y;
    }
    assert!(path[0] < 0.01, "the first frame hardly moves: {}", path[0]);
    assert!(
        path.windows(2).all(|w| w[1] >= w[0] - 1e-12),
        "and it only rises"
    );
    assert!(
        (path[199] - 0.4).abs() < 0.02,
        "and it arrives: {}",
        path[199]
    );
}

#[test]
fn readout_maps_state_to_colour_and_level() {
    let red = to_light([1.0, 0.0, 0.5]);
    assert_eq!((red.rgb, red.level), ([255, 0, 0], 50.0));
    let a = (TAU / 3.0).cos();
    let b = (TAU / 3.0).sin();
    assert_eq!(to_light([a, b, 1.0]).rgb, [0, 255, 0]);
    assert_eq!(
        to_light([0.0, 0.0, 0.2]).rgb,
        [255, 255, 255],
        "no saturation is white"
    );
    assert_eq!(to_light([0.5, 0.0, 0.2]).rgb, [255, 128, 128]);
    assert_eq!(
        to_light([9.0, 0.0, 9.0]).level,
        100.0,
        "out-of-range values are bounded"
    );
}

fn spec(dir: [f64; DIMS], priority: f64) -> FeedSpec {
    FeedSpec {
        dir,
        stiffness: 0.0,
        priority,
        memory: 100.0,
    }
}

#[test]
fn p9_fresh_feeds_blend_by_priority_and_stay_inside_their_hull() {
    let mut f = Feeds::new(0.5, 1.0e-2);
    let up = f.add(spec([1.0, 0.0, 0.0], 2.0));
    let down = f.add(spec([-1.0, 0.0, 0.0], 1.0));
    // give each a history, then a fresh reading of the same size
    for i in 0..2000 {
        let v = if i % 2 == 0 { 1.0 } else { -1.0 };
        f.set(up, v, 0.0);
        f.set(down, v, 0.0);
    }
    f.set(up, 1.0, 0.0);
    f.set(down, 1.0, 0.0);
    let (push, weight) = f.push(0.0);
    println!("blended push {push:?}, weight {weight:.3}");
    // up pushes +x with weight 2, down pushes -x with weight 1: the blend is a third of the way up
    assert!(
        (push[0] - 1.0 / 3.0).abs() < 0.05,
        "weighted average: {push:?}"
    );
    assert!(
        (weight - 3.0 / 3.5).abs() < 1e-9,
        "the imprint competes with weight 0.5: {weight}"
    );
    assert!(push[0].abs() <= 1.0 + 1e-9, "inside the hull of the pushes");
}

#[test]
fn p9_a_feed_that_goes_quiet_fades_to_the_lamps_own_replay() {
    let mut f = Feeds::new(0.5, 1.0e-3);
    let id = f.add(spec([1.0, 0.0, 0.0], 2.0));
    f.set(id, 1.0, 0.0);
    f.set(id, -1.0, 0.0);
    let fresh = f.push(0.0).1;
    let stale = f.push(1000.0).1;
    println!("weight fresh {fresh:.3}, after ten memories {stale:.6}");
    assert!(fresh > 0.7 && stale < 1e-3);
}

#[test]
fn p9_an_agent_with_a_huge_priority_takes_the_lamp() {
    let mut f = Feeds::new(0.5, 1.0e-2);
    let agent = f.add(spec([0.0, 1.0, 0.0], 1.0e6));
    let other = f.add(spec([1.0, 0.0, 0.0], 1.0));
    for i in 0..2000 {
        f.set(agent, if i % 2 == 0 { 1.0 } else { -1.0 }, 0.0);
        f.set(other, if i % 2 == 0 { 1.0 } else { -1.0 }, 0.0);
    }
    f.set(agent, 1.0, 0.0);
    f.set(other, 1.0, 0.0);
    let (push, weight) = f.push(0.0);
    assert!(
        push[1] > 0.95 && push[0].abs() < 1e-4 && weight > 0.999,
        "{push:?} {weight}"
    );
}

#[test]
fn a_constant_feed_becomes_zero_and_does_not_bias_the_lamp() {
    let mut f = Feeds::new(0.5, 0.01);
    let id = f.add(spec([1.0, 0.0, 0.0], 1.0));
    for k in 0..5000 {
        f.set(id, 7.0, f64::from(k));
    }
    assert!(
        f.push(4999.0).0[0].abs() < 1e-6,
        "a constant reading carries no information"
    );
    f.set(id, f64::NAN, 5000.0);
    f.set(id, f64::INFINITY, 5001.0);
    assert!(
        f.push(5001.0).0[0].is_finite(),
        "non-finite readings are ignored"
    );
}

#[test]
fn a_feed_through_the_whole_stack_is_imprinted_and_replayed_as_light() {
    // A sine feed enters the feed layer (normalised, weighted), drives the model, and the model's state is shown through
    // the autonomic limits as light. Afterwards the lamp keeps breathing the same rhythm without the feed.
    let period = 1500.0;
    let mut cfg = Config::new(clocks());
    cfg.gate = true;
    let mut m = Model::new(cfg, 1);
    // the normaliser must remember far longer than the rhythm, or it filters the rhythm out
    let mut feeds = Feeds::new(0.01, 1.0e-6);
    let id = feeds.add(spec([0.0045, 0.0, 0.0], 100.0));
    let mut l = Autonomic::new(
        LimitConfig {
            tau: 20.0,
            tau_fast: 1.0,
            ceiling: 1.0,
            ceiling_fast: 1.0,
            max_rate: 1.0,
            min_reversal_gap: 0.5,
            gap_fast: 0.1,
        },
        0.0,
        6.0,
    );
    let n = period as usize;
    let mut t = 0.0;
    for _ in 0..(40 * n) {
        feeds.set(id, (TAU * t / period).sin(), t);
        let (push, weight) = feeds.push(t);
        m.step_weighted(DT, push, weight);
        t += DT;
    }
    let mut shown = Vec::new();
    for _ in 0..(4 * n) {
        m.step_weighted(DT, [0.0; DIMS], 0.0);
        let x = l.apply(DT, m.state());
        shown.push(x[0]);
    }
    let amp = common::amplitude(&shown[2 * n..], DT, period);
    println!("light amplitude at the feed's period, two periods after the feed stopped: {amp:.3}");
    assert!(
        amp > 0.1,
        "the lamp still moves with the learned rhythm: {amp}"
    );
}

#[test]
fn an_unfed_lamp_is_alive_and_lawful() {
    let mut cfg = Config::new(clocks());
    cfg.noise = [0.004; DIMS];
    cfg.mean = [0.0, 0.0, 0.2];
    let mut m = Model::new(cfg, 2);
    let mut l = Autonomic::new(LIMITS, 0.1, 6.0);
    let mut level = Vec::new();
    for _ in 0..36_000 {
        m.step(0.1, None);
        let x = l.apply(0.1, m.state());
        assert!((0.0..=LIMITS.ceiling + 1e-12).contains(&x[2]) && x[0].hypot(x[1]) <= 1.0 + 1e-12);
        level.push(x[2]);
    }
    let mean = level.iter().sum::<f64>() / level.len() as f64;
    let sd = (level.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / level.len() as f64).sqrt();
    println!("an hour alone: mean level {mean:.3}, spread {sd:.3}");
    assert!(sd > 0.005, "it keeps moving on its own");
}

#[test]
fn what_the_lamp_learned_survives_a_restart_and_bad_files_change_nothing() {
    let period = 1500.0;
    let rate = 1.0 / 600.0;
    let mut cfg = Config::new(clocks());
    cfg.gate = true;
    let mut a = Model::new(cfg.clone(), 1);
    let n = period as usize;
    for k in 0..(30 * n) {
        a.step(DT, Some(common::push_at(k as f64, period, rate)));
    }
    let text = a.save();
    let mut b = Model::new(cfg.clone(), 9);
    b.load(&text).unwrap();
    assert!((a.imprint_norm() - b.imprint_norm()).abs() < 1e-9 * a.imprint_norm().max(1.0));
    assert_eq!(a.omegas().len(), b.omegas().len());
    assert!(a
        .omegas()
        .iter()
        .zip(b.omegas())
        .all(|(x, y)| (x - y).abs() < 1e-12));
    // damaged or mismatched text is refused and leaves the lamp as it was
    let before = b.save();
    for bad in [
        "",
        "lamp-model 2\n",
        "lamp-model 1\nclocks 3\nomega 1 2 3\n",
        &text.replace("theta 1", "theta 9"),
        &text.replace("e-", "nan e-"),
    ] {
        assert!(b.load(bad).is_err(), "{bad:?}");
        assert_eq!(b.save(), before);
    }
    let mut c = Config::new(clocks());
    c.periods.truncate(5);
    assert!(
        Model::new(c, 1).load(&text).is_err(),
        "a different number of clocks"
    );
}

#[test]
fn breathing_belongs_to_the_lamp_at_rest_and_fades_as_the_world_takes_over() {
    let cfg = LimitConfig {
        tau: 0.4,
        tau_fast: 0.03,
        ceiling: 1.0,
        ceiling_fast: 1.0,
        max_rate: 4.0,
        min_reversal_gap: 0.5,
        gap_fast: 0.1,
    };
    let spread = |arousal: f64| {
        let mut l = Autonomic::new(cfg, 0.12, 6.0);
        let levels: Vec<f64> = (0..1200)
            .map(|_| l.apply_aroused(0.05, [0.0, 0.0, 0.5], arousal)[2])
            .collect();
        let tail = &levels[600..];
        tail.iter().copied().fold(f64::MIN, f64::max)
            - tail.iter().copied().fold(f64::MAX, f64::min)
    };
    let (rest, driven) = (spread(0.0), spread(1.0));
    println!("level swing over a minute at rest {rest:.3}, fully driven {driven:.6}");
    assert!(rest > 0.05, "at rest the lamp breathes");
    assert!(driven < 0.002, "a commanded level is shown steadily");
}

#[test]
fn the_flicker_is_a_quick_small_shimmer_that_fades_as_the_world_takes_over() {
    let cfg = LimitConfig {
        tau: 0.4,
        tau_fast: 0.03,
        ceiling: 1.0,
        ceiling_fast: 1.0,
        max_rate: 4.0,
        min_reversal_gap: 0.0, // let the governor stay out of the way: this measures the flicker itself
        gap_fast: 0.0,
    };
    for dt in [0.025, 0.05] {
        let mut l = Autonomic::new(cfg, 0.0, 7.0);
        l.set_life(0.0, 0.10);
        let levels: Vec<f64> = (0..(600.0 / dt) as usize)
            .map(|_| l.apply_aroused(dt, [0.0, 0.0, 0.5], 0.0)[2])
            .collect();
        let mean = levels.iter().sum::<f64>() / levels.len() as f64;
        let sd = (levels.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / levels.len() as f64)
            .sqrt()
            / mean;
        println!("dt {dt}: flicker standard deviation {sd:.3} of the level (asked for 0.100)");
        assert!(
            (sd - 0.10).abs() < 0.02,
            "the asked-for depth at dt {dt}: {sd}"
        );
        let mut driven = Autonomic::new(cfg, 0.0, 7.0);
        driven.set_life(0.0, 0.10);
        let held: Vec<f64> = (0..2000)
            .map(|_| driven.apply_aroused(dt, [0.0, 0.0, 0.5], 1.0)[2])
            .collect();
        let spread = held.iter().copied().fold(f64::MIN, f64::max)
            - held.iter().copied().fold(f64::MAX, f64::min);
        assert!(
            spread < 1e-6,
            "no flicker when the world is driving: {spread}"
        );
    }
}

#[test]
fn stirring_happens_at_random_about_as_often_as_asked_and_fades_when_the_world_takes_over() {
    let cfg = LimitConfig {
        tau: 0.4,
        tau_fast: 0.03,
        ceiling: 1.0,
        ceiling_fast: 1.0,
        max_rate: 4.0,
        min_reversal_gap: 0.0,
        gap_fast: 0.0,
    };
    let dt = 0.05;
    let mut l = Autonomic::new(cfg, 0.0, 7.0);
    l.set_stir(60.0, 0.30);
    let hour = (3600.0 / dt) as usize;
    let levels: Vec<f64> = (0..hour)
        .map(|_| l.apply_aroused(dt, [0.8, 0.3, 0.2], 0.0)[2])
        .collect();
    // count the rises of at least half the boost that start from the resting level
    let (mut stirs, mut armed) = (0, true);
    for v in &levels {
        if armed && *v > 0.2 + 0.15 {
            stirs += 1;
            armed = false;
        }
        if *v < 0.2 + 0.05 {
            armed = true;
        }
    }
    println!(
        "{stirs} stirs in an hour, asked for about 60 (a stir is felt as a rise of 0.15 or more)"
    );
    assert!(
        (30..=75).contains(&stirs),
        "about every minute, at random: {stirs}"
    );
    let peak = levels.iter().copied().fold(0.0, f64::max);
    // the soft attack means a stir is felt as about 0.8 of its boost, and two close together add up
    assert!(
        peak > 0.2 + 0.20 && peak < 0.2 + 0.30 * 1.7,
        "a stir lifts the level by about the boost: {peak}"
    );
    // while the world drives, nothing stirs
    let mut driven = Autonomic::new(cfg, 0.0, 7.0);
    driven.set_stir(5.0, 0.30);
    let held: Vec<f64> = (0..hour / 4)
        .map(|_| driven.apply_aroused(dt, [0.8, 0.3, 0.2], 1.0)[2])
        .collect();
    let spread = held.iter().copied().fold(f64::MIN, f64::max)
        - held.iter().copied().fold(f64::MAX, f64::min);
    assert!(spread < 1e-6, "no stirs while driven: {spread}");
}

#[test]
fn the_hue_sways_far_stays_saturated_and_stops_when_the_world_takes_over() {
    let cfg = LimitConfig {
        tau: 0.4,
        tau_fast: 0.03,
        ceiling: 1.0,
        ceiling_fast: 1.0,
        max_rate: 4.0,
        min_reversal_gap: 0.0,
        gap_fast: 0.0,
    };
    let dt = 0.1;
    let want: [f64; 3] = [0.84, 0.30, 0.25];
    let sat = want[0].hypot(want[1]);
    let base = want[1].atan2(want[0]);
    let mut l = Autonomic::new(cfg, 0.0, 7.0);
    l.set_sway(60.0);
    let mut angles = Vec::new();
    for _ in 0..(3.0 * 3600.0 / dt) as usize {
        let x = l.apply_aroused(dt, want, 0.0);
        assert!(
            (x[0].hypot(x[1]) - sat).abs() < 1e-6,
            "the colour stays as saturated as it was: {x:?}"
        );
        let mut d = x[1].atan2(x[0]) - base;
        d = (d + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI;
        angles.push(d.to_degrees());
    }
    let mean = angles.iter().sum::<f64>() / angles.len() as f64;
    let sd = (angles.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / angles.len() as f64).sqrt();
    let (lo, hi) = (
        angles.iter().copied().fold(f64::MAX, f64::min),
        angles.iter().copied().fold(f64::MIN, f64::max),
    );
    println!("hue sway over three hours: standard deviation {sd:.0} degrees (asked for 60), reaching {lo:.0} to {hi:.0}");
    assert!((sd - 60.0).abs() < 18.0, "about the asked-for sway: {sd}");
    assert!(
        hi - lo > 150.0,
        "it travels through many colours: {lo} to {hi}"
    );
    let mut driven = Autonomic::new(cfg, 0.0, 7.0);
    driven.set_sway(60.0);
    for _ in 0..3000 {
        let x = driven.apply_aroused(dt, want, 1.0);
        assert!(
            (x[0] - want[0]).abs() < 1e-6 && (x[1] - want[1]).abs() < 1e-6,
            "no sway while driven"
        );
    }
}
