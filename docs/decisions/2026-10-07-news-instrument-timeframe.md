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
