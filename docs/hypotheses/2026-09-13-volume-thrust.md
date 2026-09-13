# 2026-09-13-volume-thrust: a bar on unusual volume that closes at its extreme continues

**Registered:** (commit time is authoritative) — before any run
**Status:** decided → docs/decisions/2026-09-13-volume-thrust.md (BTC PF 0.960 / 50th, gold 0.931 / 78th; closed on both primaries)
**Batch file:** `docs/hypotheses/2026-09-13-volume-thrust.toml` (BTC, Binance volume), `2026-09-13-volume-thrust-gold.toml` (gold, Vantage tick volume)

## Claim

A bar with volume ≥ 2.5× the mean of the last 20 and a close in the top
(bottom) fifth of a range of at least one ATR is a bar where one side was
filled in size and not pushed back. The participant who did that is not
done. Going with the bar — stop beyond its other extreme, target 1.5R —
pays. This is the first hypothesis in the registry that uses volume as
the *signal* rather than a filter, and the only mechanism left in the
backlog's "needs volume" note that the collectors do not gate.

## Falsifier

Gate and ≥ 95th percentile of the count-matched null gated to the same
hours; survives the confirmation; direction null ≥ 95th. The `fixed` rows
pin the defaults; the others walk `volMult × riskReward` forward.

## Base method

`NEW: volume-thrust` — `Exits::Engine`; first thrust of a run only (the
previous bar was not itself a thrust). Needs `volume` on the bars.

## Data

Vantage BTCUSD bars carry no volume (checked: zero in the 15m file), so
the BTC test is a time split of one feed: primary `btc:15m` (Binance)
`2024-09-12 → 2025-12-31`, confirmation `btc:15m` from `2026-01-01`
(eight months). Gold uses Vantage's own tick volume, which is what the bot
would see: primary `xauusd:15m` `2022-06-16 → 2025-04-10`, confirmation
from `2025-04-11`. Dukascopy has no volume; the seven-year window is not
available to this hypothesis and the record will say so.

## Sample needed

Thrusts at 2.5× are a few a day on BTC 15m: ~1,000 on the primary. Gold
tick volume is spikier; similar order.

## What each outcome means

- Survives both on either market → record; the first volume-signal method
  with a record, and a reason to keep the tape collectors running.
- Survives the primary only → regime; closed.
- Fails → closed; "unusual volume" on a 15-minute bar is news, and news is
  priced by the time the bar closes.
