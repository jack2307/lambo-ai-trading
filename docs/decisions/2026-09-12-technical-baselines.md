# The four technical baselines are inside the noise

**Date:** 2026-09-12
**Question:** Do `ema-cross`, `rsi-reversion`, `donchian-breakout` or `bb-fade`
have an edge on BTC 15m worth pursuing?
**Outcome:** No. Closed. Not "needs tuning" — closed.

## What was measured

Two years of BTCUSDT 15m from Binance: 70,080 bars, zero gaps, 2024-09-12 to
2026-09-12. Walk-forward, four folds, parameters selected on each training
window. Costs from `trading_rules_for(config, "btc")` — spread $5.00, contract
size 1 BTC.

```
strategy              trades     win%   profit   expect
ema-cross                787    39.4%    1.033    0.024
rsi-reversion           1247    46.4%    1.011    0.010
donchian-breakout       2088    37.5%    0.972   -0.007
bb-fade                 2463    45.4%    0.952   -0.026
```

Gate: profit factor ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 trades. All four fail.

## Why this is stronger than "they failed the gate"

**The gate is calibrated.** `search --mode=null`, 300 runs of random entry
through the identical pipeline — same stops, same sizing, same costs, same
parameter selection — produced this out-of-sample distribution of profit factor:

```
p05 0.855   p25 0.920   p50 0.964   p75 1.005   p95 1.072   max 1.216
```

Only 1 of 300 noise runs (0.3%) clears the gate. The gate is not loose.

**The four methods sit inside that distribution:**

```
ema-cross          1.033   85th percentile   inside the noise
rsi-reversion      1.011   78th percentile   inside the noise
donchian-breakout  0.972   54th percentile   inside the noise
bb-fade            0.952   44th percentile   inside the noise
```

`ema-cross` being "best of the four" is what picking the maximum of four draws
from this distribution looks like. It is not evidence that it is better.

## Two hypotheses tested and rejected along the way

**"Costs are eating the edge."** Wrong. `search --mode=costs` at 0×, 0.25×,
0.5×, 1× and 2× the configured cost: at **zero** cost — a market that cannot
exist — the best profit factor is 1.051. Costs account for about 0.018; the
distance to the gate is 0.15, roughly eight times larger. There is no gross edge
to rescue.

**Earlier numbers were flattered by a bug.** The first run used the top-level
`[trading]` table, which describes gold: a $0.30 spread instead of $5.00,
understating cost about seventeen-fold. Corrected figures are the ones above.
Locked by `fd-backtest/tests/trading_rules.rs`.

## What would reopen this

More data does not. Two years is not the constraint; the effect is absent at
zero cost. Reopening would need a different instrument, a different timeframe,
or a materially different entry rule — and it would have to clear the null
distribution, not the gate alone.

## What this does not say

Nothing about the options-flow strategies. Those have never been tested: the
tape covers 114 of 70,080 bars (0.16%), and all four returned zero
out-of-sample trades. Not rejected — unasked.
