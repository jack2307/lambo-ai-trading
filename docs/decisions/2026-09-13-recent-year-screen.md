# The recent-year screen: 170 looks at the last twelve months nominate nothing; the long window, run as context, shows an evening drift

**Date:** 2026-09-13 (afternoon and evening)
**Question:** Under the owner's criterion — good on the most recent year at
Vantage is enough — which of the registry's seventeen mechanisms (twelve
existing, five added: Keltner break, MACD cross, RSI(2) pullback, squeeze
break, stochastic reversal), all day / New York / Asia / London, on 15m and
5m, and which two-hour hold of the day, pass the gate and both nulls on
`xauusd` 2025-09-13 → 2026-09-12? (`docs/hypotheses/2026-09-13-recent-year-screen.md`,
registered at `b196cd7`, amended `843900d`, `a29d521`.)
**Outcome:** Under the program's own falsifier, **one row of about 170 passes**
(`hold/18-20-long`), against eight or nine expected by luck; the registration
said in advance that a list shorter than that is noise, and it is. Every
mechanism with a stop and a target fails on the last year, in every session,
on both timeframes. The long window (2022–25), run as context, shows one
structure the year did not invent: **long across the New York close and the
evening — 16:00 to 22:00 — beats random holds and its own flipped sides at the
100th percentile on 460–560 sessions**, and the same blocks sit at the
94th–96th on the last year. That is not a candidate from this screen; it is
the next registration, under the original rule.

## What was measured

Primary: Vantage `xauusd` 15m (23,531 bars) and 5m, 2025-09-13 → 2026-09-12;
walk-forward with four folds, so the scored trades are the last 80% of the
year (from 2025-11-25 — 9.6 months, as data-integrity counted); the
count-matched null gated to each row's hours, 200 seeds; the direction null
on the method's defaults over the whole year, 1,000 draws. Context: Dukascopy
`xauduka` 2022-06-16 → 2025-04-10, the same batches. Spread $0.28, no swap
(the account is swap-free, measured). The gap rows were amended once (the
weekday and break filters had excluded the Sunday reopen bar). Receipts:
`docs/research/runs/2026-09-13-recent-year-{screen,screen-5m,sessions,sessions-5m,hours,gap}/`.

The spread at the daily break, which the adversary said would erase any hold
that trades the reopen, was measured from 4.6 million MT5 ticks over twelve
days (`py/ingest/mt5_spread_probe.py`, read-only): p50 $0.26, p90 $0.27, max
$0.27 in the first five minutes after 18:00 New York; $0.20 in the sessions.
The flat $0.28 model is not generous to the holds; it is slightly severe.

## Evidence

Mechanisms, 15m, all day / New York (`recent-year-screen/in-sample.txt`), the
best rows — nothing survives:

```
hypothesis                 trades  OOS PF  expect  null p50 null p95   pct
keltner-break/all             533   1.052   0.028    0.965    1.210   78%
rsi2-pullback/ny              198   1.110   0.039    0.989    1.387   74%
macd-cross/all                923   1.010   0.011    0.965    1.210   66%
donchian-breakout/all         519   0.979  -0.005    0.965    1.210   58%
```

Mechanisms, 5m (`recent-year-screen-5m/in-sample.txt`): best `ict-sweep-mss-fvg/ny`
1.098 on 24 trades (83rd), `orb/all` 1.026 (80th); nothing survives.

Sessions, 15m (`recent-year-sessions/in-sample.txt`) — the Asian rows:

```
hypothesis                 trades  OOS PF  expect  null p50 null p95   pct  verdict
ema-cross/asia                 37   1.632   0.294    1.004    1.441   99%  SURVIVES
macd-cross/asia               300   1.556   0.276    1.004    1.441   98%  SURVIVES
keltner-break/asia            262   1.315   0.154    1.004    1.441   90%  gate pass, inside the noise
```

Their direction nulls, on the defaults over the whole year: macd-cross/asia
417 trades PF 1.167 → 93rd (p = 0.074); keltner-break/asia 1.155 → 93rd;
ema-cross/asia 130 trades PF 0.845 → 49th. None at the 95th. The walk-forward
numbers are the selected cell's out-of-fold trades; the direction file is the
default cell over the year, and the two are not the same trades — the
adversary's first point, and a limit of the instrument for gridded rows.

Sessions, 5m: nothing passes the gate; the Asian breakouts have the side
right (direction 95th–98th for keltner, squeeze, donchian) at PF 1.07–1.13.

Two-hour holds, recent year (`recent-year-hours/in-sample.txt`, random-hold
null) and the long window (`…/out-of-sample.txt`, 2022–25):

```
                        recent year (164-206 sessions)          2022-25 (460-578 sessions)
row                     PF     drift   direction                PF     drift   direction
hold/16-18-long         1.390  94th    96th                     1.508  100th   100th  SURVIVES
hold/18-20-long         1.386  95th    95th   SURVIVES          1.140  100th   100th  (PF < 1.2)
hold/20-22-long         0.856  28th    —                        1.238  100th   —      SURVIVES
hold/04-06-short        1.481  100th   89th   SURVIVES          0.588    0th   8th
hold/04-06-long         0.604   1st    —                        1.139  100th   —
```

## What each role said

- **adversary:** VETO on every row of the recent year. *"Under the file's
  own falsifier exactly one of ~170 rows passes … by the program's own
  pre-registered arithmetic, the recent-year screen nominates nothing."* On
  the holds: a parameterless session-hold has nothing to select, so *"the
  'OOS PF' column is an in-sample number on 80% of a trending year"*; the
  drift null and the direction null of a hold *"are the same coin"*; 95th on
  200 seeds *"is not distinguishable from 16-18-long's failing 94th."* The one
  row it could not break: `hold/16-18-long` on the long window, *"1.359 fixed,
  699 trades, direction p = 0.000 … it fails the program it was registered
  under and passes the one it wasn't."* Its cost objection — a reopen spread
  of several dollars — was falsified by the tick measurement above; it had
  named the measurement as what would stop the objection.
- **data-integrity:** NO VETO. Gates and clocks correct (the Asian window
  wraps midnight; the New York weekday mask drops Sunday; DST at 2025-11-02
  and 2026-03-08 handled; 99.5% bar coverage; no splice). Two caveats that
  change the reading: the walk-forward trade sets are **9.6 months**, and
  `hold/16-18-long` *"exits at the first bar ≥ 18:00 — on Fridays that bar is
  Sunday 18:00, so ~38 of its 197 trades are 50-hour weekend holds."* The row
  is mislabelled; its Friday leg is a weekend hold.
- **risk:** VETO on both holds as paper candidates, NO VETO on
  `macd-cross/asia` at 0.5% with guards on (and it fails the gate on its
  default cell). On `hold/18-20-long`: *"top five trades = 61.9% of net; PF
  without them 1.153 … worst open excursion −9.11R = −$1,212 (12% of the
  account, unstopped) … 112 days under water … there is no exit in these two:
  the only exit is the clock."* The ATR of a single 15-minute bar is not a
  risk unit for an eight-bar hold. And the premise: *"28.9 round-turn lots a
  year on $10k … $231 of rebate against $808 of spread. Rebate never pays for
  trading; only the edge does."*

## Reading

The last twelve months are not the year the earlier records feared. On
2025-04 → 2026-09 the New York morning passed almost anything; on 2025-09 →
2026-09 nothing with a stop passes anywhere. What the year does show, and
the long window confirms at the 100th percentile, is a drift that no
stop-and-target method captures because it is not a move within a bar's
reach: gold rises across the New York close and through the early evening.
It survives a 24%-volatility year and a 13% one. It is the kind of claim the
loop was built to test properly — long window primary, a risk unit that
fits a timed hold, the Friday leg separated from the weekday one, the
reopen spread measured — and it is registered next as
`2026-09-13-close-reopen-drift`, not promoted from here.

## What would reopen this program's rows

For any Asian-session mechanism: a direction null on the trades the table
scored (the walk-forward's selected cell), at the 95th — an instrument change,
noted in the backlog. For the 04:00–06:00 short: nothing; it is
sign-inverted on the long window.

## What this does not say

- It does not say the last year is untradable; it says nothing in this
  registry with a stop and a target is distinguishable from noise on it.
- It does not say the evening drift is a bot; risk's arithmetic on lots and
  spread stands until a sized, stopped version is tested.
- The sessions and 5m context on the long window had not finished when this
  was written; they are context, and the record will carry them when they land.
