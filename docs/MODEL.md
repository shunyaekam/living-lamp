# The model behind the lamp (draft 3, for review)

Status: **design, nothing built**. It replaces drafts 1 and 2. Statements marked **[ASSUMPTION]** are not established.

## 1. The idea

An organism with four layers, in order of how fast they change:

```
L0  AUTONOMIC     fixed laws: smooth transitions, breathing, limits.             never learned
L1  SPONTANEOUS   a random process that keeps it alive on its own.              the default
L2  ALIGNMENT     feed data pulls the state along, for a while.                 fast, temporary
L3  IMPRINT       what it was pulled through changes L1, so its "random"        slow, lasting
                  behaviour afterwards carries traces of that experience.
```

Everything outside the lamp is a feed: weather, music, a screen, an agent, the sun. Nothing in the model knows what a
day is, what the sun is, or what a clock says. **The frame of reference is the lamp's own state**: an imprint says "when
I was in this state, the world pulled me this way". Time enters only as the length of a step.

## 2. The laws

`x` is the lamp's own state: a few numbers, fully known to the lamp (nothing about it is hidden).

```
L1  dx = f(x) dt + sigma dW                      f(x) = -K (x - m) + sum_j theta_j phi_j(x)
L2  dx = f(x) dt + sum_feeds B_f u_f(t) dt + sigma dW      (while feeds are present)
L3  d theta = eps * ( dx/dt - f(x) ) * phi(x)            (only while a feed is present)
            - theta / H                                   (slow forgetting)
L0  light = Limit( Readout(x) ) + breathing(t)
```

- `f` starts as plain mean reversion (`theta = 0`), so a newborn lamp is just the random process of L1. The `phi_j`
  are a small fixed set of functions of `x` only, so memory is indexed by the lamp's own state and nothing else.
- Aligned, the feed's push `B_f u_f` is added to the drift. Because `f` pulls back, alignment fades when the feed stops.
- **Imprint is regression of the observed drift on the state.** It learns the average push the world gave at each state,
  so it teaches the free-running model to do on its own what the feed made it do. `eps` is small, so only repeated
  patterns leave a mark.
- `Limit` and `breathing` are L0: smoothing, brightness ceiling, reversal cap and a fixed breathing wave. They are applied
  to whatever the lower layers produce and are never learned.

## 3. Why a rhythm can appear without a clock

Driven by a periodic feed, the state traces a closed loop. L3 learns the drift along that loop, so when the feed is
gone the free-running state keeps travelling the same loop: a learned limit cycle. Its period comes from how fast the
state was carried round while driven. A loop needs at least two state dimensions (a single dimension cannot
oscillate), and the state needs a timescale comparable to the rhythm. So the lamp can learn only rhythms its own
timescales span. **[ASSUMPTION]** a spread of relaxation times (seconds to hours) in `K`, chosen once.

## 4. Principles, each with a test

**P1. Limits are enforced, never learned (L0).** *Test:* adversarial feeds (jumps, noise, NaN, huge values) never break
the ceiling, the reversal cap or the glides, in any frame.

**P2. Imprint learns only from being driven.** With no feed, `theta` changes only by forgetting. Otherwise the lamp
would train on its own noise and hallucinate structure. *Test:* run alone for weeks; `theta` stays at its prior.

**P3. Noise leaves no mark.** A feed with no structure relative to the state averages out. *Test:* drive with white
noise; `theta` stays near zero.

**P4. Rehearsal matters.** One episode leaves a faint trace; repetition builds a lasting one. *Test:* trace strength
grows with the number of episodes, and fades with half-life `H` when they stop.

**P5. Persistence.** After repeated driving by a periodic feed and then removing it, the free-running lamp keeps a
rhythm near the driven period, and keeps it longer than a lamp with no imprint. *Test:* synthetic sinusoid.

**P6. Exact in time.** Stepped with the exact solution for the linear part, so statistics do not depend on the frame
rate. *Test:* long runs at 10 and 60 fps agree in mean, spread and autocorrelation.

**P7. Bounded and observable.** `theta` is norm-bounded, can be printed and can be wiped.

**P8. Earn every addition.** More basis functions, more state dimensions or a nonlinearity are added only if they
improve behaviour on data the model has not seen.

**P9. Authority is a strict order.** A person, then an agent, then the lamp's own dynamics.

## 5. The ledger of assumptions

1. The state size and the spread of timescales in `K` (section 3).
2. The basis `phi` (start: the state itself plus a small set of smooth bumps; the family limits what can be imprinted).
3. Noise is Gaussian.
4. Each feed is given a fixed direction `B_f` in state space when it is registered. The model does not learn what a feed
   means; the readout fixes how a pull looks as light. **[ASSUMPTION]** Feeds are normalised to zero mean and unit
   variance by running statistics so that a constant feed does not bias the lamp forever.
5. The readout from state to light, and the L0 limits. These are about the lamp, not the model.
6. `eps`, `H` and the norm bound of `theta`: how fast it learns and forgets. Set once, then tuned by replay tests.

## 6. Where this is weakest

- Whether a small fixed basis can represent a loop sharp enough to show a "sunrise"; this may need more state or a
  richer basis, under P8.
- Alignment with several feeds at once: their pushes add, and nothing yet decides which wins when they disagree.
- The readout decides what a pull looks like. A person's reactions (a poke, an override) could later adjust it.
- Stability: `f` with a learned part must stay bounded; the mean reversion in `K` must dominate it.

## 7. Build order

1. A pure, deterministic simulator of L1 to L3 with no lamp and no network, replayable faster than real time.
2. The tests P2 to P6 above, on synthetic feeds. If P5 fails, the idea changes before anything else is built.
3. L0 and the readout, reproducing today's calm behaviour from the model alone.
4. The feed channel, then the first real feeds.
