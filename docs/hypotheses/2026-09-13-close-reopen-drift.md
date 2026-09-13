# 2026-09-13-close-reopen-drift: gold rises across the New York close and through the early evening

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** registered
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

- `close/1630-1815` — the break itself, Mon–Thu (entry signalled 16:25,
  filled 16:30; exit at the first bar ≥ 18:15, filled 18:20).
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
