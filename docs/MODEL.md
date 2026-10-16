# The model behind the lamp (draft 6, for review)

Status: **design, with a first simulation** (`crates/lamp-model`, section 9). It replaces drafts 1 to 5. Statements marked **[ASSUMPTION]** are not established.

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
- **Several feeds.** Fresh feeds are blended by priority and freshness; the imprint competes with a fixed weight, so a
  feed that has gone quiet fades to the lamp's own replay (`forcing = C * push + (1 - C) * replay`, so the result stays
  inside the hull of the pushes and the replay). An agent's feed has authority by carrying a very large priority.

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

**P9. Authority is a strict order.** A person, then an agent, then the lamp's own dynamics.

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
7. The priorities and memories of the feeds, and the imprint's competing weight.

## 7. Where this is weakest

- **Entrainment** works in the simulation with a phase-locked loop on the part of the state near each clock's
  frequency, but only for rhythms the clock ladder reaches, and only on synthetic feeds.
- A small basis may not make a loop sharp enough for a "sunrise"; that would need more state or a richer basis (P8).
- Several feeds are blended by fixed priorities (section 4). Whether priorities should instead be learned is untested.
- Stability: the mean reversion in `K` must dominate the learned part of `f`.

## 8. Build order

1. ~~A pure, deterministic simulator of the model~~ (done: `crates/lamp-model`).
2. ~~The tests of P1 to P9, the with-and-without-clocks comparison and the coherence gate~~ (done, section 9).
3. Wire the model into the creature: a feed channel (a UDP command that sets a feed's value), the model's state shown
   through the L0 limits as light, and a way to see and wipe what it has learned. This is the first step that touches
   the real lamp, so each part is tested live, one question at a time.
4. The first real feeds: time-free ones first (a ping, a mood from an agent), then weather and music.

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

**Not yet tested:** the real lamp, and feeds that matter (weather, music), and whether the priorities should be learned.
