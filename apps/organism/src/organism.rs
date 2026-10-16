//! The organism: the model, its feeds and its limits, with the rules for what each message does. It is pure state: it
//! takes messages and time and returns the light to show and the commands for the motor, so it can be tested without a
//! lamp, a socket or a clock.

use crate::messages::{Agent, Msg, Power};
use crate::motor;
use lamp_model::feeds::{FeedSpec, Feeds};
use lamp_model::light::{to_light, Light};
use lamp_model::limit::{Autonomic, LimitConfig};
use lamp_model::{geometric, Config, Model, DIMS};

/// L0: the autonomic limits. Gentle at rest, quick and unrestricted when a person or the screen is driving.
pub const LIMITS: LimitConfig = LimitConfig {
    tau: 0.4,
    tau_fast: 0.03,
    ceiling: 0.4,
    ceiling_fast: 1.0,
    max_rate: 4.0,
    min_reversal_gap: 0.5,
    gap_fast: 0.1,
};

/// The organism's own nature when nothing drives it: a slow ember that breathes. Everything else is learned from feeds.
pub fn config() -> Config {
    let mut c = Config::new(geometric(600.0, 700_000.0, 1.26));
    c.mean = [0.80, 0.29, 0.12];
    c.noise = [0.003, 0.003, 0.001];
    c.eta = 1.0e-4;
    c.half_life = 7.0 * 86_400.0;
    c.gate = true;
    c
}

/// Where a colour sits in the model's state: saturation times the hue's cosine and sine, and the level.
pub fn target_of(rgb: [f64; 3], percent: f64) -> [f64; DIMS] {
    let [r, g, b] = rgb.map(|c| c / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let sat = if mx <= 0.0 { 0.0 } else { (mx - mn) / mx };
    let d = mx - mn;
    let hue = if d <= 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * std::f64::consts::PI
        / 3.0;
    [
        sat * hue.cos(),
        sat * hue.sin(),
        (percent / 100.0).clamp(0.0, 1.0),
    ]
}

/// The colour an agent's state pulls the lamp toward, as (hue in degrees, saturation, level). These are choices, not
/// learned: a meaning given to a feed on purpose (see the ledger in `docs/MODEL.md`).
fn agent_look(a: Agent) -> Option<(f64, f64, f64)> {
    match a {
        Agent::Working => Some((210.0, 0.5, 0.30)),
        Agent::Done => Some((130.0, 0.8, 0.35)),
        Agent::NeedsYou => Some((40.0, 0.9, 0.45)),
        Agent::Error => Some((0.0, 1.0, 0.50)),
        Agent::Idle => None,
    }
}

/// The organism.
pub struct Organism {
    model: Model,
    feeds: Feeds,
    limits: Autonomic,
    person: usize,
    screen: usize,
    agent: usize,
    home: usize,
    person_at: Option<f64>,
    person_target: [f64; DIMS],
    awake: bool,
    delegated: bool,
    last_power_cmd: Option<f64>,
    /// What the lamp last reported of its power (see `motor`).
    lamp_power: u8,
    outbox: Vec<String>,
}

impl Organism {
    /// A newborn organism, awake.
    pub fn new(seed: u64) -> Organism {
        let mut feeds = Feeds::new(0.5, 1.0e-6);
        let none = [0.0; DIMS];
        let person = feeds.add(FeedSpec {
            dir: none,
            stiffness: 5.0,
            priority: 1.0e6,
            memory: 300.0,
        });
        let screen = feeds.add(FeedSpec {
            dir: none,
            stiffness: 20.0,
            priority: 1.0e3,
            memory: 1.0,
        });
        let agent = feeds.add(FeedSpec {
            dir: none,
            stiffness: 2.0,
            priority: 1.0e4,
            memory: 60.0,
        });
        // going home: a gentle pull to the organism's own resting light, used when a person lets go
        let home = feeds.add(FeedSpec {
            dir: none,
            stiffness: 0.05,
            priority: 1.0e6,
            memory: 10.0,
        });
        Organism {
            model: Model::new(config(), seed),
            feeds,
            limits: Autonomic::new(LIMITS, 0.12, 7.0),
            person,
            screen,
            agent,
            home,
            person_at: None,
            person_target: [0.0; DIMS],
            awake: true,
            delegated: false,
            last_power_cmd: None,
            lamp_power: motor::UNKNOWN,
            outbox: Vec::new(),
        }
    }

    /// What the model has learned, as text (see [`Model::save`]).
    pub fn save(&self) -> String {
        self.model.save()
    }

    /// Restore what was saved.
    pub fn load(&mut self, text: &str) -> Result<(), String> {
        self.model.load(text)
    }

    /// Commands for the motor (lampbus control objects, as JSON) produced since the last call.
    pub fn take_motor(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outbox)
    }

    /// Start asleep: it wakes when the lamp reports that it is on, so it never lights a dark room by itself.
    pub fn start_asleep(&mut self) {
        self.awake = false;
    }

    /// Whether the organism is awake and driving the lamp.
    pub fn driving(&self) -> bool {
        self.awake && !self.delegated
    }

    fn wake(&mut self, now: f64) {
        // a power-on also ends lampbus's pause on streams, which a relayed effect starts: without it the lamp would
        // ignore the organism for an hour after an effect
        if !self.awake || self.delegated || self.lamp_power != motor::ON {
            self.outbox.push("{\"power\":true}".into());
            self.last_power_cmd = Some(now);
        }
        self.awake = true;
        self.delegated = false;
    }

    fn sleep(&mut self, now: f64) {
        if self.awake {
            self.outbox.push("{\"power\":false}".into());
            self.last_power_cmd = Some(now);
        }
        self.awake = false;
        self.delegated = false;
        self.feeds.clear(self.person);
        self.person_at = None;
    }

    fn person_wants(&mut self, target: [f64; DIMS], now: f64) {
        self.wake(now);
        self.person_target = target;
        self.person_at = Some(now);
        self.feeds.set_target(self.person, target, now);
    }

    /// Act on one message and say what to reply.
    pub fn handle(&mut self, msg: Msg, now: f64) -> Result<String, String> {
        match msg {
            Msg::Look { rgb, percent } => {
                let level = percent.unwrap_or_else(|| self.model.state()[2] * 100.0);
                self.person_wants(target_of(rgb, level), now);
            }
            Msg::Brightness(p) => {
                let fresh = self.person_at.is_some_and(|t| now - t < 3600.0);
                let base = if fresh {
                    self.person_target
                } else {
                    self.model.state()
                };
                self.person_wants([base[0], base[1], (p / 100.0).clamp(0.0, 1.0)], now);
            }
            Msg::Power(Power::On) | Msg::Wake => self.wake(now),
            Msg::Power(Power::Off) | Msg::Sleep => self.sleep(now),
            Msg::Power(Power::Toggle) => {
                if self.awake {
                    self.sleep(now);
                } else {
                    self.wake(now);
                }
            }
            Msg::Release => {
                self.feeds.clear(self.person);
                self.feeds.clear(self.agent);
                self.person_at = None;
                self.feeds.set_target(self.home, config().mean, now);
            }
            Msg::Act(json) => {
                // something that is not a plain light: relay it to the motor and stop streaming so it shows
                self.awake = true;
                self.delegated = true;
                self.feeds.clear(self.person);
                self.person_at = None;
                self.outbox.push(json);
            }
            Msg::Agent(a) => match agent_look(a) {
                Some((hue, sat, level)) => {
                    let h = hue.to_radians();
                    self.feeds
                        .set_target(self.agent, [sat * h.cos(), sat * h.sin(), level], now);
                }
                None => self.feeds.clear(self.agent),
            },
            Msg::Status => {
                let x = self.model.state();
                return Ok(format!(
                    "awake={} delegated={} state=({:.3},{:.3},{:.3}) imprint={:.5}",
                    self.awake,
                    self.delegated,
                    x[0],
                    x[1],
                    x[2],
                    self.model.imprint_norm()
                ));
            }
        }
        Ok("ok".into())
    }

    /// A frame from the screen app.
    pub fn screen(&mut self, rgb: [f64; 3], percent: f64, now: f64) {
        self.feeds
            .set_target(self.screen, target_of(rgb, percent), now);
    }

    /// Follow what the lamp itself reports (a phone app, a hardware timer): ignored just after our own power command.
    pub fn resync(&mut self, lamp: u8, now: f64) {
        self.lamp_power = lamp;
        if lamp == motor::UNTOUCHED
            && self.driving()
            && self.last_power_cmd.map_or(true, |t| now - t >= 8.0)
        {
            // lampbus has just started and knows nothing of the lamp: say that it is on, or it will ignore the stream
            self.outbox.push("{\"power\":true}".into());
            self.last_power_cmd = Some(now);
            return;
        }
        if self.last_power_cmd.is_some_and(|t| now - t < 8.0) {
            return;
        }
        if lamp == motor::ON && !self.awake {
            self.awake = true;
            self.delegated = false;
        } else if lamp == motor::OFF && self.awake {
            self.awake = false;
            self.delegated = false;
        }
    }

    /// Advance by `dt` seconds. Returns the light to show when the organism is driving the lamp.
    pub fn tick(&mut self, dt: f64, now: f64) -> Option<Light> {
        let drive = self.feeds.drive(now);
        self.model.step_drive(dt, &drive);
        let x = self
            .limits
            .apply_aroused(dt, self.model.state(), drive.weight);
        self.driving().then(|| to_light(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(o: &mut Organism, from: f64, seconds: f64) -> (Option<Light>, f64) {
        let dt = 0.05;
        let mut t = from;
        let mut last = None;
        while t < from + seconds {
            last = o.tick(dt, t);
            t += dt;
        }
        (last, t)
    }

    #[test]
    fn colours_map_into_the_state_and_back() {
        for rgb in [
            [255.0, 0.0, 0.0],
            [0.0, 255.0, 0.0],
            [0.0, 0.0, 255.0],
            [255.0, 136.0, 0.0],
            [255.0, 255.0, 255.0],
        ] {
            let l = to_light(target_of(rgb, 60.0));
            for (got, want) in l.rgb.iter().zip(rgb) {
                assert!(
                    (f64::from(*got) - want).abs() <= 2.0,
                    "{rgb:?} came back as {:?}",
                    l.rgb
                );
            }
            assert!((l.level - 60.0).abs() < 1e-9);
        }
        // a dim colour keeps its chroma: the level carries the brightness
        assert_eq!(
            to_light(target_of([128.0, 0.0, 0.0], 30.0)).rgb,
            [255, 0, 0]
        );
    }

    #[test]
    fn a_persons_colour_takes_the_lamp_and_wakes_it() {
        let mut o = Organism::new(1);
        let (_, t) = run(&mut o, 0.0, 30.0);
        o.handle(Msg::Power(Power::Off), t).unwrap();
        assert_eq!(o.take_motor(), vec!["{\"power\":false}".to_string()]);
        assert!(
            !o.driving() && run(&mut o, t, 1.0).0.is_none(),
            "asleep: nothing is shown"
        );
        o.handle(
            Msg::Look {
                rgb: [255.0, 0.0, 0.0],
                percent: Some(80.0),
            },
            t + 1.0,
        )
        .unwrap();
        assert_eq!(
            o.take_motor(),
            vec!["{\"power\":true}".to_string()],
            "a colour wakes the lamp first"
        );
        let (l, _) = run(&mut o, t + 1.0, 3.0);
        let l = l.unwrap();
        assert!(
            l.rgb[0] >= 253 && l.rgb[1] <= 2 && l.rgb[2] <= 2,
            "{:?}",
            l.rgb
        );
        assert!(
            (l.level - 80.0).abs() < 1.0,
            "full brightness is allowed when a person asks: {}",
            l.level
        );
    }

    #[test]
    fn brightness_alone_keeps_the_persons_colour() {
        let mut o = Organism::new(2);
        o.handle(
            Msg::Look {
                rgb: [0.0, 0.0, 255.0],
                percent: Some(50.0),
            },
            0.0,
        )
        .unwrap();
        let (_, t) = run(&mut o, 0.0, 3.0);
        o.handle(Msg::Brightness(90.0), t).unwrap();
        let (l, _) = run(&mut o, t, 3.0);
        let l = l.unwrap();
        assert!(
            l.rgb[0] <= 2 && l.rgb[1] <= 2 && l.rgb[2] >= 253,
            "{:?}",
            l.rgb
        );
        assert!((l.level - 90.0).abs() < 3.0, "{}", l.level);
    }

    #[test]
    fn releasing_lets_the_lamp_ease_back_to_its_own_ember() {
        let mut o = Organism::new(9);
        o.handle(
            Msg::Look {
                rgb: [0.0, 0.0, 255.0],
                percent: Some(60.0),
            },
            0.0,
        )
        .unwrap();
        let (l, t) = run(&mut o, 0.0, 3.0);
        assert!(l.unwrap().rgb[2] >= 253, "held blue");
        o.handle(Msg::Release, t).unwrap();
        let (l, _) = run(&mut o, t, 240.0);
        let l = l.unwrap();
        assert!(
            l.rgb[0] > l.rgb[2],
            "back to its warm self within four minutes: {:?}",
            l.rgb
        );
    }

    #[test]
    fn an_effect_is_relayed_and_the_organism_steps_aside() {
        let mut o = Organism::new(3);
        o.handle(Msg::Act("{\"effect\":\"rainbow\"}".into()), 0.0)
            .unwrap();
        assert_eq!(o.take_motor(), vec!["{\"effect\":\"rainbow\"}".to_string()]);
        assert!(!o.driving(), "it does not stream over the effect");
        o.handle(
            Msg::Look {
                rgb: [255.0, 255.0, 255.0],
                percent: Some(40.0),
            },
            5.0,
        )
        .unwrap();
        assert!(o.driving(), "a new colour takes the lamp back");
        assert_eq!(
            o.take_motor().last(),
            Some(&"{\"power\":true}".to_string()),
            "and clears lampbus's pause on streams, which the effect started"
        );
    }

    #[test]
    fn the_lamp_being_turned_off_elsewhere_puts_it_to_sleep_but_not_right_after_our_own_command() {
        let mut o = Organism::new(4);
        o.resync(motor::OFF, 100.0);
        assert!(
            !o.driving(),
            "a hardware timer or a phone app turned it off"
        );
        o.resync(motor::ON, 101.0);
        assert!(o.driving());
        o.handle(Msg::Power(Power::Off), 200.0).unwrap();
        o.resync(motor::ON, 202.0); // a poll that has not caught up with our command
        assert!(
            !o.driving(),
            "stale polls right after our own command are ignored"
        );
        o.resync(motor::ON, 220.0);
        assert!(o.driving(), "later, the lamp's own report wins");
    }

    #[test]
    fn an_on_command_turns_on_a_lamp_that_lampbus_thinks_is_off_even_if_we_think_we_are_awake() {
        let mut o = Organism::new(10);
        // just after a lampbus restart: we believe we are awake, the lamp reports off, and the poll has not yet put us to sleep
        o.lamp_power = motor::OFF;
        o.handle(Msg::Power(Power::On), 1.0).unwrap();
        assert_eq!(o.take_motor(), vec!["{\"power\":true}".to_string()]);
        // when the lamp is already on, an on command does not touch it again
        o.resync(motor::ON, 50.0);
        o.handle(Msg::Power(Power::On), 51.0).unwrap();
        assert!(o.take_motor().is_empty());
    }

    #[test]
    fn after_a_lampbus_restart_an_awake_organism_says_the_lamp_is_on_and_a_sleeping_one_stays_quiet(
    ) {
        let mut o = Organism::new(11);
        o.resync(motor::UNTOUCHED, 100.0);
        assert_eq!(o.take_motor(), vec!["{\"power\":true}".to_string()]);
        o.resync(motor::UNTOUCHED, 102.0);
        assert!(o.take_motor().is_empty(), "once, not every poll");
        let mut sleeping = Organism::new(12);
        sleeping.start_asleep();
        sleeping.resync(motor::UNTOUCHED, 100.0);
        assert!(
            sleeping.take_motor().is_empty() && !sleeping.driving(),
            "it does not light a dark room by itself"
        );
        sleeping.resync(motor::ON, 101.0);
        assert!(sleeping.driving(), "but it wakes when it sees the lamp on");
    }

    #[test]
    fn alone_it_is_a_gentle_ember_inside_its_limits() {
        let mut o = Organism::new(5);
        let dt = 0.05;
        let mut max_level = 0.0f64;
        let mut t = 0.0;
        for _ in 0..40_000 {
            if let Some(l) = o.tick(dt, t) {
                max_level = max_level.max(l.level);
            }
            t += dt;
        }
        assert!(
            max_level <= 40.0 + 1e-9,
            "its own light stays gentle: {max_level}"
        );
        let l = o.tick(dt, t).unwrap();
        assert!(l.rgb[0] > l.rgb[2], "warm, not blue: {:?}", l.rgb);
    }

    #[test]
    fn the_agent_pulls_the_lamp_and_lets_go_when_idle() {
        let mut o = Organism::new(6);
        o.handle(Msg::Agent(Agent::Error), 0.0).unwrap();
        let (l, t) = run(&mut o, 0.0, 4.0);
        assert_eq!(l.unwrap().rgb, [255, 0, 0]);
        o.handle(Msg::Agent(Agent::Idle), t).unwrap();
        let (l, _) = run(&mut o, t, 600.0);
        assert!(
            l.unwrap().rgb[0] > l.unwrap().rgb[2],
            "back to its own warm light"
        );
    }

    #[test]
    fn what_it_learns_can_be_saved_and_restored() {
        let a = Organism::new(7);
        let text = a.save();
        let mut b = Organism::new(8);
        assert!(b.load(&text).is_ok());
        assert!(b.load("nonsense").is_err());
    }
}
