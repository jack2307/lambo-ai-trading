# Gold intraday, batch 1: sessions and regimes do not rescue the technical baselines

**Date:** 2026-09-12
**Question:** Does gating the four technical baselines by session (New York
morning, London open, Asia), by volatility regime (expansion, compression), or
merely by staying flat over the CME break, produce anything that is both past
the gate and outside its own matched null on Vantage `XAUUSD.sc` 15m?
**Outcome:** No. Thirteen declared hypotheses, zero survivors. The best
percentile (rsi-reversion intraday, 92nd) has a negative expectancy. Recorded;
the list is closed.

## What was measured

- Data: `XAUUSD-15m.parquet`, 100,249 bars, 2022-06-16 → 2026-09-11, UTC,
  from the live Vantage terminal (`2026-09-12-vantage-bars-baselines.md`).
- Costs: spread $0.28, contract 1 oz. Swap was charged in this run at
  −82.76 / +31.98 per lot-night — **a misreading**: `symbol_info` reports that
  rate in points (`swap_mode=1`), i.e. −$0.83 per oz per night, and this
  account's own history (87 overnight positions, swap 0.00) shows it pays none.
  The swap column below is therefore ~100× too large and applies to nights this
  account is not charged for; every verdict stands regardless, since the
  intraday rows carry almost none of it and every row fails the gate on the
  price alone. Re-run with the corrected config in the addendum.
- Machinery: `fd_strategy::filter::Filtered` wraps an unchanged base method
  with gates on the New York clock (`fd_core::clock`); `search --mode=hypotheses`
  runs each through the 4-fold walk-forward and against **200 runs of the
  random-entry control wrapped in the same filters**. Gate: PF ≥ 1.2,
  expectancy ≥ 0.05R, ≥ 30 trades. Survival = gate *and* ≥ 95th percentile
  of the matched null.
- Every hypothesis: weekdays only, flat 16:30–18:15 New York.

## Evidence

```
hypothesis   base               trades  OOS PF  expect null p50 null p95    swap$   pct
intraday     ema-cross            1026   0.695  -0.132    0.836    1.015    -6570    8%
intraday     rsi-reversion        1269   0.966  -0.057    0.836    1.015     5239   92%
intraday     donchian-breakout    3493   0.871  -0.030    0.836    1.015    -3462   64%
intraday     bb-fade              3352   0.853  -0.069    0.836    1.015    -7457   56%
ny-morning   ema-cross             281   0.859  -0.076    0.790    1.269        0   62%
ny-morning   donchian-breakout    1194   0.838  -0.023    0.790    1.269    -4319   58%
london-open  donchian-breakout     693   0.872  -0.051    0.883    1.160        0   48%
asia         rsi-reversion         386   0.937  -0.031    0.914    1.136        0   59%
asia         bb-fade               720   0.920  -0.045    0.914    1.136        0   54%
expansion    donchian-breakout     925   0.856  -0.068    0.848    1.289        0   52%
expansion    ema-cross             218   0.895  -0.055    0.848    1.289        0   59%
compression  rsi-reversion         318   0.661  -0.148    0.772    1.070       0   28%
compression  bb-fade              1278   0.747  -0.102    0.772    1.070    -6759   44%
```

All thirteen fail the gate outright; none is above the 92nd percentile of its
null. The nulls themselves say something: a session gate cuts the trade count
to a few hundred, and the 95th percentile of *noise* traded in those hours is
1.27–1.29 — above the gate. A profit factor of 1.25 in the New York morning
would have passed the gate and meant nothing. The matched null is not optional.

Swap on the "intraday" rows is not zero. The flat window works by seeing a bar
inside 16:30–18:15; on early-close days (Christmas Eve, Thanksgiving Friday…)
there is none, and the position rides to Sunday. Roughly eight nights in four
years, at ~10 lots — the −$6,570 on ema-cross is those. A holiday calendar
would close it; it does not change any verdict here.

## What each role said

- **data-integrity:** NO OBJECTION. Bars are UTC-verified; the New York clock
  has its own tests against the 2026 DST dates; the swap count is tested
  Monday→Friday = 6.
- **adversary:** SURVIVED (the null did). Notes that thirteen hypotheses read
  against thirteen nulls carries its own multiplicity: at 95% one false survivor
  in twenty is expected, so a single survivor at 95–97% would have needed a
  direction null before belief. None reached even that.
- **risk:** NO OBJECTION. Withdraws the swap remark made on the first pass:
  the rate was read in the wrong unit and the account turns out to be
  swap-free (measured). The billing machinery stays for accounts that pay.
- **researcher:** the filters were applied to signals that were already inside
  the noise; a gate cannot create a signal, only select from one. Batch 2 should
  test signals that are intraday by construction — opening-range breakout,
  London-range breakout, previous-day high/low, VWAP fade — not baselines with
  a clock in front of them.
- **execution-realist:** a NY-morning method at 281 trades in four years is one
  trade a week; nothing about its costs was misrepresented.
- **portfolio:** nothing to size.
- **historian:** first batch under the hypothesis discipline. Thirteen
  reasons written down, thirteen answers, no re-tuning. Keep doing it this way.

## What would reopen this

A different base signal (see researcher). Not a different window on these four:
every session and both regimes have been read.

## What this does not say

- It does not say gold has no intraday structure. It says these four signals
  do not find it in these windows.
- It does not say the options thesis failed; no options strategy ran.
- It does not say swap never matters: this account is swap-free today; the
  generic Vantage rate is −$0.83/oz/night and a different account type pays it.

## Addendum — corrected run, swap at zero (same day)

```
hypothesis   base               trades  OOS PF  expect null p50 null p95   pct
intraday     ema-cross            1026   0.768  -0.132    0.911    1.020    1%
intraday     rsi-reversion        1269   0.890  -0.057    0.911    1.020   38%
intraday     donchian-breakout    3493   0.902  -0.030    0.911    1.020   42%
intraday     bb-fade              3352   0.889  -0.069    0.911    1.020   36%
ny-morning   ema-cross             281   0.859  -0.076    0.923    1.167   27%
ny-morning   donchian-breakout    1194   0.930  -0.023    0.923    1.167   52%
london-open  donchian-breakout     693   0.872  -0.051    0.883    1.160   48%
asia         rsi-reversion         386   0.937  -0.031    0.914    1.136   59%
asia         bb-fade               720   0.920  -0.045    0.914    1.136   54%
expansion    donchian-breakout     925   0.856  -0.068    0.898    1.207   41%
expansion    ema-cross             218   0.895  -0.055    0.898    1.207   49%
compression  rsi-reversion         318   0.661  -0.148    0.870    1.122    3%
compression  bb-fade              1278   0.815  -0.102    0.870    1.122   32%
```

Zero survivors, best percentile 59th. The nulls moved up (p50 0.91 instead of
0.84 on the intraday rows) because noise was no longer paying a phantom
financing bill; the methods moved with them. The early-close remark above is
moot on this account. Verdict unchanged.
