# 2026-09-13-btc-us-hours: BTC's return is earned during US equity hours

**Registered:** 2026-09-13 02:03 (commit time; the times first typed here were estimates) — before any run
**Status:** decided → docs/decisions/2026-09-13-btc-us-hours.md (failed the primary; confirmation not opened)
**Batch file:** `docs/hypotheses/2026-09-13-btc-us-hours.toml`

## Claim

A documented crypto anomaly: BTC's drift is concentrated in US trading
hours (ETF creations, futures desks, correlation with equities) and absent
or negative overnight. Holding long from 09:30 to 16:00 New York every
weekday captures the drift with a fraction of the exposure; the complement
window (16:00 → 09:30) does not. This is a *drift* claim, not a signal: no
entry rule, no stop, one cell.

## Falsifier

The US-hours row must pass the gate and sit ≥ 95th percentile of a
random-entry null gated to the same hours on the primary window **and** the
confirmation; the complement row must not. If both rows pass, the drift is
the asset's, not the hours'. Direction null: if the flipped-side null is not
below the actual, the side carries nothing.

## Base method

`NEW: session-hold` — enter at the window's first bar, exit at its first bar
after; long by default; `Exits::Strategy`, so nothing else closes it. No
grid.

## Data

- Primary: `btc:5m` Binance 2024-08 → 2026-09 ($5 spread)
- Confirmation: `btcusd:5m` Vantage 2025-09 → 2026-09 ($17 spread) — opened
  after the primary result is recorded; overlapping dates, so it confirms
  the cost, not the period
- Filters: weekdays; `hours:0930-0935` for the US row's null (entries only
  at the window open, like the method) — see the batch file

## Sample needed

~500 holds per row on the primary. Enough.

## What each outcome means

- US row survives both windows, complement fails → record; the paper-run
  proposal would be a once-a-day hold, which is also the cheapest thing in
  lots this project could ever propose to an IB.
- US row survives the primary only → the $17 spread eats it; record says so.
- Fails primary → closed.
