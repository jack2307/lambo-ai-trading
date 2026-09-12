# Volatility does not rescue the breakouts: the 2025–26 survivals were the year, not the regime

**Date:** 2026-09-13
**Question:** Do the London-range and opening-range breaks pay when absolute
volatility (ATR14/close on five-minute bars) is at or above 0.075%, and not
below it? (`docs/hypotheses/2026-09-13-volcond-breakout.md`, threshold fixed
from both windows' ATR distributions before the run; registered at `e8713e8`.)
**Outcome:** No. On the primary window the high-volatility rows fail the gate
(PF 0.950 and 1.004) and sit at the 70th–84th percentile of nulls gated to
the same hours and the same condition; the low-volatility contrast rows are
indistinguishable from them (77th–78th). Direction nulls inside on all four.
The confirmation window was not opened. Closed, and with it the breakout
family: no further range-break variant runs without a new mechanism.

## What was measured

- `orb`, London preset and New York 60-minute preset, each under
  `volabs:14:0.075-9` and its complement `volabs:14:0-0.075`, hours-gated,
  flat over the break, weekdays. `Filter::VolAbs` added for this test.
- Primary `xauduka:5m` 2022-06-16 → 2025-04-10 (30% of bars above the line,
  measured beforehand). 4-fold walk-forward, 200 gated null seeds, direction
  nulls per row with the same filters.
- Receipts: `docs/research/runs/2026-09-13-volcond-breakout/`.

## Evidence

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
hivol/london           341   0.950  -0.015    0.909    1.091   70%  fail
lowvol/london          212   0.962  -0.020    0.816    1.241   77%  fail
hivol/orb60            457   1.004   0.004    0.884    1.115   84%  fail
lowvol/orb60            73   1.074   0.033    0.809    1.489   78%  fail
```

Direction nulls: hivol/london 0.950 → 32nd; lowvol/london 0.966 → 67th;
hivol/orb60 1.043 → 91st; lowvol/orb60 1.020 → 85th. Inside.

## Reading

The claim was the most charitable reading of three failures, stated so it
could fail: if breaks pay when volatility is high, the high-volatility 30%
of 2022–25 should show it and the low-volatility 70% should not. Neither
half shows anything, and they do not differ. So the 2025–26 survivals were
not "high volatility"; they were one year on one feed, and the project
cannot trade a year that has ended.

What the night's five records say together: on gold five-minute bars at
Vantage's cost, no range break (opening, London, ICT sweep), no level fade
(yesterday's extremes), no session or regime gate, and no indicator baseline
carries information about direction on a multi-year window. The direction
nulls are the sharpest statement — 43rd–54th percentile on every long-window
test — and they are symmetric: the fade of a break is as uninformed as the
break.

## What each role said

Reviews were not spawned; the row that came closest (`hivol/orb60`, 84th /
direction 91st) fails the gate on expectancy (0.004R) and is inside both
nulls, and the contrast rows did the adversary's work. The record stands or
falls on the receipts.

## What would reopen this

A mechanism, not a gate. The two the backlog still holds: options-flow
levels once the tape covers months (the project's founding thesis, untested
because the tape is a week), and — if a volume feed is found for the long
window — reversion to a session volume-weighted price. Not another window,
threshold or session on a break.

## What this does not say

- It does not say gold is untradeable intraday; it says these mechanisms
  at this cost are not distinguishable from noise over 2022–25.
- It does not say the 2025–26 in-sample results were bugs: they replicate,
  they simply do not generalise.
