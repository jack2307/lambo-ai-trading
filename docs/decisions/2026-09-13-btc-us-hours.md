# BTC US-hours drift: holding through the equity session lost money in 2024–26

**Date:** 2026-09-13
**Question:** Is BTC's return concentrated in US equity hours (09:30–16:00 New
York), such that holding only then captures the drift and the complement
window does not? (`docs/hypotheses/2026-09-13-btc-us-hours.md`, registered
at `4c42a13`.)
**Outcome:** No. On Binance 5m, 2024-08 → 2026-09, the US-hours hold has
PF 0.781 over 435 sessions; the complement 0.946; the Asian window 0.892.
None passes the gate; direction nulls 48th / 76th / 53rd. BTC rose over the
window and the rise did not happen between 09:30 and 16:00 New York. The
confirmation was not opened. Closed.

## Evidence

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
hold/us-hours          435   0.781  -0.740    0.994    2.976   37%  fail
hold/off-hours         435   0.946  -0.151    0.995    2.963   47%  fail
hold/asia              435   0.892  -0.337    0.882    2.550   51%  fail
```

Direction nulls: us-hours 0.913 → 48th; off-hours 0.974 → 76th; asia
0.925 → 53rd. Inside.

## Reading

Two caveats on the receipts, both making the negative *more* secure:
the expectancy column is in units of an ATR-sized risk the method does not
enforce (a six-hour hold travels many ATRs, so R is not the right unit — read
PF); and the random-entry null gated to a five-minute window has few trades
and a wide spread (p95 ≈ 3), so "37th percentile" is a loose statement. The
profit factor is not loose: long through US hours lost, on 435 sessions, at
$5 a round trip, while the asset went up.

The claim is an anomaly that has been written about; on these two years it
is not there, or the window it lives in is not this one.

## What each role said

Reviews not spawned: every row fails the gate on profit factor alone; no
null was needed to reject it.

## What would reopen this

A different window with a reason (e.g. the ETF creation/redemption window)
and a null built for drift tests — random *hold* windows of the same length,
not random entries with stops. The second is an infrastructure item.

## What this does not say

- It does not say the anomaly never existed; the literature's samples end
  before this one begins.
- It does not say anything about the Vantage feed; not opened.

## Addendum (2026-09-13, late morning) — re-run after the fold-boundary fix

The engine let a hold open at a walk-forward fold's end run to the end of
the data (`2026-09-13-instrument-faults.md`). Re-run at `9fde7c1` on the
same window: `hold/us-hours` PF 0.781 → **0.881** (26th percentile),
`hold/off-hours` 0.946 (43rd), `hold/asia` 0.892 (32nd); the null's 95th
percentile fell from 2.98 to 1.24. Every row still fails the gate; the
verdict stands. The record's original receipts are kept under
`docs/research/runs/2026-09-13-btc-us-hours/before-fold-close/`.
