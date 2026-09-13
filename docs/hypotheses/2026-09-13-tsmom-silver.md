# 2026-09-13-tsmom-silver: the sign of silver's trailing return predicts its next weeks, on sixteen years of a second asset

**Registered:** (commit time is authoritative) — before any run of this batch;
the silver bars were fetched and converted first and no backtest has read them
**Status:** registered
**Batch file:** `docs/hypotheses/2026-09-13-tsmom-silver.toml`

## Where this comes from

`2026-09-13-tsmom-2` closed on gold: the 60-day row passed the gate on
2018–25 (PF 2.03 on 75 trades) and the sized random-hold null (98th) and
failed its own permuted-sides null (86th) — three trades were all of the
profit — and the 17-month Vantage confirmation could not reach 30 trades.
Its record named what would reopen the family: *a second asset with seven
or more years (Dukascopy silver or EURUSD are fetchable), a warmup counted
in days so the confirmation window exists, the Friday close and Sunday
reopen handled in the method, registered as its own claim with those
reasons — and the row's own permuted null at the 95th.* This is that
registration, on silver, with two eight-year windows.

## Claim

At the first bar after 16:15 New York on each weekday, hold the side of
silver's trailing 20-, 60- or 120-day return; flip when the sign flips;
otherwise stay. Time-series momentum (Moskowitz, Ooi & Pedersen 2012) is the
one anomaly with evidence across every asset class and every decade the
literature has looked at; silver is a second asset with its own price
history, not gold's sample re-labelled (2010–2018 silver: a $49 top in 2011,
a bear to $14, a range). Sized at one percent of equity per two daily
ranges (`riskDailyRanges = 2`, `rangeDays = 20`, the sizing stop used for
lots and R, not enforced — `Exits::Strategy`). Multi-day; admissible because
the account is swap-free (measured, deal history).

The clock is the reason for `rebalanceHHMM = 1615` and the gate
`hours:1615-1700`: the decision is taken on the last bars before the 17:00
close on every weekday including Friday, so the Friday position is chosen
before the weekend rather than skipped (at 18:00 Friday there is no bar);
Sunday's 18:00 reopen is not a weekday, so no Sunday rebalance. That is the
"Friday close and Sunday reopen handled" of the closed record, done with
parameters and the weekday filter rather than new code.

Warmup: the method's warmup is `(lookbackDays + 5) × 288` bars, written for
five-minute bars; on 15-minute bars it is three times what it needs — the
first 75 / 195 / 375 days of each window for the 20 / 60 / 120-day rows.
Both windows are eight years, so the confirmation exists with the warmup
paid; the method is not changed for this registration and the days lost are
stated here.

## Falsifier

Per row, on the primary, with the registered parameters replayed (no grid,
`fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 trades), ≥ 95th
percentile of 300 sized random holds of the same count on the same window,
and ≥ 95th of 1,000 permutations of the row's own sides — the null that
closed the gold row. The confirmation window below, the same three
conditions. A row that passes both is a survivor; anything less is closed,
and the three rows are three lookbacks of one claim, not three claims: a
pass on one lookback with the other two failing is a pass on one of three
draws, and the record will say so.

## Base method

`tsmom`, existing (`crates/fd-strategy/src/tsmom.rs`), parameters
`lookbackDays ∈ {20, 60, 120}`, `rebalanceHHMM = 1615`, `riskDailyRanges = 2`,
`rangeDays = 20`, filters `weekdays`, `hours:1615-1700`. No code change.

## Data

- Primary: `xagduka:15m` bounded `2010-06-01 → 2018-06-15` (Dukascopy bid
  m1 resampled, 374,188 fifteen-minute bars over 2010–2026 in the file;
  fetched and converted 2026-09-13 night, `py/ingest/dukascopy_to_parquet.py`).
  Never read by any run.
- Confirmation: `xagduka:15m` bounded `2018-06-16 → 2026-05-31`. Also never
  read; opened only after the primary's record is written.
- Costs: Vantage XAGUSD.sc, spread $0.021 an ounce (measured read-only from
  205k ticks over three days, p50 = p90), swap-free, contract unit one ounce
  (`config/default.toml`, `[markets.xagduka]`).

## Sample needed

A 60-day sign flips a handful of times a year: roughly 40–80 trades per
window for the 60-day row, more for 20, fewer for 120 — the 120-day row may
not reach 30 on one window, and that would be a fact about the row, not a
reason to shorten the lookback. The gold row's 75 trades carried its whole
profit in three; the permuted null exists for that.

## What each outcome means

- A lookback passes both windows on all three conditions → the first
  survivor in this loop with two eight-year windows behind it; a decision
  record, then a **paper-run proposal** on silver — which waits on the risk
  role's guards list (unrealised-loss cap, maximum hold, notional cap,
  guards applied by the runner) before it is written. Not an intraday bot,
  and not gold.
- Passes the primary, fails the confirmation (or the reverse) → closed;
  TSMOM on silver at this clock is a property of one decade.
- Fails the primary → closed; with gold's 2026-09-13-tsmom-2 that is two
  assets, and the family is closed on the intraday-bar implementation.
  (The literature's evidence is on monthly bars and futures; a daily-bar
  implementation on futures would be a different registration.)

## Coverage, counted before the run

`xagduka` 15m, weekdays with a 16:15 New York bar (the rebalance bar):

```
primary: 190968 bars, 2503 days; weekdays with a 16:15 NY bar 2017 of 2089
confirmation: 183220 bars, 2468 days; weekdays with a 16:15 NY bar 1935 of 2056
```
