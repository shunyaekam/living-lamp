//! The feed layer: how many sources of data become one drive on the lamp (principles P2 and P9).
//!
//! Every feed is the same kind of thing: a value or a target, a priority and a memory. There are two kinds. A **flow** feed
//! pushes the lamp along a direction (weather, a mood, a ping). A **setpoint** feed says where to be and pulls the state
//! there with some stiffness (a person's command, the screen). The weights of fresh feeds are blended, and the imprint
//! competes with a fixed weight, so a feed that has gone quiet fades out and the lamp's own replay takes over. A person's
//! command is a very stiff, very high-priority setpoint with a long memory; nothing else about it is special.

use crate::DIMS;

/// What a feed is, fixed when it is registered.
#[derive(Clone, Copy, Debug)]
pub struct FeedSpec {
    /// Where in state space a flow feed pushes, for a value of 1.
    pub dir: [f64; DIMS],
    /// How hard a setpoint feed pulls toward its target, per second. 0 makes the feed a flow feed.
    pub stiffness: f64,
    /// Its weight while fresh. A very large priority gives a person or an agent authority over the others.
    pub priority: f64,
    /// Seconds over which a value goes stale (the weight falls as `exp(-age / memory)`).
    pub memory: f64,
}

#[derive(Clone, Debug)]
struct Feed {
    spec: FeedSpec,
    mean: f64,
    var: f64,
    value: f64,
    target: [f64; DIMS],
    at: f64,
    seen: bool,
}

/// What the feeds ask of the lamp right now.
#[derive(Clone, Copy, Debug, Default)]
pub struct Drive {
    /// The blended push of the flow feeds.
    pub push: [f64; DIMS],
    /// The blended stiffness of the setpoint feeds.
    pub spring: f64,
    /// Where the setpoint feeds, blended, want the state to be.
    pub target: [f64; DIMS],
    /// The weight `0..1` of the world against the lamp's own replay.
    pub weight: f64,
}

/// A set of registered feeds.
#[derive(Clone, Debug)]
pub struct Feeds {
    feeds: Vec<Feed>,
    rest: f64,
    norm_rate: f64,
}

impl Feeds {
    /// `rest` is the imprint's fixed weight as a competitor; `norm_rate` is how fast a flow feed's running mean and spread
    /// adapt per update (so a constant feed becomes zero and does not bias the lamp forever).
    pub fn new(rest: f64, norm_rate: f64) -> Feeds {
        Feeds {
            feeds: Vec::new(),
            rest: rest.max(1e-9),
            norm_rate: norm_rate.clamp(0.0, 1.0),
        }
    }

    /// Register a feed; its handle is returned.
    pub fn add(&mut self, spec: FeedSpec) -> usize {
        self.feeds.push(Feed {
            spec,
            mean: 0.0,
            var: 1.0,
            value: 0.0,
            target: [0.0; DIMS],
            at: 0.0,
            seen: false,
        });
        self.feeds.len() - 1
    }

    /// A new raw reading from the flow feed `id` at time `now`. Non-finite readings are ignored.
    pub fn set(&mut self, id: usize, raw: f64, now: f64) {
        let rate = self.norm_rate;
        let Some(f) = self.feeds.get_mut(id) else {
            return;
        };
        if !raw.is_finite() {
            return;
        }
        if f.seen {
            let d = raw - f.mean;
            f.mean += rate * d;
            f.var += rate * (d * d - f.var);
        } else {
            f.mean = raw;
            f.seen = true;
        }
        f.value = ((raw - f.mean) / (f.var.sqrt() + 1e-9)).clamp(-3.0, 3.0);
        f.at = now;
    }

    /// A new target from the setpoint feed `id` at time `now`. Targets are absolute, not normalised. Non-finite targets are
    /// ignored.
    pub fn set_target(&mut self, id: usize, target: [f64; DIMS], now: f64) {
        let Some(f) = self.feeds.get_mut(id) else {
            return;
        };
        if target.iter().any(|v| !v.is_finite()) {
            return;
        }
        f.target = target;
        f.seen = true;
        f.at = now;
    }

    /// Forget feed `id`'s last value, so it stops pulling at once (an agent that has gone idle, a person who has turned
    /// the lamp off).
    pub fn clear(&mut self, id: usize) {
        if let Some(f) = self.feeds.get_mut(id) {
            f.seen = false;
        }
    }

    /// What the feeds ask for at time `now`.
    pub fn drive(&self, now: f64) -> Drive {
        let mut push = [0.0; DIMS];
        let mut target = [0.0; DIMS];
        let (mut total, mut spring_sum) = (0.0, 0.0);
        for f in self.feeds.iter().filter(|f| f.seen) {
            let c = f.spec.priority * (-(now - f.at).max(0.0) / f.spec.memory.max(1e-9)).exp();
            total += c;
            if f.spec.stiffness > 0.0 {
                spring_sum += c * f.spec.stiffness;
                for (t, v) in target.iter_mut().zip(f.target) {
                    *t += c * f.spec.stiffness * v;
                }
            } else {
                for (p, d) in push.iter_mut().zip(f.spec.dir) {
                    *p += c * f.value * d;
                }
            }
        }
        if total <= 0.0 {
            return Drive::default();
        }
        Drive {
            push: push.map(|p| p / total),
            spring: spring_sum / total,
            target: if spring_sum > 0.0 {
                target.map(|t| t / spring_sum)
            } else {
                [0.0; DIMS]
            },
            weight: total / (total + self.rest),
        }
    }

    /// The blended push of the flow feeds and the weight it should have against the lamp's own replay (setpoint feeds are
    /// not included; use [`Feeds::drive`] for those).
    pub fn push(&self, now: f64) -> ([f64; DIMS], f64) {
        let d = self.drive(now);
        (d.push, d.weight)
    }
}
