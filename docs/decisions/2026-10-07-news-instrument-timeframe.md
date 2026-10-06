# The news rule on a second feed and a second bar size

**Date:** 2026-10-07 (pre-registration; written and committed before the first run)
**Branch:** `agent/news-tf`, cut from `agent/n1`
**Parent:** `docs/decisions/2026-10-06-news-entry.md` — which closed saying its
survivor "has never been read on a second instrument, a second timeframe, or
with a fill model that reflects what a market order does in those minutes".
This file is the first two of those three.
**Outcome:** open. This file is the registration, not the result.

## 0. What is inherited and not re-opened

`agent/n1` registered twelve rows of `news-pulse` and measured them on
`xauduka` 15m. One row passed the desk's gate on two non-overlapping four-year
windows in both guard arms:

    np-brk-p4-m15 = probeBars 4, mode 0 (with the impulse), minMoveAtr 1.5,
                    filter newsonly:45/60:3:USD
    A' 2022-01-01→2026-01-01  PF 1.636 / +0.106R / 96 trades
    B' 2018-01-01→2022-01-01  PF 1.734 / +0.114R / 90 trades

Nothing about the rule, the gate, the windows A'/B', the null or `matched_rate`
is changed here. The rows in `receipts/news-entry.toml` are used **byte for
byte** for the instrument arm. Only two things vary: **which feed** and **which
bar size**.

## 1. Hypothesis

**(a) Instrument.** If the rule is a property of the market, it survives on the
broker's own bars (`XAUUSD`, MT5, the feed the desk would actually trade) and
not only on a third party's bid feed (`XAUDUKA`, Dukascopy).

**(b) Timeframe.** A release bar whose range is 5.96× the day's median (XAUDUKA,
504 events) or 7.02× (XAUUSD, 137 events) — the parent's §2, measured — is one
15m candle. If that expansion carries tradable structure, a finer bar should
see *more* of it, not less. The cost of looking is that the stop shrinks with
the bar, and `cost/R = spread / stop` follows the horizon: so a 1m read is only
worth anything if the post-release move is **several times** the 1m stop.

Prediction, stated so it can come back no: the instrument arm **passes** (the
feeds agree) and the short-probe timeframe arm **fails at every probe on both
windows**, because the first minutes after a release are a variance event whose
direction has not yet been decided, and the stop there is small enough that the
spread eats the whole of R.

## 2. Measured BEFORE this file was committed, and disclosed

These are store and calendar checks — spans and event counts, no profit factor,
expectancy or trade count — read so the windows below are arithmetic rather
than hope. Each number was read in this session.

Bar stores, read straight out of the parquet (`pyarrow`, min/max of `time`):

| store | rows | span |
|---|---|---|
| `XAUDUKA-15m` | 378,749 | 2010-06-01 00:00 → 2026-05-31 23:45 |
| `XAUDUKA-5m` | 1,135,389 | 2010-06-01 00:00 → 2026-05-31 23:55 |
| `XAUDUKA-1m` | **5,635,777** | **2010-06-01 00:00 → 2026-05-31 23:59** |
| `XAUUSD-15m` | 100,586 | 2022-06-16 10:30 → 2026-09-17 13:00 |
| `XAUUSD-5m` | 101,242 | **2025-04-11 01:05 → 2026-09-15 16:00** |
| `XAUUSD-1m` | 100,000 | **2026-06-02 20:15 → 2026-09-11 20:56** |

Two facts the brief did not carry. **All three `XAUUSD` stores stop at about
100,000 rows** — 100,586 / 101,242 / 100,000 — so they are the most recent
100k bars of each interval and nothing older. The 1m store covering only
2026-06-02 → 2026-09-11 (brief, from agent n3) is confirmed; **and the 5m store
is equally short**, 2025-04-11 → 2026-09-15, which the brief did not say. So
there is **no broker-feed store at any interval below 15m** that reaches back
far enough to read. All sub-15m work here runs on `XAUDUKA`, and that is a
store limit, not a choice.

Calendar, `data/news/events.parquet`, `currency == USD and impact >= 3`:
**562** such events of 747 rows, 2010-01-08 → 2027-12-08. Counted per window:

| window | USD impact-3 events |
|---|---|
| A' 2022-01-01 → 2026-01-01 (parent's) | 127 |
| B' 2018-01-01 → 2022-01-01 (parent's) | 134 |
| `xauusd` full span 2022-06-16 → 2026-09-18 | **139** |
| `xauusd` 2022-06-16 → 2024-09-01 | **69** |
| `xauusd` 2024-09-01 → 2026-09-18 | **70** |
| `xauusd-5m` span 2025-04-11 → 2026-09-15 | 44 |
| `xauusd-1m` span 2026-06-02 → 2026-09-11 | 9 |

Arrival rate is ~32 a year and flat (33/32/32/32/32/32/32/32/32/33/37/32/32/32/
32/31/36 for 2010–2026), so these counts are a property of the calendar.

Bars kept by each declared window, read from the tool's own `bounds:` line with
`--mode=nothing` (a mode no branch handles, so it prints the header and runs
nothing — **zero cells**):

    xauusd  15m  2022-06-16→2026-09-18   100,586 bars
    xauusd  15m  2022-06-16→2024-09-01    52,275 bars
    xauusd  15m  2024-09-01→2026-09-18    48,311 bars
    xauduka  5m  2022-01-01→2026-01-01   281,178 bars
    xauduka  5m  2018-01-01→2022-01-01   283,110 bars
    xauduka  1m  2022-01-01→2026-01-01 1,404,272 bars
    xauduka  1m  2018-01-01→2022-01-01 1,408,090 bars

## 3. Windows, declared in advance

**Instrument arm.** `xauusd` 15m. The store's span is 2022-06-16 → 2026-09-17,
which is **shorter than the parent's four-year windows**, so the split is
declared here, before any outcome, and is split **on event count** rather than
on calendar length so the two halves carry the same statistical weight:

- **I1, in-sample, `2022-06-16 → 2024-09-01`** — 69 events, 52,275 bars
- **I2, out-of-sample, `2024-09-01 → 2026-09-18`** — 70 events, 48,311 bars
- **I-full, `2022-06-16 → 2026-09-18`** — 139 events, 100,586 bars. Run once as
  the best-powered single read of the feed, **not** counted as a second window
  and **not** a gate pass on its own.

The split point 2024-09-01 is the calendar date that halves the event count
(69 vs 70) and was chosen on the event count alone, before any backtest.

**Timeframe arm.** `xauduka` 1m and 5m on **exactly** the parent's A'
(2022-01-01 → 2026-01-01) and B' (2018-01-01 → 2022-01-01), so bar size is the
only thing that differs from the published 15m result. 127 and 134 events.

## 4. Sample size, and the thing that can make the instrument arm unreadable

One entry per release caps the book at the event count. On A' the parent's
`np-brk-p4-m15` took **96 trades from 127 events** — a 76% pass rate through
the 1.5-ATR gate. At that rate I1 (69 events) yields ~52 and I2 (70) ~53, both
over the 40-trade floor but **thin**, and a stricter gate on this feed could
drop either below it.

Declared now, not after seeing the number: **any window where a row takes fewer
than 40 trades is reported as not measured, not as a fail.** A failure there
would be a failure of the store's length, which is the one thing about this arm
that was known before it ran.

And brief §4(v) applies with full force. `matched_rate` probes once at
`entryRate = 0.02` on the admitted bars and scales linearly; I1/I2 admit ~69
and ~70 bars against the ~127 of A', so the probe draws about one trade and the
control's calibration has little purchase. Therefore: **no percentile is
published for I1, I2 or any 1-bar-wide gate unless that row's own count match
is inside `COUNT_MATCH_BAND = 0.25`.** Where it is outside, the number is
withheld and the gate carries the row alone. `null p50` is printed beside every
percentile that is published. `matched_rate` is not touched.

## 5. The two translations of "a second timeframe", both declared

The rule is written in **bars**, so "the same rule at 1m" has two readings and
both are registered, because choosing one after seeing the other would be a
search.

**T-clock — the same wall-clock minute, a finer ruler.** The 15m survivor
enters at +45 minutes. Keep that minute and let the bar size change what the
impulse and the stop are measured over:

| interval | probeBars | release bar → signal bar | signal-bar gate |
|---|---|---|---|
| 5m | **10** | offset 0 → +45 min | `newsonly:45/50:3:USD` |
| 1m | **46** | offset 0 → +45 min | `newsonly:45/46:3:USD` |

Rows: `mode` {0, 1} × `minMoveAtr` {0.5, 1.5} = **4 rows per interval**. This is
the direct answer to "is the 15m number an artefact of 15m aggregation": the
entry minute, the event, the direction rule and the horizon are identical, and
only the resolution of `impulse`, `range` and `ATR(14)` differ. Note the
consequence, declared: `ATR(14)` on 1m bars is 14 **minutes** of range, so the
1.5-ATR gate is a different absolute threshold at each interval. That is the
scale-consistent translation of the gate and it is what makes this a
resolution test rather than a new rule.

**T-probe — structure inside the expansion.** This is the parent's own stated
gap: *"a 6× range event inside one 15m bar may well be a tradable sequence of
1m bars"*. Keep the parent's row shape — `probeBars` {1, 2, 4} × `mode` {0, 1}
× `minMoveAtr` {0.5, 1.5} = **12 rows per interval** — and let the bars be
minutes:

| interval | probe | signal bar opens at | gate on the row |
|---|---|---|---|
| 1m | 1 / 2 / 4 | +0 / +1 / +3 min | `newsonly:0/1` / `1/2` / `3/4` `:3:USD` |
| 5m | 1 / 2 / 4 | +0 / +5 / +15 min | `newsonly:0/5` / `5/10` / `15/20` `:3:USD` |

Each gate is one bar wide at that interval, which is what makes the matched
null a **timing** match: the control draws from the same single bar after the
same release as the method (parent §5).

**Declared before running: the desk's own guard forbids this entire arm.**
`--guards` prints `news flat 60/30 (impact>=3, USD)` — no entry from 60 minutes
before to 30 minutes after a high-impact USD release. Every T-probe entry minute
(+0 to +20) is inside that blackout, so the guarded arm is expected to take
**0 trades on all 24 T-probe rows**. It is run and reported anyway (brief
§4(ii)); it is not worked around and `[trading.guards]` is not touched. The
T-clock arm at +45 min clears the blackout, as the parent's probe 4 did.

## 6. Cost, which is the whole reason the short probes might be dead

`cost/R = spread / stop` and follows the horizon (brief §3 item 3). The brief
quotes 4.04% for gold at a 1.5-ATR stop and 19.9% at 1m (agent n4). **Neither
is this rule's number**: `news-pulse` stops one *impulse range* away, and the
parent's receipts print a realised stop of **5.0 ATR = 17–23 price points** and
`cost 1.22%–1.65% of R` on the 15m rows. So the 15m survivor's true cost
fraction is ~1.2–1.7% of R, not 4.04%.

Declared reading, not a new axis: for **every** row of this job the printed
`cost-matched null: control stop <x> ATR = <y> points, cost <z>% of R` line is
quoted with the stop in points beside it, and set against that row's measured
expectancy in R. No remembered percentage is used anywhere. That line is free —
the tool already prints it — so no spread fan is declared here. The parent
already ran the fan on 15m and measured the release-minute spread off the live
account (`data/spreads/XAUUSD_sc.csv`, max 0.260 at +45 to +60 min vs the 0.28
charged); repeating it is not this job's gap.

Declared in advance: if a T-probe row's printed stop is small enough that
`cost/R >= 100%`, the row is reported as **structurally untradeable** — the
spread exceeds the whole of R — and not as an expectancy failure.

## 7. Gate — not adjustable

    profit factor >= 1.200  AND  expectancy >= +0.050R  AND  >= 40 trades
    on BOTH windows of the arm, in the SAME guard arm

Instrument arm: both of I1 and I2. Timeframe arm: both of A' and B'.
One window is nothing. A percentile is not a gate. `--exit-mix` on every run.

## 8. Falsifiers — specific, and each can fire on a printed line

1. **F1, the brief's own instrument falsifier.** If `np-brk-p4-m15` takes >= 40
   trades on both I1 and I2 and **fails** the gate on either, the rule does not
   hold on the broker's own bars and is a property of the Dukascopy bid feed,
   not of the market.
2. **F2, resolution.** If the T-clock rows fail on A' and B' while the parent's
   15m probe-4 row passes on the same windows with the same entry minute, the
   15m number is a property of bar aggregation.
3. **F3, the brief's timeframe falsifier.** If every T-probe row fails on both
   A' and B' **while** the printed geometry shows the post-release move running
   several times its own stop, then the scheduled expansion contains no
   tradable structure at 1m or 5m, and the desk can close that direction.
4. **F4, void rather than result.** Any window where a row takes < 40 trades,
   or where count match falls outside 0.25, or where the row's own exits never
   fire, is reported **void**. Not as a fail.
5. **F5, the ceiling.** If >= 80% of a row's exits are `TIMEOUT`, the row
   measures `max_hold_ms = 4 h`, not the rule. Reported as such with the ratio,
   and **not** pursued: `agent/news-geom` owns the exit geometry and this job
   does not duplicate it.
6. **F6, `wrong_side_stop`.** Any row printing it non-zero is reported
   unreadable and not fixed (brief §4(vi)).

## 9. Multiple testing — declared BEFORE the first run

| block | cells (run x row) |
|---|---|
| plumbing: `xauduka` 1m, A', T-probe 12 rows, `--seeds=20`, no guards — a timing and flag check, numbers NOT read at the gate | 12 |
| I: `xauusd` 15m, 12 rows x {I1, I2} x 2 guard arms | 48 |
| I-full: `xauusd` 15m, 12 rows x 1 window x no-guards, reference | 12 |
| T-clock 5m: 4 rows x {A', B'} x 2 guard arms | 16 |
| T-clock 1m: 4 rows x {A', B'} x 2 guard arms | 16 |
| T-probe 5m: 12 rows x {A', B'} x 2 guard arms | 48 |
| T-probe 1m: 12 rows x {A', B'} x 2 guard arms | 48 |
| **declared total** | **200** |

The plumbing cell is declared here on purpose: the parent's ledger had to
confess twelve undeclared smoke cells, and the fix is to declare them, not to
skip the smoke run.

No spread fan, no `--null-sides=exposure`, no `--mode=rescore` is declared.
Contingency, counted if used: if a row's printed long share falls outside
0.40–0.60 it is re-run with `--null-sides=exposure` (12 rows x 1 window = 12
cells per re-run), and that is recorded in a dated note at the end of this file.

The real `(run, row)` count is published at the end against the 200. Anything
looked at beyond it goes in a dated note appended **below**, never by editing a
line above.

## 10. What this will not say

- Nothing here is a fill model. A market order into the first minute after a
  release is the one thing the T-probe arm most needs and cannot have: the
  engine fills at the next bar's open at one constant spread.
- Nothing here measures a per-minute spread at 1m. The parent's spread log
  samples about three reads a minute, which cannot see the first seconds.
- Nothing here measures the broker's feed below 15m, because §2 shows no such
  store exists at a readable length.
- Nothing here measures entering **before** a release, which is a separate
  branch (`agent/news-pre` has it), nor the exit geometry (`agent/news-geom`).
- `events-extended.parquet` is not used. The `news:` line of every receipt is
  quoted to prove which calendar each run read.
- `data-sealed/` is not opened, read or counted.

## What each role said

- **data-integrity:** the `XAUUSD` 100k-row truncation in §2 is the finding to
  carry out of this file whatever the backtests say; the brief's premise that a
  broker-feed 5m store existed is wrong and §2 shows the span.
- **adversary:** the split point in §3 is the move to watch. It is declared
  before any run, it is chosen on event count (69 vs 70), and the full span is
  published beside the halves so the halving cannot hide anything.
- **execution-realist:** BLOCK on any T-probe number quoted without its printed
  stop in points and its `cost % of R` beside it.
- **risk:** both guard arms on every row, including the 24 rows the guard is
  expected to refuse outright.
- **historian:** the parent closed with three named gaps; this takes two.

## What would reopen this

A broker-feed store below 15m that reaches back more than a year; a limit-fill
model; a per-second spread series at a release.

---

## Note added 2026-10-07, results. Append-only; no line above is altered.

### The gate: 0 cells pass, on either arm

**Instrument arm, `xauusd` 15m.** 0 of 12 rows pass both I1 and I2 in any arm.
The parent's survivor, on the broker's own bars:

| row | window | trades | PF | expectancy | pct (null p50) | count match |
|---|---|---|---|---|---|---|
| `np-brk-p4-m15` | I1 no guards | 58 | **2.226** | **+0.158R** | 99% (1.020) | 1.17 |
| `np-brk-p4-m15` | I1 guards | 58 | **2.270** | **+0.161R** | 99% (1.017) | 1.17 |
| `np-brk-p4-m15` | **I2** no guards | 45 | **1.125** | **+0.045R** | withheld (0.000) | **0.04 unmatched** |
| `np-brk-p4-m15` | **I2** guards | 45 | **1.116** | **+0.042R** | withheld (0.000) | **0.04 unmatched** |
| `np-brk-p4-m05` | I1 no guards | 60 | 1.979 | +0.138R | 99% (0.988) | 1.13 |
| `np-brk-p4-m05` | **I2** no guards | 61 | 1.085 | +0.036R | withheld (0.000) | **0.03 unmatched** |

Both I2 rows clear the 40-trade floor (45 and 61), so this is a **read, not a
void**: the gate fails on profit factor and on expectancy, in both guard arms.
`np-brk-p4-m15` goes from PF 2.226 to PF 1.125 between the first and the second
half of the same store. **F1 fired.**

The full span reads `np-brk-p4-m15` PF **1.563** / **+0.109R** / 103 trades and
`np-brk-p4-m05` PF 1.397 / +0.086R / 121 trades, and both would clear the gate
on that one window. They are published here precisely so the halving cannot be
read as hiding a pass: the full-span figure is the average of a 2.226 half and
a 1.125 half, and the gate asks for two windows for exactly this reason.

**Timeframe arm, clock-matched (+45 min, finer ruler).** 0 of 16 cells pass
both windows. The signal bar opens at +45 min in every row; the fill is the
next bar's open, so it lands at +60 min on 15m, **+50** on 5m and **+46** on
1m. That is inherent to the engine's next-open fill and is named here because
it is part of what changed.

Same store, same window, same releases, same entry minute; only the ruler:

| ruler (fill) | row | A' trades / PF / expectancy | B' trades / PF / expectancy |
|---|---|---|---|
| 15m (fill +60), parent | `p4-m15` | 96 / **1.636** / **+0.106R** | 90 / **1.734** / **+0.114R** |
| 5m (fill +50) | `c45-m15` | 99 / 1.450 / +0.095R | 92 / **0.754** / **-0.065R** |
| 1m (fill +46) | `c45-m15` | 110 / 1.298 / +0.082R | 113 / **0.847** / **-0.043R** |
| 15m (fill +60), parent | `p4-m05` | 107 / 1.768 / +0.137R | 118 / 1.220 / +0.0499R |
| 5m (fill +50) | `c45-m05` | 118 / 1.444 / +0.108R | 115 / **0.869** / **-0.035R** |
| 1m (fill +46) | `c45-m05` | 119 / 1.332 / +0.092R | 126 / **0.878** / **-0.035R** |

The `m05` line is the like-for-like one on sample size: **118 / 115 / 126**
trades on B' out of the same 134 releases, and expectancy moves **+0.0499R ->
-0.035R -> -0.035R** as the ruler goes 15m -> 5m -> 1m. The sign of the edge on
B' is a property of the ruler. **F2 fired.**

On A' the edge survives every ruler but **degrades monotonically** as the fill
moves earlier: 1.768 -> 1.444 -> 1.332 on `m05` and 1.636 -> 1.450 -> 1.298 on
`m15`. Monotone on both rows and both gate settings. So whatever A' has is
weighted toward the ten to fourteen minutes *after* the 15m fill, not toward
the release.

The guarded arm matches the unguarded one to three decimals on these rows
(+45 min clears the `news flat 60/30` blackout): 1m A' `c45-m15` reads 1.298
unguarded and 1.296 guarded.

**Timeframe arm, short probes (structure inside the expansion).** 0 of 96
cells pass. No row passes on either window, let alone both:

| interval | window | best row | trades | PF | expectancy |
|---|---|---|---|---|---|
| 1m | A' | `np1m-rev-p4-m15` | 115 | 1.176 | +0.088R |
| 1m | **B'** | `np1m-brk-p4-m15` | 106 | 0.994 | +0.003R |
| 5m | A' | `np5m-rev-p2-m05` | 112 | 1.298 | +0.124R |
| 5m | **B'** | `np5m-brk-p2-m05` | 115 | 1.015 | +0.011R |

On B' at 1m **all twelve rows** read PF 0.622-0.994 and expectancy -0.202R to
+0.003R. The two rows that cleared the gate's profit-factor leg on 5m A'
(`rev-p2-m05` 1.298, `rev-p4-m05` 1.249) read 0.601 and 0.757 on B'. The
winning branch also flips with the ruler: breakout at 15m/+45 min, reversion at
5m/+5 to +15 min. **F3 fired.**

### The short-probe arm is the one place the measuring instrument works

This matters more than the verdicts. On the 1m short-probe rows the matched
null calibrates: **count match 1.02-1.13**, inside `COUNT_MATCH_BAND`, on every
row of A'. Nowhere else in this family does it. The parent's 15m survivor is
1.28 on A' and 0.38 on B'; the clock rows are 1.03-1.12 on A' and **0.37** on
B'; the `xauusd` I2 rows are **0.03-0.04**. So the only readable percentiles in
the whole axis belong to rows that fail: on 1m A' the breakout branch sits at
**12-13%** of its own matched null (null p50 0.971) - worse than seven eighths
of random entry at the same minute, count, cost and side ratio.

It is also the one place the **registered geometry actually fires**. Exit mix:

| row | window | STOP | TARGET | TIMEOUT | own-rule share |
|---|---|---|---|---|---|
| 15m `brk-p4-m15` (parent) | A' | 6 | 3 | 87 | 9 of 96 = **9%** |
| 15m `brk-p4-m15` (parent) | B' | 7 | 3 | 80 | 10 of 90 = **11%** |
| 5m `brk-c45-m15` | A' | 10 | 3 | 86 | 13 of 99 = 13% |
| 1m `brk-c45-m15` | A' | 19 | 6 | 85 | 25 of 110 = 23% |
| 1m `brk-p4-m15` | A' | 56 | 11 | 48 | 67 of 115 = 58% |
| 1m `brk-p1-m05` | A' | 68 | 23 | 29 | 91 of 120 = **76%** |
| 1m `brk-p1-m05` | B' | 69 | 28 | 25 | 97 of 122 = **80%** |

So the single sentence this job earns: **the only configuration in which the
rule registered in `2026-10-06-news-entry.md` §4 is the thing being measured is
1m short probes, and there it has negative expectancy on both windows against
the only calibrated null in the family.** Every configuration that passes
anything leaves 77-93% of its trades by the four-hour cap, which is F5. The
ratios are reported and the cap is not pursued: `agent/news-geom` owns it.

### The guard forbids two thirds of the family at 15m and all of it below

Exactly as declared in §5. `--guards` prints
`news flat 60/30 (impact>=3, USD)` and refused **every** entry on all 24
short-probe rows at both intervals and both windows - `refused NEWS_FLAT 89`
through `refused NEWS_FLAT 123` per row, 0 trades, across all 48 guarded
short-probe cells. A desk running this configuration cannot take any 1m or 5m
post-release entry at all. Reported as a fact about the configuration;
`[trading.guards]` was not touched.

### Cost: the brief's 19.9% does not apply to this rule, and the measured range is 0.85% to 5.62% of R

Every figure below is from that run's own
`cost-matched null: control stop <x> ATR = <y> points, cost <z>% of R` line,
with the stop in price points beside it, at the configured spread 0.28 per
round trip.

| store | ruler | row | window | stop (points) | cost % of R |
|---|---|---|---|---|---|
| XAUUSD | 15m | `brk-p4-m15` | I2 | **32.89** | **0.85%** |
| XAUUSD | 15m | `brk-p4-m15` | I1 | 22.81 | 1.23% |
| XAUDUKA | 15m (parent) | `brk-p4-m15` | A' | 23.02 | 1.22% |
| XAUDUKA | 5m clock | `brk-c45-m15` | A' | 21.48 | 1.30% |
| XAUDUKA | 1m clock | `brk-c45-m15` | A' | 20.02 | 1.40% |
| XAUDUKA | 5m probe 1 | `brk-p1-m05` | A' | 13.03 | 2.15% |
| XAUDUKA | 15m (parent) | `brk-p4-m15` | B' | 12.46 | 2.25% |
| XAUDUKA | 5m clock | `brk-c45-m15` | B' | 11.96 | 2.34% |
| XAUDUKA | 1m clock | `brk-c45-m15` | B' | 10.46 | 2.68% |
| XAUDUKA | 1m probe 1 | `brk-p1-m05` | A' | 10.16 | 2.76% |
| XAUDUKA | 5m probe 1 | `brk-p1-m05` | B' | 7.25 | 3.86% |
| XAUDUKA | 1m probe 1 | `brk-p1-m05` | B' | **4.99** | **5.62%** |

**The brief's premise that 1m costs 19.9% of R does not hold for this rule, and
the reason is structural rather than a difference in measurement.** 19.9%
belongs to a 1.5-ATR(1m) stop, about 1.4 price points. `news-pulse` stops **one
impulse range** away, and the release's own range is 5 to 13 points at 1m -
because the move is in the release minute rather than spread over the hour.
Going from a 15m probe 4 to a 1m probe 1 shrinks the stop from about 23 points
to about 10, a factor of 2.2, not the factor of 15 the bar ratio suggests. So
cost/R roughly doubles instead of multiplying by five, and **cost is not what
kills the short-probe arm**: at 2.76% of R, an edge of +0.088R would clear it
comfortably. The arm dies on direction.

The widest figure in the table, 5.62% of R, is B'-era gold (a 4.99-point stop
at a 1,200 to 1,800 dollar price level), not a bar-size effect. Cost/R here is
driven by **price level** at least as much as by horizon, which sharpens brief
§3 item 3: the same ruler on the same instrument reads 2.76% in 2022-2025 and
5.62% in 2018-2021.

One consequence worth stating plainly, because it also answers the brief's "a
6x move inside one 15m bar": the 1m release-minute range is 10.16 points on A'
against a 23.02-point range over the whole first hour. **Forty-four percent of
the first hour's range happens in the first minute.** The expansion really is
concentrated where the brief said. It just has no direction this rule can take.

### Two instrument findings this axis produced

**(i) `matched_rate` is not a "months versus years" failure - it is a coin flip
at about seventy admissible bars.** The parent recorded count matches of 0.04
on about 32 admissible bars and 0.82-1.52 on about 127. Measured here: I1 (69
USD impact-3 events) calibrates at **1.13-1.17**, while I2 (**70** events, one
more) collapses to **0.03-0.04** - the control took **2 trades against the
method's 61**, and the printed `null p50` is **0.000**. Same tool, same rows,
the same bar count, adjacent windows, two orders of magnitude apart. So "a
`newsonly:` row needs years" understates it: at this gate width the calibration
is *unreliable*, not merely *coarse*, and a published percentile needs its
count match quoted beside it or it means nothing. `matched_rate` was not
touched.

A `null p50` printed as **0.000** is brief §7's rule running the wrong way too:
the null's median run genuinely had no winning trade, but the quantity a reader
wants there is "not calibrated", and 0.000 reads as a measurement. Reported,
not fixed.

**(ii) All three `XAUUSD` stores are truncated to about 100,000 rows.** §2
above, measured before any run. The consequence for anyone planning work: there
is no broker-feed store below 15m with a readable span, so a second instrument
and a second timeframe cannot be had at the same time on this disk.

`wrong_side_stop` printed **0 times across all 200 cells**, so F6 did not fire.
1m store contiguity, checked directly against the bars: of 127 USD impact-3
events in A', **123** anchor on a 1m bar inside `maxLagMin = 15` and all 123
have 46 contiguous 1m bars from offset 0 to +45; on B', **130 of 134**. So
`probeBars = 46` anchors on what it is supposed to, and the 110-126 trade
counts are the gate rejecting releases, not the store dropping them.

### Falsifiers

| # | what it tested | fired? | where |
|---|---|---|---|
| F1 | the rule is a property of the Dukascopy feed | **FIRED** | `np-brk-p4-m15` on I2: PF 1.125 / +0.045R / 45 trades, both arms |
| F2 | the rule is a property of the 15m ruler | **FIRED** | B' at +45 min: 15m +0.114R vs 5m -0.065R vs 1m -0.043R |
| F3 | no tradable structure at 1m or 5m | **FIRED** | 0 of 96 short-probe cells pass; 12 of 12 non-positive on 1m B' |
| F4 | void rather than result | partly | I2 percentiles withheld (count match 0.03-0.04); no window fell under 40 trades, so no gate read was voided |
| F5 | the four-hour cap | **FIRED** | 77-93% TIMEOUT on every +45 min row; not pursued (`agent/news-geom`) |
| F6 | `wrong_side_stop` | did not fire | 0 across 200 cells |

F3's premise needs one correction, because the printed geometry answered it in
a way this registration did not anticipate. The brief asked whether the
post-release move runs "several times its own stop". It **cannot**, by
construction: this rule's stop *is* the move's own range, so at entry the move
and the stop are the same size and the rule needs a *further* 1.8x move. The
hypothesis "a 6x range event is a tradable sequence of 1m bars" is therefore
not refuted in general - what is refuted is that *this* rule, whose
invalidation distance is the release range itself, can take it. A rule whose
stop sits *inside* the release range is a different registration and is not
measured here.

### Multiple-testing ledger

| block | declared | looked at |
|---|---|---|
| plumbing: `xauduka` 1m, A', 12 rows, `--seeds=20`, no guards | 12 | 12 |
| I: `xauusd` 15m, 12 rows x {I1, I2} x 2 guard arms | 48 | 48 |
| I-full: `xauusd` 15m, 12 rows, no guards | 12 | 12 |
| T-clock 5m: 4 rows x {A', B'} x 2 guard arms | 16 | 16 |
| T-clock 1m: 4 rows x {A', B'} x 2 guard arms | 16 | 16 |
| T-probe 5m: 12 rows x {A', B'} x 2 guard arms | 48 | 48 |
| T-probe 1m: 12 rows x {A', B'} x 2 guard arms | 48 | 48 |
| **total** | **200** | **200** |

Declared and looked at agree exactly; the count was taken off the receipts by
`scripts/newstf_table.py`, which prints one line per (run, row) and totals 200.
The plumbing cell was declared in §9 before it ran and its numbers are quoted
nowhere above.

The `--null-sides=exposure` contingency was **not** spent, and the reason is
recorded rather than left implicit: `np-brk-p4-m15` on I2 printed a long share
of 0.644, outside the 0.40-0.60 band, which §9 says triggers a re-run. That row
**fails the gate on both legs** and its percentile is already withheld because
its control took two trades. Spending twelve cells to re-read a percentile on a
failed row with an uncalibrated control is the multiple-testing charge this
record exists to avoid, so those cells were left unspent. No spread fan, no
`--mode=rescore`, no extra window, no parameter outside the four batch files.

### Outcome

The parent's rule is **not** a method. It survives on one feed and dies on the
second half of the other; at its own entry minute it survives one ruler and
inverts on the other two; and the only resolution at which its registered
invalidation actually fires is the one where it loses money on both windows
against the only calibrated null in the family. Both gaps the parent named as
open - a second instrument, a second timeframe - are now closed in the
negative. The family's remaining live question is a different one: a rule whose
stop sits *inside* the release range, with a fill model, which this
registration explicitly does not measure.

### Receipts

    receipts/newstf-{1m,5m}-{probe,clock}.toml              the 32 declared rows
    receipts/newstf_I{1,2}_{noguards,guards}.txt            the instrument arm
    receipts/newstf_Ifull_noguards.txt                      the full xauusd span
    receipts/newstf_clock{1,5}_{A,B}_{noguards,guards}.txt  clock-matched
    receipts/newstf_probe{1,5}_{A,B}_{noguards,guards}.txt  short probes
    receipts/newstf_plumb_1m.txt                            the declared plumbing cell
    receipts/newstf_table.txt                               all 200 cells, one line each
    receipts/newstf_runlog.txt                              every run, with its clock time
    scripts/newstf_runs.sh                                  every command, in order
    scripts/newstf_table.py                                 the ledger count, from the receipts

Every receipt header carries `news: 747 events (2010-01-08 to 2027-12-08) from
E:/rust/flowdesk/data/news/events.parquet` and
`news scope: USD (news_currencies)`, so the calendar and the scope each run
read are quotable rather than assumed. `events-extended.parquet` was not used.
`data-sealed/` was not opened, read or counted.

### Disk, for the record

`df -h /e` read **24 GB free** before the first run and **9.5 GB free** after
the last. This job built nothing: its worktree has no `target/`, and everything
it wrote is about 1 MB of text under `receipts/`. The 14 GB came from elsewhere
on the machine during the same hour. Reported because brief §1(c) sets the
floor at 8 GB and the next job to start may find it already crossed.
