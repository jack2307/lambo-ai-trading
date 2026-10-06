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
