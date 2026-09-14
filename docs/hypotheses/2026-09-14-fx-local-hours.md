# 2026-09-14-fx-local-hours: the euro falls during European hours and rises during American ones

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** in-sample run — every row has the sign right at the 96th–100th of both nulls and fails the profit-factor gate; closed as registered, under review
**Batch file:** `docs/hypotheses/2026-09-14-fx-local-hours.toml`

## Where this comes from

Not from this loop. Breedon and Ranaldo (2013, *Journal of International
Money and Finance*, "Intraday patterns in FX returns and order flow") find
that a currency depreciates during its own local trading hours and
appreciates outside them, across the major pairs and a decade of data, with
a mechanism: local commercial demand for foreign currency is net one-sided
during local business hours (importers and investors buy the foreign
leg), and the order flow moves the price. The effect is a few basis points
a day. Ranaldo (2009) found the same pattern earlier. It has not been asked
of this feed, and it is not a breakout, a level, a trend or a sign of a
past return: it is a clock.

## Claim

On each weekday, EURUSD held short from the 03:00 New York open (the
European morning, 08:00 London) to the 11:00 open (the end of European
liquidity, 16:00 London), and held long from 11:00 to 17:00 (the American
afternoon), earns more than sized random holds of the same windows and
more than the same holds with their sides flipped, on Dukascopy 2010-06 →
2018-06 and again on 2018-06 → 2026-05. Rows:

- `fx/eu-short` — short 03:00 → 11:00 New York (signal 02:45, exit signal
  10:45). The test.
- `fx/us-long` — long 11:00 → 17:00 (signal 10:45, exit signal 16:45 —
  which on this feed is filled at the 17:00 bar's open; the feed has no
  halt). The test's other half.
- `fx/asia-long` — long 19:00 → 03:00 (signal 18:45, exit 02:45): outside
  both currencies' local hours the paper predicts little; a contrast.

Sized at one percent of equity per one mean daily range (`riskDailyRanges
= 1`, `rangeDays = 20`), `Exits::Strategy`, `weekdays`. The expectancy gate
is in daily-range units for an eight- or six-hour hold; a hold of a third
of a day has to be worth 5% of a day after 1.4 pips. Stated before the run.

## Falsifier

Per row, fixed replay against 300 sized random holds of the same window
and count and 1,000 side permutations: gate, ≥ 95th, ≥ 95th, on the
primary; then the confirmation, the same. The two local-hours rows are one
claim in two halves; the record reads them together and says if only one
half holds. The Asian row passing does not help the claim.

## Base method

`session-hold`, existing. `from`/`to`/`side` per row, `riskDailyRanges = 1`,
`rangeDays = 20`, filters `weekdays`, `hours:<from>-<from+5>`.

## Data

- Primary: `eurduka:15m` bounded `2010-06-01 → 2018-06-15`. Read once
  before, by `2026-09-14-tsmom-eurusd`, for a different claim (a 60-day
  sign, 108 trades); these windows and this clock have not been read.
- Confirmation: `eurduka:15m` bounded `2018-06-16 → 2026-05-31`. Never read.
- Costs: Vantage EURUSD.sc, spread 0.00014, swap-free (stated), one euro a
  unit.

## Sample needed

About 2,000 sessions a row per window. The effect the paper reports is
2–4 bp a day for the local-hours leg; 1.4 pips is 1.2 bp at 1.15. A pass
is a PF near 1.2 on 2,000 holds, and the nulls at 2,000 holds are narrow.

## What each outcome means

- Both local-hours rows pass both windows → the paper's effect exists on
  this feed at this cost; decision record, then a paper-run proposal (an
  intraday clock hold; the risk role decides the guards).
- One half passes both windows → recorded as such; no proposal from one
  half alone unless the risk role is satisfied it is a whole.
- Fails the primary → closed. Not re-run on another pair; the paper's
  pattern on GBP or JPY would be a separate registration with its own
  data.

## Coverage, counted before the run

The null for this batch is the first to draw its holds from the method's realised hold distribution (`e0d46c7`); for a fixed window that is the window itself.

```
primary: 2098 weekdays with bars; 02:45 NY bar on 2092, 10:45 on 2087, 18:45 on 1673
confirmation: 2071 weekdays with bars; 02:45 NY bar on 2062, 10:45 on 2056, 18:45 on 1651
```

## In-sample (2010-06-01 → 2018-06-14, `eurduka` 15m)

Fixed replay (`in-sample-fixed.txt`, 300 sized random holds of the same
window, the corrected control) with the direction percentile
(`direction-fx-*.txt`):

```
hypothesis      trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fx/eu-short       2087   1.069   0.014    0.922    1.027  100%   99th       fail: profit factor 1.069 < 1.2
fx/us-long        2082   0.983  -0.002    0.879    0.971   96%   97th       fail: profit factor 0.983 < 1.2
fx/asia-long      1669   0.979  -0.002    0.847    0.949   98%   98th       fail: profit factor 0.979 < 1.2
```

Walk-forward (`in-sample.txt`): 1.104 (100%) / 0.975 (93%) / 0.913 (87%).

Every row fails the gate and every row has the side right: the euro falls
through European hours and rises through American and Asian ones, on
1,669–2,087 holds each, at the 96th–100th of random holds of the same
window and the 97th–99th of the rows' own sides permuted. The Asian
contrast was registered as "the paper predicts little"; it came in with
the sign of "outside the euro's hours the euro rises", which is the paper's
statement, and a profit factor under one. The drift is worth about the
spread: 0.014R a hold on the short — 1.4% of a daily range for eight hours
— after 1.4 pips. Closed on the gate as registered; the confirmation is
not opened by this registration.
