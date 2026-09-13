# 2026-09-14-tsmom-eurusd: the sign of EURUSD's trailing 60-day return predicts its next weeks, on a third asset that is not a metal

**Registered:** (commit time is authoritative) — before any run of this batch;
the EURUSD bars were fetched and converted first and no backtest has read them
**Status:** in-sample run — passes the gate and the sized null, fails the direction null (85th); closed as registered, under review
**Batch file:** `docs/hypotheses/2026-09-14-tsmom-eurusd.toml`

## Where this comes from

`2026-09-13-tsmom-silver` closed with the adversary's terms for what would
change its mind: *a third, uncorrelated asset (EURUSD 2010–2026 is on disk)
with one pre-registered lookback, the sized null's hold matched to realised
holds, the spread measured inside the rebalance gate, ≥ 95th of both nulls
on both eight-year windows.* Gold and silver passed the first window each
was shown and failed the next; the adversary's reading was that two
correlated metals on one rally are one regime, and only an asset that is
not a metal can say whether the family or the regime failed. This is that
registration. The null now holds what the method held (`f65a061`); the
sizing range no longer counts Sunday evenings (same commit).

## Claim

At the first bar after 16:15 New York on each weekday, hold the side of
EURUSD's trailing 60-day return; flip when the sign flips; otherwise stay.
One lookback, chosen now: 60 days is the row gold nominated
(`2026-09-13-tsmom-2`), the middle of the literature's 1–12-month range on
daily data, and neither of the two silver picked (20 passed then failed;
120 inverted). Sized at one percent of equity per two daily ranges
(`riskDailyRanges = 2`, `rangeDays = 20`), used for lots and R, not
enforced; `Exits::Strategy`. Multi-day. Swap: the account is swap-free by
gold's deal history; no EURUSD deal is known to be in it, so zero is the
account's stated terms, not a measurement — the record will say so, and a
generic EURUSD swap over a two-week hold is small next to the row's
expectancy either way (−6.09 / +2.64 points a night on symbol_info).

The weekday filter gates entries only, so — as data-integrity found on
silver — a sign flip is acted on at the Sunday 17:00 reopen bar as an exit,
with re-entry Monday 16:15. Stated, not changed. Warmup `(60 + 5) × 288`
bars ≈ 274 days of each window on this feed.

## Falsifier

On the primary, the row replayed with the registered parameters (no grid,
`fixed = true`): gate (PF ≥ 1.2, expectancy ≥ 0.05R, ≥ 30 trades), ≥ 95th
percentile of 300 sized random holds of the same count and the method's own
mean hold, ≥ 95th of 1,000 permutations of the row's own sides. Then the
confirmation window, the same three conditions. One row; a miss on either
window closes it, and with gold and silver that is three assets on this
implementation.

## Base method

`tsmom`, existing and unchanged. `lookbackDays = 60`, `rebalanceHHMM = 1615`,
`riskDailyRanges = 2`, `rangeDays = 20`, filters `weekdays`, `hours:1615-1700`.

## Data

- Primary: `eurduka:15m` bounded `2010-06-01 → 2018-06-15` (Dukascopy bid
  m1 resampled; 398,220 fifteen-minute bars over 2010–2026 in the file;
  fetched and converted 2026-09-13 night). Never read by any run.
- Confirmation: `eurduka:15m` bounded `2018-06-16 → 2026-05-31`. Never read;
  opened only after the primary's record is written.
- Costs: Vantage EURUSD.sc, spread 0.00014 (measured read-only from 119k
  ticks over three days, p50 = p90; not measured inside the 16:15–17:00 gate
  specifically — the adversary asked for that and the probe buckets by
  minute of the server day, so the 23:15–00:00 server bucket will be read
  and written into the record before the run), contract unit one euro,
  P&L in dollars.

## Sample needed

Sixty-day sign flips on EURUSD: roughly 40–80 trades per window. The gate's
30 is not the issue; the null's width at 60 trades is (silver's 60-day
direction null spanned 0.42–2.22 on 95 trades), so a pass looks like PF 1.6
or better.

## What each outcome means

- Passes both windows → the metals were one regime and the family reopens
  on the third asset: a decision record and a paper-run proposal on EURUSD,
  which waits on the risk role's guards (an unrealised-loss cap in R on the
  open position evaluated every bar, a notional cap, a weekend rule, guards
  applied by the runner) before it is written.
- Passes one window and not the other → closed; three assets have shown
  the same shape.
- Fails the primary → closed; the family on this implementation is closed
  on three assets and the backlog says what a different implementation
  would be (daily bars, futures, a diversified set, the 12-month sign).

## Measured before the run

The gate spread (read-only, `copy_ticks_range`) and the coverage of the rebalance bar:

```
EURUSD.sc spread inside 16:15-17:00 NY (server 23:15-00:00), 2180 ticks over 3 days: p50 0.00014 p90 0.00020 max 0.00035
primary: 200576 bars, 2516 days; weekdays with a 16:15 NY bar 2088 of 2098; bars in the 17:xx NY hour 8365
confirmation: 197548 bars, 2485 days; weekdays with a 16:15 NY bar 2056 of 2071; bars in the 17:xx NY hour 8187
```

## In-sample (2010-06-01 → 2018-06-14, `eurduka` 15m)

Fixed replay (`in-sample-fixed.txt`, 300 sized random holds with the
method's own mean hold) and the direction null (`direction-tsmom-60d.txt`):

```
hypothesis     trades  OOS PF  expect  null p50 null p95   pct  direction   verdict
tsmom/60d         108   1.809   0.284    0.998    1.445  100%   85th       fails the falsifier: direction 85th < 95th
```

Walk-forward (`in-sample.txt`): 88 trades PF 1.903, 0.349R, 99th of the
sized null.

The runner prints SURVIVES because it reads the gate and the sized null;
the registration's falsifier has a third condition, the row's own sides
permuted at the 95th, and the row sits at the 85th (p = 0.152). That is the
condition gold's 60-day row failed (86th) and the one this registration was
written to test. Closed on the primary; the confirmation is not opened.
