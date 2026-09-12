# 2026-09-13-london-range: the New York morning resolves the range London built

**Registered:** 2026-09-13 01:30 — before any run
**Status:** registered
**Batch file:** `docs/hypotheses/2026-09-13-london-range.toml`

## Claim

The 02:00–08:00 New York range (the London session on gold) is where the
day's early positioning is built; the first five-minute close outside it in
the New York morning (08:00–12:00) is New York flow resolving that range,
and continues at least as far as the range is wide.

## Falsifier

Fails the gate or sits below the 95th percentile of a null gated to the same
entry window in-sample; or survives in-sample and fails on the pre-registered
disjoint out-of-sample window; or the direction null is inside (< 95th).

## Base method

`orb` (exists), preset `rangeStart = 0200`, `rangeMinutes = 360`,
`entryUntil = 1200`. Stop at the other side of the range; target
`riskReward × risk`; one trade per day. Grid: `riskReward` × `entryBufferPips`.

## Data

- In-sample: `xauusd:5m` (Vantage, 2025-04-11 → 2026-09-11)
- Out-of-sample: `xauduka:5m` **cut at 2025-04-10** (2022-06-16 → 2025-04-10,
  dates the in-sample feed never covered; flat-filled bars removed) — opened
  only after the in-sample result is recorded
- Costs: `xauusd` config; flat 16:30–18:15 New York; weekdays
- **Null:** gated to `hours:0800-1200`, the strategy's own entry window

## Sample needed

One trade a day at most; the six-hour range is broken on most days, so ~250
in-sample opportunities. ≥ 60 out-of-sample trades across the folds.

## What each outcome means

- Survives in-sample AND out-of-sample AND direction null → record, paper-run
  proposal.
- Survives in-sample only → closed, the ORB pattern again.
- Fails in-sample → closed. No wider grid, no second window.
