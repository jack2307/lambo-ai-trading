# 2026-10-09-options-one-window: the founding thesis, measured once, on one window

**Written:** 2026-10-09, before the first line of code and before any trade
count, profit factor or expectancy was produced on this axis.
**Agent:** `options-first`, branch `agent/options-first`, worktree
`/e/rust/fd-options-first` (cut from `agent/stop-width`).
**Axis:** gold options flow — the claim this project was built to test.

**Parent registrations, both executed here and neither re-written:**

* `docs/decisions/2026-10-06-options-thesis-scope.md` §6 (branch `agent/n4`) —
  9 declared cells, of which the pre-check spent 1.
* `docs/decisions/2026-10-08-options-ceiling-precheck.md`, closing section
  "Carried forward" (branch `agent/options-precheck`) — the remaining **8
  cells**, 1 market-config x 4 strategies x 2 guard arms, gate unchanged.

That registration says it is **not to be run until the overlap reaches ~410
market-hours**. It is being run now anyway, deliberately, and **this document
is the amendment that says so in advance rather than after a number.** The
reason is in the three facts the pre-check measured:

1. The entry ceiling is **>= 40 on all ten arms**, 5.7x-94x the floor. Entry
   opportunity is not the scarce thing.
2. `XAUUSD-15m` holds **196.5 h** of tape-and-bar overlap and already measured
   **38-40 trades at the occupancy pessimum** — one window is at the floor
   today.
3. Nothing is accumulating tradable bars (`py/ingest/mt5_export.py` is not
   running), so the 410 hours are **not arriving on their own**. Waiting for
   the second window is not a plan with a date on it.

And four mechanisms that this repository was written around have **never been
run once** in the whole record.

## Hypothesis, one sentence

On the single window where the gold option tape overlaps a tradable bar series
(`XAUUSD-15m`, 196.5 market-hours), at least one of the four option-reading
mechanisms clears the desk gate — PF >= 1.200 **and** expectancy >= +0.050R
**and** >= 40 trades — read on `PF_r` as well as `PF_usd`.

## What this is NOT, declared before the run

**A pass here is not a gate pass.** The desk gate is two windows (brief §4);
this is one. Eleven window artefacts in the record say a single window is not
a result, and the eleventh is the stop size itself. If a mechanism clears, the
product of this job is **"one window, and here is what would confirm it"** —
not a candidate. Declared here so it cannot be read otherwise later.

**No percentile will be published.** `matched_rate` is a coin toss at this
sample size (the brief's own measurement: adjacent windows of 69 and 70 events
giving count match 1.13-1.17 and 0.03-0.04), and `SURVIVES` depends on where a
row sits in a TOML file (null p95 1.297 vs 2.036 for the same method). The
gate is reported **alone**, with `count match` printed beside it so a reader
can see the percentile is unreadable rather than being told a number.

## The config change, and why it is a config change

`options_source = "none"` on `[markets.xauusd]` is **CONFIG, not missing
data** — it is why every run in the record prints `timeline: none`. A separate
config directory in this worktree (`config-ofirst/`, beside `config/`, the way
`agent/m2` did it) sets

    options_source = "reference"
    tape = "gold"

on `[markets.xauusd]` and nothing else. `config/` is not edited; `--config=`
selects the directory and the receipt header prints which one it read.
`[markets.btcusd]` already uses exactly this pair of keys, so no new field is
invented.

## The basis, and why one offset is not a measurement

The tape is COMEX GC; the bars are Vantage spot. GC - XAUUSD, re-measured by
the pre-check on 6,718 overlapping minutes: mean **+43.70**, sd **1.91**, p10
**41.26**, p90 **45.78**, quartile means drifting monotonically **45.66 ->
41.35**.

A constant offset is therefore **not good enough for a measurement**: at
`entryAtr = 0.35` the entry tolerance is **1.58 USD** on the 5m arm against a
residual sd of **1.91 USD**, so a basis error is larger than the tolerance the
rule triggers on. The pre-check established that this does not matter for a
*count*; it has to matter for a *gate*.

So every cell runs at **three offsets — p10 41.26 / mean 43.70 / p90 45.78** —
and all three are reported. **If the verdict changes between them, the answer
is "not decided", not the best of the three.** This is declared as a required
sensitivity axis, not a widening: it triples the declared cell count up front
rather than being added after a number was seen.

Implementation: `--basis-offset=<usd>` shifts the GC-axis prices of every
timeline frame **down** onto the spot axis (cluster low/high/center, and
`max_pain` / `poc` / `w_sup` / `w_res` / `call_be` / `put_be` / `spot` in every
context). Bars are never touched, so P&L, lots, notional and spread stay on
real tradable prices — the arithmetic difference from shifting the bars up is
only in `notional`, and shifting levels is the direction that leaves the
account alone. `flow-momentum` reads no price level at all, so **its numbers
must be identical across the three offsets; if they are not, the shift is
wrong and the job stops.**

## Pre-check inside the registration, run before any cell is spent

**`--mode=hypotheses` may be structurally blind to options.** Read off source
before building: every `run_backtest_guarded` call in
`crates/fd-backtest/src/hypotheses.rs` (lines 796, 859, 1541, 1600, 1735,
2025, 2082) passes **`None`** for the timeline, and unlike `sweep.rs:122` there
is no `needs_options()` guard to say so — an options strategy in a hypotheses
batch would take **0 trades in silence**. If that holds, the registered gate
reading is not producible by the registered path without a code change, and the
change is: one new function that takes the timeline, with every existing
signature left alone so that the `None` path stays identical and no earlier
receipt moves.

**F0 (fires at pre-check, costs 0 cells).** If the four mechanisms still take
0 trades with a config that reaches the tape **and** the timeline threaded in,
then the blocker is not config and not calendar, and this job reports that
instead of a gate.

## Falsifiers, specific and firable

**F1 — the one that matters.** All four mechanisms miss the gate on **all
three** basis offsets in the guards arm (the only arm the owner permits) ⇒ the
project's founding thesis **has its first measurement, and it is negative**;
and the desk then knows that reaching 410 hours buys a second window that can
only confirm a negative, not open anything.

**F2.** A mechanism clears the gate on this window ⇒ **not** a candidate, a
**one-window reading**; the report must state what confirms it (a second
disjoint window of >= ~205 market-hours of overlap, which needs
`py/ingest/mt5_export.py` running).

**F3.** The verdict moves across p10 / mean / p90 ⇒ **undecided**, and the
basis needs the rolling correction before this axis is read again.

**F4.** A mechanism clears the gate while `--exit-mix` shows **its own rule
fired zero times** (the `tsmom/120d` trap: PF 2.236 and `SURVIVES` on exits
that were all `NEWS_FLAT` / `WEEKEND_FLAT`) ⇒ the row is not a reading of that
mechanism and is reported as such.

## Multiple-testing budget, declared before the first run

**Gate cells: 24** = 3 basis offsets x 4 strategies x 2 guard arms.

The parent registration's unspent **8** are the `4 strategies x 2 guard arms`
core at the mean offset. The **x3** is the basis sensitivity argued above and
is declared here, in advance, as part of the same single reading. **No
parameter grid is swept:** every cell runs the strategy's own
`default_params()` under `--fixed` over the whole window — no fold, no
selection, no best-of. Any widening past these 24 is a new registration.

Nothing else on this axis is touched. Still closed, do not re-ask: the gamma
wall (IV, bid, ask 100% null), venue-flag sweep/block detection (0 true on
every print), and per-level-type attribution (the cluster pools 9 level types,
5 marked experimental).

## How it will be read

* **Gate alone**, counted by hand against **40**, not against the tool's
  `need 30`. Which leg bound, stated: per addendum I/§I the expectancy leg is
  redundant exactly when `Lbar >= 0.250R`, and `lbar_line` prints it.
* **`PF_r` beside `PF_usd`, units named on both.** `PF_usd` read **higher** than
  `PF_r` on 320/452 cells (71%) in the `pfr-audit` measurement, and 5/200 cells
  straddle the gate's own 1.200 line between the two.
* **`E = total_r / n`**, never the 3-decimal `expectancy` field: a gross of
  ~0.0005 R/leg is smaller than that rounding.
* **Drawdown in USD** beside every profit number; `_pct` is a floor; a row
  printing `_pct > 100%` has blown the account and its PF is not read.
  `[markets.xauusd]` runs a 100.0 USD book at `min_lot`, so defect 17 applies:
  `PF_usd` here is the PF of a 0.01-lot book. That is one more reason `PF_r` is
  the one quoted.
* **`--exit-mix` on every cell**, and the mechanism's own rule verified to have
  fired.
* **The realised stop, in ATR and in points**, from the receipt's own
  `the method's own realised stop` line — and the **L1/L2/L3 class** of each
  mechanism read off `Exits::` in source, not off a parameter name.
* **This run's own print count and bar count**, printed by the binary's header.
  The store is being written while it is read (the tape moved 46,846 -> 53,245
  in two days), so two reads give two numbers and both are right; a receipt on
  this axis is comparable only to a run that printed the same counts.
* **Arm**: the guards arm is the only one the owner permits. A result that
  lives only in the no-guards arm is reported as **not tradable**.
* `null` is never printed as `0`.

## Effort declared

If this runs past ~4 hours of wall clock, stop and report what is measured
rather than overrun.
