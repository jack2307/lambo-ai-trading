# BTC at the US equity open: the break does not pay at Binance cost, let alone Vantage's

**Date:** 2026-09-13
**Question:** Does a five-minute close outside the range BTC builds in the
first 30–60 minutes after the US equity open (09:30 New York), or around the
08:30 data releases, continue a range's width? (`docs/hypotheses/2026-09-13-btc-us-open.md`,
registered at `c93ebe3`.)
**Outcome:** No, on the primary window (Binance 5m, 2024-08 → 2026-09,
218,880 bars). Best row `btc/eq-open-60m`: PF 1.128, expectancy 0.037R on
362 trades, 84th percentile of a null gated to the same hours; direction
null 73rd. The 08:30 row loses (PF 0.882, 21st). The confirmation window
(Vantage, $17 spread) was not opened: an edge that fails at $5 does not
appear at $17. Closed.

## Evidence

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
btc/eq-open-60m        362   1.128   0.037    0.964    1.278   84%  fail
btc/eq-open-30m        415   1.046   0.019    0.975    1.233   71%  fail
btc/data-0830-60m      432   0.882  -0.050    0.993    1.216   21%  fail
```

Direction nulls (gated, per preset): 60m 1.045 → 73rd; 30m 1.069 → 73rd;
0830 0.908 → 22nd. Inside.

## Reading

The only row with a positive expectancy is the first hour after the equity
open, and it is where noise traded in the same hour lands 16% of the time.
The mechanism claimed — equity-hours desks moving BTC — may exist as a
*volatility* fact without being a *direction* fact; the direction null says
the side of the break carries nothing. Same shape as gold, on a different
market, at a lower cost.

## What each role said

Reviews not spawned: primary failed, direction nulls inside, gate not met on
any row. The receipts are the review.

## What would reopen this

Nothing about the range. A drift claim for the same hours (hold, not
break) is a different mechanism and is registered separately
(`2026-09-13-btc-us-hours`).

## What this does not say

- It does not say BTC is quiet at the equity open; it says the direction of
  the break is not informative over two years.
- It does not say Vantage's BTC feed disagrees; it was not opened.
