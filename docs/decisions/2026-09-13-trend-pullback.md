# Trend pullback: buying the dip in a rising EMA200 loses on gold and on BTC, whichever side you take

**Date:** 2026-09-13
**Question:** Does the first close back through the fast EMA, in the direction
of a rising (falling) slow EMA, with the stop under the swing and a 1.5R
target, pay on 15-minute gold (2018–25) and BTC (2024–26)?
(`docs/hypotheses/2026-09-13-trend-pullback.md`, registered at `53db567`.)
**Outcome:** No, decisively. Gold replays at PF 0.379 on 5,766 trades, the 0th
percentile of its null, direction 58th; BTC at 0.511 on 2,931, 0th percentile.
BTC's direction null reads 97th–99th, which says the chosen side beats its
mirror, not that the trade makes money: it loses 0.38R a time either way.
Closed on both primaries; neither confirmation was opened. With breakouts
already closed, trend-following on intraday bars is closed as a family.

## What was measured

Gold: Dukascopy bid 15m, 2018-06-16 → 2025-04-10, weekdays, flat over the
daily break, pip 0.1, spread $0.28. BTC: Binance 15m 2024-09 → 2026-09, pip
1.0, spread $17.05; the confirmation would have been Vantage BTCUSD
2023-10 → 2024-09. EMA20 / EMA200, slope over 20 bars, run of 3 closes,
5-bar swing, 3-pip buffer, 1.5R; the `fixed` rows pin fast and reward, the
others walk a 3×3 grid. Nulls gated to each row's hours, count-matched.
Receipts: `docs/research/runs/2026-09-13-trend-pullback/` and
`…-trend-pullback-btc/`, final run at `9fde7c1`.

## Evidence

Gold, registered parameters replayed (`trend-pullback/in-sample-fixed.txt`):

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
tp/fixed     trend-pullback       5766   0.379  -0.393    0.755    0.815        0    0%  fail: profit factor 0.379 < 1.2; expectancy -0.393R < 0.05R
tp/allday    trend-pullback       5766   0.379  -0.393    0.755    0.815        0    0%  fail: profit factor 0.379 < 1.2; expectancy -0.393R < 0.05R
tp/ny        trend-pullback       2086   0.417  -0.363    0.800    0.879        0    0%  fail: profit factor 0.417 < 1.2; expectancy -0.363R < 0.05R
```

Gold, walk-forward (`trend-pullback/in-sample.txt`): `tp/fixed` 4,651 trades
PF 0.452, `tp/allday` 6,048 PF 0.560, `tp/ny` 2,236 PF 0.604 — all 0th
percentile. Direction nulls: 0.379 → 58th (p = 0.421) on both all-day rows,
0.417 → 58th on New York hours.

BTC, registered parameters replayed (`trend-pullback-btc/in-sample-fixed.txt`):

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
tp-btc/fixed trend-pullback       2931   0.511  -0.376    0.958    1.030        0    0%  fail: profit factor 0.511 < 1.2; expectancy -0.376R < 0.05R
tp-btc/all   trend-pullback       2931   0.511  -0.376    0.958    1.030        0    0%  fail: profit factor 0.511 < 1.2; expectancy -0.376R < 0.05R
tp-btc/us    trend-pullback        550   0.616  -0.250    0.951    1.139        0    0%  fail: profit factor 0.616 < 1.2; expectancy -0.250R < 0.05R
```

BTC, walk-forward (`trend-pullback-btc/in-sample.txt`): 0.500 / 0.691 / 0.699,
0th–2nd percentile. Direction nulls (`direction-tp-btc-*.txt`): 24/7 rows
null p05 0.404 … p95 0.492, actual 0.511 → 99th (p = 0.009); US hours
actual 0.616 → 97th (p = 0.032).

## Reading

A swing-low stop three pips under a five-bar low on 15-minute bars is a
stop the next pullback bar takes; the method wins 29% of the time and the
1.5R target does not cover it. On gold it is below every random entry in
the same hours. On BTC the side is better than its mirror — the 99th
percentile is real as a comparison — and the structure loses 0.38R a trade
anyway: information about direction that a wick-tight stop throws away is
not information a bot can bill. The adversary reads the BTC direction
percentile as an artefact of the mirrored geometry; the manager reads it as
a true but useless comparison (a mirrored short carries the same distances
as the long it replaces). Either way it reopens nothing.

## What each role said

- **adversary:** BLOCK — *"Not a finding — an artefact. `DirectionFlipped`
  flips each entry independently with p = 0.5 and mirrors stop and target
  around the close, so every null draw is a ~50/50 blend of the real trades
  and mirrored ones, and the actual run is the zero-flip endpoint of that
  mixture … The percentile says only 'the mirrored variant is worse than
  this one', never 'the direction carries information'. A re-registration
  can use none of this; what it could use is the reverse question, and even
  that is answered: the reverse also loses."*
- **data-integrity, risk:** not spawned for this hypothesis; every row
  fails the gate at the 0th percentile and no number needed defending.

## What would reopen this

An exit with a reason of its own — a time-based exit (the drift-null shape)
or a structure stop at the slow average — registered as a new claim, on BTC
only, citing the 99th-percentile side comparison as the reason. Not a
wider stop, not another EMA.

## What this does not say

- It does not say trends do not exist on these markets; it says the first
  pullback with a swing stop is not a way to buy them at this cost.
- It does not say anything about either confirmation window; not opened.
