# Volume thrust: a high-volume bar closing at its extreme is priced by the time it closes

**Date:** 2026-09-13
**Question:** Does going with a 15-minute bar on ≥ 2.5× the recent volume that
closes in the top or bottom fifth of a ≥ 1-ATR range (stop beyond its other
end, target 1.5R) pay on Binance BTC and on Vantage gold tick volume?
(`docs/hypotheses/2026-09-13-volume-thrust.md`, registered at `53db567`.)
**Outcome:** No. BTC replays at PF 0.960 on 840 thrusts, the 50th percentile of
its null, direction 30th; gold at 0.931 on 297, 78th, direction 78th. The
walk-forward's best row (BTC 24/7, PF 1.119 at the 96th percentile) fails the
gate on profit factor and expectancy. Closed on both primaries; neither
confirmation was opened.

## What was measured

BTC: Binance BTCUSDT 15m, 2024-09-12 → 2025-12-31, real volume. Gold: Vantage
XAUUSD 15m, 2022-06-16 → 2025-04-10, tick volume, weekdays, flat over the
daily break. Vantage BTCUSD bars carry no volume (checked: zero throughout
the 15m file), so BTC's confirmation would have been a time split of the same
feed, and Dukascopy has no volume, so gold's seven-year window is not
available to this hypothesis. Spread $0.28 (gold) / $17.05 (BTC), no swap.
Nulls gated to each row's hours and matched to its count. Receipts:
`docs/research/runs/2026-09-13-volume-thrust/` and
`…-volume-thrust-gold/`, final run at `9fde7c1`.

## Evidence

BTC, registered parameters replayed (`volume-thrust/in-sample-fixed.txt`):

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
vt-btc/fixed volume-thrust         840   0.960  -0.014    0.960    1.085        0   50%  fail: profit factor 0.960 < 1.2; expectancy -0.014R < 0.05R
vt-btc/all   volume-thrust         840   0.960  -0.014    0.960    1.085        0   50%  fail: profit factor 0.960 < 1.2; expectancy -0.014R < 0.05R
vt-btc/us    volume-thrust         261   0.987  -0.000    0.965    1.208        0   56%  fail: profit factor 0.987 < 1.2; expectancy -0.000R < 0.05R
```

BTC, walk-forward (`volume-thrust/in-sample.txt`):

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
vt-btc/fixed volume-thrust         677   0.992   0.002    0.962    1.112        0   59%  fail: profit factor 0.992 < 1.2; expectancy 0.002R < 0.05R
vt-btc/all   volume-thrust         362   1.119   0.064    0.962    1.112        0   96%  fail: profit factor 1.119 < 1.2
vt-btc/us    volume-thrust         179   1.036   0.019    1.014    1.405        0   56%  fail: profit factor 1.036 < 1.2; expectancy 0.019R < 0.05R
```

Gold, registered parameters replayed (`volume-thrust-gold/in-sample-fixed.txt`):

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct  verdict
vt-gold/fixed volume-thrust         297   0.931  -0.027    0.839    1.032        0   78%  fail: profit factor 0.931 < 1.2; expectancy -0.027R < 0.05R
vt-gold/all  volume-thrust         297   0.931  -0.027    0.839    1.032        0   78%  fail: profit factor 0.931 < 1.2; expectancy -0.027R < 0.05R
vt-gold/ny   volume-thrust         139   0.777  -0.098    0.853    1.175        0   30%  fail: profit factor 0.777 < 1.2; expectancy -0.098R < 0.05R
```

Gold, walk-forward (`volume-thrust-gold/in-sample.txt`): `vt-gold/fixed` 241
trades PF 0.951 (89th), `vt-gold/all` 57 trades 0.986 (95th), `vt-gold/ny` 43
trades 0.800 (25th); every row fails the gate.

Direction nulls: BTC 0.960 → 30th (p = 0.697), US hours 0.987 → 38th; gold
0.931 → 78th (p = 0.221), New York hours 0.777 → 28th.

## Reading

The thrust bar's close is where the information stops. On BTC the method is
the median of random entries in the same hours with the same geometry; on
gold's tick volume it is a little better than random and still loses. The
one row the walk-forward could push to the 96th percentile (BTC, 24/7, 362
out-of-sample trades) has an expectancy of 0.064R and a profit factor of
1.12 — not a method, a selection. The New York-hours rows, where the story
said participation lives, are the worst rows on both markets.

## What each role said

Reviews not spawned: every row fails the gate; the direction nulls are
inside on both markets.

## What would reopen this

Order-flow volume (aggressor side) rather than bar volume, on the options
tape or a futures feed — a different signal, its own registration. Not a
different multiple.

## What this does not say

- It does not say volume carries no information; it says a 15-minute bar's
  total volume, read after the close, does not on these feeds.
- It does not say anything about the confirmation windows; not opened.
