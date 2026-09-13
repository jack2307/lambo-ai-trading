# 2026-09-14-intraday-momentum: gold's first half hour of New York predicts its last hour before the close

**Registered:** (commit time is authoritative) — before any run of this batch
**Status:** registered
**Batch file:** `docs/hypotheses/2026-09-14-intraday-momentum.toml`

## Where this comes from

Not from this loop's earlier rows. Gao, Han, Li and Zhou (2018, *Journal of
Financial Economics*, "Market intraday momentum") found that the first
half-hour return of the trading day predicts the last half-hour return, in
SPY over 1993–2013, out of sample, with a mechanism: late-informed traders
and end-of-day hedgers trade in the direction the morning set, and the
effect is strongest on high-volume and high-volatility days. The finding
has been replicated in other equity markets and in commodity futures. It
has not been asked of gold on this feed, and nothing in this loop resembles
it: the opening-range breakout trades a level, this trades a sign.

## Claim

On each weekday, the sign of gold's return from the 08:30 New York bar's
open to the 08:45 bar's close (the first half hour of the COMEX session)
predicts the sign of its return over the last hour and a quarter before the
17:00 close. Held from the 15:30 open (signalled on the 15:15 bar) to the
16:45 open (signalled on the 16:30 bar), sized at one percent of equity per
one mean daily range, `Exits::Strategy`, the position earns more than sized
random holds of the same window and more than the same trades with their
sides flipped. Three rows:

- `im/first` — the test: predictor 08:30–09:00, hold 15:30–16:45.
- `im/day` — the paper's second predictor: the day so far, 08:30–15:00,
  same hold. Weaker in the paper; registered as the same claim's other
  spelling, not a second draw.
- `im/mid` — the contrast: predictor 12:00–12:30, same hold. The paper
  finds the middle of the day predicts nothing. If this row passes too, the
  effect is not what the paper describes.

## Falsifier

On the primary, each row replayed with the registered parameters (no grid,
`fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 trades), ≥ 95th
percentile of 300 sized random holds of the same window and count, ≥ 95th
of 1,000 side permutations. The `first` row must pass, on the primary and
then on the confirmation; `day` may pass or not; `mid` must not pass (if
it does, the record says the effect is a time-of-day drift, not intraday
momentum, and the claim is closed as stated). The expectancy gate is in
units of one daily range for a 75-minute hold — the same units the
close-reopen record found unpassable for a two-hour hold. That is stated
here, before the run, and the gate is not changed: a hold this short has
to be worth 5% of a day after spread to be worth anything.

## Base method

`NEW: intraday-momentum` — `crates/fd-strategy/src/intraday_momentum.rs`,
implemented and committed before this file (the commit before this one); parameters `firstFrom`,
`firstTo`, `from`, `to` (New York HHMM), `minMove` (daily ranges, 0 = any),
`riskDailyRanges = 1`, `rangeDays = 20`. Entry on the first bar ≥ `from`,
filled at the next open; side = sign of close(last bar < `firstTo`) /
open(first bar ≥ `firstFrom`) − 1, same New York day, both bars ≤ the
signal bar; none if either bar is missing; exit on the first bar ≥ `to`,
filled at the next open. Nothing after the signal bar is read (tested).

## Data

- Primary: `xauduka:15m` bounded `2010-06-01 → 2018-06-15` (the eight years
  before the screen). Note: before 2013 this feed had no 17:00 break, so
  the "close" the hold runs into is a thin hour, not a halt.
- Confirmation: `xauduka:15m` bounded `2018-06-16 → 2026-05-31`.
- Costs: Vantage's, $0.28 spread, swap-free, one ounce per unit.

## Sample needed

Every weekday with an 08:30 bar and a 15:15 bar: about 2,000 sessions per
window. The floor is not the question; the effect size is — the paper's
last-half-hour return is a few basis points, and $0.28 on a $1,300–5,000
ounce is 0.6–2 bp. The units statement in the falsifier is the honest one.

## What each outcome means

- `first` passes both windows, `mid` does not → the paper's effect exists
  on gold at this feed; a decision record and a paper-run proposal (an
  intraday, clock-exited hold: the guards list is shorter than for a
  multi-day hold, and the risk role decides).
- `first` passes the primary only → closed; the record says which years.
- `first` fails the primary → closed. The mechanism is then tested on
  silver and EURUSD only if a new reason appears; it is not re-run with a
  different half hour.

## Coverage, counted before the run

```
primary: 2090 weekdays with bars; 2023 carry the 08:30, 08:45, 15:15 and 16:30 New York bars
confirmation: 2061 weekdays with bars; 1975 carry the 08:30, 08:45, 15:15 and 16:30 New York bars
```
