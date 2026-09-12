# Research backlog

Ideas waiting for a pre-registration. One line each: the claim and why it
would exist. The `/research next` loop takes the top unchecked item, writes
its hypothesis file, and runs the pipeline. Add reasons, not just names — an
idea without a reason is a search.

Closed ideas move to the bottom with a pointer to their decision record, so
`historian` does not let them come back under a new name.

## Open

- [ ] **London-range breakout.** The 02:00–08:00 New York range broken in the
  New York morning. Reason: the London session builds the range the New York
  session resolves. Base method `orb` with `rangeStart = 0200`,
  `rangeMinutes = 360`. *Only with the null gated to its window and a
  disjoint out-of-sample (xauduka cut at 2025-04-10) — see the ORB record.*
- [ ] **Previous-day high/low.** Fade the first touch of yesterday's high/low
  in the Asian session; break it in the New York session. Reason: resting
  orders sit at the prior day's extremes. NEW base (`pdhl`).
- [ ] **VWAP fade.** Enter against a stretch of > k·ATR from the session VWAP,
  target VWAP. Reason: intraday mean reversion to the volume-weighted average
  is where large orders are worked. Existing `vwap` indicator; NEW thin
  strategy.
- [ ] **Asian-range breakout at London open.** Reason: the tightest range of
  the day is broken by the first real volume. Same base as ORB.
- [ ] **ICT variant: order block instead of FVG.** Reason: the manual's own
  alternative entry zone; the chain is already ported.
- [ ] **Options: cluster-at-level with the accumulated tape** — *blocked until
  the collector has months of tape*; check `search --market=btc` tape line.

## Infrastructure (found by reviews; not hypotheses)

- [ ] `search --from/--to` so an out-of-sample market can be cut to dates
  the in-sample feed does not cover (data-integrity, ORB pass).
- [ ] `dukascopy_to_parquet.py`: drop flat-filled zero-volume bars (9,732 at
  17:xx New York) (data-integrity, ORB pass).
- [ ] `[trading] max_stop_atr, min_reward_risk, cluster_pad_atr,
  require_basis` are read by no code: enforce or delete (risk, ORB pass).

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
