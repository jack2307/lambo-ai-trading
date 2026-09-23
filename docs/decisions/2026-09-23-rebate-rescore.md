# The rebate moves nothing across the line, and the null had to be paid too

**Written:** 2026-09-23
**Status:** the registered run is complete and the registration is answered.
`docs/hypotheses/2026-09-23-rebate-rescore.md` claimed that at least one
closed construct crosses both its profit-factor gate and its matched null once
the introducing-broker rebate is credited. **It does not. Nothing crossed.**
**Scope:** `crates/fd-backtest` (a new `rebate` module, a `direction` module
lifted out of the search binary, a `rescore` runner and a `--mode=rescore`),
`crates/fd-api/src/paper.rs` (one method made public so the two sides can be
held against each other), and two receipts under
`docs/research/runs/2026-09-23-rebate-rescore/`. No verdict changed, so
`ui/src/lib/verdicts.ts` is untouched.

## The question

The owner asked for a scalping strategy built to maximise the Backcom rebate:
*"lãi thấp nhưng tối ưu được Backcom"*. The arithmetic said that idea loses
money and it was not built. What remained was the only honest version of the
question: the rebate **halves the bar** a construct has to clear — 10.5% of R
becomes 5.8% at a two-point stop, 1.8% becomes 1.0% at twelve — so does the
lower bar admit anything the record already rejected?

## What was run

Twenty-nine rows of `ui/src/lib/verdicts.ts` quote a profit factor. Eight of
them are indicators quoting their own strategy's number verbatim — `ema` is
`ema-cross`, `rsi` is `rsi-reversion`, `macd`, `bbands`, `vwap`, `stoch`,
`donchian` and `keltner` likewise — so they are the same run and were not run
twice. The ninth indicator, `atr`, is not a base method at all: ATR is this
desk's risk unit, and its recorded 0.950/1.004 came from a volatility
CONDITION on the opening-range break
(`docs/hypotheses/2026-09-13-volcond-breakout.md`), so it is carried as those
two gated rows. **Twenty-two runnable rows cover all twenty-nine constructs,
and every one of the twenty-two ran.**

- Primary: `xauusd:15m`, `2025-09-13 → 2026-09-12`, 23,531 bars, walk-forward
  4 folds, selection on expectancy, gate `min_trades 30 / min_profit_factor
  1.2 / min_expectancy_r 0.05`, matched null 200 runs, direction null 1,000
  draws.
- Context, reported and never a gate: `xauduka:15m`,
  `2022-06-16 → 2025-04-10`, 66,243 bars, same everything else.
- Spread **0.28**, the configured value. The logger's measured median is 0.21
  and using it here would have flattered every row against a record measured
  at 0.28. It was not used and `--spread=` was not passed.
- Rebate **0.45 of the round-turn spread**, read from `config/accounts.toml`
  by the runner rather than typed into it.
- Seventeen of the twenty strategies were closed as the all-day rows of
  `docs/hypotheses/2026-09-13-recent-year-screen.toml`, so their two filters —
  weekdays, flat across 16:30–18:15 New York — are repeated unchanged. Their
  gross trade counts reproduce the receipt exactly (keltner-break 533,
  macd-cross 923, stoch-reversal 1,297, donchian-breakout 519, squeeze-break
  160, rsi2-pullback 542), which is the check that the batch is the same
  batch. Their gross profit factors differ from the 2026-09-13 receipt in the
  third decimal (keltner-break 1.048 against the recorded 1.052, macd-cross
  1.014 against 1.010, stoch-reversal 0.908 against 0.912): the engine has
  changed since — the exit-bar excursion correction and the price-decimals
  rounding both landed after that screen — and this is not a rebate effect.
  It is noted rather than chased, because no conclusion here turns on the
  third decimal.

Receipts: `docs/research/runs/2026-09-23-rebate-rescore/primary-xauusd-15m.txt`
and `…/context-xauduka-15m.txt`.

## The clause the registration turned on: both nulls carry the rebate

They do. **The matched null carries it and the direction null carries it**,
computed by the same arithmetic on the controls' own trades.

- The **matched null** is a full walk-forward of the random-entry control
  through the identical pipeline, once per seed. Its own out-of-sample trades
  are credited and a second profit factor is taken from them, so each of the
  200 runs contributes one gross figure and one net figure and the two curves
  are the same runs in the same order.
- The **direction null** is the control `--mode=null-dir` has always used: the
  method re-run with its side flipped and its stop and target mirrored, or —
  for a method that manages its own exits — the method's own trades with each
  side re-drawn by a coin. The credit does not depend on the side (it is a
  share of the spread the trade paid, and a flipped trade pays the same spread
  over the same lots), so the control is paid exactly what the method is paid.

That is visible in the receipts: the matched null's own p95 moves from 1.198
gross to 1.217 net on the primary year, and from 0.866 to 0.944 on
intraday-momentum's hold null in the context window. A credit withheld from
the control would have left those at the gross figure and lifted every
method's percentile against a bar that had not moved.

## The result

**0 of 22 rows passed all three legs. The registration expected about 1.1 of
22 by luck at the 95th percentile — one or two of the twenty-nine constructs,
as it said. Zero is below even that.** There is nothing here to call a
candidate and therefore nothing whose long-window number decides anything; the
context window is reported in full below regardless, as the registration
requires.

Primary year, `xauusd:15m`, 2025-09-13 → 2026-09-12. `reb/R` is the credit as
a fraction of the risk taken, averaged per trade — the unit the registration's
break-even arithmetic is in. `matched` and `dir` are percentiles of the nulls
**with the rebate in them**; the gross percentile is given where it differs.

| construct | trades | PF gross | PF net | rebate $ | reb/R | matched (gross → net) | direction net | outcome |
|---|---:|---:|---:|---:|---:|---|---:|---|
| ema-cross | 222 | 0.639 | 0.651 | 1.65 | 0.86% | 0 → 0 | 24 | fails gate and both nulls |
| rsi-reversion | 417 | 0.864 | 0.882 | 3.19 | 0.86% | 20 → 20 | 18 | fails gate and both nulls |
| donchian-breakout | 519 | 0.962 | 0.974 | 1.97 | 0.45% | 50 → 47 | 24 | fails gate and both nulls |
| bb-fade | 994 | 0.897 | 0.915 | 8.35 | 0.94% | 30 → 29 | 67 | fails gate and both nulls |
| keltner-break | 533 | 1.048 | 1.063 | 3.25 | 0.65% | 76 → 75 | 85 | fails gate and both nulls |
| macd-cross | 923 | 1.014 | 1.032 | 8.03 | 0.93% | 68 → 68 | 58 | fails gate and both nulls |
| rsi2-pullback | 542 | 0.873 | 0.890 | 3.45 | 0.72% | 21 → 22 | 18 | fails gate and both nulls |
| squeeze-break | 160 | 0.841 | 0.859 | 1.76 | 1.17% | 14 → 14 | 38 | fails gate and both nulls |
| stoch-reversal | 1297 | 0.908 | 0.927 | 10.50 | 0.92% | 34 → 34 | 36 | fails gate and both nulls |
| orb | 149 | 0.856 | 0.867 | 0.62 | 0.49% | 18 → 16 | 41 | fails gate and both nulls |
| pdhl | 177 | 0.824 | 0.849 | 3.06 | 1.80% | 12 → 14 | 58 | fails gate and both nulls |
| ict-sweep-mss-fvg | 25 | 0.757 | 0.770 | 0.14 | 0.62% | 4 → 4 | 47 | under the 30-trade floor; fails on trades and on everything else |
| vwap-fade | 860 | 0.932 | 0.952 | 9.35 | 1.07% | 40 → 40 | 80 | fails gate and both nulls |
| doji-reversal | 102 | 0.570 | 0.593 | 2.48 | 2.57% | 0 → 0 | 31 | fails gate and both nulls |
| gap-fade | 0 | — | — | 0.00 | — | — | — | **took no trade**: see below |
| trend-pullback | 687 | 0.629 | 0.699 | 37.19 | 7.00% | 0 → 0 | 78 | fails gate and both nulls |
| volume-thrust | 10 | 0.737 | 0.751 | 0.05 | 0.57% | 2 → 2 | 58 | under the 30-trade floor |
| volman-box | 2 | 1.621 | 1.650 | 0.02 | 0.99% | 100 → 100 | — | **two trades**: the percentile is an artefact, not a result |
| tsmom | 0 | — | — | 0.00 | — | — | — | **took no trade**: see below |
| intraday-momentum | 197 | 1.088 | 1.121 | 0.26 | 0.13% | 98 → 99 | 66 | clears the matched null, fails the gate (1.121 < 1.2) and the direction null |
| atr/hivol-london | 179 | 1.001 | 1.011 | 0.45 | 0.31% | 56 → 56 | 78 | fails gate and both nulls |
| atr/hivol-orb60 | 181 | 0.809 | 0.819 | 0.68 | 0.45% | 28 → 28 | 31 | fails gate and both nulls |

The nearest thing to a candidate is **intraday-momentum**, which clears its
matched null at the 99th and then fails the gate by 0.079 of profit factor and
the direction null at the 66th. It is not a pass and is not promoted. It is
also the row where the credit did the most work in percentile terms (98th →
99th gross to net), and it is worth saying why: its trades are nearly
flat in R — expectancy 0.005R — so a credit worth 0.13% of R is a large
fraction of what the book earned, while the same credit is a small fraction of
what its hold null earned. That is a real effect and it is the reason the
control had to be paid too.

Context window, `xauduka:15m`, 2022-06-16 → 2025-04-10, never a gate:

| construct | trades | PF gross | PF net | rebate $ | reb/R | matched net | direction net |
|---|---:|---:|---:|---:|---:|---:|---:|
| ema-cross | 642 | 0.797 | 0.851 | 1983.33 | 3.48% | 16 | 15 |
| rsi-reversion | 837 | 0.882 | 0.942 | 2514.69 | 3.18% | 60 | 95 |
| donchian-breakout | 2876 | 0.808 | 0.861 | 4348.77 | 1.89% | 20 | 68 |
| bb-fade | 2297 | 0.908 | 0.977 | 7588.45 | 3.83% | 76 | 71 |
| keltner-break | 785 | 0.847 | 0.885 | 1328.54 | 1.80% | 28 | 29 |
| macd-cross | 2827 | 0.832 | 0.890 | 6800.52 | 3.57% | 30 | 85 |
| rsi2-pullback | 1177 | 0.862 | 0.929 | 3043.88 | 2.88% | 54 | 82 |
| squeeze-break | 522 | 0.852 | 0.922 | 2220.16 | 4.61% | 52 | 65 |
| stoch-reversal | 3588 | 0.852 | 0.912 | 8754.97 | 3.57% | 48 | 70 |
| orb | 346 | 0.942 | 0.986 | 524.42 | 1.52% | 78 | 87 |
| pdhl | 531 | 0.728 | 0.826 | 2743.63 | 5.78% | 8 | 19 |
| ict-sweep-mss-fvg | 121 | 0.636 | 0.670 | 240.98 | 2.04% | 0 | 20 |
| vwap-fade | 1907 | 0.851 | 0.907 | 4868.00 | 3.05% | 42 | 83 |
| doji-reversal | 299 | 0.832 | 0.934 | 2074.92 | 7.40% | 57 | 88 |
| gap-fade | 0 | — | — | 0.00 | — | — | — |
| trend-pullback | 2445 | 0.588 | 0.720 | 13754.88 | 12.08% | 0 | 35 |
| volume-thrust | 0 | — | — | 0.00 | — | — | — |
| volman-box | 62 | 0.573 | 0.625 | 256.86 | 4.24% | 0 | — |
| tsmom | 0 | — | — | 0.00 | — | — | — |
| intraday-momentum | 559 | 0.840 | 0.987 | 264.59 | 0.48% | 100 | 87 |
| atr/hivol-london | 476 | 0.925 | 0.952 | 454.25 | 0.98% | 56 | 46 |
| atr/hivol-orb60 | 500 | 1.025 | 1.069 | 642.52 | 1.27% | 79 | 93 |

**0 of 22 on the context window too.** The credit is larger there — the
Dukascopy market is configured at a hundred ounces a lot and 2022–25 gold has
roughly half the ATR of 2025–26 gold, so the same 0.45 of 0.28 is 2 to 12% of
R rather than 0.1 to 7% — and it still moves nothing across. `rsi-reversion`
reaches the 95th on the direction null and fails the gate at 0.942 and its
matched null at the 60th; `atr/hivol-orb60` reaches 1.069 and the 93rd. Two
near misses on two legs each, and neither is a pass.

## What the numbers say about the original idea

The registration's arithmetic survives contact with the data. The credit is
**0.13% to 7% of R on the primary year and 0.5% to 12% on the context
window**, and it is largest exactly where the construct is worst:
`trend-pullback` collects 7.00% of R per trade and has a profit factor of
0.629, because both the cost and the credit are fixed in points and dividing
by a tight stop makes both larger. A 45% refund on a cost you pay in full
cannot turn a losing method into a winning one; it makes a losing method lose
slightly less. That is what every row here shows, and it closes "optimise for
Backcom" as a strategy question with a number rather than an opinion.

## What could not be done, and one thing that should not be believed

**1. Five constructs cannot be asked this question on the primary window.**
`gap-fade` and `tsmom` took **no trade at all** under the screen's filters —
gap-fade trades the Sunday reopen, which is inside the 16:30–18:15 flat window
and outside the weekday gate, and tsmom holds for weeks off 15-minute bars.
`volman-box` (2 trades), `volume-thrust` (10) and `ict-sweep-mss-fvg` (25) are
under the registry's 30-trade floor. These are methods built for 1-minute or
5-minute bars, or for a market session this window gates away; a year of
15-minute Vantage bars is the wrong instrument for them and the registration's
single primary window is why they were asked anyway. An unregistered probe
without the flat window (weekdays only, and no filter at all) was run
afterwards purely to see whether the zero was the filter's doing: gap-fade
reaches 24 trades unfiltered and tsmom 12, both still under the floor. **Those
probe figures are a second look, they are not in the table above, and they do
not enter the pass count.**

**2. THE MATCHED NULL IS NOT MATCHED TO THE TRADE COUNT IN THE WALK-FORWARD
PATH, AND THIS IS NOT NEW.** Nineteen of the twenty-two rows — trade counts
from 2 to 1,297 — were measured against *exactly the same* null distribution,
p50 0.962 and p95 1.198 gross. That is not a coincidence and not a bug in
anything written today. `hypotheses::matched_rate` calibrates the random-entry
control's `entryRate` to the method's count and its own comment says the rate
is "pinned, so the control's grid loses its rate axis too" — but the
walk-forward wraps the control in `Preset::bare`, which pins nothing, and
`RandomEntry::grid` sweeps `entryRate` over {0.01, 0.02, 0.04}. Every cell the
walk-forward selects from therefore carries a grid rate and the calibrated one
never appears. The count-matching survives only in
`run_hypothesis_fixed_guarded`, which does not sweep.

This is exactly the defect the adversary caught once before — "a 61-trade row
measured against a 300-trade null" — and it affects **every walk-forward
`--mode=hypotheses` receipt in `docs/decisions/`**, not just this one. It was
left alone here deliberately: repairing it changes the percentile of every
published row, which is a restatement that needs its own registration and must
not ride in on a rebate run. **It is the reason `volman-box`'s "100th
percentile" on two trades is in the table with a warning beside it rather than
as a number anyone should read.**

The direction null is unaffected — it is matched by construction, being the
method's own entries with the side flipped.

**3. The conclusion does not depend on either problem.** Both push in the
direction of flattering thin rows against an over-tight null, and nothing
passed anyway. A repair can only move rows further inside their nulls.

## What shipped

- `crates/fd-backtest/src/rebate.rs` — `Rebate`, read from
  `config/accounts.toml`, with the live side's arithmetic to the rounding:
  `share × spread × lots × contract_size`, one whole round turn per trade,
  four decimals. It never mutates a trade and never touches `spread`.
  `credited()` returns a new book; `usd_per_r` recovers the risk unit from the
  stop, or from the P&L identity for a stopless self-managed trade.
- `crates/fd-backtest/src/direction.rs` — `DirectionFlipped` and the
  permuted-sides control, lifted out of `bin/search.rs` unchanged so the
  rescore reads the same control the receipts were written against rather than
  a second copy. `bin/search.rs` now calls it.
- `crates/fd-backtest/src/hypotheses.rs` — `rescore_hypothesis` and
  `RescoreRow`: one construct, two columns everywhere, both nulls credited.
- `crates/fd-backtest/src/bin/search.rs` — `--mode=rescore`, with
  `--rebate-share=` for a sensitivity run that says so in its header.

Three gates hold the arrangement in place:

- `crates/fd-backtest/tests/rebate_gross.rs` — drives a real backtest through
  the real engine and asserts the gross book is **bit-identical** either side
  of the credit: the trades, the metrics, and the configured spread. Every
  receipt in `docs/decisions/` was measured with no rebate; if crediting one
  could move a gross figure they would all be silently restated.
- `crates/fd-api/tests/rebate_parity.rs` — holds `fd_backtest::Rebate::on`
  against the live desk's `fd_api::paper::RebateTerms::on` on a table of
  cases, including every case both must refuse, and asserts both read the same
  rate out of the same file. `RebateTerms::on` was made public for this and is
  otherwise unchanged.
- The unit tests in `rebate.rs` pin the worked example
  `config/accounts.toml` states in full (0.07 lots at 0.28 credits 0.88 USC on
  a 1.96 USC round turn) and the registration's own claim that the credit as a
  fraction of R grows as the stop tightens.

One rounding decision is worth naming: the credited book does **not** re-round
`pnl_usd` to the engine's two decimals. The engine rounds there because two
decimals is a cent of the account currency, and re-rounding after the credit
would discard any credit smaller than a cent — which on this workspace's
per-ounce gold is most of them. The credit itself is rounded once, to four
decimals, exactly where the live side rounds it.

## What this does not say

Whether a purpose-built high-frequency strategy could earn more rebate than it
pays. The registration's arithmetic says it cannot at any share short of a
full refund, because the net cost of a round turn is `(1 − share) × spread`
and that is positive below 100%. Nothing measured here contradicts it and
nothing measured here tests it. If the IB terms ever become **per lot** rather
than a share of the spread, the arithmetic is different in kind and the
question is worth asking again.

## What would reopen this

- The per-lot rebate form. `config/accounts.toml` carries it commented out
  with the reason; it is not the same number wearing two hats.
- A repair to the walk-forward matched null, under its own registration. It
  would restate percentiles across `docs/decisions/` and this record's
  matched-null column with them. The gate legs and the direction null would be
  unaffected.
- Nothing else. A construct that failed three legs by this much is not a
  re-run away from passing, and re-running it until it does is how a backtest
  stops measuring the market and starts measuring the search.

## Amendment, 2026-09-23 (later the same day): the matched null was repaired and this record's matched-null column is restated

The defect this record named in full under *"2. THE MATCHED NULL IS NOT
MATCHED TO THE TRADE COUNT IN THE WALK-FORWARD PATH"* was registered, fixed
and re-run the same day. **The original numbers above are left exactly as they
were published**; the corrected ones are beside them in
`docs/decisions/2026-09-23-matched-null-repair.md` and in the receipts
`docs/research/runs/2026-09-23-matched-null-repair/primary-xauusd-15m-repaired.txt`
and `…-before-instrumented.txt`.

What changed and what did not, on this record's own primary table:

- **Nothing in the gate columns.** Trades, PF gross, PF net, rebate $ and
  reb/R are identical on all 22 rows, measured by running the same binary
  twice on the same day with the pin off and on. **The direction column is
  identical on all 22 rows too.** Only the matched-null column moved.
- **The matched-null percentile moved on 14 of 22 rows.** The largest moves
  are the thin ones the defect was hurting: `ict-sweep-mss-fvg` 4 → 28,
  `volume-thrust` 2 → 42, `orb` 18 → 32, `pdhl` 12 → 22, `squeeze-break`
  14 → 24. `macd-cross` 68 → 78 and `keltner-break` 76 → 83; `stoch-reversal`
  34 → 30, `bb-fade` 30 → 24, `atr/hivol-orb60` 28 → 14.
- **The conclusion is unchanged. 0 of 22 still pass all three legs**, gross or
  net, and the sentence *"nothing crossed"* stands.
- The claim above that *"a repair can only move rows further inside their
  nulls"* is **wrong** and is corrected here rather than edited out. A repair
  moves a row toward wherever a correctly sized null puts it, which for a row
  below its null's median is outward and for one above it is inward. Most rows
  here are below, which is why most moved up.

Two things this record got right and one it could not see:

- The warning beside `volman-box`'s "100th percentile on two trades" was
  right, and the repair does not rescue it: its count match is 150.50.
- The 19-of-22 figure is confirmed by measurement. The pre-repair control's
  median was **280 trades** for methods of 2 to 1,297.
- `intraday-momentum`, called here "the nearest thing to a candidate" at the
  98th–99th, is **unaffected by this repair and unmeasured for a different
  reason**: it is a self-managed-exit method, so its matched null is
  `RandomHold`, which is not count-matched at all — 197 out-of-sample trades
  against a control taking 2,914. That is a second defect, recorded in the
  repair document and not fixed there.

The context window (`xauduka` 2022-06-16 → 2025-04-10, the second table) was
**not** re-run: it is context and never a gate. Its matched-null column
carries the same defect and stands uncorrected.
