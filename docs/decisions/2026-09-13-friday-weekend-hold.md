# The weekend hold in gold fails the eight years before anyone looked at it: 2018–2025 was the decade, not the structure

**Date:** 2026-09-13 (late night)
**Question:** Does gold held from Friday's 16:30 New York to Sunday's 18:30
beat sized random holds and its own flipped sides on Dukascopy 2010-06 →
2018-06 — the window the adversary named, before the run, as what would
reopen the Friday leg of `2026-09-13-close-reopen-drift`? Registered at
`fd5b99c`, coverage counted and committed at `63bd7b9`, before any run.
`docs/hypotheses/2026-09-13-friday-weekend-hold.md`.
**Outcome:** **Closed for good.** 400 weekend holds, PF 0.983, net −$73;
91st percentile of the sized random-hold null, 92nd on direction; 2011 and
2013 good, 2014 through 2018 negative in every year. The 2018–2025 pass (336
holds, PF 2.06, 100th) was a property of those years. The confirmation stage
was not run, as registered. The prediction row — the weekday break at
$1,050–1,900 gold — has the side right (100th on direction) and nets −$1,459
on 1,621 sessions: the closed record's "every year since 2018" stands, and its
"drift eaten by the spread" reading now has eight more years in which the
spread ate more than the drift.

## What was measured

- Method: `session-hold` long, entry on the 16:15 New York bar signal
  (`hours:1615-1620`), filled 16:30; exit at the first bar ≥ 18:15, which on
  Friday is Sunday's, filled 18:30. No stop; sized at 1% of $10,000 over one
  mean New York-day range (20 days). The closed registration's row, unchanged.
- Data: `xauduka` 15m extended tonight from a fresh `dukascopy-node` m1
  download (2,842,014 minutes 2010-06-01 → 2018-06-15, 677,967 flat
  closed-market bars dropped, `py/ingest/dukascopy_to_parquet.py`; merged by
  `py/ingest/merge_bars.py`, no overlap with the existing file, which began
  2018-06-17). Bounded 2010-06-01 → 2018-06-15: 191,893 bars, 402 of 411
  Fridays with a 16:15 bar, Friday's last bar 16:45 and Sunday's first 18:00
  in every year. Costs: Vantage's, $0.28 spread, no swap, contract unit one
  ounce.
- Fixed replay (the test) against 300 sized random holds; walk-forward as the
  check; 1,000 side permutations. Receipts:
  `docs/research/runs/2026-09-13-friday-weekend-hold/`. Diagnostics
  (`diag-*.txt`, not receipts) from `examples/diag_close.rs`. Guards off, as
  in every receipt.

## Evidence

Fixed replay (`in-sample-fixed.txt`) with the direction percentile from
`direction-*.txt`:

```
hypothesis          trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
fri/1630-1815          400   0.983  -0.001    0.799    1.022   91%   92nd       fail: profit factor 0.983 < 1.2
close/1630-1815       1621   0.733  -0.010    0.596    0.669   99%  100th       fail: profit factor 0.733 < 1.2
```

Walk-forward (`in-sample.txt`): `fri/1630-1815` 321 holds PF 0.798 (51%);
`close/1630-1815` 1,301 PF 0.708 (99%). "Nothing survived".

The Friday row by year (`diag-fri-2010-2018.txt`): 2010 1.36 (30 holds),
2011 2.48, 2012 0.97, 2013 2.03, 2014 0.76, 2015 0.78, 2016 0.49, 2017 0.66,
2018 (to June) 0.21. Net −$73; max drawdown $1,144 (10.3%) at 1% risk per
hold; worst holds −1.32R (2017-04-21 → 04-23, 1285.00 → 1269.81) and −1.25R
(2016-11-04 → 11-06, the US election weekend); top five = $462 against a
negative net; PF without them 0.85. At a fixed one ounce a hold
(`diag-fri-2010-2018-per-ounce.txt`, the adversary's check on the inverse-vol
sizing): PF 1.136, net $73 an ounce over eight years, 2014–2018 at 0.84,
0.73, 0.58, 0.67, 0.22 — under the gate on either sizing. Median |P&L| per hold $11.85 on 6.6 oz,
i.e. $1.80 an ounce; the spread is $0.28 of that.

The four windows the row has now been read on:

```
window                       holds   PF     direction   net $      what it was
2010-06 → 2018-06 (this)       400   0.983   92nd         -73      registered test, before any look
2018-06 → 2022-06              194   2.213  100th       1,235      post-hoc diagnostic, then a registered confirmation never run
2018-06 → 2025-04              336   2.064  100th       1,975      the closed record's primary
2025-04 → 2026-09 (Vantage)     68   1.720   91st         416      the closed record's confirmation, "inside the noise"
```

The prediction row, weekday 16:30 → 18:30 (`diag-close-2010-2018.txt`):
negative in every year but 2013 (1.23); net −$1,459; direction 100th. The
registration predicted "direction ≥ 95th, net near zero"; the side came in
as predicted and the net did not — it is below zero, not near it. On
2010–2012 this feed had no daily break at all (bars at 17:00–17:45), so
"across the close" is literal only from 2013; the 2013–2018 years alone are
1.23, 0.48, 0.79, 0.51, 0.82, 0.94.

## What each role said

- **adversary:** SURVIVED, for "the Friday leg is closed for good".
  Registration precedes receipts (`fd5b99c` 22:36 → `63bd7b9` 22:46 →
  receipts 23:17). Not an artefact: *"the fill is the 18:30 Sunday open, not
  the 18:00 reopen print … the worst/best-5 are all identifiable news
  weekends in both directions"* (2011-08-05 the S&P downgrade, 2016-11-04
  the election, 2017-04-21 the French first round, 2013-08-30 Syria,
  2015-06-26 the Greek referendum). Not power: *"sized-null p95 1.022,
  direction p95 1.017, max 1.211 on 400 holds … a PF of 2.0 would have sat
  at the 100th; even the gate's 1.2 clears the 95th. The disagreement is
  real: 92nd, p = 0.077."* Not the trend either way: *"2013 2.03 in gold's
  −28% year; 2016 0.49 and 2017 0.66 in +8%/+13% years"*; what the good years
  share is stress. On the closed record's reading: the weekend move was
  *"$0.25/oz in 2010–18 … against $1.31/oz in 2018–25 — 5.2× smaller while
  price was only ~1.4× lower"*; the weekday drift $0.13 against $0.28,
  *"not proportional; the closed record's Reading should be amended"* — done
  at `3e31e76`. The direction null pays the spread once on each side
  (`search.rs:465`); the percentiles across all records stand. Its
  falsifiable caveat — inverse-vol sizing puts the largest lots (9.0–9.3 oz)
  in the losing years, so *"a fixed-1-oz replay has PF between 1.00 and 1.15
  and 2014–2018 each under 1.0"* — came back PF 1.136, 2014–2018 at 0.84,
  0.73, 0.58, 0.67, 0.22 (`diag-fri-2010-2018-per-ounce.txt`). A caveat, not
  a reopening: *"changing sizing now would be moving the gate."*
- **data-integrity:** NO OBJECTION. The extension is *"a clean, complete,
  real-quote series"*: 98.28% of scheduled slots, zero bars outside the
  schedule, zero overlap with the old file (old rows identical to the
  `.bak`), zero OHLC violations, the seam a real $2.29 Sunday gap. Sunday
  reopens: 400 of 414 at 18:00 New York, *"0 exact duplicates, 0 flat 18:00
  bars"*, the 18:00 bar holds a median 15 quoted minutes — *"the exit is a
  live print, not a stale one."* Friday 16:15 bars at exactly 20:15Z (EDT
  ×266) and 21:15Z (EST ×136); 16 holds straddle a DST switch correctly.
  Caveats: 14 Sundays open late (19:00, DST-start Sundays; five in a row in
  March–April 2014) and five Christmas weekends exit Monday — longer holds
  at real prices; 2010–2012 has *"thin, genuine quoting through the CME
  break"* (32–38 quoted minutes an hour), which widens the prediction row's
  "across the close" and touches nothing on Fridays; the feed is bid only,
  so whether $0.28 was achievable at a 2010 Sunday reopen cannot be checked —
  but *"the gross before spread is ≈ +$670 on 400 holds — a PF near 1.1,
  below the 1.2 gate even at zero cost."* Warm-up dropped one Friday, not
  four; the `--to` bound is exclusive and dropped 2018-06-15.
- **risk:** NO OBJECTION to closing with no promotion; BLOCK stands on any
  paper run of a weekend hold. Paper boundary unchanged (the commits since
  the last review touch `docs/` and an ingest helper). Tail on the new
  window at 1% per hold: *"maxDD $1,144 / 10.33%; worst hold −1.318R / −$137
  (2017-04-21, 9.03 oz ≈ $11,600 notional — already more than the account,
  unstopped, through a closed market); −1.245R on the 2016-11-04 US-election
  weekend; worst run 2014–2018, five negative years, −$879."* At the 3.45×
  that would have earned 10% a year on 2018–25: *"2014–2018 ≈ −$3,033
  (−30%), the peak drawdown ≈ $3,950 (≈ 40%) … the size that earns on
  2018–25 loses a third of the account on 2014–18."* One sharpening for the
  guards list: *"an unrealised-loss cap cannot act between Friday 17:00 and
  Sunday 18:00 — nothing can close — so for this family the notional cap set
  before entry is the only limit that exists on the day it matters."*

## Reading

This is what a pre-registered out-of-sample window is for. The weekend hold
had passed everything it had been shown — seven years, the pre-screen years
inside them, a 100th percentile twice over — and the honest objection was
that every one of those windows sat inside one decade. The eight years before
it say no, and not narrowly: five consecutive losing years, a 10% drawdown
at the registered size, a net that the spread alone explains. The mechanism
offered in the registration — an unhedgeable weekend premium — would not
take 2014–2018 off; the gold trend would (2011 top, a bear to 2015, flat to
2018, a bull from 2019). The row was the trend, held over weekends.

The weekday prediction adds a smaller thing to the closed record: the side
of the close-to-reopen move is right in every year since 2010, and the move
is smaller than $0.28 in most of them. That is not an edge and never was; it
is a fact about gold and a fixed spread.

## What would reopen this

Nothing. The Friday leg has been read on four windows spanning 2010–2026,
passed the two inside the 2018–2025 bull, and failed the one before it and
the one after. There is no further gold window on this feed, and a different
asset would be a different registration with its own reason.

## What this does not say

- It does not say the 2018–2025 result was an error. Those 336 holds earned
  what the receipts say; the claim that they would keep earning is what
  failed.
- It does not say weekend holds are negative in general; it says the long
  gold weekend hold nets zero over 2010–2018 after a $0.28 spread.
- It does not say the 2010–2018 Dukascopy feed is the broker's price at the
  Sunday reopen; the exit fills at the feed's 18:30 open minus half the
  spread, and the data-integrity role's checks on the seam and the reopen
  prints are quoted above.

## Amendment (2026-09-14): the sized-null column was computed with a late-exiting control

The random-hold control signalled its exit one bar after the method's
(`2026-09-13-instrument-faults.md`, addendum item 3); on a weekend hold that
is the Sunday 18:15 fill against the method's 18:30 — fifteen minutes. The
91st percentile of the sized null and the 92nd of the direction null agree
with each other; the direction null is unaffected and decided the row.
Fixed at `4eaef94`.
