# The London fix: no drift into or out of it at retail cost, 2022–25

**Date:** 2026-09-13
**Question:** Does gold still drift down into the LBMA afternoon fix (09:30 →
10:00 New York) and recover after it (10:00 → 10:30), as the pre-2015
literature found — measurable as a 30-minute hold on five-minute bars at
Vantage's spread? (`docs/hypotheses/2026-09-13-london-fix.md`, registered at
`d98576d`.)
**Outcome:** No. On the primary window (Dukascopy 5m, 2022-06 → 2025-04-10,
578 holds per row) the afternoon rows have PF 0.961 (short into) and 0.904
(long after) against random holds of the same length at p50 0.84–0.86, p95
1.01–1.02: 84th and 74th percentile, below the gate on profit factor and
expectancy. The morning-fix contrast rows are worse (0.491, 0.811). Direction
nulls 78th / 88th / 23rd / 64th, all inside. Confirmation not opened. Closed.

## Evidence

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
fix/pm-into            578   0.961  -0.037    0.860    1.020   84%  fail
fix/pm-after           578   0.904  -0.061    0.837    1.008   74%  fail
fix/am-into            578   0.491  -0.312    0.698    0.837    0%  fail
fix/am-after           578   0.811  -0.123    0.715    0.891   84%  fail
```

Direction nulls (same holds, coin-flip side, 1000 assignments): pm-into
0.916 → 78th; pm-after 0.943 → 88th; am-into 0.608 → 23rd; am-after
0.708 → 64th.

The control here is the new drift null — random *holds* of the row's length
from the row's start bar, 200 seeds — not random entries with ATR stops; its
95th percentile sits near 1.0 where the old control's sat near 3.0, which is
what a control for a 30-minute hold should look like.

## Reading

The afternoon rows are the only ones that are not obviously bad, and "not
obviously bad" here means a short into the fix that loses 4 cents a round
trip less than noise. Whatever the fix did to prices when the auction was a
phone call among five banks, on 2022–25 five-minute bars at $0.28 it is not
there, in either half hour. The morning-fix contrast losing more is the
sample telling the truth: a half-hour short in a rising market loses.

## What each role said

Reviews not spawned: no row near the gate, all inside both nulls, and the
control was built for this shape. The receipts are the review.

## What would reopen this

Tick data around 10:00 New York with the actual auction timestamps, on a
cost below $0.10. Not more days of five-minute bars.

## What this does not say

- It does not say the fix has no price impact within the minute; five-minute
  bars cannot see that and this project does not trade it.
- It does not say the 2025–26 window would agree; not opened.

## Addendum (2026-09-13, late morning) — re-run after the fold-boundary fix

The engine let a hold open at a fold's end run to the end of the data
(`2026-09-13-instrument-faults.md`). The batch file has no lower bound and
the Dukascopy file has since been extended back to 2018, so the re-run at
`9fde7c1` is on 2018-06 → 2025-04 (1,401 holds per row), not this record's
2022–25 (578): `fix/pm-into` 0.935 (92nd), `fix/pm-after` 0.932 (96th, gate
fail on profit factor), `fix/am-into` 0.536 (0th), `fix/am-after` 0.683
(52nd). No row passes the gate on the longer window either; the verdict
stands. The 2022–25 receipts are kept under
`docs/research/runs/2026-09-13-london-fix/before-fold-close/`.
