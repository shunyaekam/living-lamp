//! The feed layer: how many sources of data become one push on the lamp (principles P2 and P9).
//!
//! Every feed is the same kind of thing: a value, a direction in the lamp's state space, a priority and a memory. The
//! weights of fresh feeds are blended, and the imprint competes with them with a fixed weight, so a feed that has gone quiet
//! fades out and the lamp's own replay takes over. The result is always inside the convex hull of the pushes and the replay.

use crate::DIMS;

/// What a feed is, fixed when it is registered.
#[derive(Clone, Copy, Debug)]
pub struct FeedSpec {
    /// Where in state space the feed pushes, for a value of 1.
    pub dir: [f64; DIMS],
    /// Its weight while fresh. A very large priority gives an agent's feed authority over the others.
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
    at: f64,
    seen: bool,
}

/// A set of registered feeds.
#[derive(Clone, Debug)]
pub struct Feeds {
    feeds: Vec<Feed>,
    rest: f64,
    norm_rate: f64,
}

impl Feeds {
    /// `rest` is the imprint's fixed weight as a competitor; `norm_rate` is how fast a feed's running mean and spread adapt
    /// per update (so a constant feed becomes zero and does not bias the lamp forever).
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
            at: 0.0,
            seen: false,
        });
        self.feeds.len() - 1
    }

    /// A new raw reading from feed `id` at time `now`. Non-finite readings are ignored.
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

    /// The blended push at time `now`, and the weight `0..1` it should have against the lamp's own replay.
    pub fn push(&self, now: f64) -> ([f64; DIMS], f64) {
        let mut sum = [0.0; DIMS];
        let mut total = 0.0;
        for f in self.feeds.iter().filter(|f| f.seen) {
            let c = f.spec.priority * (-(now - f.at).max(0.0) / f.spec.memory.max(1e-9)).exp();
            total += c;
            for (s, d) in sum.iter_mut().zip(f.spec.dir) {
                *s += c * f.value * d;
            }
        }
        if total <= 0.0 {
            return ([0.0; DIMS], 0.0);
        }
        (sum.map(|s| s / total), total / (total + self.rest))
    }
}
