# 2026-09-13-btc-us-open: BTC breaks the range built around the US equity open

**Registered:** 2026-09-13 04:05 — before any run
**Status:** decided → docs/decisions/2026-09-13-btc-us-open.md (failed the primary; confirmation not opened)
**Batch file:** `docs/hypotheses/2026-09-13-btc-us-open.toml`

## Claim

BTC trades continuously, but its volatility is not flat: the US equity open
(09:30 New York) and the 08:30 data releases bring the day's largest flows
through ETF and futures desks that keep equity hours. The range built in the
first 30–60 minutes after 09:30 absorbs that flow; a five-minute close
outside it before 13:00 continues at least a range's width. A different
market from the closed gold family, with its own reason: the participants
that move BTC at that hour are the ones whose day starts then.

## Falsifier

Fails the gate or sits below the 95th percentile of a null gated to the same
entry window on the primary window; or survives the primary and fails the
confirmation window; or the direction nulls are inside. Both windows must
survive.

## Base method

`orb` (exists), New York clock, presets `rangeStart 0930 / 0830`,
`rangeMinutes 60 / 30`, `entryUntil 1300`, `maxRangeAtr 100`. Grid
`riskReward` × `entryBufferPips`. `pipSize` for BTC is 1.0 (a dollar), so
the buffer grid reads in dollars.

## Data

- Primary: `btc:5m` — Binance BTCUSDT, 2024-08 → 2026-09 (two years), Binance
  market costs ($5.00 spread as configured)
- Confirmation: `btcusd:5m` — Vantage BTCUSD.sc, 2025-09-26 → 2026-09-12
  (one year, $17.05 spread) — opened after the primary result is recorded;
  overlaps the primary in time, so it confirms *at the broker's cost on the
  broker's feed*, not on unseen dates. Stated so it is not read as more.
- Filters: weekdays, `hours:1030-1300` (or `1000-1300`, `0930-1300`) — no
  flat window: no break, swap-free.

## Sample needed

One trade a day at most, ~500 weekdays in the primary: ≥ 60 walk-forward
trades per row.

## What each outcome means

- Survives both windows and direction nulls → record, paper-run proposal.
- Survives primary only → closed; the confirmation is the broker's cost, so
  that failure would read "the edge is smaller than $17".
- Fails primary → closed. No other hour, no other range.
