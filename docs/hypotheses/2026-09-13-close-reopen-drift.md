# 2026-09-13-close-reopen-drift: gold rises across the New York close and through the early evening

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** in-sample done — one row survives (the Friday leg); confirmation running
**Batch file:** `docs/hypotheses/2026-09-13-close-reopen-drift.toml`

## Where this comes from

The recent-year screen (`2026-09-13-recent-year-screen.md`) nominated
nothing under its own criterion, but its long-window context showed one
structure at the 100th percentile of both nulls on 460–560 sessions:
two-hour long holds at 16:00–18:00, 18:00–20:00 and 20:00–22:00 New York
on 2022–25, with the same blocks at the 94th–96th on the last year. The
16–18 row also carried a flaw the reviewers found — its Friday leg is a
50-hour weekend hold — and a risk unit built for a bar, not a timed hold.
This registration tests the claim properly, under the loop's original rule:
long window primary, the Friday leg separated, sizing on daily ranges.

## Claim

Held long from the last bars before the 17:00 New York close through the
reopen and the early evening, gold's return is positive beyond what random
holds of the same length earn, and beyond the same intervals with the side
flipped, on seven years of Dukascopy bars (2018-06 → 2025-04) and again on
the broker's own bars (2025-04 → 2026-09). The account is swap-free
(measured), so the 17:00 rollover costs nothing; the reopen spread was
measured at $0.26–0.27 (`py/ingest/mt5_spread_probe.py`, 4.6 M ticks), so
the flat $0.28 model is honest for it.

## Falsifier

Per row, on the primary with the registered parameters replayed (no grid,
`fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 sessions), ≥ 95th
percentile of random holds of the same length on the same bars, sized the
same; direction null (the same intervals, sides permuted) ≥ 95th. The
confirmation must agree. The rows are the *parts* of the evening, so the
record can say which part carries the drift; a pass on the whole window
alone is a pass.

## Base method

`session-hold` with a sizing stop: `riskDailyRanges` × the mean New York-day
range of the last `rangeDays` days, used by the engine for lots and R and
not enforced (the clock is the exit), as in `tsmom`. `weekdays:MoTuWeTh`
excludes the Friday leg; the `Fr` row carries it alone.

## Rows

- `close/1630-1815` — the break itself, Mon–Thu (entry signalled on the
  16:15 bar, filled at the 16:30 open; exit signalled on the 18:15 bar,
  filled at the 18:30 open). Amended once: the first spelling gated 16:25,
  a minute no 15-minute bar carries, and took no trades.
- `close/1630-2000`, `close/1630-2200` — the break plus the evening.
- `close/1815-2200` — the evening without the break (the contrast).
- `close/1400-1630` — the afternoon before the close (the contrast).
- `close/fri-1630-1815` — the Friday leg only: a weekend hold.

## Data

- Primary: `xauduka:15m` bounded `2018-06-16 → 2025-04-10`.
- Confirmation: `xauusd:15m` from `2025-04-11`.

## Sample needed

Four sessions a week for seven years is ~1,400 holds per row; the floor is
not in question. The question is the null.

## What each outcome means

- The break rows pass both windows and the contrasts do not → the drift is
  the close-to-reopen move; a paper run of that hold, sized as registered,
  and the first record in this loop with two windows behind it.
- The evening rows pass and the break row does not → the drift is the Asian
  open, not the settlement; record it as such.
- Nothing passes the primary → the long-window context of the screen was
  the 2022–25 rally seen through 560 sessions; closed.

## In-sample (2018-06-16 → 2025-04-10, `xauduka` 15m, 160,368 bars)

Fixed replay (`in-sample-fixed.txt`, the test for a strategy-managed hold;
300 sized random holds per row) and the direction null (`direction-close-*.txt`,
1,000 permutations of the same trades' sides):

```
hypothesis            trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
close/1630-1815         1355   1.254   0.006    0.599    0.706  100%  100th       fail: expectancy 0.006R < 0.05R
close/1630-2000         1355   1.237   0.011    0.760    0.859  100%  100th       fail: expectancy 0.011R < 0.05R
close/1630-2200         1355   1.258   0.019    0.837    0.936  100%  100th       fail: expectancy 0.019R < 0.05R
close/1815-2200         1397   0.985  -0.001    0.830    0.936   99%   99th       fail: profit factor 0.985 < 1.2
close/1400-1630         1375   0.739  -0.017    0.784    0.913   21%   25th       fail: profit factor 0.739 < 1.2
close/fri-1630-1815      336   2.064   0.054    0.785    1.177  100%  100th       SURVIVES
```

Walk-forward (`in-sample.txt`, the check): the same order — 1.288 / 1.234 /
1.292 / 1.061 / 0.826 / 2.300, the Friday row at 0.061R on 270 sessions.

Reading before the confirmation: the break rows sit at the 100th percentile
of both nulls on 1,355 sessions and fail the registered expectancy gate,
because R here is one full daily range and a two-hour hold moves a tenth of
it; that is the registration's own unit, and the gate stands as written. The
evening without the break (`1815-2200`) is flat; the afternoon before it is
negative. The Friday leg — the weekend hold the screen's 16–18 row had hidden
— is the one row that passes everything.
