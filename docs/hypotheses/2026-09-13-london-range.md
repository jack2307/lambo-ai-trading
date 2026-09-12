# 2026-09-13-london-range: the New York morning resolves the range London built

**Registered:** 2026-09-13 01:30 — before any run
**Status:** in-sample run — recorded; out-of-sample opened next
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

## Amendment before any result was read (2026-09-13 01:40)

The first in-sample run produced **0 trades** on every row: `orb`'s default
`maxRangeAtr = 3` was written for a one-hour range and refuses a six-hour
one (≈ $20 against 3 × ATR14 ≈ $8). The preset now sets `maxRangeAtr = 100`
(the gate off). A run with no trades carries no information, so this is a
correction of the specification, not a tuning step; the first receipt is
kept as `in-sample-0trades.txt`.

## In-sample result (xauusd:5m, 4 folds, 200 window-matched nulls) — recorded 2026-09-13 01:50, before the out-of-sample run

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct  verdict
london/nyam            257   1.317   0.086    0.956    1.241   98%  SURVIVES
london/early           203   1.309   0.084    0.942    1.293   96%  SURVIVES
london/expansion       131   1.209   0.054    0.951    1.374   88%  gate pass, inside
```

Direction nulls, each on its own preset, 1000 assignments: nyam 1.246 →
96th (p 0.042); early 1.268 → 96th (p 0.036); expansion 1.246 → 96th.
All outside. Every in-sample condition of the falsifier is met for the first
two rows. The out-of-sample window decides.
