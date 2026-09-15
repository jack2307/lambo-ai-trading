# 2026-09-15-quote-asymmetry: when a dealer moves one side of its quote and not the other, it is saying which side it does not want

**Registered:** 2026-09-15 — written before any feature was built or any return
read. The only numbers read beforehand are the tick-history inventory in
`py/ingest/mt5_tick_depth.py`'s output (depth, tick counts, one-sided share).
**Status:** decided (-> `docs/decisions/2026-09-15-quote-asymmetry.md`). No cell
reached the 97.5 gate (best 94.08), every day-block interval contained zero, the
declared 40% concentration ceiling failed in all four cells, and the mechanism's
own decomposition contradicted the story three ways: the quantity strengthens
with fill latency, only the NARROWING half carries anything, and days with more
one-sided revisions carry less signal. A harness self-test finds a planted
+0.34 bp edge above the gate and never scores noise above it, so the instrument
is sound and the effect is one standard error wide (0.22 bp on 5,047 trades).
The out-of-sample window and the euro confirmation were NOT opened.
**Script:** `scripts/quote_asymmetry.py` (to be written after this commit)
**Batch file:** none — a tick-level measurement, not a `search` batch.

## Why this one

The owner's instruction of 2026-09-15 was to stop testing what is published and
to invent mechanisms from data this repository owns. The first attempt at that,
`2026-09-15-venue-residual`, closed the same day — not on a percentile but on
**resolution**: it compared two venues' minute bars to measure a quantity worth
0.1 to 6 seconds of tape, and two vendors' clock latency cannot be separated
from a dealer's decision at that resolution.

This hypothesis is the same question asked in the one place where that objection
cannot be made: **inside a single feed, on a single clock**. Vantage's own tick
stream carries bid and ask with millisecond stamps, and about a third of its
quote revisions move **one side only**. No second venue is involved anywhere in
the design, deliberately.

## Claim

When price simply moves, a dealer shifts bid and ask together. When it moves
**one side alone**, it has changed the price of one direction of trade and not
the other — and a dealer does that about its own book, not about the market.

A dealer takes the other side of its clients. If it makes buying expensive (ask
up, bid unchanged) it is discouraging the flow that would leave it **short**;
the adverse-selection reading gives the same answer, since it widens the ask
when it expects informed buying. If it makes selling attractive (bid up, ask
unchanged) it is inviting the flow that would leave it **long**. Both readings,
and both event types, point one way:

**Declared direction: a net upward one-sided revision precedes a RISE; a net
downward one-sided revision precedes a FALL.** Declared before any run, and the
mirror is not available — a result in the opposite direction closes this and is
recorded as such. (`2026-09-15-venue-residual` spent its credibility on a
two-sided wording; this one does not.)

## The control is inside the claim, not bolted on

Every tick revision falls into exactly one of two buckets:

- **one-sided** — ask alone, or bid alone. The dealer's decision. The signal.
- **two-sided** — both move together. Price. The control.

Both arms are built by identical machinery, thresholded identically, traded
identically. If the two-sided arm predicts as well as the one-sided arm, this
mechanism is momentum wearing a costume and it closes **regardless of what the
signal arm scores**. That is gate 6 below, and it is the gate this hypothesis
most expects to die on.

## Features, all from one clock, strictly causal

Per tick pair within a minute, with `point` = 0.01 and `eps` = point/2:

    db = bid_i - bid_{i-1}          da = ask_i - ask_{i-1}
    moved_b = |db| > eps            moved_a = |da| > eps
    one-sided  <=>  moved_a XOR moved_b        two-sided <=> moved_a AND moved_b
    s = da if ask-only else db                          (signed, in points)

    OS_t = sum of s over one-sided events in minute t
    TS_t = sum of (da+db)/2 over two-sided events in minute t

    zOS_t = (OS summed over the trailing W minutes) / (trailing sd of that sum)
    zTS_t = the same construction on TS

The scaling sd is computed over the trailing 5 trading days and **shifted by one
minute**, so nothing at or after t enters the denominator. Derivation is from
price changes, not from `TICK_FLAG_BID/ASK`; the two will be cross-checked for
agreement and the disagreement rate reported.

## Fills, and why no spread has to be assumed

The tick carries both sides, so a round trip is priced from the quotes that
would have filled it:

- signal at the close of minute t; **entry at the first tick at least 1.0 s
  after the minute boundary**, at the **ask** for a long, the **bid** for a short
- **exit at the first tick at least 1.0 s after** the close of minute t+k, at the
  **bid** for a long, the **ask** for a short
- return in bp from those two prices, spread paid in full at both ends

This is the first study in this loop that does not assume a spread anywhere.
Latency is reported as a curve at **0 / 1 / 5 / 30 seconds**; 1 s is the
registered cell and a signal that needs 0 s is not a signal.

Trades do not overlap: nothing fires again until the position is off. No trade
may span the daily stop or a weekend — the fault
`2026-09-15-venue-residual` paid for.

## Cells

W in {1, 5} minutes x threshold |z| >= 2.0 x k in {5, 30} minutes = **4 cells**.
Four is the multiplicity and it is printed with the results. The widening-only
and narrowing-only decompositions are **diagnostics reported after the gate is
read**, never additional cells.

## Falsifier — all seven, on the in-sample window

1. At least **200** non-overlapping trades in the cell.
2. Mean net return per trade **> 0**, spread paid from the filling quotes.
3. At or beyond the **97.5th percentile** of a sign permutation matched on
   **both the UTC hour and the volatility decile**, at **100,000 draws**
   (SE at that p: 0.05 pp). Hour-matching is required because a volatility
   decile is a liquidity proxy — `2026-09-15-venue-residual` proved that.
4. The **day-block bootstrap 95% interval excludes zero** (20,000 resamples,
   the calendar day as the block). A trade-level permutation on a same-day
   clustered sample overstates the evidence.
5. **No single calendar month carries more than 40% of the net.** Risk's
   concentration gate, declared in advance rather than discovered.
6. **The two-sided control arm does NOT clear gate 3.** If it does, this closes.
7. The sign **survives the repository's own configured guards**
   (`max_trades_per_day = 4`, 30-minute cooldown) as a second reading. A
   measurement whose sign flips under the project's own limits is not a
   candidate for anything.

A pass on 3 and 4 with a failure on 2 is "direction exists, not a trade".

## Data

- **In-sample:** `XAUUSD.sc` ticks, **2026-01-20 → 2026-06-15** (~5 months).
- **Out-of-sample, named now, opened only after the in-sample record exists:**
  `XAUUSD.sc` ticks, **2026-06-16 → 2026-09-12** (~3 months).
- **Mechanism confirmation, opened at the same time as the out-of-sample and
  not before:** `EURUSD.sc` ticks over the in-sample dates — a different
  instrument at the **same dealer**, which tests the mechanism rather than the
  metal.
- Broker tick history reaches back to about **2026-01-16** and no further
  (probed 2026-09-15: present at 240 days, absent at 255). Fetch date is
  recorded with the features, because this history is the broker's and could be
  refetched differently.
- Features are aggregated to one row per minute and stored; raw ticks are not
  kept (about 0.4M ticks a day).

**Declared before the run:** the one-sided share is **not stationary** — 33% of
revisions on 2026-09-08, 15% on 2026-02-02. The fire rate will vary across the
window for reasons that have nothing to do with the claim, and an effect that
lives only in the high-one-sided-share months is a regime finding, to be
reported as one.

## Sample needed

At least 200 non-overlapping trades per cell; at ~7,000 one-sided revisions an
hour the constraint is the threshold, not the tape.

## What each outcome means

- All seven gates, then the out-of-sample window and the euro → decision record,
  then a **paper** proposal. Nothing places an order.
- Gates pass in-sample, fails out-of-sample → closed. No second out-of-sample.
- Gate 6 fails (the two-sided control also predicts) → closed as momentum, and
  the signal arm's score is not reported as a finding.
- Direction opposite to the declared one → closed. There is no mirror here.
