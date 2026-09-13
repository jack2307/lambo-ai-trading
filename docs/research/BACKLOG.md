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
- [ ] **TSMOM on a second asset with seven years** (Dukascopy silver or
  EURUSD): the gold row beat random holds (98th) but not its own permuted
  sides (86th) on 75 trades; needs a warmup in days, the Friday close and
  Sunday reopen handled in the method, and its own registration.
  `2026-09-13-tsmom-2.md`.
- [ ] **Trend pullback on BTC with a time exit**: the side beat its mirror
  (99th) while the swing-stop structure lost 0.38R a trade; a re-registration
  needs an exit with a reason, not a wider stop. `2026-09-13-trend-pullback.md`.
- [ ] **Close-to-reopen drift on gold** (registered `2026-09-13-close-reopen-drift`):
  long across the 17:00 New York close and the early evening, 100th
  percentile of both nulls on 2022–25 in the recent-year screen's context;
  the proper test with the Friday leg separated and daily-range sizing.
- [ ] **Direction null on the walk-forward's selected cell** (adversary,
  recent-year screen): `null-dir` runs the defaults over the whole window,
  so for a gridded row it scores different trades than the table. An
  instrument change.
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

- [x] **Volatility-targeted sizing for strategy-managed exits**: a sizing
  stop of N daily ranges carried by the entry, used for lots and R, never
  enforced; the random-hold control sized the same way (2026-09-13,
  `edf7d96`).
- [x] Three engine faults fixed with pinning tests — sizing stop enforced on
  a self-managed hold, presets at the default value not pinned, a hold open
  at a fold's end running to the end of data — and the direction null for
  self-managed methods rebuilt as a permutation of the method's own trades.
  `docs/decisions/2026-09-13-instrument-faults.md`.
- [x] `research-run.py --stage dir` re-runs only the in-sample direction
  nulls (after a null amendment).
- [x] **Time-series momentum on gold** — re-registered as `2026-09-13-tsmom-2`
  with the sizing above and the 2018–25 window; see its record.

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
- [x] The closed mechanisms on 15-minute bars (gold ×4, BTC ×2) → every row
  fails; the timeframe is not the variable. `2026-09-13-m15-check.md`.
- [x] Doji reversal (gold 15m, BTC 15m; three gold windows) → inside the
  count-matched null on 2022–25, loses on 2018–22 and 2025–26; BTC fails.
  `2026-09-13-doji.md`.
- [x] Weekend gap fade (gold 5m, 2018–25) → PF 0.42 on 207 gaps, 1st
  percentile; large gaps 0.77; direction 33rd/66th. `2026-09-13-gap-fade.md`.
- [x] Volume thrust (BTC Binance volume, gold Vantage tick volume) → PF 0.96
  / 0.93, 50th / 78th, direction 30th / 78th; the best walk-forward row fails
  the gate. `2026-09-13-volume-thrust.md`.
- [x] Time-series momentum, sized on daily ranges (gold 15m, 2018–25) → 60d
  PF 2.03 on 75 trades, 98th of the sized random-hold null, 86th of its own
  trades with sides permuted; three trades are the profit; the confirmation
  window cannot produce 30 trades. `2026-09-13-tsmom-2.md`.
- [x] Trend pullback (EMA20 in an EMA200 trend, swing stop; gold 2018–25,
  BTC 2024–26) → PF 0.38 / 0.51, 0th percentile on both; trend-following on
  intraday bars closed as a family. `2026-09-13-trend-pullback.md`.
- [x] The recent-year screen (owner's criterion: the last twelve months at
  Vantage; 17 mechanisms × 4 sessions × 2 timeframes + 24 hour-holds) → one
  row of ~170 passes, noise by the registration's own count; five indicator
  mechanisms added to the registry. `2026-09-13-recent-year-screen.md`.
