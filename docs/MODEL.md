# The model behind the lamp (draft 2, for review)

Status: **design, nothing built**. This replaces draft 1, which assumed solar time, a three-part emotion latent and a
fixed day length. Those were assumptions the model should not need. Statements marked **[ASSUMPTION]** are not
established.

## 1. The idea

The lamp is a small dynamical system that exists on its own, and the world pulls on it. Time of day, weather, music, a
screen and an agent are all the same thing: data. Nothing in the model knows what a day is.

```
x[t+1] = A x[t] + B u[t] + noise
         its own     the world
         existence   pulling on it
```

- `x`: the lamp's internal state, a few numbers.
- `A`: how the state evolves by itself. With no data (`u = 0`) it keeps running, kept moving by the noise.
- `u`: the world's data at that moment, any number of feeds, each one a coordinate of `u`.
- `B`: how strongly each feed pulls on each part of the state.
- **Alignment** means adjusting `A` and `B` so the lamp predicts what comes next.

A rhythm is not put in. If the data repeats every 24 hours, the learned `A` has a rotation with a 24 hour period; if
every 25 hours or every week, that is what it learns. When the feeds stop, `A` keeps running, so the lamp behaves as the
data taught it. Time enters only as the length of a step, `dt`, which is the lamp's own elapsed time.

## 2. Principles

**P1. As few assumptions as possible, listed.** The ledger is in section 4. Anything not on it is not in the model.

**P2. Limits are enforced, never learned.** Brightness ceiling, reversal rate, glides, habituation, and stability of
`A` (no eigenvalue grows). They belong to the lamp, not the model, and hold in every frame whatever is learned.
*Test:* property tests with adversarial feeds (jumps, noise, NaN, huge values).

**P3. Exact in time.** The model is written in continuous time, `dx = F x dt + G u dt + noise`, and stepped with the
exact discretisation `A = exp(F dt)`. Its statistics then do not depend on the frame rate.
*Test:* long runs at 10 and 60 fps have the same mean, spread and autocorrelation.

**P4. Learning is plain prediction.** The objective is the squared error of predicting the next data from the current
state. No other reward. *Test:* synthetic data with a known period; the learned period converges to it.

**P5. No hallucinated structure.** With pure-noise data the learned dynamics stay near their prior (small, decaying).
*Test:* feed noise; assert no rotation grows.

**P6. Persistence and forgetting.** Remove the feeds and the lamp keeps its learned behaviour, then relaxes to the prior
over a stated half-life. *Test:* learn a rhythm, stop the data, assert the rhythm continues and then fades.

**P7. Observable and resettable.** The learned `A` and `B`, with each mode's period and decay, can be printed and wiped.

**P8. Earn every addition.** A piece (a nonlinearity, more modes, a feed) is added only if it improves prediction on
data the model has not seen. This is the project's rule that a tool is added only if its output changes a decision.

**P9. Authority is a strict order.** A person, then an agent, then the model's own state. A higher layer wins outright.

## 3. Learning, honestly

The hard part is that `x` is hidden: we see the data `u`, not the state. Two workable forms, to be chosen by replay
tests and not by taste:

1. **Diagonal modes.** `A` is a set of independent damped rotations, each with a frequency and a decay rate. Each mode
   adapts its frequency to lock onto a rhythm in the data (adaptive-frequency oscillators). This is the cheapest and is
   my starting guess. Its assumption: the rhythms are independent, and each is a sinusoid. Sharp shapes such as a
   sunrise then need several modes (harmonics), added only under P8.
2. **General `A`.** Estimate the full matrix from a window of data by standard system identification. More flexible,
   heavier, harder to keep stable.

A feed with no rhythm gives no mode anything to lock to, and the lamp simply keeps wandering. That is correct.

## 4. The ledger of assumptions

1. The number of state variables `n`. **[ASSUMPTION]** start small, for example 6 (three modes).
2. The dynamics are linear. Richer shapes come from more modes first, and from nonlinearity only if P8 demands it.
3. The noise is Gaussian.
4. "Aligned" means "predicts well" (P4).
5. A fixed readout from state to light, plus the limits of P2. These are about the lamp and not the model, so they
   cannot be learned away.

## 5. Where this model is weakest

- **Predicting is not expressing.** The model learns to predict its feeds. The light is a fixed readout of the state, so
  a feed pulls the light, but which way "up" goes is a choice in the readout, not something the data decides. We may
  also let a person's reactions (a poke, an override, a "like") steer the readout. That is a second, later loop.
- **Hidden state.** Section 3 is an open technical choice.
- **Meaning.** The model does not know what a feed means. If we want "a stock fell, so it should look uneasy", that is
  an assumption we would add on purpose and list.

## 6. Build order

1. The state-space core with exact stepping and limits (P2, P3), driven by synthetic data and replayed faster than real
   time.
2. Mode learning with its tests (P4 to P6).
3. A readout to the lamp that reproduces today's calm behaviour from the model alone.
4. The feed channel, then the first real feeds.
