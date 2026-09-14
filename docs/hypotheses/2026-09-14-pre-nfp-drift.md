# 2026-09-14-pre-nfp-drift: gold falls in the hour before the US employment report

**Registered:** (commit time is authoritative) — before the confirmation window
is read; the script and the primary's figures are committed with this file
**Status:** decided — `docs/decisions/2026-09-14-pre-nfp-drift.md`. All three
conditions hold on the test window (3.2nd percentile, −1.536 $/oz, 35.6% up,
n=90) on the instrument corrected at `436582c`. The exploratory figures quoted
below are **withdrawn**: measured with a New-York-clocked null the primary sits
at the 7.8th percentile, not the 0.4th, and would have failed this file's own
falsifier. Nothing is promoted; the news guard makes the hour untradeable.
**Instrument:** `scripts/news_drift.py` (no batch file — this is a sign claim,
not a strategy; see the falsifier)

## Where this comes from

The owner sent a survey of day-trading strategies
(sarwa.co, 2026-09-14). Eight of its nine are families this loop has closed on
multi-year windows — trend, range, momentum, breakout, pullback, gap, price
action, and a moving-average scalp. The ninth, **news trading**, had never been
tested here, and since 2026-09-14 the repository has the calendar to test it:
747 scheduled releases from 2010 (`data/news/events.csv`, `docs/news/README.md`).

So the first step was a measurement, not a registration:
`scripts/news_drift.py` on `xauduka` 2010-06-01 → 2018-06-15 — four event
classes (FOMC, US CPI, US Employment Situation, ECB) across five windows
(the day before, the hour before, the print, the hour after, the session
after), each against the same clock window seven days earlier. Twenty cells.

Nineteen of them say what everything else in this loop has said. One does not:

```
US Employment Situation, the hour before (97 releases, 2010-06 → 2018-06)
  actual        -0.886 $/oz   t -2.23   up 32.0%
  7 days earlier -0.480 $/oz   t -1.25   up 39.2%
  1000 date permutations, same weekday and same minute: mean -0.181 $/oz
  the actual sits at the 0.4th percentile of them
```

The permutation holds Friday and 07:30 New York fixed and varies only *which*
Fridays were release days, so it asks the right question: is this the release,
or is it Friday at half past eight? On this window it answers: the release.

**The multiplicity, stated before the test.** That 0.4th percentile is the most
extreme of twenty cells I looked at. Twenty looks at a 0.4% tail gives a
family-wise probability near 8%, which is why this is registered and not
believed. The other cells are context in the record and are not registered:
the session after NFP (+4.29 $, t +2.71) has a placebo of +3.59 (t +3.24) and
is a Friday afternoon, not a release; the hour before FOMC (+0.85, t +1.78) and
before the ECB (+0.58, t +2.06) are suggestive and were not chosen.

## Claim

On the eight years no run has read for this question — `xauduka`
2018-06-16 → 2026-05-31 — gold's move over the hour before the US Employment
Situation release (the open of the first bar at or after 07:30 New York to the
open of the first bar at or after 08:30) is negative by more than the same
hour on other Fridays. The mechanism offered, not shown: the month's
highest-impact US number is the one position books are squared into, and gold
is what gets sold to square them.

## Falsifier

All three, computed by
`python scripts/news_drift.py xauduka 2018-06-16 2026-05-31 --permute="US Employment Situation (NFP):-60:0" --draws=1000`:

1. The actual mean sits **at or below the 5th percentile** of 1000 date
   permutations that hold the weekday and the minute fixed.
2. The mean move is **≤ −0.40 $/oz** — half the primary's −0.886, the same
   halving the adversary demanded of `2026-09-14-fx-local-hours-sign`.
3. The share of releases whose hour was up is **≤ 42%** (the primary's 32%,
   halved toward a coin's 50%).

Any one failing closes the claim. The window is opened once and there is no
third window on this feed.

**The profit-factor gate is declared unreachable here, in advance.** The
effect is 0.886 $/oz gross on the primary; the spread is 0.28; twelve releases
a year at 1% of $10,000 on a ~$12 daily range is about 8 ounces, so the whole
claim is worth roughly $60 a year on a $10,000 account. **No outcome of this
registration is a paper run or a strategy**, and none of the ten paper books
will be changed because of it. It is a question about the world.

## Data

- Read (exploratory): `xauduka:15m` 2010-06-01 → 2018-06-15. The figures above
  come from it and it can never be the test.
- The test: `xauduka:15m` 2018-06-16 → 2026-05-31, never read for this
  question. Coverage was counted for `2026-09-13-friday-weekend-hold`: the
  feed is complete to 98% of its schedule over this span.
- The calendar: `data/news/events.csv`, the scheduled layer only (the live
  ForexFactory week is excluded by the script). US Employment Situation
  releases 2018-06 → 2026-05 are on file; the README records that 2025 has
  eleven rather than twelve because of the October shutdown.

## Sample needed

About 95 releases on the test window against 97 on the primary. The
permutation null is exact at that count; the t-statistic is the weaker of the
two readings and is reported, not gated.

## What each outcome means

- All three hold → the loop has one pre-registered, out-of-sample,
  clock-controlled directional effect on gold, worth about the spread. The
  record says so in those words, adds it to what is known about the hour, and
  proposes nothing.
- Any fails → closed. The primary was one of twenty looks, and this is what
  twenty looks does.
