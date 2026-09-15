# Does a trailing stop help the Vantage gold book out of sample?

**Registered 2026-09-16, before the sweep was run.** The owner asked whether
the desk should decide on one-minute bars so a model could manage an open
position — a trailing stop being the example. That is a backtest question and
costs nothing to answer, so it is answered here before any token is spent on
the live campaign.

## Why this is not the measurement already in the repository

Commit `f237c4a` compared trail-on against trail-off across 23 methods and
found more winners, smaller drawdowns and *slightly less money*. It was
labelled EXPLORATORY and it meant it: **one cell, in sample, unguarded, no
null and no held-out window.** Nothing in it was a result, and it cannot be
promoted into one by being quoted.

This keeps that shape and adds the three things it lacked: a grid, a held-out
window, and a null for the selection itself.

## The question

Does a stop that follows the trade, measured in R of the original stop
distance, raise the expectancy of the Vantage gold 15m book on a window it was
not chosen on?

## What will be run

`scripts/trail_sweep.py`, on `xauusd` (Vantage XAUUSD.sc, the cent pair the
account trades), 15m bars.

* **Grid:** `distance_r` ∈ {0.5, 0.75, 1.0, 1.5, 2.0} × `activate_r` ∈
  {0.5, 1.0, 1.5, 2.0} — 20 cells, plus `off` as the control arm.
* **In-sample window:** 2022-06-16 → 2025-09-15.
* **Held-out window:** 2025-09-15 → 2026-09-15, the recent Vantage year. This
  is the primary window, by the criterion the owner set on 2026-09-13.
* **Method set:** fixed by the CONTROL arm — every method with at least 100
  trades on the in-sample window with the trail off. Fixed by the control on
  purpose: if each cell scored its own method set, a cell could win by
  silently dropping the methods it hurts.
* **Statistic:** the **median** expectancy in R across that method set. Median,
  not mean and never the best cell, because one method with nine trades and a
  huge expectancy must not be allowed to carry a grid.

## The selection rule, fixed now

The cell with the highest in-sample median expectancy is carried forward.
Exactly one cell. No re-reading the held-out window and picking again.

## The falsifier, fixed now

The trailing stop **fails** unless all three hold on the held-out window:

1. The chosen cell's median expectancy is **above** the `off` control.
2. The number of methods with profit factor > 1.0 is **not lower** than with
   the trail off. The exploratory run's one clear signal was that the trail
   converts large wins into small ones faster than large losses into small
   ones; seven methods cleared 1.0 without it and six with it. If that repeats,
   a higher median is not worth having.
3. The chosen cell sits at or above the **50th percentile** of all 20 cells
   scored on the held-out window.

Test 3 is the null, and it is the one that matters. Any grid has a best cell
in sample. If the cell chosen on the first window lands in the bottom half of
the second window's distribution, then choosing it was noise, and a "winning"
trail setting would be a number picked out of twenty by luck.

## What is expected

That it fails. Thirty-two registrations are closed in `docs/decisions/` and
none survived out of sample; the exploratory comparison already pointed at
less money; and a stop that only ever tightens must cut the tail of any method
whose edge lives in its tail.

Worth running anyway, for the same reason as always: it is cheap, it is
falsifiable, and the alternative is deciding the question by opinion.

## What no outcome of this changes

Nothing automatically. `[trading.trail] enabled = false` stays false, because
**every number in `docs/decisions/` was measured without it** and turning it on
globally would make all of them unreproducible without editing a single
receipt. A pass here makes the trail a candidate for a registered change to the
live books, not a config edit.

And it does not answer the owner's original question in the affirmative either
way. Even a trail that survived here would be an *engine* rule, evaluated on
closed bars, needing no model call and no one-minute feed. The reason not to
put a model on one-minute bars is separate and unchanged: it would cost about
fifteen times the tokens, and a model managing its own position cannot be
mirrored by the coin that controls it, so the campaign would lose the only
thing that makes it readable.
