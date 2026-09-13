# 2026-09-13-friday-weekend-hold: gold held from Friday's close to Sunday's reopen rises, on eight years nobody has looked at

**Registered:** (commit time is authoritative) — before any run of this batch;
the data for the window was fetched first and no backtest has touched it
**Status:** in-sample run — the Friday row fails the primary; closed, under review
**Batch file:** `docs/hypotheses/2026-09-13-friday-weekend-hold.toml`

## Where this comes from

`2026-09-13-close-reopen-drift` closed with one row it could not settle: long
from Friday 16:30 New York to Sunday 18:30, the weekend hold. It passed the
2018-06 → 2025-04 primary (336 holds, PF 2.064, 0.054R, 100th of the sized
random-hold null and of the direction null), passed the years before the
screen that nominated it when the adversary asked (2018-06 → 2022-06, 194
holds, PF 2.213, 100th on direction — a post-hoc diagnostic, not a receipt),
and could not be confirmed on the 67 Vantage Fridays since 2025-04 (PF 1.720,
95th/91st, where nothing under PF 1.9 could have passed). The adversary named
what would change its mind: *Dukascopy XAUUSD 2010-06 → 2018-06, about 400
Fridays, registered before the run, the Friday row at or above the 95th of
the direction null.* This is that registration.

## Claim

Gold bought at the last Friday bar before the New York close and sold at the
first Sunday bar after the reopen earns more than the same holds with the side
flipped, on a window that predates every window the claim has been seen on.
The mechanism offered — and it is offered, not shown — is that the weekend is
the one period when gold's risk premium accrues with no way to hedge it, so
the Friday close underprices Sunday's open. If that is the reason, it does not
depend on the 2020s bull market, and 2010–2018 (a $1,900 top, a four-year
bear, a $1,050 floor, a recovery) is a fair place to ask.

A second row is a **prediction, not a test**: the weekday break row
(`close/1630-1815`, Monday–Thursday) on the same window. The closed record
read its net-zero-then-positive history as a proportional drift eaten by a
fixed $0.28 spread while gold was $1,300–1,900. Gold was $1,050–1,900 in
2010–2018. The reading predicts: direction ≥ 95th, net near zero, gate
failed. If the row is instead clearly profitable here, the reading was
wrong; if the direction is a coin flip here, the weekday drift was a 2018+
phenomenon and the record's "every year" needs an amendment. Either way it
does not change this hypothesis's verdict, which is the Friday row alone.

## Falsifier

On the primary, the Friday row replayed with the registered parameters (no
grid, `fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 holds), ≥ 95th
percentile of 300 sized random holds of the same length on the same Fridays,
≥ 95th of 1,000 side permutations. Then the confirmation window below, the
same three conditions. A miss on either closes the Friday leg for good; there
is no third window on this feed, and the record will say so.

## Base method

`session-hold`, `from = 1615`, `to = 1815`, `side = 1`, `riskDailyRanges = 1`,
`rangeDays = 20`, filters `weekdays:Fr`, `hours:1615-1620` — exactly the
closed registration's row, unchanged, including the risk unit the closed
record called a hundredth of a day for a two-hour hold; for a fifty-hour hold
it is what it was, and the gate stands as written.

## Data

- Primary: `xauduka:15m` bounded `2010-06-01 → 2018-06-15` (Dukascopy bid
  m1, resampled; flat closed-market bars dropped by the converter; fetched
  2026-09-13 night, `py/ingest/dukascopy_to_parquet.py`). This window has
  never been read by any run in this repository.
- Confirmation: `xauduka:15m` bounded `2018-06-16 → 2022-06-15` — the years
  before the screen that nominated the row. Seen once, post hoc, as a
  diagnostic at the adversary's request (`direction-close-fri-1630-1815-pre-screen.txt`);
  re-run here as a receipt so the decision quotes a registered file. The
  2022–25 and 2025–26 windows are on record already and are not re-run.
- Costs: Vantage's ($0.28 spread, no swap) on Dukascopy's bars, as before.

## Sample needed

About 415 Fridays on the primary, 208 on the confirmation; the earlier
windows show the null's p95 near 1.18 at 194–336 holds, so a PF of 1.5 or
better is what a pass looks like. Coverage is to be counted before the run
and written into the batch's receipt header (the runner prints it).

## What each outcome means

- The Friday row passes the primary and the confirmation → the weekend hold
  is a persistent structure on 2010–2025 (four windows, ~800 Fridays). The
  record reopens the Friday leg for a **paper-run proposal**, which then waits
  on the risk role's list from the closed record: a weekend guard, an
  unrealised-loss cap, a maximum hold and a notional cap, enforced and tested.
  No proposal is written before those exist.
- Passes the primary and not the confirmation, or the reverse → closed for
  good; the row is a property of some years and not others.
- Fails the primary → closed for good; the 2018–2025 pass was the decade.

## Coverage, counted before the run

`xauduka` 15m, 2010-06-01 → 2018-06-15: 191,893 bars on 2,505 days (the
converter dropped 677,967 flat closed-market minutes). Fridays with a 16:15
New York bar: 402 of 411 Friday days with bars; Monday–Thursday: 1,624 of
1,680. Friday's last bar is 16:45 New York in every year and season (the
17:00 close held throughout); Sunday's first bar is 18:00 in every year. The
weekday daily break did not exist on this feed before 2013 — Monday–Thursday
carry bars at 17:00–17:45 through 2012 — so the prediction row's "across the
close" is literal only from 2013; the Friday row's structure is the same in
all eight years. Fridays by year: 2010 31, 2011–2017 48–51, 2018 23.

## In-sample (2010-06-01 → 2018-06-15, `xauduka` 15m)

Fixed replay (`in-sample-fixed.txt`, 300 sized random holds) and direction
null (`direction-*.txt`, 1,000 permutations):

```
hypothesis          trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fri/1630-1815          400   0.983  -0.001    0.799    1.022   91%   92nd       fail: profit factor 0.983 < 1.2
close/1630-1815       1621   0.733  -0.010    0.596    0.669   99%  100th       fail: profit factor 0.733 < 1.2
```

Walk-forward (`in-sample.txt`): `fri/1630-1815` 321 holds PF 0.798 (51%),
`close/1630-1815` 1,301 PF 0.708 (99%). "Nothing survived".

The Friday row fails the primary: net −$73 on 401 weekend holds
(`diag-fri-2010-2018.txt`), 2011 and 2013 positive, 2014–2018 negative in
every year (PF 0.76, 0.78, 0.49, 0.66, 0.21). Closed for good, as
registered; the confirmation stage is not run. The prediction row: the side
is right (100th on direction) and the net is not "near zero" but −$1,459 on
1,621 sessions — the spread took more than the move at $1,050–1,900 gold
(and on 2010–2012 there was no daily break on this feed). The closed record's
"every year" is wrong for 2010–2018 and is amended in the decision.
