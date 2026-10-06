# Registration: the bar interval as an axis — does a wider ATR stop make the spread affordable?

**Date:** 2026-10-06 (written and committed BEFORE the first measuring run)
**Axis:** `--interval=` on the whole scan. Every desk measurement to date ran on
15m bars (plus 1m/5m for the ICT batch). The flag exists; no scan has used it.
**Outcome:** pending — this file is the registration, not the result.

## Hypothesis, one sentence

Cost in R is `spread / stop`, the stop is a multiple of ATR, and ATR grows with
the bar interval, so the same 0.28 spread should be a SMALLER share of R on 1h
and 4h bars than the 1.06% measured on 15m — and if cost is the binding
constraint on this desk, the gate should improve with it.

## Prediction, split so it can be wrong in two different places

- P1 (arithmetic): `cost % of R` falls monotonically 15m > 1h > 4h.
- P2 (consequence): at least one of the 13 gold-intraday rows passes
  `PF >= 1.200 AND expectancy >= +0.050R` on BOTH windows at 1h or 4h, with
  at least 40 trades.

P1 and P2 are independent. P1 true and P2 false is the interesting outcome and
is the one the caller named as the falsifier.

## Falsifiers — each one is fireable from the receipts this run produces

- **F1 (the caller's):** `cost % of R` drops at 1h and/or 4h exactly as P1
  predicts, AND zero of the 1h/4h cells pass the gate on both windows with a
  usable trade count. Then cost is NOT the deciding variable on this axis and
  "go to a bigger bar to make it cheap" is a dead end. DECLARE IT DEAD.
- **F2 (sample size, fires before any conclusion):** if the median row at an
  interval has fewer than 40 trades in a window, that interval is **NOT
  MEASURED** in that window. Not a pass, not a fail, not 0 — `null`. The desk's
  own CRT record has the same rule giving PF 1.753 on 14 trades and PF 0.682 on
  178 trades of the same rule in the same window; below about 40 trades there is
  no reading to have.
- **F3 (the premise itself):** if `cost % of R` does NOT fall with the interval,
  then ATR does not scale the way the arithmetic assumed and the axis is dead on
  arrival, before the gate is even consulted.
- **F4 (self-attack, declared now because I already know it):** `[trading]
  max_hold_ms = 14_400_000` is FOUR HOURS and there is no per-market override
  for xauusd. On 4h bars a position entered at the open of bar `i+1` is force
  closed at that same bar's close. On 1h bars it gets four bars. So Arm 1 at 4h
  measures the CEILING, not the interval. If more than 50% of a 4h row's exits
  are TIMEOUT under Arm 1, Arm 1's 4h numbers are void as a timeframe reading
  and only Arm 2's count. `--exit-mix` is run on every Arm 1 cell to settle this
  with a count rather than an opinion.

## Arms, declared before running

- **Arm 1 — standing rules.** `max_hold_ms = 14_400_000` unchanged, the value
  every number in `docs/decisions/` was measured under. Intervals 15m, 1h, 4h.
  The 15m leg is the in-run baseline: the desk's 1.06% R is from a different
  batch and a different window, and a comparison across intervals has to come
  from the same batch, same windows, same binary.
- **Arm 2 — equal rope in BARS, not in wall clock.** 16 bars of 15m is exactly
  the 4h ceiling, so Arm 2 sets `max_hold_ms = 16 x bar_ms`: 57_600_000 ms at 1h
  and 230_400_000 ms at 4h. This is a per-market config override in THIS
  worktree only (`config/default.toml`, `[markets.xauusd.trading]`). It is
  declared here, before any result is seen, and it is the only way the 4h
  interval can be read as an interval at all. Arm 2 numbers are never compared
  against a `docs/decisions/` number measured under the standing ceiling.

## The bars are DERIVED, and that is a limitation of this whole run

`/e/rust/flowdesk/data/bars/` holds XAUUSD at 1m, 5m and 15m only. There is
**no XAUUSD-1h.parquet and no XAUUSD-4h.parquet**. `search.exe` does not
resample: `crates/fd-backtest/src/bin/search.rs:57` builds
`<data>/bars/<SYMBOL>-<interval>.parquet` and `read_bars` opens that path, so
`--interval=1h` fails with `Io(NotFound)` — probed and confirmed before this
registration was written, a plumbing probe that measured nothing.

So the 1h and 4h series are **built by me**, by aggregating the existing 15m
parquet into UTC-aligned buckets (open = first, high = max, low = min,
close = last, volume = sum, buckets with no 15m bar dropped). They are written
to `/e/rust/fd-a3/data/bars/` — never into `/e/rust/flowdesk`, which this run
only reads. Consequences a reader must carry:

- intrabar order inside a bucket is 15m-resolution, not tick. A cell whose stop
  and target both sit inside one 15m bar is resolved by the engine's stop-wins
  rule at 15m granularity, not finer.
- bucket boundaries are UTC, not the session. A 4h bucket is 00/04/08/12/16/20
  UTC and does not respect the 17:00 New York break the gold-intraday batch's
  `flat(1630,1815)` filter is written around. At 4h that filter can only drop
  whole 4-hour blocks, so the batch's session and flat filters are COARSER at
  4h than the batch author intended. This is a confound I cannot remove without
  changing code, and the brief forbids building.
- the 15m leg reads the real store file; the 1h and 4h legs read files I made.
  Every receipt prints its `bars from` path, so which is which is checkable.

## Multiple-testing book — declared count

- Arm 1: 3 intervals x 2 windows x 13 rows = **78 cells**
- Arm 2: 2 intervals x 2 windows x 13 rows = **52 cells**
- **Declared total: 130 cells (run x row).**

`--exit-mix` adds lines to rows already counted; it is not extra cells. Null
seeds are not cells. If I look at anything beyond 130 I publish the real number
beside this one, and I do not revise this number downward.

## How it will be read

- Gate, unchanged: `profit factor >= 1.200` AND `expectancy >= +0.050R`.
- Both windows. Window A `2025-07-01 .. 2025-10-01`, window B
  `2025-04-01 .. 2025-07-01`. One window is nothing.
- Minimum 40 trades per cell or the cell is `null`, per F2.
- Every percentile written carries its `null p50` next to it, because this
  desk's median null has PF 0.867 — under 1 — so a high percentile means
  "loses less than random entry at the same cost" and never "makes money".
- `cost % of R` is quoted from the `cost-matched null: ... cost X% of R` line
  the binary prints. I do not compute it myself and I do not retype it.
- Settings held fixed across every cell: `--mode=hypotheses --batch=gold-intraday`
  `--market=xauusd --fixed --seeds=20`, guards off, trail off, no companion.
  `--fixed` because walk-forward selection inside the window is itself a choice
  and this axis is about the interval, not about selection.

## What would reopen this

A native 1h or 4h export from the broker terminal, so the interval can be read
without my aggregation in the path. Or a `max_hold_ms` decision from the owner,
since Arm 2 is my own override and not a desk rule.

---

## Added 2026-10-06, after the runs — THE RESULT. Nothing above this line was edited.

**Outcome: the axis is DEAD. F1 fired. The arithmetic was right and it bought
nothing.** Cost in R fell by a factor of four from 15m to 4h, exactly as P1
predicted, and not one cell passes the gate on both windows with a trade count
anyone on this desk would read.

### The table

Arm 1, standing `max_hold_ms` = 4 h. The `cost%R` columns are quoted from the
`cost-matched null: ... cost X% of R` line the binary printed. `gate PASS` counts
rows with `PF >= 1.200 AND expectancy >= +0.050R`, trade count ignored, which is
why it is not the same thing as the survivor line the engine prints.

| window | interval | med cost%R | min cost%R | med trades | total trades | rows >=30 trades | gate PASS /13 | engine survivors | TIMEOUT exits |
|---|---|---|---|---|---|---|---|---|---|
| A | 15m | 4.16 | 2.63 | 64 | 1315 | 10/13 | 3 | ny-morning/donchian-breakout | 14% |
| A | 1h  | 2.11 | 1.31 | 17 |  358 |  4/13 | 7 | ny-morning/donchian-breakout | 54% |
| A | 4h  | 1.04 | 0.73 |  6 |  129 |  1/13 | 3 | none | **84%** |
| B | 15m | 2.85 | 1.68 | 60 | 1328 | 10/13 | 1 | none | 12% |
| B | 1h  | 1.37 | 0.61 | 18 |  355 |  5/13 | 3 | none | 55% |
| B | 4h  | 0.73 | 0.51 |  7 |  119 |  2/13 | 5 | none | **81%** |

Arm 2, ceiling scaled to sixteen bars (1h to 16 h, 4h to 64 h):

| window | interval | med cost%R | med trades | total trades | rows >=30 trades | gate PASS /13 | engine survivors | TIMEOUT exits |
|---|---|---|---|---|---|---|---|---|
| A | 1h | 2.09 | 17 | 339 | 4/13 | 7 | ny-morning/donchian-breakout | 11% |
| B | 1h | 1.37 | 17 | 337 | 5/13 | 5 | none |  5% |
| A | 4h | 1.04 |  5 | 108 | 0/13 | 5 | none | 31% |
| B | 4h | 0.73 |  7 | 102 | 0/13 | 2 | none | 32% |

### P1: confirmed, and it is the ATR scaling doing it

The stop widens with the bar and the fixed 0.28 spread shrinks against it. Same
rule, same window, Arm 1, `intraday/donchian-breakout`, window A:

| interval | control stop | cost | PF | expectancy | gate |
|---|---|---|---|---|---|
| 15m | 7.64 points  | 3.66% of R | 1.117 | +0.054R | fail |
| 1h  | 16.87 points | 1.66% of R | 1.340 | +0.084R | PASS |
| 4h  | 31.99 points | 0.88% of R | 1.130 | +0.029R | fail |

4h is **4.2x cheaper in R than 15m** on the same rule. Monotone in every one of
the six rule-by-window triples checked (`receipts/Z-summary.txt`, last block).
P1 holds.

### P2: false. Zero usable cells.

Seven of the 130 cells pass the gate in BOTH windows. **Zero of those seven have
30 trades in both windows, let alone the 40 this registration declared.** The
largest is `A2 1h intraday/ema-cross` at 21 and 17 trades. Under F2 all seven are
`null`, not passes:

    A1 15m compression/rsi-reversion   A: 10 trd PF 1.560 +0.213R (null p50 0.672) | B:  8 trd PF 2.278 +0.282R (null p50 0.615)
    A1 1h  intraday/ema-cross          A: 22 trd PF 1.669 +0.191R (null p50 0.740) | B: 18 trd PF 1.330 +0.120R (null p50 1.159)
    A1 1h  expansion/ema-cross         A:  4 trd PF 1.646 +0.193R (null p50 0.865) | B:  1 trd PF inf  +0.526R (null p50 0.000)
    A1 4h  london-open/donchian        A: 12 trd PF 1.455 +0.084R (null p50 0.650) | B:  7 trd PF 1.210 +0.082R (null p50 0.500)
    A2 1h  intraday/ema-cross          A: 21 trd PF 1.205 +0.087R (null p50 0.911) | B: 17 trd PF 1.538 +0.214R (null p50 1.106)
    A2 1h  expansion/ema-cross         A:  4 trd PF 1.639 +0.257R (null p50 0.979) | B:  1 trd PF inf  +1.796R (null p50 0.006)
    A2 4h  intraday/donchian-breakout  A: 22 trd PF 1.688 +0.198R (null p50 0.890) | B: 19 trd PF 1.369 +0.111R (null p50 1.494)

The survivor line the engine prints — gate AND outside its own null AND its
30-trade floor — fires in exactly three of the ten receipts, all on window A, all
naming the same row (`ny-morning/donchian-breakout`), and that row fails window B
at every interval in both arms. Seven of ten receipts print "Nothing survived".

### The gate PASS count RISES with the interval, and that is the trap, not a signal

3 of 13 at 15m window A, 7 of 13 at 1h. It looks like the cost drop working.
It is not: total trades fall 1315 to 358 to 129 as the interval widens, so each
row computes its PF from a quarter and then a tenth of the evidence and swings
further from 1.0 in both directions. The desk already has this number from the
CRT record (PF 1.753 on 14 trades against PF 0.682 on 178 trades of the same rule
in the same window) and this run reproduces the mechanism on a different axis.
The honest reading of "7 of 13 pass at 1h" is **noise widened by a smaller
sample**, and the both-windows intersection with a trade floor, which is empty,
is what settles it.

### F4, the self-attack this registration declared, fired

Arm 1 at 4h closes **84% (window A) and 81% (window B)** of its positions on
TIMEOUT, against 14% and 12% at 15m. Over the declared 50% trigger, so Arm 1 4h
rows are void as a timeframe reading: they measure the four-hour ceiling meeting
a four-hour bar. Arm 2 is the repair and it works — TIMEOUT drops to 31% and
32% — and Arm 2 at 4h still produces **zero rows with 30 trades** and zero
survivors in either window. Lifting the ceiling did not rescue 4h; it only made
the 4h failure honest.

Arm 2 at 1h: TIMEOUT 11% and 5%, against 54% and 55% in Arm 1. So the standing
ceiling was binding on more than half of all 1h trades, which is a fact no 1h
receipt before this one carried, and it is a reason to distrust any future 1h
measurement taken without either lifting it or counting it.

### F2 fired first, and it is the hard limit on this whole axis

Median trades per row: 64 and 60 at 15m, 17 to 18 at 1h, 5 to 7 at 4h. Both
windows, both arms. **4h is NOT MEASURED on this batch over a 3-month window.**
It is not a fail and it is not 0 — there is no reading there. 1h is below the
declared floor at the median; only 2 to 3 of its 13 rows ever reach 40 trades.
The widest bar is the cheapest in R and also the one with the least evidence, and
over three months those two move against each other faster than the cost falls.

### Multiple-testing book, settled

Declared 130 cells (run x row). Looked at **130**: ten runs x 13 rows, every row
parsed and printed in `receipts/Z-summary.txt`, including the six single-trade
rows whose PF prints as `inf`. One extra run exists and is NOT a cell — the 15m
window A run was executed twice, identically, the first time as a timing check
before the loop, and the second overwrote it and is the receipt kept. One
plumbing probe (`--interval=1h` with no parquet present) returned
`Io(NotFound)` and measured nothing. A second plumbing failure is worth
recording: the first build of the derived parquets used SNAPPY and the binary
refused them with `Parquet error: Disabled feature at compile time: snap`, so
they were rewritten as ZSTD to match the store, which is why
`scripts/htf_bars_from_15m.py` pins the codec.

### What was NOT measured, and why

- **1D and anything wider.** Not attempted. At 4h a 3-month window already
  yields only 397 to 408 bars and 5 to 7 trades a row; 1D would be about 65
  bars. There is no sample to read, so running it would produce numbers and not
  evidence.
- **Native 1h/4h bars.** The store has none. Every 1h and 4h figure here comes
  from bars I aggregated from 15m, so intrabar sequencing inside a bucket is
  15m-resolution and the stop-wins rule is applied at that granularity, not at
  tick level.
- **Session-correct 4h buckets.** My buckets are UTC (00/04/08/12/16/20) and the
  filters in this batch were written for 15m resolution. At 4h they can only cut
  whole four-hour blocks, so `ny-morning` 08:00-12:00 becomes a single bar a day
  and `asia` 19:00-02:00 straddles bucket edges. The session rows at 4h measure a
  coarser filter than their label claims, and I could not fix that without
  changing code.
- **Whether a longer window rescues 1h/4h.** Both windows are fixed at three
  months. The one change that could make this axis readable, several years at 4h
  where a row would hold an order of magnitude more trades, is outside what was
  authorised, and the long history in the sealed store is not to be opened.
- **Any market but gold.** `xauusd` only, as instructed. Whether the same cost
  collapse appears on BTC, where the spread is 5.16% of R on a weekday and 9.96%
  at the weekend and therefore has far more room to fall, is untouched and is the
  obvious next question if anyone revisits this.

### The one number to keep

**4.2x.** That is how much cheaper the same rule makes its spread in R at 4h than
at 15m — 0.88% against 3.66% of R, `intraday/donchian-breakout`, window A, Arm 1.
It is the largest cost reduction this desk has produced without asking a broker
for a discount, and it moved nothing. The same rule reads `fail / PASS / fail`
across 15m / 1h / 4h in window A and `fail / fail / PASS` in window B. The
verdict does not travel with the cost. On the gold-intraday batch over these two
windows, **cost in R is not the binding constraint**, and "go to a bigger bar to
make it cheap" is a dead end — which is exactly the falsifier the registration
said would kill this axis.

### What would reopen it

A native multi-year 1h or 4h export, so that a 4h row carries thousands of
trades instead of five. Until then the axis cannot be distinguished from its own
sample size, and this record exists to stop the next person re-running it on
three months.
