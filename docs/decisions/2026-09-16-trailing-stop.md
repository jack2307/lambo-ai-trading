# The trailing stop: it helps the losers lose less, and makes the only winners worse

**Registered** `docs/hypotheses/2026-09-16-trailing-stop.md`, commit `d1f302c`,
before the sweep ran. **Run** `scripts/trail_sweep.py` on `xauusd` (Vantage
XAUUSD.sc 15m), 21 cells × 2 windows.

## The declared verdict, and why it overstates

The script prints **SURVIVES**. All three declared tests passed on the held-out
year:

| test | result |
|---|---|
| 1. chosen cell beats `off` | pass — median expectancy −0.0345R vs −0.0440R, **+0.0095R** |
| 1b. PF>1 count not reduced | pass — 3 with the trail, 3 without |
| 2. selection above the 50th percentile | pass — **53rd** |

**I am not reporting this as a win, because my own test 1 was badly chosen.**
On the held-out window **19 of the 20 cells beat `off`**. A test that nearly
every cell passes carries almost no information about the cell that was
selected — it measures whether tightening stops helped this particular year,
not whether the selection found anything. And test 2, the one designed to catch
exactly that, came back at the **53rd percentile**: a coin flip. Choosing
`2.0R / 1.0R` on the first window was noise.

A falsifier that can be passed by accident is a falsifier that needs rewriting,
not a result that needs announcing.

## What the numbers actually say

Per method on the held-out year, trail off → trail `2.0R / 1.0R`:

| | methods |
|---|---|
| profit factor better | 6 |
| worse | 5 |
| unchanged | 7 |

The median rose. But look at **which** methods moved:

```
                    PF off   PF on
volume-thrust        1.622   1.622   unchanged (the trail never fired)
keltner-break        1.079   1.066   WORSE
donchian-breakout    1.020   1.015   WORSE
------------------------------------ every other method is below 1.0
ict-sweep-mss-fvg    0.890   0.937   better
stoch-reversal       0.888   0.899   better
bb-fade              0.885   0.893   better
```

**Both profitable methods that the trail touched got worse.** Every improvement
is in a method that loses money either way, and none of them crosses 1.0. The
set of profitable methods is identical before and after: `donchian-breakout`,
`keltner-break`, `volume-thrust`.

So the effect is real, consistent, and points the wrong way: a stop that only
ever tightens converts large wins into small ones faster than it converts large
losses into small ones. That is what the exploratory run in `f237c4a` said, and
a pre-registered held-out year now says it again with the mechanism visible.

The median expectancy remains **negative** in every cell of the grid, on both
windows. Nothing here makes the desk profitable; it makes an unprofitable desk
slightly less unprofitable, by clipping the tail that any real edge would have
to live in.

## Decision

**The trail stays off.** `[trading.trail] enabled = false` is unchanged and no
run overrides it.

Turning it on would cost the two methods that clear 1.0 a little of what they
have, in exchange for making sixteen losing methods lose marginally less —
which is worth nothing, because nobody is going to trade a method with a
profit factor of 0.89.

## What this says about the question that prompted it

The owner asked whether the desk should decide on **one-minute bars** so a
model could manage an open position, with a trailing stop as the example.

This answers the example: on four years of the broker's own gold, the example
does not help. It does not answer the general question, and two things about
the general question are unchanged by this run:

1. A trail is an **engine** rule evaluated on closed bars. It needs no model
   call and no one-minute feed. If faster management ever does help, it will
   almost certainly be findable this way — for free — before any token is spent.
2. A model managing its own position **cannot be mirrored by the coin that
   controls it**, so the campaign would lose the only thing that makes it
   readable. That objection is about the experiment, not about cost, and no
   backtest can remove it.

## Backlog

Rewrite test 1 for any future exit-rule registration. "Beats the control on the
held-out window" is too easy when the whole grid shifts in one direction; the
test should be against the **distribution of the grid**, not against `off`.
