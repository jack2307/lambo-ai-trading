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

---

## Note added 2026-10-11 - these rows and the window their argv did not state (appended by `agent/receipt-reconcile`; nothing above is rewritten)

`agent/pfr-audit` (`76c0b57`) listed 4 rows of this record among the 34 published decision-record rows that no longer print a cell at their published trade count when their own echoed argv is re-run. Re-measured today with the shared patched binary (`/e/rust/fd-stop-width/target-sw/release/search.exe`, `29c8635`):

```
row            published      verbatim argv today   with --from=2022-06-16 pinned
hivol/london   341  PF 0.950  1712                  341  PF 0.950   EXACT
hivol/orb60    457  PF 1.004  2115                  457  PF 1.004   EXACT
lowvol/london  212  PF 0.962   894                  212  PF 0.962   EXACT
lowvol/orb60    73  PF 1.074   478                   73  PF 1.074   EXACT
```

**Cause: the data store, not this measurement.** The published command omits
`--from=`, and `XAUDUKA-5m` has since been backfilled from 2022-06-16 to
2010-06-01 (198,716 bars -> 1,135,389). The "What was measured" section above
already states the window ("Primary `xauduka:5m` 2022-06-16 -> 2025-04-10");
the prose was right and the argv was under-specified. **All four rows return
exactly, profit factor included. Nothing in this decision moves.**

Cost/R per row, from the engine's own cost-matched-null line on those control
runs, with the stop size that sets it - on ONE instrument, one feed, one
spread:

    hivol/london   stop 6.534 ATR(5m) = 14.51 pts   cost  1.93% of R
    hivol/orb60    stop 4.437 ATR(5m) = 10.42 pts   cost  2.69% of R
    lowvol/london  stop 7.424 ATR(5m) =  9.53 pts   cost  2.94% of R
    lowvol/orb60   stop 3.957 ATR(5m) =  5.32 pts   cost  5.26% of R

The spread charged, 0.28, is **ASSUMED BY TRANSFER**: no `XAUDUKA` spread log
exists on disk and 0.28 is the Vantage `XAUUSD.sc` read applied to a Dukascopy
BID series.

Full table, every control run and the per-row spread provenance: `docs/decisions/2026-10-11-receipt-reconcile-result.md`, `receipts/receipt-reconcile/`.
