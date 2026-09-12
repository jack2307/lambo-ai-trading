# London-range breakout: everything passes in 2025–26, nothing in 2022–25

**Date:** 2026-09-13
**Question:** Does the first New York-morning close outside the 02:00–08:00 New
York range continue at least a range's width? (`docs/hypotheses/2026-09-13-london-range.md`,
pre-registered at `2f793d0`; preset amended once before any informative run,
see below.)
**Outcome:** No. In-sample on Vantage 5m (2025-04 → 2026-09) two of three
variants pass the gate against a null gated to their own window *and* the
direction null is outside (96th) — the cleanest in-sample pass this project
has produced. On the pre-registered disjoint out-of-sample (Dukascopy 5m,
2022-06 → 2025-04-10) all three fail (PF 0.945–0.953, 338–525 trades) and the
direction null is a coin flip (43rd–46th). Closed. Third method today with
the same shape, which is now the finding.

## What was measured

- Method: `orb` with `rangeStart 0200, rangeMinutes 360, entryUntil 1200|1000`,
  `maxRangeAtr 100`. Grid `riskReward` × `entryBufferPips`, 4 folds, 200
  null seeds, direction nulls per preset (1000 assignments).
- In-sample `xauusd:5m` 2025-04-11 → 2026-09-11. Out-of-sample `xauduka:5m`
  cut with `--to=2025-04-10` (198,716 bars), flat-filled bars removed; the
  windows are disjoint (OOS max 2025-04-09 23:55, IS min 2025-04-11 01:05).
- Null gated to `hours:0800-1200` / `0800-1000` (lesson from the ORB pass).
- Receipts: `docs/research/runs/2026-09-13-london-range/`.

**Amendment.** The registered preset inherited `orb`'s `maxRangeAtr = 3`,
written for a one-hour range; a six-hour range trips it and the first run had
0 trades on every row (`in-sample-0trades.txt`). `maxRangeAtr = 100` was set
and the run repeated. The null did not move (p50/p95 identical in both
receipts), so nothing was fitted; but the amendment was committed together
with the result (`5e7f7be`), which the adversary correctly flags — the skill
now requires an amendment to be its own commit before the run.

## Evidence

In-sample:

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
london/nyam            257   1.317   0.086    0.956    1.241   98%  SURVIVES
london/early           203   1.309   0.084    0.942    1.293   96%  SURVIVES
london/expansion       131   1.209   0.054    0.951    1.374   88%  inside
```

Direction nulls in-sample: nyam 1.246 → 96th (p 0.042), early 1.268 → 96th
(p 0.036). (The `expansion` direction receipt is byte-identical to `nyam`'s:
`null-dir` did not apply the batch's `vol:` filter. Struck; fixed in the
runner after this pass.)

Out-of-sample, disjoint:

```
hypothesis          trades  OOS PF  expect null p50 null p95   pct
london/nyam            525   0.953  -0.018    0.890    1.073   77%  fail
london/early           458   0.947  -0.021    0.895    1.133   66%  fail
london/expansion       338   0.945  -0.017    0.894    1.119   66%  fail
```

Direction nulls out-of-sample: 0.956–0.957 → 43rd–46th. Inside.

The two windows, described from the bars: in-sample total +34%, up-days
53%, **annualised volatility 24%**; out-of-sample +67%, up-days 53%,
**13%**. Same drift, half the volatility.

## Reading

This is the third method today — ICT, ORB, London — to pass on the 2025–26
Vantage window and fail on 2022–25. They share nothing in their rules and
everything in their sample. The direction nulls say it plainly: the side of
an intraday break carried information in the 24%-volatility year and none in
the 13%-volatility years. A breakout method's edge, if it has one, is a
function of how far price travels after the break relative to a fixed $0.28
cost; at half the volatility the same break does not travel. That is a regime,
and the 2025–26 window is one regime.

Consequences for the loop, adopted now:

1. **The long window is the primary test.** Pre-registrations use
   `xauduka` 2022-06 → 2025-04 as in-sample and Vantage 2025-04 → 2026-09 as
   the confirmation; survival is required on both. A method that only works
   in the high-volatility year should be *registered* as conditional on
   volatility, with a null gated the same way, not discovered afterwards.
2. An amendment before a run is its own commit.
3. `null-dir` takes the batch's filters, so a filtered variant's direction
   null is its own.

## What each role said

- **adversary:** SURVIVED. Amendment was a specification fix (null unchanged
  between receipts); the in-sample null is twice as wide as the out-of-sample
  one (p95 1.24 vs 1.07) on four ~4-month folds; *"the side of the trade only
  carried information in the trending window."* Recommends the long window as
  primary. Caught the duplicated direction receipt.
- **data-integrity:** NO OBJECTION. Flat-bar removal drops every O=H=L=C bar
  (the CSV has no volume column — docstring corrected); 270,658 of them are
  weekends, 48,660 the 17:00 hour, 62 genuine quiet minutes in four years.
  Range completeness 98.4% OOS vs 98.9% in-sample. Windows disjoint. Notes
  that the out-of-sample receipt is a walk-forward *re-fit* on the
  out-of-sample data, more generous than replaying the in-sample parameters —
  and it still failed.
- **risk:** not re-run; the method and exits are the ORB's, reviewed the same
  day (BLOCK for any promotion, none proposed).
- **historian:** by the manager: the ORB record of the same day predicted
  this outcome and named the condition (disjoint window, gated null) under
  which London-range was worth running; both were met; it failed.

## What would reopen this

A pre-registered, volatility-conditional version (entries only when realised
volatility is above a stated absolute level, null gated identically) that
survives on *both* windows. Not another window on this one.

## What this does not say

- It does not say the New York morning break is random in 2025–26; it says
  it does not generalise, and the project cannot trade a year that has ended.
- It does not say Dukascopy and Vantage disagree: they correlate 0.983 where
  they overlap.
