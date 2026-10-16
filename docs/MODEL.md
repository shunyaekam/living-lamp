# The model behind the lamp (draft for review)

Status: **design, nothing built**. Each rule below is derived from a principle; if a rule looks wrong, argue with the
principle. Statements marked **[ASSUMPTION]** are not established and must not be reported as fact.

## 1. What the lamp is, in one paragraph

A small stochastic process (the *brainstem*) that wanders inside hard limits. Anything that wants to influence it,
whether the time of day, the sun, the weather, a stock, music, a screen or an agent, is a **feed**: a stream of data
that pulls the process toward a target. The lamp also **learns what the feeds have been asking for at each time of
day**, so when the feeds go quiet it keeps behaving as they taught it. Time is an input, not a state.

## 2. Principles

**P1. Limits are enforced, never learned.** Brightness ceiling, reversal rate, always-glide, habituation. Learning
moves the lamp only inside the space these allow, so no feed, learned value or bug can make it flash or glare.
*Test:* property tests that drive the model with adversarial feeds (jumps, noise, NaN, huge values) and assert the
output limits hold in every frame.

**P2. One interface for all influence.** A feed is a timestamped signal with a declared mapping into the lamp's own
latent state (section 3). There are no special cases per source; campfire, weather and the screen are mappings, not
code paths. A feed is either **continuous** (a value that sets a target) or **discrete** (an event that causes a
poke). *Test:* every existing mood and the screen mimic can be expressed as a feed with no extra code.

**P3. Few latent dimensions, fixed rendering.** The state is three numbers (section 3), and a fixed hand-written map
turns them into colour and level. The learner never sees the colour, so it cannot learn an ugly colour.
**[ASSUMPTION]** Three dimensions are enough to express every mood we have. *Test:* reproduce each mood from its
latent target and compare to its current output.

**P4. Time is an input, as a phase, not a timestamp.** The model is a *cyclostationary* process: its statistics repeat
with the day. We give it phases (angles on a circle), never raw clock time. This makes the function periodic and
smooth by construction, and it removes the hourly-slot problem of neighbouring slots disagreeing.

**P5. Do not claim what the data cannot show.** Solar time and clock time differ by a nearly constant offset within a
season, so for weeks their effects cannot be told apart. They separate only through the equation of time (about
16 minutes over the year), a daylight-saving change (the clock jumps an hour, the sun does not) or travel. So: start
with solar features, which are physical and location-aware, add one clock feature for habit, and fit both with a
shrinkage penalty so collinear weights share the load instead of fighting. Report which one the data prefers only
after a daylight-saving change or after enough weeks to measure it.

**P6. Start from today's behaviour and move only with evidence.** The prior is the current hand-set day curve. With
no data the lamp behaves exactly as it does now. Each observation shifts the learned curve by an amount that depends
on how much data there is (a Bayesian ridge prior), never by a fixed jump.

**P7. Randomness has a stated law.** The brainstem is a mean-reverting (Ornstein-Uhlenbeck) process: for each latent
dimension we state its mean, its stationary spread and its correlation time. Nothing else about the randomness is
free. It is stepped with the exact discretisation, so its statistics do not depend on the frame rate.
*Test:* long runs at 10 and 60 fps have the same mean, spread and autocorrelation to within sampling error.

**P8. Learning is bounded, observable and resettable.** The learned offset from the prior is clipped to a stated
range, decays back to the prior when feeds are absent for weeks, can be printed (`status`) and can be wiped
(`forget`). A learner you cannot inspect cannot be trusted or debugged.

**P9. Every added piece must beat a simpler one, measured.** Learning is judged by held-out prediction error of the
feed-driven target from time alone, against a constant (global) model. If time features do not beat the constant on
days the model has not seen, they are removed. This is the project's rule that a tool is added only if its output
changes a decision.

**P10. Authority is a strict order; blending is soft.** Person, then agent, then feeds, then learned imprint, then
prior. A higher layer wins outright. Within the feed layer, targets are blended by weight, so the output stays inside
the convex hull of the targets.

## 3. The model

**Latent state** `z = (warmth, arousal, level)`, each in `[-1, 1]`. The colour-emotion literature ties pleasure to
brightness and arousal to saturation, and hue weakly **[ASSUMPTION: the hue effect is small and varies by person]**;
that is why the latent is these three and not hue directly.

**Brainstem (per dimension, exact step over `dt`):**

```
z' = mu + exp(-dt/tau) * (z - mu) + sigma * sqrt(1 - exp(-2 dt/tau)) * N(0,1)
```

`tau` is the correlation time (how slowly it wanders) and `sigma` the spread; both are personality, fixed by mood.

**The mean `mu(t)`** is the layered target of P10:

```
mu(t) = layer( person, agent, blend(feeds), imprint(t), prior(t) )
imprint(t) = prior(t) + Phi(t)^T w          clipped to +/- delta of the prior
```

**Features** `Phi(t)` are 8 numbers: a constant, `sin` and `cos` of 1, 2 and 3 times the solar hour angle, and the sine
of the sun's elevation. A clock-phase version of the harmonics is added as a second block (P5). The constant is the
"global" model, and the harmonics are the "time-based" one, so global versus time is not a choice.

**Learning.** While a continuous feed is present it supplies a target `z*` for `z`. We update `w` by recursive least
squares with forgetting factor `lambda` toward the prior (a Bayesian ridge). The features are shared by the three
dimensions, so there is one 8x8 matrix and three weight vectors: a few hundred bytes. When the feed stops, `mu(t)`
keeps using `imprint(t)`: the lamp keeps behaving "like solar", which is the behaviour you described.

**Solar phase (no network).** From latitude, longitude and UTC: declination from the day of year, the equation of
time (a standard approximation, accurate to a minute or two, to be checked against published almanac values), hour
angle from true solar time, elevation `sin(e) = sin(lat) sin(dec) + cos(lat) cos(dec) cos(h)`. *Tests:* at an
equinox the noon elevation is `90 - lat`; at a solstice it is `90 - lat` plus or minus 23.44 degrees.

**Arbitration weights** for the feed layer: `c_f` proportional to `priority_f * exp(-age_f / tau_f)` (a stale feed
fades out), normalised, with the imprint as a fixed-weight competitor so that no feed reaches 100% unless it carries
agent authority. **[ASSUMPTION]** Fixed priorities are enough to start; learned interest is a later experiment.

## 4. How we will know it works

All offline, deterministic, no lamp needed. The core is already deterministic, so feeds can be recorded and replayed
faster than real time.

1. **Recovery.** Feed a synthetic daily shape for N days. The learned `imprint(t)` recovers it within a stated error.
2. **No hallucination.** Feed pure noise. The learned harmonics stay near zero (the ridge prior holds).
3. **Persistence.** Remove the feed. The lamp keeps the learned shape, then decays to the prior over the stated
   half-life.
4. **Frame-rate invariance** and the **limit properties** of P1 and P7.
5. **Held-out skill (P9).** On days not used for fitting, time features have lower error than the constant model.

Only after these pass do we run it on the real lamp, and then we ask one question per test.

## 5. Build order

Each step is useful alone and is merged only when its tests pass: (1) the solar function; (2) latent state, mean
reversion and rendering, matching today's moods; (3) the learner and the replay harness; (4) the feed channel and
arbitration; (5) the first feeds (solar, ping), then weather and music.

## 6. Open questions

- The lamp's rough location (latitude and longitude, to about a degree is enough).
- Whether `delta`, `lambda` and the clock-phase block are set by you or found by the held-out test.
- How the three-dimensional latent should render hue on an RGB-only strip whose reds, greens and blues are unequal
  (the white balance is `[0.85, 1, 0.75]`).
