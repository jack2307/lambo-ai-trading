# TSMOM on EURUSD: the 60-day sign passes the gate and the sized null on 2010–2018 and its own permuted sides do not — three trades are the row; three assets, one shape

**Date:** 2026-09-14 (small hours)
**Question:** On a third asset that is not a metal, with one lookback chosen
before the run, the sized null holding what the method held, and the gate
spread measured, does the sign of EURUSD's trailing 60-day return — read on
the last bars before 17:00 New York on each weekday and held until it flips
— beat sized random holds and its own flipped sides on Dukascopy 2010-06 →
2018-06, and again on 2018-06 → 2026-05? The adversary's terms from
`2026-09-13-tsmom-silver`. Registered at `d4c3c35` before any run, on bars no
run had read. `docs/hypotheses/2026-09-14-tsmom-eurusd.md`.
**Outcome:** **Closed on the primary; the confirmation was not opened.** 108
trades, PF 1.809, 0.284R, 100th percentile of the sized random-hold null —
and the 85th of its own sides permuted (p = 0.152), under the registered
95th. That is the condition gold's 60-day row failed at the 86th. The
row's net is three trades: without its top five the profit factor is 0.617,
and the other six years of the eight net 0.04R a trade. With gold and
silver, daily-rebalanced time-series momentum on intraday CFD bars has now
been read on three assets and shows one shape on each: a handful of long
trend holds carry the window, the sized null cannot produce them, and the
direction null cannot resolve them. No paper run.

## What was measured

- Method: `tsmom`, unchanged — first bar ≥ 16:15 New York on a weekday
  (`rebalanceHHMM = 1615`, `hours:1615-1700`, `weekdays`), hold the side of
  the trailing 60-day return, flip on a sign change; 1% of $10,000 per two
  mean New York-day ranges (Sunday evenings no longer counted, `f65a061`),
  a sizing stop only; `Exits::Strategy`. On EURUSD 16:15 is not before a
  close — the feed has bars in every 17:xx hour (8,365 in the window); it is
  the thinnest hour of the FX day, and a rebalance time, nothing more.
- Data: `eurduka` 15m, Dukascopy bid minutes converted 2026-09-13 night.
  Primary 2010-06-01 → 2018-06-14 (200,576 bars; 2,088 of 2,098 weekdays
  carry the 16:15 bar). Confirmation registered as 2018-06-16 → 2026-05-31
  and not opened. Costs: Vantage EURUSD.sc, spread 0.00014 (0.00020 p90
  inside the 16:15–17:00 gate, from 2,180 ticks over three days, read-only);
  swap zero as the account's stated terms, not measured on EURUSD; contract
  unit one euro.
- Fixed replay (the test) against 300 sized random holds of the method's
  own mean hold (33,558 minutes ≈ 23 days); walk-forward as the check;
  1,000 side permutations. Receipts `docs/research/runs/2026-09-14-tsmom-eurusd/`;
  `diag-tsmom-60d-primary.txt` from `examples/diag_tsmom.rs` (re-run on the
  current sizing at `6a64e06`; not a receipt). Guards off, as in every
  receipt.

## Evidence

```
hypothesis     trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
tsmom/60d         108   1.809   0.284    0.998    1.445  100%   85th       fails the falsifier: direction 85th < 95th
```

(`in-sample-fixed.txt`; `direction-tsmom-60d.txt`: null p05 0.421, p50
0.987, p95 2.271, max 3.571, actual 1.809 → 85th, p = 0.152. The runner's
own verdict column says SURVIVES because it reads the gate and the sized
null; the registration's third condition is the one that decides.)
Walk-forward (`in-sample.txt`): 88 trades PF 1.903, 0.349R, 99th.

The row, trade by trade (`diag-tsmom-60d-primary.txt`; $10,000, 1% a trade):

```
net $   return   maxDD         top-5 share  PF w/o top-5   win rate   mean hold   longs/shorts
3,155   31.6%    $793  6.0%     147%          0.617         38.0%      23.3 d      52 / 56
```

Per year: 2011 1.09, 2012 2.49, 2013 0.62, **2014 8.56 (6 trades, $2,123)**,
2015 0.65, 2016 0.73, 2017 1.88, **2018 19.96 (5 trades, $631)**. The two
years are 87% of the net; the other six, 97 trades, net $401. Walk-forward
folds 1.27 / 4.57 / 0.47 / 3.01. The largest trade is a short held
2014-05-09 → 2015-04-30, 356 days, +23.1R — the euro's fall from 1.39 to
1.12 — and a coin flip on that one interval is ±23R.

## What each role said

- **adversary:** SURVIVED, for "closed on the primary, confirmation not
  opened; three assets". Sequencing clean. The closure is the registered
  text. On the direction null: valid under the null hypothesis, and
  *"saturated: on this trade list it could not have reached 0.05 however
  good the sign rule was"* — because the top three trades sum $3,933
  against a net of $3,155 and *"the p the test reports is therefore
  approximately P(the top three keep their sign) ≈ 1/8"*; *"a result made
  of three trades is unresolvable by any test of 108."* On the sized null:
  *"mean-matched, tail-unmatched — a fixed 23-day random hold cannot
  produce a 356-day, +23R trade, so the null's PF is bounded where the
  method's is not; the 100th is inflated by the exit rule, not by the
  sign."* On EURUSD: 16:15 *"is the thinnest hour of the FX day"*; spread
  ≈ $120 over the window, immaterial; a generic swap ≈ −$316, 10% of net,
  *"a 2026 rate applied to 2010–18 carry."* Falsifiable pattern across the
  three assets: *"≤ 5 trades carry ≥ 100% of net, the direction null never
  falls below p = 0.02"* — and *"removing the three largest trades from any
  tsmom window in this repo leaves PF ≤ 0.9."* What would change its mind:
  the EURUSD confirmation at PF ≥ 1.2 with its top three excluded, and a
  null drawing holds from the realised hold *distribution* that the row
  still beats at the 95th. It also caught a stale diagnostic file
  (pre-`f65a061` sizing), recommitted at `6a64e06`.
- **data-integrity:** NO OBJECTION. 1m 5,881,397 rows, 15m 398,220, monotone and
  unique, no OHLC violations, the 15m a zero-difference recompute of the
  1m; no volume on this feed; bid only. Coverage 99.59% of the FX schedule
  (Sunday 17:00 → Friday 17:00); no halt Monday–Thursday (the 17:xx bars are
  every weekday), Friday's last bar 16:45 on 417 of 418, Sunday's first
  17:00 on 415 of 418; ten non-weekend holes, all Christmas/New Year plus
  one lost Monday (2017-12-11). The 16:15 bar: 2,088, at 20:15Z ×1,373 and
  21:15Z ×715 — the New York clock, not a fixed UTC hour. Eleven 15-minute
  moves over 1% in eight years, all known events (SNB, Brexit, the 2016
  election, the 2017 French election gap); *"all ten worst/best diag holds
  match the 16:15 close to 2 dp … all exits sit inside the exit day's
  low/high."* The day-range fix drops Sundays (6.75 h) and holiday stubs
  and keeps Fridays (16.75 h); the 20-day mean is 1–27% higher than before,
  so lots are smaller, not larger. Scan-back: 60 days old 1,640×, 61 days
  411× (Saturday targets), 62 twice, never more — with one note for the
  method, not the data: *"the scan matches by day only, so the reference
  close is 23:45 New York of that day, not 16:15."* Spread cannot be
  checked historically (no ask column; retail EURUSD was 1–2 pips in
  2010–12). *"That is a claim about one 2014–15 trend, not about a method,
  and the 85th percentile is the honest reading of it"*: the one trade
  2014-05-09 → 2015-04-30 is 70.6% of net.
- **risk:** NO OBJECTION to closing with no promotion; BLOCK on any
  promotion. Paper boundary unchanged (the commits since its last review
  are the null's hold length, the day-range fix, and docs). Tail: maxDD
  $793 / 6.0%; worst hold −2.37R / −$295 (2016-05-27 → 06-08 short, 10,378
  units); avgMAE −0.54R against a two-daily-range unit; *"2014 $2,273 of
  $3,532 net (one trade, +24.9R)"* on the pre-fix diagnostic, the same
  trade at +23.1R on the current one; fold 3 −$741. At the 3.2× that
  would have earned 10% a year: maxDD 22–30%, the 2015–16 stretch ≈ −$1,670,
  *"still PF 0.63 without five trades. That is a loan against 2014."* Swap
  if not swap-free: ≈ −$14 a long trade, 40% of the mean net trade;
  *"'small' is not the right word; it stays unmeasured in the record."*

## Reading

Three assets, four windows, one shape. On each the implementation earns
its window from three to five long trend holds of many weeks; the sized
random-hold null, matched to the mean hold, cannot generate a 356-day
interval and so the method sits at its 100th; and the permuted-sides null,
which can, puts the row at the 85th–86th on gold and EURUSD because the
question "would those three intervals have paid with the sign flipped" has
about one answer in eight. Silver's 20-day row was the one case where the
sign cleared the 95th, and it was a coin flip on the next eight years. The
adversary's sentence is the record's: a result made of three trades is
unresolvable by any test of a hundred, and a test that cannot resolve it is
not a test the trade can pass. The family on this implementation is closed
on three assets.

## What would reopen this

- Only the adversary's two numbers, on the confirmation window this
  registration named and did not open: the 60-day row on `eurduka`
  2018-06-16 → 2026-05-31 at PF ≥ 1.2 with its top three trades excluded,
  and at or above the 95th of a sized null that draws its holds from the
  primary's realised hold distribution (an instrument change, in the
  backlog). No re-tune, no other lookback, no fourth asset first.
- Otherwise the family is closed on this implementation, and the different
  one — daily bars, futures, a diversified set, the twelve-month sign, a
  volatility-scaled book — is a research program with its own data and its
  own reason, not a row in this loop.

## What this does not say

- It does not say time-series momentum is false, on EURUSD or anywhere. It
  says a daily-rebalanced 60-day sign on 15-minute CFD bars, sized on daily
  ranges and held unstopped, produced three trades of profit in eight years
  and nothing a null can price.
- It does not say the direction null is wrong. It is valid under the
  null and saturated by the tails; the saturation is a fact about the
  trade list, not the test.
- It does not measure EURUSD swap on the account; zero is stated. A generic
  rate would cost about a tenth of the net.
- It does not say what the confirmation window shows; it was not opened,
  as registered.
