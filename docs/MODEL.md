# The model behind the lamp (draft 8, for review)

Status: **design, with a first simulation** (`crates/lamp-model`, section 9). It replaces drafts 1 to 7. Statements marked **[ASSUMPTION]** are not established.

## 1. The idea

An organism with four layers, in order of how fast they change:

```
L0  AUTONOMIC     fixed laws: smooth transitions, breathing, limits.             never learned
L1  SPONTANEOUS   a random process that keeps it alive on its own.              the default
L2  ALIGNMENT     feed data pulls the organism along, for a while.              fast, temporary
L3  IMPRINT       what it was pulled through changes L1, so its "random"        slow, lasting
                  behaviour afterwards carries traces of that experience.
```

Everything outside the lamp is a feed: weather, music, a screen, an agent, the sun. Nothing in the model knows what a
day is, what the sun is or what a clock says. **The frame of reference is the lamp itself**: its light and its own
clocks. An imprint says "when I looked like this, at this point of my own rhythm, the world pulled me this way".

**A true organism: everything is a stream into it, and it is the only thing that drives the lamp.** Weather, music, an
agent, the screen and a person's command are all feeds. There is no side door to the lamp: a person's command is a very
stiff, very high-priority feed with a long memory, and the screen is a stiff feed that refreshes many times a second.
Lampbus is the organism's motor layer (Bluetooth, protocol, reliability) and nothing else decides what the light does.

```
the world and the person  ─ feeds ─▶  organism (this model)  ─ light ─▶  lampbus  ─▶  lamp
(weather, music, agent, screen,       L2 alignment, L1 spontaneous,      UDP stream
 commands from Siri, CLI, app)        L3 imprint, then the L0 limits     input
```

## 2. The state

**The light.** One variable per continuous parameter the lamp can control, taken from the lamp's definition. For this
lamp the wire has four numbers (red, green, blue, brightness) but only three independent degrees of freedom, so the
state is `x = (a, b, v)`: colour as a point on a disc, `(a, b) = saturation * (cos hue, sin hue)`, and level `v`. A disc
has no seam where hue wraps from 359 to 0, so the dynamics are smooth. A lamp with a colour-temperature control gets one
more variable. On/off and firmware effects are commands, not state. The state is the light, so there is no separate
readout to design.

**Its own clocks.** `M` internal phases, each advancing by itself: `d phi_k = omega_k dt` (plus a little phase noise). They
are the lamp's own time, as an object has its own proper time in physics, not the sun's and not UTC. A single dimension
cannot oscillate, so a rhythm needs a second dimension; a clock supplies it. The speeds `omega_k` are not told: they
start spread geometrically across timescales **[ASSUMPTION: from minutes to about a week]** and are entrained by what
drives the lamp (section 4, L2).

## 3. Why the laws contain no clock

The laws of an isolated system do not depend on what time it is (time-translation symmetry). A law with explicit time
dependence means something outside is acting on the system. So the lamp's equations are autonomous, time appears only as
the clock variables `phi_k` that the lamp carries itself, and any rhythm in the world, including the sun, reaches the lamp
as a feed.

## 4. The laws

Write `z = (x, cos phi_k, sin phi_k for k = 1..M)` for everything the lamp can refer to about itself.

```
L1  dx = ( -K (x - m) + replay(z) ) dt + sigma dW            replay(z) = sum_j theta_j g_j(z)      (no feed present)
L2  dx = ( -K (x - m) + push(t) ) dt + sigma dW              push = sum_feeds B_f u_f(t)           (feed present)
    d phi_k = omega_k dt + entrainment(state, k) dt          (only while a feed is present)
L3  d theta = eps * ( push - replay(z) ) * g(z) dt           (only while a feed is present)
            - theta / H dt                                    (slow forgetting)
L0  light = Limit( x ) + breathing(t)
```

- `theta = 0` at birth, so a newborn lamp is the mean-reverting random process of L1 with clocks that tick.
- **While a feed is present the world's push drives the lamp and replaces the imprint; when it is silent the imprint is
  replayed.** The total forcing is the same in both modes, so a lamp that has learned a feed does on its own what the
  feed made it do. Because `-K (x - m)` pulls back, alignment fades when the feed stops.
- **The imprint is a forward model of the world's pull:** least-squares regression of the push on the lamp's own state
  and phases. `eps` is small, so only repeated patterns leave a mark. (Draft 4 regressed the observed drift minus the
  imprint, `dx/dt - f`. Working the maths for the simulator showed that this equals the push itself, so the imprint
  would grow for as long as a feed lasts and never converge. The version above converges to the average push.)
- A periodic feed carries the state round a loop while the clocks lock to it. After it is gone the lamp keeps travelling
  the loop at the clocks' learned speed: a learned limit cycle with no outside time.
- `Limit` and `breathing` are L0: a three-stage smoother, a rate limit, the brightness ceiling and colour disc, a fixed
  breathing wave, and a **reversal governor** (a parameter may turn round only after a minimum gap, so reversals are
  bounded by construction; smoothing alone cannot bound them). They are applied to whatever the lower layers produce and
  are never learned.
- **Coherence gate.** Imprint for a clock learns only as much as the push is coherent with that clock across periods
  (a slow average of the push against the clock, over the push's own size: 1 for a steady rhythm, near 0 for noise). So
  only what repeats leaves a mark.
- **Several feeds, of two kinds.** A *flow* feed pushes along a direction (weather, a mood, a ping). A *setpoint* feed
  says where to be and pulls the state there with a stiffness `k` (a person's command, the screen); its pull joins the
  light's own pull toward its mean, and the step stays exact. Fresh feeds are blended by priority and freshness; the
  imprint competes with a fixed weight, so a feed that has gone quiet fades to the lamp's own replay
  (`forcing = C * drive + (1 - C) * replay`, so the result stays inside the hull of the drives and the replay). Authority
  is priority, not a bypass: a person's or an agent's feed carries a very large priority, and its memory sets how long it
  holds (it holds until `priority * exp(-age / memory)` falls to the imprint's weight, then hands the lamp back
  gradually).
- **L0 loosens when the world drives hard.** The smoothing time constant and the minimum reversal gap move from their rest
  values toward fast ones with *arousal*, which is the drive weight `C`: languid at rest, quick when strongly pulled. The
  reversal bound is 1/gap at rest (three a second) and 1/gap_fast when driven (ten a second).

## 5. Principles, each with a test

**P1. Limits are enforced, never learned (L0).** *Test:* violent random input (values to a million, NaN and infinity)
never breaks the colour disc or the ceiling, never produces a non-finite value, and never makes a parameter reverse
more than three times in any second; a step change always glides.

**P2. Imprint learns only from being driven.** With no feed, `theta` changes only by forgetting. *Test:* run alone for
weeks; `theta` stays at its prior and the clocks keep their speeds.

**P3. Noise leaves little mark.** A feed with no structure relative to the lamp must not imprint. Plain least-squares
fails this; the coherence gate (section 4) passes it. *Test:* a random feed leaves under 15% of what a real rhythm does.

**P4. Rehearsal matters.** One episode leaves a faint trace; repetition builds a lasting one. *Test:* trace strength
grows with the number of episodes and fades with half-life `H` once they stop.

**P5. Persistence.** After repeated driving by a periodic feed and then removing it, the free-running lamp keeps a
rhythm near the driven period, longer than a lamp with no imprint. *Test:* synthetic sinusoids, including a period no
clock started at, to check the clocks really entrain.

**P6. Exact in time.** Stepped with the exact solution for the linear part, so statistics do not depend on the frame
rate. *Test:* long runs at 10 and 60 fps agree in mean, spread and autocorrelation.

**P7. Bounded and observable.** `theta` is norm-bounded, and it and the clock speeds can be printed and wiped.

**P8. Earn every addition.** The clocks, more basis functions or a nonlinearity stay only if they improve behaviour on
data the model has not seen. In particular the clocks are compared against the same model without them, which has only
the light's own state.

**P9. Authority is priority, not a bypass.** A person's command, an agent and the screen are feeds with priorities and
memories; nothing reaches the lamp except through the organism. *Test:* a command reaches its target quickly, holds,
and hands the lamp back; a feed with a huge priority takes the lamp from the others.

## 6. The ledger of assumptions

1. The number of clocks and their starting speeds (section 2).
2. The basis `g`: start with the state itself and a small set of smooth bumps and harmonics of the clocks. The family
   limits what can be imprinted.
3. Noise is Gaussian, and the stiffness `K` and spread `sigma` are set per mood.
4. Each feed is given a fixed direction `B_f` in state space when it is registered, and feeds are normalised by running
   statistics so a constant feed does not bias the lamp forever. The normaliser's memory is itself a timescale: it must
   be much longer than any rhythm the lamp should learn, or it filters that rhythm out. The model does not learn what
   a feed means.
5. The L0 limits and the breathing wave. They are about the lamp, not the model.
6. `eps`, `H`, the norm bound of `theta`, and the coherence gate's rate and floor: how fast it learns and forgets and
   how coherent a push must be. Set once, then tuned by replay tests.
7. The priorities, stiffnesses and memories of the feeds, and the imprint's competing weight.
8. The fast values of the L0 limits (smoothing and reversal gap when strongly driven).

## 7. Where this is weakest

- **Entrainment** works in the simulation with a phase-locked loop on the part of the state near each clock's
  frequency, but only for rhythms the clock ladder reaches, and only on synthetic feeds.
- A small basis may not make a loop sharp enough for a "sunrise"; that would need more state or a richer basis (P8).
- Several feeds are blended by fixed priorities (section 4). Whether priorities should instead be learned is untested.
- **Availability.** Because the organism is the only path to the lamp, if it stops, commands stop working until it is
  restarted. Mitigations that keep the architecture pure: the service is supervised and restarts itself within seconds,
  lampbus keeps the last light when the stream goes quiet (so the lamp is never left dark by a crash), and a human can
  turn the organism off with one switch and use lampbus directly. That switch is for a person, not a reflex path.
- **Mirroring lags a little.** Through the organism the screen is tracked to 90% in about 0.47 s, against roughly 0.1 to
  0.15 s for the direct stream the screen app uses today.
- **The reversal cap is looser when driven** (ten a second, against three at rest). Whether that flickers on real video is
  untested on the real lamp.
- Stability: the mean reversion in `K` must dominate the learned part of `f`.

## 8. Build order

1. ~~A pure, deterministic simulator of the model~~ (done: `crates/lamp-model`).
2. ~~The tests of P1 to P9, the with-and-without-clocks comparison, the coherence gate, setpoint feeds and arousal-scaled
   limits~~ (done, section 9).
3. ~~Make the organism the only path to the lamp~~ (done, section 11): `apps/organism`, lampbus's `controller` option and
   `lampbus direct`, tested live.
4. The first feeds that matter: weather and music (the screen, an agent and a person's commands are already feeds). The
   imprint has so far been learned only from synthetic feeds; it will be learned from a person's commands and the screen
   as they happen.

## 9. What the first simulation showed

Run with `cargo test -p lamp-model -- --nocapture`. Time is scaled (clock periods from 100 s to about 4000 s, mean
reversion over 600 s); the model has no absolute time unit, so only the ratios matter. Everything below is a result on
synthetic feeds, not on the real lamp.

**Held:**
- **P5, persistence.** A sine feed of period 1500 s, which matches no clock (they sit at 1270 s and 1600 s), is learned:
  a clock locks to within 0.04% of its speed after 30 periods of driving, and for the eight periods measured after the feed stops the lamp keeps oscillating at about
  the driven amplitude (1.06 to 1.2 times it). The control (no learning, no entrainment) falls to 25% in the first
  period and 2% in the second.
- **Capture.** Sweeping the feed period from 0.85 to 1.18 times a clock's, a clock locks every time. The loop (a
  second-order phase-locked loop on the strongest oscillation of the state) is a workable form of entrainment.
- **Limits of reach.** A feed of period 9000 s, beyond every clock, is not followed (5% retained). The lamp learns only
  rhythms its clocks reach, as section 2 says.
- **P6, frame rate.** Spread and autocorrelation agree at 0.1 s, 1 s and 6 s steps.
- **P2.** Alone, the imprint stays exactly zero and the clocks do not change speed.
- **P4.** The trace grows with repetition (2.4 times larger after eight episodes than after one) and, with
  no feed, halves in one half-life. The growth is slower than a first-order guess because forgetting competes with
  learning: the trace settles at `learning / (learning + forgetting)` of full size, so `H` must be much longer than the
  gap between episodes.

**Did not hold as first written:**
- **P3, noise leaves no mark.** False for plain least-squares. A random feed of the same size leaves a trace that
  falls with learning rate (0.87, 0.50, 0.46 and 0.35 of the real rhythm's at four rates) but does not vanish. Slower
  learning helps, at the cost of slower imprinting. Rejecting noise properly needs a rule that learns only what repeats
  coherently across periods (a gate), which is the intent of "only repetition leaves a lasting mark". Open.
- The replayed amplitude is 1.06 to 1.2 times the driven one, not exactly 1: the imprint spreads across neighbouring
  clocks that lock to the same feed. Harmless for light, but not exact.

**Added after the first run:**
- **P8, the clocks earn their place.** Over 12 seeds with noise, the clock model keeps the learned rhythm steady
  (retained amplitude 0.83 to 1.07 of the driven one, median 0.99, twelve periods after the feed stops). A model with no
  clocks, whose imprint is a function of the light's state alone, sometimes holds it but is only marginally stable
  (0.13 to 5.33, median 1.48). The clocks are kept. (Both models were given a second state dimension that follows the
  first; a one-dimensional state cannot oscillate at all.)
- **The coherence gate works.** A random feed leaves 0.073 of a real rhythm's imprint with the gate against 0.50 without,
  while the real rhythm's own imprint is unchanged (0.00313 against 0.00302), and persistence (P5) still holds with it.
- **P1.** Under violent random input (values to a million, NaN, infinity) the colour and level never leave their limits,
  stay finite, and no parameter reverses more than three times in any second. Before the reversal governor the same
  input gave nine or ten reversals a second: smoothing alone cannot bound them.
- **P9 and the feed layer.** Fresh feeds blend by priority (2:1 gives the weighted average), a quiet feed fades to the
  lamp's own replay (weight 0.80 fresh, 0.0002 after ten memories), a feed with a huge priority takes the lamp, a
  constant feed normalises to zero, and non-finite readings are ignored.
- **The whole stack.** A sine feed enters the feed layer, drives the model, is imprinted, and after the feed stops the
  light shown through the L0 limits keeps oscillating at the feed's period with amplitude about 1.06 of the driven one.
- **Alone.** An hour with no feed, the lamp keeps moving on its own (level 0.19 with spread 0.04 about its mean).
- **A lesson about the normaliser.** The first whole-stack test saw almost no push, because the feed normaliser started
  at variance 1 and adapted too fast or too slowly for the rhythm. Its memory must be much longer than the rhythms it
  should pass (ledger item 4).

**The purer architecture, in the simulator:**
- **A person's command is a feed.** A stiff, high-priority setpoint with a 300 s memory reaches its target in 0.60 s,
  holds it (100% of the time between 30 and 40 minutes later), and gives the lamp back to the organism over the next
  hours.
- **The screen is a feed.** Targets arriving at 15 per second reach 90% of a red-to-cyan switch in 0.47 s when the limits
  loosen with arousal, and 2.17 s if the limits stayed at their rest values: the arousal scaling is what makes the screen
  usable through the organism.
- **Limits still hold when driven.** Under random screen frames no parameter reverses more than nine times in any second
  (bound ten).

**Not yet tested:** the real lamp, feeds that matter (weather, music), and whether the priorities should be learned.

## 10. What the purer architecture requires of lampbus

The organism changes nothing in lampbus's core, but its doors change what they do with a command:
1. **Forward, don't execute.** A command arriving at any door (HTTP, MCP, Matter and so on, so Siri too) is sent to the
   organism as a setpoint feed, and the organism's light comes back over the existing UDP stream input. Lampbus keeps
   everything below that: Bluetooth, the protocol, belief and safety.
2. **Discrete commands are the organism's lifecycle.** Off puts the organism to sleep and powers the lamp off; on wakes it.
   Firmware effects and other discrete commands pass through as actions the organism may issue.
3. **Failure.** If the stream goes quiet, lampbus keeps the last light (it already falls back to the last command after two
   seconds, and a supervisor restarts the organism). A switch in lampbus's config puts it back in direct mode for a person.
4. **Order of work.** The escape switch first, then forwarding behind a flag, then the feed channel in the organism, each
   tested live.

## 11. What running it live showed

Run on the real lamp (an iStrip/SSL strip), through lampbus's `controller` option. Seven problems appeared that no simulation
could have shown; each is now fixed and has a test.

- **A command was shown with a breathing wobble.** A person asking for 80% got 70% to 90%. Breathing is the lamp at rest, so
  it now fades out as arousal rises, and a commanded level is shown steadily.
- **"Let go" took ten minutes.** The light's own pull back to its mean is slow. Release is now a gentle two-minute pull to
  the organism's resting light, after which it wanders again.
- **After an effect the next colour was ignored for an hour.** lampbus pauses live streams after a person's command, and a
  relayed effect counts as one. A power-on ends the pause, so taking the lamp back sends one. Handing the lamp back with
  `lampbus direct off` also ends it (a new core command).
- **After a lampbus restart the organism went to sleep.** Streamed frames counted as touching the lamp, which made it
  "known but off". Streams no longer do, so an untouched lamp stays unknown, and an awake organism re-asserts power-on by
  itself (the lamp was back within three seconds). It starts asleep so it never lights a dark room at login.
- **Home apps showed a stale colour.** The daemon's state ignored streamed light. It now shows it, published about once a
  second or at once for a big change, so Siri and Matter follow a lamp that something else drives.
- **A power-on sent while the organism believed it was awake was dropped.** It now follows the lamp's own reported power.
- **The organism's log was silent**, so it was hard to see why it slept. It now logs each message and motor call.

What the live runs confirmed: commands through the organism (`lampbus color blue 30`, off and on) take effect within about a
second; the relayed effect works; the escape switch bypasses the organism at once and hands it back; and the lamp recovers
from a lampbus restart without a command.

**Not yet judged by a person on the real lamp:** how the organism's ember looks over hours (it is much simpler than the
earlier creature: no flicker or stirring yet), and how mirroring the screen through the organism feels (about half a second
slower than the direct stream).
