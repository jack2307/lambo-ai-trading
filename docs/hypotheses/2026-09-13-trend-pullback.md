# 2026-09-13-trend-pullback: the first pullback against a slow trend continues it

**Registered:** (commit time is authoritative) — before any run
**Status:** registered
**Batch file:** `docs/hypotheses/2026-09-13-trend-pullback.toml` (gold), `2026-09-13-trend-pullback-btc.toml` (BTC)

## Claim

The breakout family is closed: taking direction from the short-term move
itself does not pay on gold at this cost. This is the opposite
composition: direction from a slow horizon (EMA200 rising and price above
it), timing from a fast one (the first close back under EMA20 after a run
above it), stop under the swing low, target 1.5R. If trends exist on
15-minute gold and BTC at all, this is where a trend-follower's edge is
said to sit — buying the dip in the trend, not the break.

## Falsifier

Gate and ≥ 95th percentile of the count-matched null gated to the same
hours, on the primary; survives the confirmation; direction null ≥ 95th.
The `fixed` row pins the defaults (`fast 20, riskReward 1.5`); the other
rows walk the grid (fast × riskReward) forward. Both windows must survive.

## Base method

`NEW: trend-pullback` — long when the slow EMA is above its value
`slopeBars` ago, close above it, the last `runBars` closes were above the
fast EMA and this close is below; stop under the lowest low of the last
`swingBars` bars minus a buffer; target `riskReward` × risk. Symmetric
short. `Exits::Engine`.

## Data

- Gold primary: `xauduka:15m` bounded `2018-06-16 → 2025-04-10`;
  confirmation `xauusd:15m` from `2025-04-11`. Weekdays, flat over the
  daily break.
- BTC primary: `btc:15m` (Binance, 2024-09 → 2026-09); confirmation
  `btcusd:15m` (Vantage) bounded `2023-10-05 → 2024-09-11` — disjoint and
  *earlier*, the broker's own feed.

## Sample needed

Pullbacks in a 200-EMA trend on 15-minute bars: several a day; thousands
of trades on the primary. No count problem; the question is the null.

## What each outcome means

- Survives both on gold → record; a candidate for the M15 bot at last.
- Survives BTC only → a BTC record; gold stays closed for trends.
- Fails the primaries → closed; with breakouts already closed, "trend on
  intraday bars" is closed as a family for this registry.
