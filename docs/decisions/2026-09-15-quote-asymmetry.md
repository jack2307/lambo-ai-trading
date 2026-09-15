# 2026-09-15-quote-asymmetry — the dealer's one-sided quotes are one standard error wide

**Registered:** `13178b5` (2026-09-15), before the ingest or the measurement existed
**Amended:** `5511983` (a null that permuted an already-signed return), and again below
**Status:** closed on the declared direction. The out-of-sample window and the euro
confirmation were **not opened**.
**Hypothesis:** `docs/hypotheses/2026-09-15-quote-asymmetry.md`
**Receipts:** `docs/research/runs/2026-09-15-quote-asymmetry/`

## What was claimed, and why it was worth a registration

The owner's instruction of 2026-09-15: stop testing what is published, invent
mechanisms from data this repository owns. The first attempt,
`2026-09-15-venue-residual`, closed the same day on **resolution** — it compared
two venues' minute bars to measure a quantity worth a second of tape.

This asked the same question where that objection cannot be made: **inside one
feed, on one clock**. Vantage's own tick history carries bid and ask with
millisecond stamps. When price moves, both sides move together. When one side
moves alone, the dealer has repriced one direction of trade and not the other —
a decision about its own book, not about the market.

Declared direction, with **no mirror available**: a net upward one-sided
revision precedes a rise.

Three things made this the strictest registration the loop has written:

- **No spread is assumed anywhere.** A long is filled at the ask and exits at the
  bid, both from the tick that would have filled it. The measured round trip is
  0.4534–0.4628 bp and it is paid on every trade.
- **The control is inside the claim.** Every revision is either one-sided (the
  signal) or two-sided (price). Identical machinery on both. Gate 6: if the
  two-sided arm clears, the mechanism is momentum in a costume and closes
  regardless of what the signal arm scores.
- **Seven gates, six of them lessons from failures earlier the same day** —
  hour-matched null, day-block interval, a declared concentration ceiling, no
  trade spanning a gap, a latency curve, survival under the project's own guards.

## The data

56,805,825 ticks over 104 sessions (2026-01-20 → 2026-06-15) folded into 142,991
minutes of features. 13,989,357 revisions moved one side only — **23.8%**, where
the registration guessed "about a third"; that discrepancy is recorded rather
than reconciled away.

`data-integrity` re-fetched 2026-03-10 from the broker and rebuilt the features
independently: the stored parquet reproduced **byte for byte**. Coverage 104/105
business days (the absentee is Good Friday), 98.8% of minutes, exactly one
intraday hole of 4 minutes in five months, and a 33% price range in both
directions. Causality verified numerically (max deviation 5.15e-14 against an
explicit past-only loop). Fill quotes verified by brute-force scan against raw
ticks at all four latencies.

The 0.071% disagreement between the price-derived classification and the tick's
own `TICK_FLAG_BID/ASK` is **one systematic case**: the broker re-sends a bid
with the flag set and a bit-identical price. Zero "moved but no flag" on either
side. The events the claim is about are real, dense, and correctly identified.

## The verdict, gate by gate

```
=== SIGNAL - one-sided revisions (the dealer deciding) ===
    W   k      n   net bp    gross    up%  pctile   CI low  CI high   g.month   share   g7
    1   5   5047  -0.2410  +0.2146  49.6%   79.80  -0.5911  +0.1293   2026-01   42.0% FAIL
    1  30   2295  +0.0797  +0.5376  49.8%   71.98  -1.1967  +1.3542   2026-02   88.6% FAIL
    5   5   3148  -0.0219  +0.4312  50.3%   94.08  -0.5992  +0.6404   2026-01   63.7% FAIL
    5  30   1724  +0.5184  +0.9738  51.3%   77.77  -1.4921  +2.5455   2026-02   74.2%   ok

=== CONTROL - two-sided revisions (price moving) ===
    1   5   4062  -0.8673  -0.4066  46.1%   19.99  -1.4303  -0.3207   2026-05   45.1% FAIL
    1  30   1798  -0.4657  -0.0086  45.8%   74.70  -2.3043  +1.4212       n/a         FAIL
    5   5   2724  -0.6977  -0.2366  47.2%   45.44  -1.4972  +0.1319       n/a         FAIL
    5  30   1329  +0.3649  +0.8243  46.0%   87.15  -2.4454  +3.2956       n/a           ok
```

| gate | requirement | result |
|---|---|---|
| 1 | ≥ 200 trades | **pass** (1,724–5,047) |
| 2 | net > 0 | **fails** in 2 of 4 |
| 3 | ≥ 97.5th of the matched null | **fails 4 of 4**; best 94.08 |
| 4 | day-block 95% CI excludes zero | **fails 4 of 4** |
| 5 | no month > 40% of the net | **fails 4 of 4** (42.0 / 88.6 / 63.7 / 74.2%) |
| 6 | control must NOT clear | **passes** — control best 87.15 |
| 7 | sign survives the configured guards | **fails 3 of 4** |

## Three things the registration forced into print, and each one contradicts the story

**The quantity strengthens with latency.** Gross, at fills 0 / 1 / 5 / 30 seconds
after the signal minute:

```
   W=1 k=5   +0.219   +0.215   +0.172   +0.346
   W=1 k=30  +0.547   +0.538   +0.523   +0.551
   W=5 k=5   +0.411   +0.431   +0.413   +0.599
   W=5 k=30  +0.962   +0.974   +1.023   +1.247
```

Thirty seconds is the best fill in every cell. A dealer's immediate repricing
decision should decay in thirty seconds, not sharpen.

**The premise was wrong.** The registration argued that widening the ask and
narrowing the bid "both read the same way". They do not:

```
    widening  -0.216 (t-0.75)  -1.077 (t-2.14)  +0.097 (t+0.32)  +0.024 (t+0.03)
   narrowing  +0.266 (t+1.92)  +0.111 (t+0.18)  +0.522 (t+2.01)  +0.586 (t+0.95)
```

Whatever is there sits in the **narrowing** events. The widening half — the
clearer inventory story, the dealer making a side expensive — is nothing, and
negative where it is largest.

**More dealer decisions mean less signal.** By tercile of each day's one-sided
share (which ranges 0.086 to 0.421):

```
   W=1 k=5   low +0.619   mid -0.055   high +0.108
   W=1 k=30  low +2.339   mid +0.067   high -0.621
   W=5 k=5   low +0.870   mid +0.035   high +0.383
   W=5 k=30  low +2.532   mid +0.655   high -0.129
```

If one-sided revisions carried a dealer's information, days full of them would
carry more of it. The opposite is true in three of four cells.

## And what direction there is, is one day

```
   W=1 k=5   gross +0.215 (t+1.16) | 2026-01-30 alone  -1.9% | top 5 days 110.7% | without January +0.133
   W=1 k=30  gross +0.538 (t+0.84) | 2026-01-30 alone  31.1% | top 5 days 177.4% | without January +0.582
   W=5 k=5   gross +0.431 (t+1.34) | 2026-01-30 alone  61.6% | top 5 days 112.8% | without January +0.170
   W=5 k=30  gross +0.974 (t+0.95) | 2026-01-30 alone  62.9% | top 5 days 135.0% | without January +0.700
```

2026-01-30 is the session gold fell 9.5%. One day carries 62% of the best cell.
Top-five shares above 100% mean the other 99 days are net negative.

## Is the instrument sound? It was tested, not argued

`selftest.txt`. Every real thing kept — the minutes, the quotes, the hours, the
volatility — and only the signal column replaced by `noise + β × forward return`,
planting an edge of known size. Then the registered machinery, unchanged.

```
  realized gross  +0.129  +0.148  +0.197  +0.341  +0.418
  percentile       66.06   74.15   89.23   98.53   99.33
  pure noise, three seeds: 33.11 / 29.84 / 80.06  (and 77.59 / 22.59 / 25.36 at W=5 k=30)
```

The harness maps realized gross onto percentile monotonically, finds a planted
edge of +0.34 bp above the gate, and never scores pure noise above it. It is not
broken, and it is not hallucinating.

**The 97.5 gate at the largest cell corresponds to a realized gross of about
+0.30 bp. The data produced +0.215 bp.** With a per-trade dispersion of 15.5 bp
over 5,047 trades, the standard error of that mean is **0.22 bp** — the effect
being hunted is the width of one standard error.

## Two sentences retracted

The commit message at `9c3363f` says the two arms "are the same noise" and that
the one-sided revisions "do not predict". Both overstate what was measured, and
`adversary` broke them:

- The arms share **1.2%–4.0%** of their entry minutes and their daily returns
  correlate −0.12 to +0.10. Signal gross is positive in 4 of 4 cells, control in
  1 of 4, and the hit-rate gap between them is z = +2.51 to +3.35. They are not
  the same noise. **The correct statement is that gate 6 passed and the design
  was never powered to compare the arms.**
- The control arm is significantly **anti**-predictive (gross hit rate 45.8–47.2%
  against 50; the two-sided arm mean-reverts over 5–30 minutes). That is a
  property of price, it says nothing about a dealer's book, and it is worth less
  than the round trip: gross +0.39 bp against a 0.455 bp spread.

## Three faults this study shipped, all mine

1. The null permuted an already-signed return — `mean(sign × sign × raw)` — so
   the first receipt's percentile column measured a quantity with no direction
   in it. Kept as `in-sample-VOID-permutation-bug.txt`. **The code that produced
   it was never committed**, so the amendment cannot be audited against the thing
   it amends; that is a process failure and is recorded as one.
2. The configured guards' thirty-minute cooldown counted rows of the trade list
   instead of minutes of the clock.
3. The fix for a fourth fault (`rolling` spanning session gaps) emptied both W=5
   cells silently, twice: a rolling min over a bool Series returns NaN, and then
   a 6,890-row scale window can never meet a full `min_periods` once any NaN is
   present. Both W=5 cells printed "too few" and a reader would have seen a
   two-cell study.

The best cell fell from 96.33 to **94.08** once the gap fault was actually fixed.
The near-miss the adversary was asked to rule on stopped existing.

## The verdict

**Closed on the declared direction.** No cell reaches the gate; every day-block
interval contains zero; the declared concentration ceiling fails in all four
cells; the mechanism's own decomposition contradicts the story three ways.

## What this does not say

- It does **not** say a dealer's one-sided quotes carry no information. It says
  that over five months, 56.8M ticks and 142,991 minutes, any such edge is
  smaller than **0.3 bp per trade at this trade count**, which is the floor this
  instrument can resolve.
- It does **not** establish that the mechanism is untradable. Clearing gates 2,
  3 and 4 together required a gross edge of roughly **2.4× the round-trip
  spread**; joint power at the observed effect size was about 14%. "Underpowered
  for tradability" is the honest phrase; the registration wrote gate 4 on net
  returns when it had promised that role to a direction test, and that is a
  design fault to carry forward.
- It does **not** close the tick-level family. It closes **this** construction:
  a net signed sum of one-sided point moves, thresholded at 2σ, held 5 or 30
  minutes. 91.2% of one-sided point movement cancels inside the minute before
  `os_sum` is formed, and the pieces were checked — but only as a diagnostic.

## What would reopen it

A construction that does not net opposing revisions against each other inside a
minute, and a statistic read at a trade count where 0.2 bp is resolvable. Both
are available: the tick history reaches 2026-01-16 and the out-of-sample window
2026-06-16 → 2026-09-12 has never been opened, so a re-registration has a clean
market to be judged on. It needs a new mechanism, not a new threshold.

## What it bought

An ingest that turns broker tick history into per-minute quote-revision features
(`py/ingest/mt5_quote_features.py`, read-only, resumable, 11 MB for five months
of 56.8M ticks); the first study in this loop that assumes no spread anywhere;
a self-test that plants an edge and checks the harness finds it, which every
measurement script in `scripts/` should now have and none other does; and the
knowledge that at ~5,000 non-overlapping five-minute trades this account's noise
floor is 0.22 bp per trade — the number every future intraday registration
should be sized against before it is written.
