# Time-series momentum on silver: the 20-day sign passes 2010–2018 and is a coin flip on 2018–2026; with gold, the daily-rebalanced version on intraday bars is closed on two assets

**Date:** 2026-09-13 (late night) / 2026-09-14
**Question:** Does the sign of silver's trailing 20-, 60- or 120-day return,
read on the last bars before each weekday's 17:00 New York close and held
until it flips, beat sized random holds and its own permuted sides on
Dukascopy 2010-06 → 2018-06 and again on 2018-06 → 2026-05 — the second asset
and the clock the closed gold record asked for? Registered at `57c14a7`
before any run, on bars no run had read. `docs/hypotheses/2026-09-13-tsmom-silver.md`.
**Outcome:** **Closed.** One lookback of three passed the primary — the
20-day row, 189 trades, PF 1.341, 100th of the sized random-hold null, 98th
of its own permuted sides — and on the confirmation it is PF 1.026 at the
63rd and 62nd. The 60-day row failed the primary and passes the gate on the
confirmation inside both nulls (94th, 72nd); the 120-day row was inverted on
the primary (0th of both nulls) and nothing on the confirmation. No lookback
passes both windows. Gold's `2026-09-13-tsmom-2` and this are two assets; the
family — daily-rebalanced TSMOM on intraday bars, at Vantage cost, self-managed
— is closed here. No paper run.

## What was measured

- Method: `tsmom`, existing and unchanged — at the first bar at or after
  16:15 New York on a weekday (`rebalanceHHMM = 1615`, gate
  `hours:1615-1700`, filter `weekdays`), hold the side of the trailing
  `lookbackDays` return; flip on a sign change; otherwise stay. Sizing 1% of
  $10,000 per two mean New York-day ranges over 20 days (`riskDailyRanges = 2`,
  `rangeDays = 20`), used for lots and R, not enforced; `Exits::Strategy`. The
  Friday decision is taken before the weekend and holds through Sunday's
  reopen; there is no Sunday *entry* — but the weekday filter gates entries
  only, so a sign flip is acted on at the Sunday 18:00 reopen bar as an
  exit, with re-entry Monday 16:15 (data-integrity's correction; the
  registration said "no Sunday rebalance" and that was untrue for exits).
  Warmup `(lookback + 5) × 288` bars, written for five-minute bars: on this
  feed's 92 bars a weekday that is 105 / 274 / 528 days of each window before
  the 20 / 60 / 120-day rows trade (the registration wrote 75 / 195 / 375,
  which forgot the weekends); the 20-day row's effective windows are
  2010-09-14 → 2018-06-14 and 2018-10-04 → 2026-05-29.
- Data: `xagduka` 15m from Dukascopy bid minutes (fetched and converted
  2026-09-13 night, `py/ingest/dukascopy_to_parquet.py`, flats dropped).
  Primary 2010-06-01 → 2018-06-14 (190,884 bars); confirmation 2018-06-17 →
  2026-05-29 (183,212 bars). Weekdays with a 16:15 bar: 2,017 of 2,089 and
  1,935 of 2,056, counted before the run. Costs: Vantage XAGUSD.sc, spread
  $0.021 an ounce (measured read-only from 205k ticks), swap-free, contract
  unit one ounce — "lots" in the diagnostics are ounces.
- Fixed replay (the test) against 300 sized random holds; walk-forward as
  the check; 1,000 side permutations. Receipts:
  `docs/research/runs/2026-09-13-tsmom-silver/`; diagnostics
  `diag-tsmom-20d-{primary,confirmation}.txt` from `examples/diag_tsmom.rs`
  (not receipts). Guards off, as in every receipt.

## Evidence

Primary, fixed replay (`in-sample-fixed.txt`) with the direction percentile
from `direction-tsmom-*.txt`:

```
hypothesis     trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
tsmom/20d         189   1.341   0.118    0.945    1.199  100%   98th       SURVIVES
tsmom/60d         105   1.129   0.060    0.933    1.355   75%   75th       fail: profit factor 1.129 < 1.2
tsmom/120d        108   0.406  -0.272    0.893    1.733    2%    0th       fail: profit factor 0.406 < 1.2
```

Walk-forward (`in-sample.txt`): 20d 166 trades PF 1.265 (95%, "gate pass,
inside the noise"); 60d 1.054 (66%); 120d 0.301 (0%).

Confirmation, fixed replay (`out-of-sample-fixed.txt`, `direction-tsmom-*-oos.txt`):

```
hypothesis     trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
tsmom/20d         186   1.026   0.022    0.949    1.233   63%   62nd       fail: profit factor 1.026 < 1.2
tsmom/60d          95   1.532   0.315    0.933    1.589   94%   72nd       gate pass, inside the noise
tsmom/120d        104   0.933  -0.004    0.992    2.076   45%   49th       fail: profit factor 0.933 < 1.2
```

Walk-forward (`out-of-sample.txt`): 20d 157 trades PF 1.027 (55%); 60d 76
trades 1.314 (82%); 120d 70 trades 1.248 (71%). Both files end "Nothing
survived".

The 20-day row, trade by trade (`diag-tsmom-20d-primary.txt`,
`diag-tsmom-20d-confirmation.txt`; $10,000, 1% per trade):

```
window        trades  net $   return   maxDD          top-5 share  PF w/o top-5   mean hold   per year
2010–2018       189   2,337   23.4%   $1,334  9.9%      134%          0.885        13.8 d      2010 4.87, 2011 1.57, 2012 2.15, 2013 2.04, 2014 1.72, 2015 0.58, 2016 0.79, 2017 1.63, 2018 0.53
2018–2026       186     199    2.0%   $2,034 18.4%    1,781%          0.564        13.8 d      2018 1.17, 2019 0.86, 2020 1.50, 2021 1.21, 2022 0.72, 2023 0.35, 2024 0.85, 2025 2.23, 2026 0.17
```

Walk-forward folds on the primary: 2.58, 1.53, 0.72, 0.78 — the pass is
2010–2014. On the confirmation: 1.58, 0.65, 0.60, 1.32. The five best trades
of the primary are all 2010–2014 holds of six to twelve weeks at +4.8R to
+6.1R; without them the row is under water. The worst is a sixteen-day long
in August 2015, −1.95R — with a sizing unit of two daily ranges, the average
adverse excursion of a hold is half a unit, one full daily range.

## What each role said

- **adversary:** SURVIVED, for "closed; with gold, closed on two assets".
  Registration precedes receipts (`57c14a7` 23:25 → `df8f66f` 23:28 →
  `e88b3db` 23:31). *"No arithmetic admits a survivor."* The 20-day primary
  pass *"was a maximum of three draws carried by five trades in 2010–14"*:
  top-5 133.7% of net, PF without them 0.885, 2015–18 net −$803, direction
  p = 0.024 on one of three lookbacks — *"Bonferroni 3 × 0.024 = 0.072 —
  fails 0.05 even before the confirmation."* The 120-day row's 0th
  percentile *"is exactly as informative as a 100th"* (two-sided p ≈ 0.004
  on 2010–18, dead at the 49th on 2018–26), and *"reading tails two-sided
  doubles the draw count to six."* The 60-day confirmation at 1.532 is
  *"inside the top quartile of coin flips on these intervals"* and *"the
  gold tsmom-2 shape … both on the 2018–26 metals rally (every top-5
  confirmation trade is a long). Same window, same sign, two correlated
  metals: one regime, not two confirmations."* Two instrument findings: the
  sized random-hold null for `tsmom` holds `lookbackDays × 1440 / 2` minutes
  (`hypotheses.rs:256–286`), 27% shorter than the 20-day row's realised
  13.8 days and 2.4× longer than the 120-day row's — *"not matched … the
  '100th' is inflated by ~0.06 PF"* (the direction null, on the method's own
  intervals, is the tighter test and decided nothing differently); and
  whether any XAGUSD deal was in the swap-free deal history *"is not stated
  anywhere in the repo — unknown"*, which for a 13.8-day silver hold *"only
  strengthens the closure."* Costs are not immaterial: ≈ $556 of spread
  against $2,337 net on the primary. What would change its mind: a third,
  uncorrelated asset (EURUSD is on disk) with one pre-registered lookback,
  the sized null's hold matched to realised holds, the spread measured inside
  the rebalance gate, ≥ 95th of both nulls on both eight-year windows.
- **data-integrity:** NO OBJECTION. 1m 5,285,287 rows, 15m 374,188,
  monotone, no duplicates, no OHLC violations, the 15m a byte-equal
  recompute of the 1m; coverage 94.8% / 91.9% of five-day weeks (the
  confirmation three points thinner: ~40 whole-day feed outages, 2021–24);
  every 16:15 bar at 20:15Z in EDT and 21:15Z in EST across 17 years; the
  17:00 break absent before 2013 as on gold; all ten diag holds match bar
  closes to the cent; 73 moves over 3% on 15m, all known events (2011-05-01
  −7.6% at a Sunday reopen, 2014-11-30 the Swiss vote, 2021-01-31 +6.1%).
  Seam: Friday 2018-06-15 belongs to neither window, no shared bar; *"out of
  sample in time, not in venue."* Three corrections to the description: the
  warmup is **105 / 274 / 528 days** on this feed (92 bars a weekday, not
  288 a day), not the 75 / 195 / 375 the registration wrote — the 20-day
  row's first trade, 2010-09-14, is the first post-warmup bar, and both
  effective windows still exceed 7.6 years; the spread sensitivity — *"+$0.02
  an ounce → PF ≈ 1.26; +$0.04 → ≈ 1.18, below the gate. The primary pass
  was within one spread assumption of failing"*; and the method's clock:
  *"`filter.rs` gates entries only; `tsmom.rs` fires on Sunday 18:00 (prev
  bar Friday). Three of the diag's best-5 exit on Sundays … the registration's
  'no Sunday rebalance' is untrue for exits."* The method exits at the Sunday
  reopen and re-enters Monday 16:15, on both windows alike.
- **risk:** NO OBJECTION to closing with no promotion. Paper boundary
  unchanged (the only code change is the diagnostic; it imports no network
  crate). Tail at 1% a trade: maxDD $1,334 / 9.9% and $2,034 / 18.4%; worst
  hold −3.09R / −$304 (2020-02-18 → 02-28, confirmation); *"the typical hold
  sits one full daily range under water, unstopped, for ~13.8 days"*;
  2022–2024 three losing years, −$1,217. *"To earn anything worth running —
  say 20% — sizing must be roughly 10× … the drawdown that produced 18.4% is
  account-ending, and the single −3.09R hold alone is −31%. The size that
  earns is the size that ruins."* Weekend: the 2011-02-04 → 05-04 long was
  open through Sunday 1 May 2011, when silver fell about $6 at the Asian
  open. One change to the standing list: for a self-managed hold a
  maximum-hold guard *"fights the sign-flip exit and would just re-enter";*
  the right guard is *"an unrealised-loss cap in R on the open position,
  evaluated every bar including the reopen bar, closing at market."*

## Reading

The literature's anomaly is real on monthly bars and futures across decades
and asset classes; what this loop has tested twice is a particular
implementation of it — a daily sign read on an intraday CFD feed at a
retail spread, self-managed, with a fixed lookback — and on two assets and
four windows that implementation has now passed exactly the windows it was
first shown and none of the ones after: gold 2018–25 (60-day) and not the
permuted null; silver 2010–18 (20-day) and not 2018–26. Each pass was a
handful of long trend holds in a few years (2020 on gold; 2010–2014 on
silver), which is what trend following looks like when it works, and which
the count-matched null and the permuted sides are built to price. The
120-day row's 0th percentile on 2010–2018 silver is worth a sentence: on
that window the four-month sign predicted the opposite of the next weeks,
strongly, and on the next window it predicted nothing. A sign that flips
its meaning between decades is not one to hold across a weekend unstopped.

## What would reopen this

- The adversary's terms, and only those: a third, uncorrelated asset (EURUSD
  2010–2026 is on disk as `eurduka`) with **one** pre-registered lookback,
  the sized random-hold null drawn from the method's realised holds (an
  instrument change, in the backlog), the spread measured inside the
  16:15–17:00 gate, and ≥ 95th of both nulls on both eight-year windows. If
  that passes, the two metals were one regime and the family reopens on the
  third asset; if it fails, three assets have said the same thing.
- Otherwise a different registration: the method as the literature measures
  it — monthly or daily bars, a volatility-scaled position, a diversified set
  of futures, the 12-month sign — with a futures cost model, not a CFD feed;
  and a reason to believe a retail account at Vantage can carry it. That is a
  research program, not a row in this loop.

## What this does not say

- It does not say time-series momentum is false. It says a daily-rebalanced,
  fixed-lookback version on 15-minute CFD bars at $0.021–0.28 spread does not
  survive its second window on either of two metals.
- It does not say the 2010–2014 silver run was noise; those trades earned
  what the receipts say. It says the claim that the sign keeps earning failed
  on 2015–2018 and again on 2018–2026.
- It does not test EURUSD; the bars are on disk (`eurduka`) and the terms
  under which it would be worth registering are written above. Two
  correlated metals on the same 2018–26 rally are, as the adversary said,
  one regime, and a third asset that is not a metal is the only thing that
  can say whether the family or the regime failed.
- It does not say the account is swap-free on silver. The measurement is
  gold's deal history; no silver deal is known to be in it. A generic silver
  swap over a 13.8-day hold would take more than the row's expectancy, so
  the closure does not depend on it.
- Every "100th" of the sized random-hold null on a `tsmom` row is inflated
  by about 0.06 PF, because the control holds half the lookback in minutes
  rather than what the method realised. The direction null is unaffected and
  decided every row here the same way.
