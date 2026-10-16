# The model behind the lamp (draft 4, for review)

Status: **design, nothing built**. It replaces drafts 1 to 3. Statements marked **[ASSUMPTION]** are not established.

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
L1  dx = f(z) dt + sigma dW                         f(z) = -K (x - m) + sum_j theta_j g_j(z)
L2  dx = f(z) dt + sum_feeds B_f u_f(t) dt + sigma dW           (while feeds are present)
    d phi_k = omega_k dt + entrainment(feeds, k)                 (only while feeds are present)
L3  d theta = eps * ( dx/dt - f(z) ) * g(z)         (only while a feed is present)
            - theta / H                              (slow forgetting)
L0  light = Limit( x ) + breathing(t)
```

- `theta = 0` at birth, so a newborn lamp is the mean-reverting random process of L1 with clocks that tick.
- Aligned, the feed's push `B_f u_f` is added to the drift. Because `f` pulls back, alignment fades when the feed stops.
- **Imprint is regression of the observed drift on the lamp's own state and phases.** It learns the average push the
  world gave at each state and phase, so it teaches the free-running lamp to do on its own what the feed made it do.
  `eps` is small, so only repeated patterns leave a mark.
- A periodic feed carries the state round a loop while the clocks lock to it. After it is gone the lamp keeps travelling
  the loop at the clocks' learned speed: a learned limit cycle with no outside time.
- `Limit` and `breathing` are L0: smoothing, brightness ceiling, reversal cap and a fixed breathing wave. They are
  applied to whatever the lower layers produce and are never learned.

## 5. Principles, each with a test

**P1. Limits are enforced, never learned (L0).** *Test:* adversarial feeds (jumps, noise, NaN, huge values) never break
the ceiling, the reversal cap or the glides, in any frame.

**P2. Imprint learns only from being driven.** With no feed, `theta` changes only by forgetting. *Test:* run alone for
weeks; `theta` stays at its prior and the clocks keep their speeds.

**P3. Noise leaves no mark.** A feed with no structure relative to the lamp averages out. *Test:* drive with white
noise; `theta` stays near zero.

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
   statistics so a constant feed does not bias the lamp forever. The model does not learn what a feed means.
5. The L0 limits and the breathing wave. They are about the lamp, not the model.
6. `eps`, `H` and the norm bound of `theta`: how fast it learns and forgets. Set once, then tuned by replay tests.

## 7. Where this is weakest

- **Entrainment is not settled.** How a feed pulls a clock's phase and speed is the main open choice. Candidates: an
  Adler-style coupling on a resonator tuned to each clock, or a phase-locked loop on the part of the state near its
  frequency. It is decided by replay tests (P5), not by taste.
- A small basis may not make a loop sharp enough for a "sunrise"; that would need more state or a richer basis (P8).
- With several feeds at once their pushes add, and nothing yet decides which wins when they disagree.
- Stability: the mean reversion in `K` must dominate the learned part of `f`.

## 8. Build order

1. A pure, deterministic simulator of L1 to L3 and the clocks, with no lamp and no network, replayable faster than real
   time.
2. Tests P2 to P6 and the with-and-without-clocks comparison of P8, on synthetic feeds. If P5 fails, the idea changes
   before anything else is built.
3. L0 and the output to the lamp, reproducing today's calm behaviour from the model alone.
4. The feed channel, then the first real feeds.
