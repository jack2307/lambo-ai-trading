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

---

## Note added 2026-10-09, after the run — RESULT: F0 did not fire, F3 DID. The thesis has its first measurement and the measurement is NOT DECIDED.

Tool: `search.exe` built from this branch
(`cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin
search`, target dir `target-of/` inside this worktree).
Receipts: `docs/research/runs/2026-10-09-options-one-window/` — six files,
`offset-<basis>-<arm>.txt`, plus `SUMMARY.txt` with all 24 cells in one table.
Command, identical in all six but for `--basis-offset=` and `--guards`:

    search --market=xauusd --interval=15m --mode=hypotheses --fixed --exit-mix
           --config=config-ofirst --data=/e/rust/flowdesk/data --seeds=200
           --batch-file=docs/hypotheses/2026-10-09-options-one-window.toml
           --basis-offset={41.26|43.70|45.78} [--guards]

**No percentile is published** and none of the numbers below is one.

### What this run's own header says, because the store is being written while it is read

    prints   53,357 -> 53,396 -> 53,436 -> 53,479   across the runs below
    frames    9,429 / 9,431                          (step 300,000 ms)
    bars     100,586  XAUUSD-15m  2022-06-16 -> 2026-09-17  (frozen)
    overlap     786 of 100,586 bars = 0.8%  <- the whole measurement
    GC-1m    18,393 bars 2026-09-06 -> 2026-10-09 (grew 16,903 -> 18,393 since 10-08)

Two reads of this axis give two print counts and both are right. Any future
receipt here is comparable only to a run that printed the same ones.

### F0 did not fire — but the reason the thesis was never measured is now a counted fact, and it is TWO blind spots, neither of them data

1. `options_source = "none"` on `[markets.xauusd]` — config. Changing it in
   `config-ofirst/` alone made `timeline: none` become `timeline: 9429 frames`
   over the same store.
2. **`--mode=hypotheses` was structurally blind to options.** Every
   `run_backtest_guarded` call in `crates/fd-backtest/src/hypotheses.rs` passed
   `None` for the timeline, and — unlike `sweep.rs:122`, which refuses such a
   row — nothing checked. So the four mechanisms could sit in a pre-registered
   batch and take **0 trades in silence**, which prints as a gate miss and
   reads like a measurement. Fixed additively here
   (`run_hypothesis_fixed_options`, every existing signature untouched, the
   `None` path argument-for-argument identical). **The walk-forward hypotheses
   path and `rescore` are still blind; this binary now refuses an
   options-reading row on them instead of scoring it.**

So the record's "not rejected — unasked" was not an oversight anyone could see
from a receipt. It was enforced by the tool.

### Only ONE of the four mechanisms reaches the desk's 40-trade floor at all

Counted by hand against **40**, not the tool's `need 30`:

    level-reversion   94 - 120 trades   <- the only row above 40, in all six cells
    maxpain-magnet    24 -  29 trades   FLOOR MISS in all six
    flow-at-level     20 -  30 trades   FLOOR MISS in all six
    flow-momentum      4 trades         FLOOR MISS in all six

**18 of the 24 cells have no verdict**, on the trade-count leg alone. The
pre-check's ceiling of 235 entries and its "one window is at the floor today"
are true for `level-reversion` and for nothing else: occupancy (one position,
4 h cap) plus each rule's own extra conditions cut the realised count to a
fifth or less of the ceiling. `maxpain-magnet` at 24-29 and `flow-at-level` at
20-30 would also be printed by the tool as clearing `need 30` at 30 while
missing the desk's 40 by ten — the brief's §4 trap, live.

### F3 FIRES. A 4.52 USD change in a CONSTANT basis moves level-reversion from PF_r 0.17 to a gate pass

`level-reversion`, the registered first mechanism, guards arm (the only arm the
owner permits):

    basis +41.26 (p10)   100 trades   PF_r 1.0057   E +0.0036 R   Lbar 0.6378 R   DD  7.22 USD   -> MISS (PF leg)
    basis +43.70 (mean)  108 trades   PF_r 0.1735   E -2.8924 R   Lbar 3.4998 R   DD  2.84 USD   -> MISS (PF leg)
    basis +45.78 (p90)    94 trades   PF_r 1.3883   E +0.2170 R   Lbar 0.5589 R   DD  3.77 USD   -> clears all three legs

and in the no-guards arm the p90 cell clears too (109 trades, PF_r 1.2314,
E +0.1380 R, DD 10.97 USD = 9.57% of peak).

**One offset of three passes, so per this registration's own F3 the answer is
NOT DECIDED** — and the three offsets are p10, p50 and p90 of the basis as
measured, not a widened search. The verdict is a property of a constant nobody
has a rolling correction for. That is a **twelfth window artefact: the basis
constant**, and it is the one this axis cannot be read without fixing.

### The mean-offset rows are degenerate, and the degeneracy is the finding

At +43.70 only, both structural-stop mechanisms blow up in R:

    level-reversion  Lbar  3.4998 R  |avg_loss_r|  5.4779  avg_mae  -3.642 R
    flow-at-level    Lbar 11.0194 R  |avg_loss_r| 15.0264  avg_mae -11.107 R

against Lbar 0.47-0.72 R at both neighbouring offsets. A handful of entries
land where `risk = entry - (cluster.low - 0.3 x ATR)` is near zero, and
`r = points / risk` explodes. **On those two rows the gate is reading a unit,
not a method** — the same class as addendum 7's L1 finding, arriving on an
L2 row through a STRUCTURAL stop instead of a self-managed one.

### Defect 14 fired, twice, and the owner's guards are what stand between these mechanisms and a blown book

No-guards arm, basis +43.70:

    level-reversion  max drawdown 306.86 USD = 303.85% of peak, equity to -305.61 USD on a 100 USD book
    flow-at-level    max drawdown 311.55 USD = 309.48% of peak, equity to -310.81 USD

Both rows **blew the account and the engine kept trading at `min_lot`**, so
their printed `PF_usd` (0.044 and 0.007) is not to be read. The same two cells
with `--guards` printed **2.84 USD** and **4.17 USD** of drawdown. The guard
doing it is `max_open_loss_r = 2.0`. This is the first measured instance in the
record of addendum IV's blow-up defect actually firing, and it is a second
reason the no-guards arm is not a trading arm.

### The notional cap rides EVERY trade of both cluster mechanisms, and both gate legs are blind to it

Guards arm, `sized down` out of trades taken:

    level-reversion   97/100,  107/108,  94/94
    flow-at-level      20/20,    30/30,  28/28
    maxpain-magnet      2/28,     1/24,   1/24
    flow-momentum       4/4

`max_notional_pct_equity = 300%` on a 100 USD book caps lots on essentially
every entry of the two cluster rules. `r = points / risk` never sees it and
`PF_usd` is the PF of a capped 0.01-lot book (defect 17). Said out loud per
addendum 7C.

### The real stop, measured — and the arm was chosen on a cost/R the mechanism does not run at

From each receipt's own `the method's own realised stop` line:

    level-reversion  0.542 - 0.609 ATR14(15m) =  5.28 -  5.63 points   cost/R 4.97 - 5.30%
    flow-at-level    0.468 - 0.594 ATR14(15m) =  4.68 -  5.53 points   cost/R 5.06 - 5.98%
    maxpain-magnet   2.014 ATR14(15m)         = 17.24 - 18.34 points   cost/R 1.54 - 1.64%
    flow-momentum    1.516 ATR14(15m)         = 12.82 points           cost/R 2.21%

The pre-check selected this arm on **cost/R 2.43% at 1.5 x ATR14(15m) = 8.227
USD**. Neither cluster mechanism runs at 1.5 ATR: the receipts say in their own
words *"the method DECLARES stopAtr 1.50 and does not use it"*, because the
stop is the cluster edge. **The real cost/R on the two mechanisms the arm was
chosen for is roughly twice the figure it was chosen on.** The horizon
argument survives (5% still beats `GC-1m`'s 21.19% by far); the number does
not. Brief §8: the measured number wins.

### L1 / L2 / L3, read off `Exits::` and not off a parameter name

`registry.rs:256` makes `Exits::Engine` the default and `builtin.rs:737` is the
only override in that file — it belongs to `buy-and-hold`. **None of the four
overrides it, so all four are ENFORCED-stop rows and none is L1.** This is the
first time a real stop has been measured on any of them.

    level-reversion  L2  enforced stop = cluster.low - 0.3 x ATR (STRUCTURAL); target None -> risk x reward_risk 1.8
    flow-at-level    L2  same shape
    flow-momentum    L2  enforced stop = close -/+ 1.5 x ATR;  target None -> risk x 1.8
    maxpain-magnet   L3  enforced stop = close -/+ 2.0 x ATR;  target = max_pain, an ABSOLUTE PRICE

`maxpain-magnet` is the only L3 row here and the only one whose declared stop
is the stop it runs. The two L2 cluster rows are the pair whose R unit blew up
above — an L2 row with a structural stop can degenerate the same way an L1 row
does, which addendum 7's two-class reading did not cover and addendum 8's
three-layer reading only half does.

### Which gate leg bound, on all 24 cells

**`Lbar >= 0.250R` on every one of the 24**, so per §I the expectancy leg is
**REDUNDANT in all 24** — it never bound once on this axis. What bound:

    40-trade leg   18 of 24 cells  (maxpain-magnet, flow-at-level, flow-momentum, every offset, both arms)
    PF leg          6 of 24 cells  (level-reversion), of which 2 passed (both at p90)

The identity `E = Lbar x (PF_r - 1)` held on **24/24** cells, largest residual
**0.00005 R**.

### PF_r against PF_usd, units named

`PF_usd` is not the same reading. The widest gaps:

    level-reversion +43.70 guards    PF_usd 1.1790 (USD)  vs  PF_r 0.1735 (R)   gap -1.0055  <- OPPOSITE SIDES of the gate
    flow-at-level   +43.70 guards    PF_usd 0.6238 (USD)  vs  PF_r 0.0400 (R)   gap -0.5838
    flow-at-level   +45.78 guards    PF_usd 2.1822 (USD)  vs  PF_r 1.9849 (R)   gap -0.1973
    level-reversion +45.78 guards    PF_usd 1.4015 (USD)  vs  PF_r 1.3883 (R)   gap -0.0132

One cell — `level-reversion` at the mean offset in the guards arm — straddles
the gate's own 1.200 line between the two units: **1.179 in USD and 0.174 in R,
and only the USD reading is anywhere near a pass.** It is reported as a MISS on
both, because `PF_r` is the one that is a property of the method.

And the same 30-trade set of `flow-at-level` at +43.70 prints `PF_usd` **0.624**
with guards and **0.007** without, on **identical** `E = -10.5788 R` and
identical `Lbar 11.0194 R` — the guards changed only the lots. A reader of the
USD column would call those two different methods.

### The mechanism's own rule fired on every row — F4 does not fire

`--exit-mix`, guards arm:

    level-reversion  STOP 51-68, TARGET 35-40, TIMEOUT 1-2, WEEKEND_FLAT 1, END_OF_DATA 1   mean hold 33-38 min
    maxpain-magnet   STOP 7-8, TARGET 10-13, TIMEOUT 3-5, NEWS_FLAT 1, END_OF_DATA 1        mean hold 117-142 min
    flow-at-level    STOP 13-22, TARGET 6-15, TIMEOUT 0-1                                   mean hold 17-31 min
    flow-momentum    STOP 3, TARGET 1                                                       mean hold 68 min

No row is a `tsmom/120d` artefact: the guard exits are 0-2 per row and the
method's own stop or target closed the large majority everywhere. For
`level-reversion` and `flow-at-level` the `STOP` exits ARE the mechanism's own
claim, because their stop is the cluster edge.

### The basis-shift control held

`flow-momentum` reads no price level, and its six cells are identical at all
three offsets within each arm — 4 trades, `Lbar 0.7581 R`, `PF_r 0.5897`,
realised stop 1.516 ATR = 12.82 points, exits STOP 3 / TARGET 1. The shift
moved prices and only prices.

### Percentiles not published, and here is why in this axis's own numbers

`count match`, method against its matched null:

    level-reversion  0.99 - 1.00   in band
    flow-at-level    1.63 - 2.45   OUT
    maxpain-magnet   1.75 - 2.04   OUT
    flow-momentum    12.25         OUT

Three of the four mechanisms are read against a control that is not their size.
On top of that every row printed `spread paid ... cost match 1.77 - 22.90 **
outside the band **` and `exposure ... ratio 1.09 - 24.61 ** the control did
not collect the drift the method did **`, and every row's null p50 is **below
1.000** (0.700 - 0.907), so a high percentile here would mean "loses less than
random entry", not "makes money". The gate is reported alone.

### Multiple-testing ledger: declared 24 gate cells, examined exactly 24

Declared: 24 = 3 basis offsets x 4 strategies x 2 guard arms, `--fixed` at each
strategy's own `default_params()`, no grid, no fold, no selection.
Examined: **24.** Nothing was loosened after a number was seen; the 40-trade
floor is the brief's own and was written down before the build. Of the parent
registrations, `agent/n4` §6's 9 cells and the pre-check's carried-forward 8
are hereby **spent**, inside these 24.

### The verdict, in the words this registration fixed in advance

* **F0** did not fire: the mechanisms run.
* **F1** did not fire: `level-reversion` clears all three gate legs on this
  window at the p90 basis, in both arms.
* **F2** applies to that pass: it is **one window and one of three offsets**,
  not a candidate.
* **F3 FIRED.** The verdict moves across p10 / mean / p90, so the reading is
  **NOT DECIDED** and the basis needs a rolling correction before this axis is
  read again.
* **F4** did not fire.

**The honest one-line answer: the founding thesis has been measured for the
first time, one mechanism of four can even be measured against the desk's
floor, and its sign is set by a basis constant rather than by the market.**

### What would settle it, in order, and the first item is not the tape

1. **A rolling GC-XAUUSD basis**, not a constant. The whole verdict lives
   inside the p10-p90 width of the basis that is already measured, so this is
   not a refinement — it is the measurement. (`agent/n4` §7 item 3 estimated
   1-2 days.)
2. **`py/ingest/mt5_export.py` for XAUUSD, running.** The bars are frozen at
   2026-09-17 while the tape runs to 2026-10-09, so the overlap is stuck at
   196.5 h and the second window cannot arrive, however long the collector
   runs. 28% of the tape has no tradable bar at all.
3. Only then a second disjoint window of >= ~205 market-hours, which is what
   the desk gate actually asks for.
4. Not worth doing before (1): per-level-type attribution, any parameter
   sweep, and the three mechanisms that cannot reach 40 trades.

### Defects counted, not fixed

* **Defect 14 observed firing** (two rows above 100% drawdown, still trading).
* **Defect 17 observed** on every row: a 100 USD book at `min_lot`, with the
  notional cap additionally binding on essentially every cluster-rule entry.
* **New, and the reason this job exists:** the hypotheses and rescore paths
  carried no timeline and no guard to say so. Repaired for `--fixed` only; the
  walk-forward hypotheses path and `rescore` now **refuse** an options-reading
  row rather than scoring it at zero trades.
* **One defect of my own, found and fixed before any cell was spent:** the
  first `shift_basis` was written `let (timeline, offset) = (timeline?,
  offset?)`, which silently returned `None` whenever `--basis-offset` was
  absent and turned every un-offset run into `timeline: none` over 53,396
  prints — the exact failure this job exists to undo. Caught because the header
  printed the frame count.
