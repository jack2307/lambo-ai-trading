# ICT sweep → MSS → FVG: survives three months of broker minutes, fails four years

**Date:** 2026-09-13
**Question:** Does the ICT "A+" chain (higher-timeframe fair value gap, liquidity
sweep, market structure shift with displacement, lower-timeframe gap, retrace
entry), as specified by the `ICT_APlus_Sweep_MSS_FVG` expert's manual, show an
edge on gold at the broker's costs?
**Outcome:** No. The expert's default preset passes the gate and beats its
matched null on the only three months of broker M1 available — and then loses
on four years of out-of-sample minutes from a second feed (PF 0.75–0.77,
expectancy −0.17 to −0.19R, 613–1428 trades). The direction null on the
in-sample run already put the direction rule at the 79th percentile of a coin
flip. Closed.

## What was measured

- Port: `fd-strategy::ict` from the manual (15 pages, Vietnamese), not the
  MQL5 source. Seven steps, deadlines, cancellations and presets as written.
  Differences stated on the module: next-open fill instead of a limit at the
  level, higher timeframe by bucketing, no break-even / partial take-profit.
  Three unit tests walk a synthetic sell chain (entry, cancellation on a close
  back through the sweep, no setup without a higher-timeframe gap).
- Data, in order of use:
  1. `XAUUSD-1m` — Vantage, 100,000 bars, 2026-06-02 → 09-11 (the terminal's
     cap). The expert's own timeframe: M1 entries, M15 gaps.
  2. `XAUUSD-5m` — Vantage, 1.4 years, M15 gaps as `htfFactor = 3`.
  3. `XAUDUKA-1m` — Dukascopy bid, 1,723,696 bars, 2022-06-16 → 2026-05-31,
     converted by `py/ingest/dukascopy_to_parquet.py`. Different venue, same
     structure; Vantage's costs applied. **Chosen and pre-registered
     (`--batch=ict-oos`) after the M1 result and before being looked at.**
- Costs: spread $0.28/oz, no swap (this account is swap-free, measured).
- Presets A (tight), B (balanced, the default), C (loose) as in the manual;
  kill zones = the manual's London 08–12 and New York 13–17 server time, which
  on this broker's UTC+3 clock are 01:00–05:00 and 06:00–10:00 New York.
  Walk-forward 4 folds over a 9-cell grid (displacement multiplier × RR),
  each read against 200 random-entry runs wrapped in the same session gate.

## Evidence

Vantage M1, three months (`--batch=ict-m1`):

```
hypothesis      trades  OOS PF  expect null p50 null p95   pct
ict-A-tight         33   0.917  -0.051    0.888    1.058   62%
ict-B-balanced      62   1.261   0.145    0.888    1.058  100%   SURVIVES
ict-C-loose        223   1.005  -0.003    0.883    0.986   96%
ict-B-allday       157   1.350   0.178    0.883    0.986  100%   SURVIVES
```

Direction null, same window, default parameters (all day, 198 trades): the
same entries with a coin-flip side score p50 0.927 / p95 1.155; actual 1.045 →
**79th percentile, inside its own null.**

Vantage M5, 1.4 years (`--batch=ict-m5`): A 0.400, B 0.944, C 0.905, B-allday
0.891 — nothing past the gate, nothing above the 55th percentile.

Dukascopy M1, four years, pre-registered (`--batch=ict-oos`, 100 null seeds):

```
hypothesis      trades  OOS PF  expect null p50 null p95   pct
ict-B-balanced     613   0.773  -0.171    0.770    0.828   52%
ict-B-allday      1428   0.746  -0.190    0.705    0.746   95%
```

In-sample defaults on the same four years: 1,785 trades, PF 0.750.

## Reading

The three-month survivor is what a *regime* looks like when it is mistaken for
an edge: June–September 2026 on one feed, 62 trades, and a parameter selection
that had four folds of a trending summer to choose from. The direction null
already said the side of the trade was not the source of the profit factor.
On the long sample the method loses at roughly the rate of noise traded at the
same cost — the 95th-percentile row is 95th of a null whose *best* run is
0.746, i.e. the method is as good as the least-losing coin flip.

The expert's manual warns to backtest 3–6 months. Three months is exactly the
window on which this passed.

## What each role said

- **data-integrity:** NO OBJECTION to the port's causality: swings confirm
  `right` bars late, higher-timeframe buckets exclude the forming one, the
  entry bar is read whole and filled next open. OBJECTION noted and accepted
  on the out-of-sample feed being a different venue — that is the point, and
  it is labelled.
- **adversary:** SURVIVED. Called the direction null before the out-of-sample
  run and it was already inside. Two survivors at the 100th percentile of 200
  on the same sixty-two trades was the multiplicity warning from batch 1
  coming true.
- **risk:** NO OBJECTION; nothing proposed for trading.
- **researcher:** the port is worth keeping — it is the first structure-based
  method in the registry and the `swing` indicator and session filters are
  reusable. The claim, not the code, is what failed.
- **execution-realist:** on M1 the expert's limit fill would be a few cents
  better than the next-open fill used here; at PF 0.75 that is not the gap.
- **historian:** first external strategy tested under the discipline; first
  time the pre-registration step mattered — the survivor would have been
  believed without it.

## What would reopen this

Vantage M1 beyond the terminal cap (raise "Max bars in chart" and re-export)
covering a window that is not summer 2026 — if it disagreed with Dukascopy on
the *same* dates, that would be a data question worth its own record. Nothing
about the presets would.

## What this does not say

- It does not say the expert's MQL5 code loses money; the manual was ported,
  and break-even / partial exits were not.
- It does not say ICT structure has no information; it says this chain, at
  these costs, on gold minutes, over four years, does not.
