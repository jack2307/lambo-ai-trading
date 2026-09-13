# Research backlog

Ideas waiting for a pre-registration. One line each: the claim and why it
would exist. The `/research next` loop takes the top unchecked item, writes
its hypothesis file, and runs the pipeline. Add reasons, not just names — an
idea without a reason is a search.

Closed ideas move to the bottom with a pointer to their decision record, so
`historian` does not let them come back under a new name.

## Open

- [ ] ~~**Asian-range breakout at London open.**~~ Withdrawn 2026-09-13: the
  breakout family is closed (`2026-09-13-volcond-breakout.md`); a fourth
  range on the same mechanism needs a new reason, not a new window.
- [ ] **ICT variant: order block instead of FVG.** Reason: the manual's own
  alternative entry zone; the chain is already ported.
- [ ] **Options: cluster-at-level with the accumulated tape** — *blocked until
  the collector has months of tape*; check `search --market=btc` tape line.

## Infrastructure (found by reviews; not hypotheses)

- [x] `search --from/--to` and `[run] *_from/*_to` in the runner (2026-09-13).
- [x] `dukascopy_to_parquet.py` drops flat zero-volume bars — 334,406 of them
  (weekends and breaks), not the 9,732 first counted; XAUDUKA-1m/5m
  regenerated (2026-09-13). Records before this date ran on the filled series.
- [x] The four unread `[trading]` keys deleted; they return with the code
  that enforces them (2026-09-13).
- [x] `null-dir` applies the batch's filters via `--filters=` (2026-09-13).
- [x] Out-of-sample stage also replays the registered parameters with no
  re-selection (`search --mode=hypotheses --fixed`, receipt
  `out-of-sample-fixed.txt`) (2026-09-13).
- [x] Dukascopy converter docstring: the CSV has no volume column; every
  O=H=L=C bar is dropped (data-integrity, London pass).

- [x] A null for drift claims: `null-hold`, random holds of the row's window
  length, chosen automatically for `Exits::Strategy` methods with no grid
  (2026-09-13).
- [ ] `run_null_control` (`search --mode=null`) skips options strategies;
  when the tape covers months this is the control the founding thesis needs
  (adversary, 2026-09-12).

## Closed

- [x] Technical baselines (ema-cross, rsi-reversion, donchian, bb-fade) on GC,
  Binance, Vantage → inside the noise. `docs/decisions/2026-09-12-technical-baselines.md`,
  `2026-09-12-vantage-bars-baselines.md`.
- [x] Session and regime gates on the baselines → nothing.
  `2026-09-12-gold-intraday-batch-1.md`.
- [x] ICT sweep → MSS → FVG (three presets, kill zones) → survives three
  months, fails four years. `2026-09-13-ict-sweep-mss-fvg.md`.
- [x] Logistic model on the 24 features → does not generalise.
  `2026-09-12-logistic-does-not-generalise.md`.
- [x] Opening-range breakout, New York → gate pass in-sample against a
  mis-matched null, inside the corrected one, fails four years out of sample,
  direction a coin flip. `2026-09-13-orb-ny.md`.
- [x] London-range breakout → passes everything in-sample on 2025–26 (gated
  null, direction null), fails the disjoint 2022–25 window; the window is a
  volatility regime. `2026-09-13-london-range.md`.
- [x] Previous-day high/low (fade and break) → 1st–6th percentile of the gated
  null on the long window; wick-tight stops lose to the spread.
  `2026-09-13-pdhl.md`.
- [x] Volatility-conditional breakout → high- and low-volatility halves of
  2022–25 indistinguishable, both inside; the breakout family is closed.
  `2026-09-13-volcond-breakout.md`.
- [x] BTC US equity-open range break → best row 84th percentile, direction
  73rd, on two years of Binance 5m; not opened at Vantage cost.
  `2026-09-13-btc-us-open.md`.
- [x] BTC US-hours drift (hold 09:30–16:00) → PF 0.78 over 435 sessions,
  complement 0.95; no drift by hour in 2024–26. `2026-09-13-btc-us-hours.md`.
- [x] London fix drift (into / out of the PM fix, AM contrast) → 74th–84th
  percentile of random holds, PF < 1 on every row, 2022–25.
  `2026-09-13-london-fix.md`.
- [x] VWAP fade (BTC, real volume) → PF 0.95 on 5,828 trades, direction null
  13th: a stretch continues rather than reverts. `2026-09-13-vwap-fade.md`.
