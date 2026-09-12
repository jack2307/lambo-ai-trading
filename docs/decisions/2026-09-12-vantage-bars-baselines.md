# On the broker's own bars, the technical baselines are still inside the noise

**Date:** 2026-09-12
**Question:** Do the technical strategies show an edge on the bars the system will
actually trade — Vantage `XAUUSD.sc` (4.2 years) and `BTCUSD.sc` (2.9 years) at
15m — rather than on COMEX GC reference closes or Binance spot?
**Outcome:** No. Every method fails the walk-forward gate on both markets, and on
gold every one sits inside the random-entry null distribution. Recorded; not
re-tuned.

## What was measured

- Data: `py/ingest/mt5_export.py` (read-only) from the live Vantage terminal.
  `XAUUSD-15m.parquet` 100,249 bars 2022-06-16 → 2026-09-11; `BTCUSD-15m.parquet`
  100,496 bars 2023-10-05 → 2026-09-12. Server clock converted NY-DST-anchored
  (UTC+3/+2), verified against Binance at r=0.9996. 0 duplicate bars, 0 OHLC
  violations; the only gaps > 3 days are Christmas, New Year and Good Friday.
- Costs: read from the terminal, not assumed — `XAUUSD.sc` spread $0.28/oz,
  contract 1 oz; `BTCUSD.sc` spread $17.05, contract 0.01 BTC. The Binance-based
  `btc` market assumed $5.00; the broker charges 3.4× that.
- Tool: `search --market=xauusd|btcusd --interval=15m --mode=compare|wf|null`,
  config `backtest.walk_forward_folds = 4`, gate PF ≥ 1.2, expectancy ≥ 0.05R,
  ≥ 30 trades. No options timeline for either (see "What this does not say").

## Evidence

Gold, walk-forward out of sample (4 folds):

```
strategy              trades     win%    profit   expect     maxDD  verdict
ema-cross               1087    35.6%     0.802   -0.113       11604  fail
rsi-reversion           1293    42.6%     0.853   -0.079       11147  fail
donchian-breakout       3716    37.2%     0.940   -0.022       10419  fail
bb-fade                 3598    44.8%     0.861   -0.083       20317  fail
```

Gold, 200 random-entry runs through the same pipeline (OOS profit factor):
p05 0.816 · p25 0.879 · p50 0.910 · p75 0.957 · p95 1.025 · max 1.074 —
0 of 200 clear the gate. Where the methods fall: ema-cross 3rd percentile,
rsi-reversion 12th, bb-fade 14th, donchian-breakout 62nd. All inside.

BTC on Vantage, walk-forward out of sample (4 folds):

```
strategy              trades     win%    profit   expect     maxDD  verdict
ema-cross                807    41.3%     1.075    0.044        2688  fail
rsi-reversion           2936    54.8%     0.844   -0.063       14994  fail
donchian-breakout       2359    39.5%     0.954   -0.017        6302  fail
bb-fade                 5192    44.1%     0.895   -0.054       21683  fail
```

For comparison, the same ema-cross on Binance bars at $5 spread scored PF 1.033
(85th percentile of its null, decision `2026-09-12-technical-baselines`). At the
broker's real spread and on the broker's feed it is 1.075 over a shorter window —
different data, so not a like-for-like move, and still under the gate.

## What each role said

- **data-integrity:** NO OBJECTION to the bars — provenance, clock rule and
  spacing are all checked and recorded in the file metadata. The OTL +7h stamp
  found during this work was fixed the same day (`2026-09-12-otl-timestamps.md`);
  the basis figures below are from the aligned series.
- **adversary:** SURVIVED — the null control is the one that matters and nothing
  is outside it. Notes the gold null was run without the options strategies
  (no timeline), the same limitation as before.
- **risk:** NO OBJECTION — nothing here is proposed for trading. Notes that the
  BTC drawdowns ($15k–$22k on a $10k account for the mean-reversion methods) are
  what the guards exist to bound, and that the guarded run has not been
  reported here.
- **researcher:** the interesting number is not any PF but the *spread*: $17 on
  BTCUSD.sc is 0.015% of price, comparable to Binance taker fees, so the CFD
  is not obviously worse — the edge is absent, not eaten.
- **execution-realist:** the gold spread of $0.28 on a 1 oz contract is small.
  (Corrected later the same day: the swap figures first quoted here were in
  points, not dollars, and the account's own history shows it is swap-free.)
- **portfolio:** two markets, four methods, eight walk-forwards, zero survivors.
  Nothing to size.
- **historian:** third time this question has been asked (GC reference,
  Binance, now Vantage), same answer each time. The technical baselines are
  controls, not candidates; stop expecting them to pass.

## What would reopen this

An options-aware strategy with a tape covering more than a sliver of the bar
window, run through the same null. For gold that requires either GC bars at
depth or basis-adjusted levels on XAUUSD; for BTC it requires the collector to
have run for months. Nothing about the technical methods themselves would.

## What this does not say

- It does not say the options thesis failed: no options strategy ran here.
  `xauusd` has `options_source = "none"` because the OTL tape is COMEX GC and
  spot XAUUSD trades ~$44 below it (measured median 43.97, p5–p95 40.5–46.1
  over the five overlapping days once the clocks are aligned) — nine times the
  tightest strike spacing. The basis is stable enough over days that a
  measured shift could work; over months it rolls with the front contract,
  which is why it is a research task and not a config number.
- It does not say the guards change anything: these runs are unguarded, like
  the oracle's.
- It does not say M1 is available: the terminal caps M1 at 100k bars (~3
  months). The 15m series are complete.
