# The matched null was never matched in the walk-forward, and repairing it moved 69 of 85 percentiles without touching a single gate figure

**Date:** 2026-09-23
**Registration:** `docs/hypotheses/2026-09-23-matched-null-repair.md`, committed
at `8f12e7f` before the fix was written and before any corrected number was
seen. Its five pre-commitments are answered one by one below.
**Status:** the fix is in, the priority receipts are re-run and published
beside their originals, and the rest of the scope is named with what was and
was not done.
**Scope of the change:** `crates/fd-backtest/src/hypotheses.rs` (one new
`Preset` constructor, `matched_control_for` returns what it calibrated, three
call sites, two new report fields and their accessors),
`crates/fd-backtest/src/bin/search.rs` (both receipt formats now print the
achieved count match on every row), and
`crates/fd-backtest/tests/matched_null.rs` (new). **No threshold moved.** The
gate stays `min_trades 30 / min_profit_factor 1.2 / min_expectancy_r 0.05` and
both nulls stay at the 95th.

## The defect

`hypotheses::matched_rate` calibrates the random-entry control's `entryRate`
so the control takes about as many trades as the method it is a control for,
because a control with five times the method's trades has a much tighter
profit-factor distribution and a percentile read against it is not the
method's percentile. Its own comment said the rate was *"pinned, so the
control's grid loses its rate axis too."*

It was not pinned. The two walk-forward paths wrapped the calibrated control
in `Preset::bare`, which sets `pinned: Vec::new()`, and `walk_forward_guarded`
then swept `RandomEntry::grid`, which carries `entryRate` over
`{0.01, 0.02, 0.04}` (`crates/fd-backtest/src/control.rs:73`). Every cell the
selection could choose carried a grid rate, so the calibrated rate never
appeared in a scored run. Count-matching survived only in
`run_hypothesis_fixed_guarded`, which runs the control at explicit parameters
and never consults a grid.

The fingerprint is unmistakable once it is looked for, and it is the reason
the defect could be found from the published receipts alone. **With the rate
swept away, the control depends only on the row's filters** — so rows sharing
a filter set share a null distribution exactly, whatever their size:

| receipt | rows | distinct nulls | trade counts served by the largest group |
|---|---:|---:|---|
| `2026-09-13-recent-year-screen/in-sample.txt` | 32 | 2 | 10 … 1,297 |
| `2026-09-13-recent-year-sessions/in-sample.txt` | 31 | 2 | 3 … 512 |
| `2026-09-13-recent-year-screen-5m/in-sample.txt` | 31 | 2 | 64 … 3,422 |
| `2026-09-13-recent-year-hours/in-sample.txt` | 24 | 12 | one per two-hour window, which is one per filter |
| `2026-09-23-rebate-rescore/primary-xauusd-15m.txt` | 22 | 2 | 2 … 1,297 |

Two filter sets, two nulls; twelve hour windows, twelve nulls. That is a
control keyed on the filters and blind to the method, which is exactly what
the calibration existed to prevent — the adversary's original case was "a
61-trade row measured against a 300-trade null".

Measured directly, with the same instrumentation on both sides, the
pre-repair rebate rescore ran **19 of 22 rows against a control whose median
was 280 trades**, for methods of 2 to 1,297 trades.

## The fix

`Preset::calibrated(inner, defaults, pinned)`, used in place of
`Preset::bare` wherever the control is built, and `matched_control_for` now
returns the list of parameters it calibrated so the pin is derived from the
calibration rather than typed twice. `Preset` already supported pinning; the
control was simply not using it.

Three sites build the control. Each was checked rather than assumed:

| site | path | affected | what changed |
|---|---|---|---|
| `run_hypothesis_fixed_guarded` (`hypotheses.rs:598`) | `run_backtest_guarded` at explicit params | **no** — no grid is consulted | uses `calibrated` for uniformity; the pin is a no-op there |
| `run_hypothesis_guarded` (`hypotheses.rs:699`) | `walk_forward_guarded` | **yes** | the calibrated rate now survives the sweep |
| `rescore_hypothesis` (`hypotheses.rs:903`) | `walk_forward_guarded` | **yes** | same |

What was deliberately **not** changed:

- **The rate's arithmetic.** `matched_rate` is untouched: one probe run at
  0.02, scaled to the method's count, clamped to `[0.0005, 1.0]`.
- **`stopAtr` stays in the control's grid.** Only the calibrated axis is
  pinned. The control is meant to get the same selection advantage a real
  method gets, which is the whole argument of `control.rs`.
- **The direction null.** It permutes sides on the method's own trades and is
  count-matched by construction, and this change does not reach its code
  path. On the one receipt format that prints both nulls side by side — the
  rescore — its percentile is identical on all 22 rows, pin off and pin on.
  The screen and sessions direction nulls live in their own
  `direction-*.txt` receipts from `--mode=null-dir`, which this change cannot
  touch, so they were not re-run.
- **Every threshold.**

## What holds it: three tests and a negative check

`crates/fd-backtest/tests/matched_null.rs`:

1. `a_calibrated_control_loses_the_calibrated_axis_and_keeps_the_others` —
   structural: a bare control still sweeps `entryRate` (the defect, asserted
   as such), a calibrated one does not, and `stopAtr` survives in both.
2. `the_matched_null_takes_about_as_many_trades_as_the_method` — a real
   four-fold walk-forward through `run_hypothesis_guarded` on the real engine,
   two base methods of deliberately different trade frequency, asserting that
   each control's median out-of-sample trade count is inside a quarter of its
   method's **and that the two controls come out different sizes**. One row
   would prove nothing: the defect's signature is one control serving every
   row.
3. `repairing_the_null_cannot_move_a_gate_figure` — the method's own
   walk-forward is run with no control anywhere near it and held against the
   one `run_hypothesis_guarded` reports, bit-identical by `to_bits` on profit
   factor, expectancy, total R, net P&L, drawdown, Sharpe, win rate and
   average R, plus the trade count and the exit breakdown.

The band lives in code as `hypotheses::COUNT_MATCH_BAND = 0.25`, documented as
a check on the measuring instrument and not a gate on a strategy.

The negative check, because a test that cannot fail is not a gate: reverting
the pin at the walk-forward site alone and re-running makes test 2 fail with
*"the control took a median of 54 trades against the method's 94 — ratio 0.58,
outside the registered band"*, while tests 1 and 3 still pass. Test 3 passing
in both states is the point of test 3.

## Pre-commitment 4 of 5: the achieved count match, measured and not assumed

The registration said a control whose trade count still diverges from the
method's by more than about a quarter means the work is wrong, and that the
re-run must **report** the achieved match. Both receipt formats now print it
on every row. Measured on the same day with the same binary, pin off and pin
on:

| batch | rows measurable | inside the band before | inside the band after | median ratio before → after |
|---|---:|---:|---:|---|
| rebate rescore, `xauusd` 15m primary year | 20 | **0** | **14** | 0.67 → 1.03 |
| recent-year screen, in sample | 32 | 2 | **23** | 0.67 → 0.94 |
| recent-year sessions, in sample | 31 | 6 | **18** | 0.73 → 0.91 |

**The match is achieved on most rows and not on all of them, and the rows
where it is not are named rather than averaged away.** Three causes, none of
them fixable without changing the rate's arithmetic that the registration
froze:

1. **The calibration targets the whole window; the percentile compares
   walk-forward books.** `matched_rate` scales the control's rate to the
   method's trade count *at the registered parameters over the whole window*,
   because the walk-forward's own count is a fifth of the bars per fold and
   would under-match. But the percentile is read between two out-of-sample
   walk-forward books, and a sweep that selects slower parameters than the
   registered ones for the method — or a control whose selected `stopAtr`
   widens and so holds longer and enters less — moves the two apart again.
   `stoch-reversal` is the clearest case: 1,297 out-of-sample trades against a
   control median of 814, ratio 0.63, even though the control was calibrated
   to the method's whole-window 1,492.
2. **A row with almost no trades cannot be matched at all.** `volman-box` took
   2 trades and `volume-thrust` 10; their percentiles are artefacts either
   way and were already flagged as such in `2026-09-23-rebate-rescore.md`.
3. **The hold null is not count-matched and this repair did not change
   that** — see the second defect below.

## Pre-commitment 5 of 5: no gate figure moved

The defect was in the control, so profit factor, trade count and expectancy
must be identical either side of the change. Two independent proofs, because
prose is not one:

- **The test** above proves it structurally: the gate figures come out of a
  walk-forward that runs before any control exists.
- **The receipts** prove it empirically. Running the same binary twice on the
  same day, once with the pin reverted and once with it in:

| batch | rows | gate columns that differ |
|---|---:|---:|
| rebate rescore, primary year | 22 | **0** |
| recent-year sessions, in sample | 31 | **0** |
| recent-year screen, in sample | 32 | **0** |

One thing a reader will notice and should not misread: the re-run gate figures
differ from the **2026-09-13** receipts in the third decimal and occasionally
more (`keltner-break/asia` 1.315 → 1.286, `ema-cross/asia` 1.632 → 1.622).
That is the pre-existing engine drift `2026-09-23-rebate-rescore.md` already
recorded — the exit-bar excursion correction and the price-decimals rounding
both landed after that screen — and it is why the same-day pre-fix baselines
above exist and are published. Against those, nothing moved at all.

## Pre-commitment 1 of 5: every re-run receipt, beside its original

Receipts are under `docs/research/runs/2026-09-23-matched-null-repair/`. The
`*-before-instrumented.txt` files are the same-day pre-fix baselines; the
`*-repaired.txt` and `<dir>__<name>.txt` files are the re-runs. Nothing was
overwritten and nothing was dropped — the originals under their own run
directories are untouched, and the full list of what was and was not re-run
is at the end of this record.

### The rebate rescore, `xauusd` 15m, 2025-09-13 → 2026-09-12

The cheapest complete comparison and the one the registration named first.
`pct` is the gross matched-null percentile; `cm` is the achieved count match
after the repair. The direction null did not move on any row and is omitted.

| construct | trades | PF | null trades before → after | matched pct before → after | cm after |
|---|---:|---:|---|---:|---:|
| ema-cross | 222 | 0.639 | 280 → 278 | 0 → 0 | 1.25 |
| rsi-reversion | 417 | 0.864 | 280 → 390 | 20 → 20 | 0.94 |
| donchian-breakout | 519 | 0.962 | 280 → 676 | 50 → **56** | 1.30 |
| bb-fade | 994 | 0.897 | 280 → 832 | 30 → **24** | 0.84 |
| keltner-break | 533 | 1.048 | 280 → 597 | 76 → **83** | 1.12 |
| macd-cross | 923 | 1.014 | 280 → 736 | 68 → **78** | 0.80 |
| rsi2-pullback | 542 | 0.873 | 280 → 560 | 21 → **16** | 1.03 |
| squeeze-break | 160 | 0.841 | 280 → 180 | 14 → **24** | 1.12 |
| stoch-reversal | 1297 | 0.908 | 280 → 814 | 34 → **30** | 0.63 |
| orb | 149 | 0.856 | 280 → 154 | 18 → **32** | 1.03 |
| pdhl | 177 | 0.824 | 280 → 175 | 12 → **22** | 0.99 |
| ict-sweep-mss-fvg | 25 | 0.757 | 280 → 31 | 4 → **28** | 1.24 |
| vwap-fade | 860 | 0.932 | 280 → 657 | 40 → **44** | 0.76 |
| doji-reversal | 102 | 0.570 | 280 → 172 | 0 → 0 | 1.69 |
| gap-fade | 0 | — | 280 → 301 | — | — |
| trend-pullback | 687 | 0.629 | 280 → 562 | 0 → 0 | 0.82 |
| volume-thrust | 10 | 0.737 | 280 → 5 | 2 → **42** | 0.50 |
| volman-box | 2 | 1.621 | 280 → 301 | 100 → 100 | 150.50 |
| tsmom | 0 | — | 280 → 301 | — | — |
| intraday-momentum | 197 | 1.088 | 2914 → 2914 | 98 → 98 | 14.79 |
| atr/hivol-london | 179 | 1.001 | 58 → 167 | 56 → **60** | 0.93 |
| atr/hivol-orb60 | 181 | 0.809 | 38 → 206 | 28 → **14** | 1.14 |

**The conclusion of `2026-09-23-rebate-rescore.md` is unchanged: 0 of 22 rows
pass all three legs, gross or net.** Nothing crossed before the repair and
nothing crosses after it. The record's own prediction — "a repair can only
move rows further inside their nulls" — is half right: it moved 14 of 22
matched-null percentiles, most of them up, and none of them across.

### The recent-year sessions batch, in sample

The batch that carries `xau-macd-asia`. 27 of 31 percentiles moved; **two
distinct null distributions became thirty-one**. Full table in the receipt;
the rows that matter:

| row | trades | PF | null trades before → after | matched pct before → after | cm after | direction null (unchanged) |
|---|---:|---:|---|---:|---:|---:|
| `macd-cross/asia` | 300 | 1.558 | 92 → 264 | 98 → **100** | 0.88 | 93rd |
| `keltner-break/asia` | 262 | 1.286 | 92 → 227 | 89 → **98** | 0.87 | 93rd |
| `ema-cross/asia` | 37 | 1.622 | 92 → 105 | 99 → 99 | 2.84 | 49th |
| `stoch-reversal/asia` | 420 | 0.918 | 92 → 299 | 36 → **26** | 0.71 | 31st |
| `bb-fade/london` | 431 | 1.053 | 104 → 394 | 72 → **90** | 0.91 | — |
| `ict-sweep-mss-fvg/asia` | 9 | 1.709 | 92 → 8 | 99 → **78** | 0.89 | — |
| `donchian-breakout/london` | 436 | 0.767 | 104 → 310 | 22 → **9** | 0.71 | — |
| `rsi2-pullback/london` | 143 | 1.047 | 104 → 221 | 72 → **80** | 1.55 | — |

### The recent-year screen batch, in sample

28 of 32 percentiles moved; two nulls became thirty-two. **No verdict
changed**: nothing new survives and nothing that survived stopped. The rows
that moved most are the thin ones — `volume-thrust/ny` 0 → 67 on four trades,
`ict-sweep-mss-fvg/all` 4 → 28 on twenty-five, `ict-sweep-mss-fvg/ny` 22 → 49
on ten — which is the defect's own shape: those are the rows that were being
measured against a 280-trade or 106-trade control.

`stoch-reversal/all`, the closest published matched-null figure to the funded
`xau-stoch`, moved **34 → 30** at a count match of 0.63. `keltner-break/all`
moved 78 → 83 and `macd-cross/all` 66 → 78; both remain closed on the gate.

## Pre-commitment 3 of 5: the two constructs on the funded account

Both are reported whichever way they moved, as the registration required, and
the owner is to be told unprompted.

**`xau-macd-asia` — `macd-cross`, weekdays, hours 18:00–02:00 New York,
news 60/30. Its matched null ROSE, 98th → 100th.** Trade count 300,
walk-forward PF 1.558, control median 264 trades, count match 0.88 — the
repaired null is a properly matched one and it says the row is at the top of
it. **This does not make it a pass.** Its direction null is unchanged at the
**93rd**, below the 95th the desk requires, so it still fails one of three
legs, and the registration's rule stands: a percentile that improved when its
control was repaired is a percentile measured once. Its long window is
unchanged too — PF 0.91 on 2022–25.

**`xau-stoch` — `stoch-reversal`, weekdays, news 60/30.** The badge this desk
publishes for it quotes the all-day screen row: **1,297 trades, PF 0.912,
34th → 30th**, count match 0.63. The Asian variant fell further, 36th → 26th.
Its own listed justification in `docs/paper/CANDIDATES.md` was never a matched
null at all — "the Workbench leaderboard's top on the last week (PF 1.48, 35
trades — in-sample only)" — so **the repair takes away a little of the only
out-of-sample number attached to it and adds nothing.** It was closed before,
it is closed by more now, and it is running on a funded account.

Neither construct crosses. One moved up and one moved down, and the one that
moved up moved up on the leg it was already passing.

## Pre-commitment 2 of 5: one status changed

**`keltner-break/asia` (`xau-keltner-asia`, candidate #3) now clears its
matched null where it did not before: 89th → 98th, at PF 1.286 on 262
trades, which is also past the 1.2 gate.** Before the repair it was "gate
pass, inside the noise"; the repaired receipt prints `SURVIVES`, which is the
search binary's two-leg word.

It is recorded here as a row that changed, and it is **not** promoted:

- The desk's falsifier is three legs, and its direction null is unchanged at
  the **93rd**. It clears two of three, not three.
- The registration is explicit that a construct whose percentile improved
  when its control was repaired is not funded on that evidence.
- Its long window is 0.88, and it is a member of a family closed on it.

No row that previously survived stopped surviving.

**`ui/src/lib/verdicts.ts` is not edited, and it is worth being exact about
why, because several badges do quote a percentile this repair moved.** The
`stoch` badge says "1297 trades, PF 0.912, 34th"; the repaired figure is the
30th. The `macd` badge says 66th and the repaired figure is the 78th;
`keltner` says 78th against a repaired 83rd; `squeeze-break` 14th against
24th; `rsi2-pullback` 74th against 83rd. **Not one of them changes a status**
— every one of those rows fails its profit-factor gate before the null is
consulted, and `killed` is still `killed`. The rule this desk works to is
that a badge is edited when a status changes, and none did.

`verdicts.check.mjs` passes unchanged, and it passes for the right reason
rather than by luck: every amendment above is an appended dated note, so the
original numbers the badges cite are still literally present in the records
they cite. If one of those original numbers had been edited in place instead
of annotated, the check would have failed — which is the coupling working,
and the reason this desk annotates a receipt rather than rewriting one.

## The pre-declared direction, and the two rows that fell

The registration pre-declared, before any number was seen, that **thin rows'
percentiles would RISE**, and that if they fell instead it was a second defect
to be investigated rather than published. Taking "thin" as the registration
meant it — the method's out-of-sample trade count below the size of the
control it was actually measured against — 31 rows across the three batches
qualify. **Twenty-one rose or were pinned at 0 or 99 with nowhere to go. Two
fell.** Both are investigated here rather than published quietly.

**`ict-sweep-mss-fvg/asia`, 9 trades, PF 1.709, 99th → 78th.** The
registration's reasoning was that an over-large control gives a *tighter*
distribution and so depresses a thin row's percentile. The first half is
right and the second half is a special case. A tighter null raises the
percentile of a row whose profit factor sits **above** the null's median and
lowers the percentile of one that sits below it. Nineteen of the twenty-one
rows that rose sit below their null's median — profit factors of 0.5 to 1.0
against a null centred near 1.0 — which is why the pre-declared direction held
for almost everything. This row does not: PF 1.709 against a null centred at
1.003. Before the repair its null had 92 trades and a p95 of 1.404, so 1.709
was off the top of it. The repaired null has 8 trades and a p95 of 3.023,
because on eight random entries a profit factor above 1.709 is ordinary. The
92-trade null was inflating it and the 8-trade null is telling the truth,
which is that a nine-trade row cannot be distinguished from noise in either
direction. **This is the same mechanism with the sign that applies above the
median, not a second defect.** What it does falsify is a sentence in the
registration, and that is recorded rather than smoothed over.

**`rsi-reversion/ny`, 84 trades, 38th → 36th.** Two percentile points on a
200-seed null, which is inside the sampling error of the estimate, and on a
row whose count match after the repair is **1.95** — the control moved from
106 trades to 164 while the method's out-of-sample book is 84, so this row's
null is still not matched and its percentile should be read as unmatched
either way. It is not a corrected number and is not offered as one.

## Scope: how it was established

42 files under `docs/decisions/` mention a percentile; 41 contain the literal
word. The question is which of them carry a **walk-forward `--mode=hypotheses`
matched-null** figure, because nothing else is affected.

**How it was determined, mechanically rather than by reading prose.** The
decision records do not usually carry their own command line; their receipts
do. Every receipt under `docs/research/runs/` was scanned for the header
`== hypotheses …: N declared, walk-forward (N folds) ==`, which only
`run_hypothesis_guarded` prints, against `FIXED parameters over the whole
window (no selection)`, which only `run_hypothesis_fixed_guarded` prints, and
against the `== direction control: …` header of `--mode=null-dir`. **Sixty
walk-forward hypotheses receipts exist.** A decision record is affected if and
only if it quotes a percentile from one of them.

**Affected — records whose percentile comes from a walk-forward matched
null.** Thirty-nine run directories under `docs/research/runs/` hold at least
one walk-forward hypotheses receipt, and **33 of the 41 records that contain
the word "percentile" cite one of them or are named after one**. That 33 is an
upper bound and is offered as such: some of those citations are
cross-references rather than the source of that record's own number
(`2026-09-15-pair-residual.md` mentions `close-reopen-drift` in passing;
`2026-09-14-pre-nfp-drift.md` and `2026-09-15-nfp-cross-asset.md` point at
`fx-local-hours` as prior work). The records where a walk-forward matched-null
percentile is load-bearing are:

`2026-09-13-{btc-us-hours, btc-us-open, close-reopen-drift, doji,
friday-weekend-hold, gap-fade, ict-sweep-mss-fvg, london-fix, m15-check,
night-synthesis, orb-ny, pdhl, recent-year-screen, trend-pullback, tsmom,
tsmom-2, tsmom-silver, volcond-breakout, volume-thrust, vwap-fade,
instrument-faults}.md`, `2026-09-14-{fx-local-hours, fx-local-hours-sign,
intraday-momentum, night-synthesis, tsmom-eurusd, volman-box}.md`,
`2026-09-19-smc-structure-measured.md` and `2026-09-23-rebate-rescore.md`.

**One more, found by a different route and worth naming because the mapping
above cannot see it:** `2026-09-12-gold-intraday-batch-1.md` quotes
`search --mode=hypotheses` and a 92nd percentile, and it predates the
`/research` pipeline, so **it has no receipt directory and cannot be re-run
from a recorded command**. Its percentiles are affected and stand
uncorrected.

**Unaffected, named rather than left to inference.** Eight of the 41 take
their percentile from somewhere this change cannot reach:

| record | where its percentile comes from |
|---|---|
| `2026-09-12-technical-baselines.md` | `--mode=null` (`run_null_control`) and `--mode=costs` — a different control that was never count-matched, and a different record |
| `2026-09-12-vantage-bars-baselines.md` | `--mode=compare` |
| `2026-09-14-nfp-vs-first-friday.md` | its own permutation test |
| `2026-09-14-pre-nfp-drift.md` | its own permutation test |
| `2026-09-15-nfp-cross-asset.md` | its own permutation test |
| `2026-09-15-quote-asymmetry.md` | its own permutation test |
| `2026-09-15-selftest-audit.md` | self-tests of the above |
| `2026-09-15-venue-residual.md` | its own controls |
| `2026-09-16-trailing-stop.md` and its run file | `scripts/trail_sweep.py`, which computes its own percentile |

And two categories are unaffected wherever they appear:

- **Every `--fixed` figure.** `in-sample-fixed.txt` and
  `out-of-sample-fixed.txt` come from `run_hypothesis_fixed_guarded`, which
  runs the control at explicit parameters and never touches a grid. The
  count-matching always worked there.
- **Every direction null.** `direction-*.txt` and `--mode=null-dir` permute
  sides on the method's own trades and are count-matched by construction.
  Verified empirically where a receipt prints it beside the matched null: the
  rescore's direction percentile is identical on all 22 rows.

**One caution about the fingerprint.** The identical-null signature above
*detects* the defect; its absence does not *exclude* it. A receipt with one
row, or with rows that happen to share a filter and a size, shows no
fingerprint and is affected all the same. Every walk-forward hypotheses
receipt is affected; the fingerprint only makes it visible without re-running.

## The second defect, found on the way and not fixed here

**The hold null is not count-matched at all, and never was.** A method that
manages its own exits gets `RandomHold` rather than `RandomEntry` as its
matched null, and `matched_rate` is not called for it — `rate` is `None` on
that branch. `RandomHold` enters on every flat bar and holds for the method's
realised hold length, so its trade count follows the hold, not the method.

`intraday-momentum` on the primary year is **197 out-of-sample trades measured
against a control taking 2,914** — a count match of 14.79, unchanged by this
repair because there is no rate to pin. That is the row
`2026-09-23-rebate-rescore.md` called "the nearest thing to a candidate" at the
98th–99th percentile of its matched null. **A 197-trade book read against a
2,914-trade null is exactly the error this repair was written to remove, in a
different control, and its percentile should be treated as unmeasured until
that is fixed.**

It is not fixed here because the registration scoped this work to the entry
rate and because `RandomHold` has no rate axis to calibrate — matching it
would mean changing what the hold null *is*, which is a restatement needing
its own registration. `ema-cross/asia` (count match 2.84 after repair) and
`doji-reversal` (1.69) are smaller instances of the related whole-window
versus walk-forward gap described above.

## A caution that applies to every re-run except the three above

The three batches with a **same-day pre-fix baseline** — the rebate rescore,
the recent-year screen and the recent-year sessions — are clean before-and-
after pairs: the same binary, the same bars, the same hour, the pin off and
the pin on. Everything said above about them isolates this repair and nothing
else.

**The bulk re-runs of the other receipts do not isolate it, and are not
offered as if they did.** Each was driven from the command line its own
receipt records, but the world has moved under those commands since
2026-09-13:

- **The engine changed.** The exit-bar excursion correction and the
  price-decimals rounding landed after the September receipts. They move a
  profit factor in the third decimal, and because the walk-forward *selects*
  on the score, a small move can flip which grid cell a fold picks and change
  the trade count outright. `btc-m15-check/m15/vwap-allday` is 3,392 trades in
  its original receipt and 2,557 in the re-run on **identical bars**. That is
  selection, not the null.
- **The bars changed.** A command with an open end now reads more of them —
  `doji/out-of-sample` and `close-reopen-drift/out-of-sample` gain six days of
  Vantage bars — and one parquet was rebuilt with a longer history:
  `gold-m15-check` read 66,243 `xauduka` bars from 2022-06-16 when it was
  written and reads 352,261 from 2010-06-01 now. Four re-runs so far are in
  this category and each is flagged by comparing the `bars:` line of the two
  receipts.

So for those receipts the re-run is **the current, correct number**, and the
difference from the published one is the repair *plus* ten days of everything
else. It is published as a corrected figure and not as a measurement of this
change. Anyone wanting the isolated effect on one of them should do what was
done for the three above: run the same binary twice with the pin reverted.

## What was re-run, and what was not

**Re-run with a same-day pre-fix baseline — clean before-and-after pairs:**

| batch | rows | receipts |
|---|---:|---|
| `2026-09-23-rebate-rescore.toml`, `xauusd` 15m primary year | 22 | `primary-xauusd-15m-{repaired,before-instrumented}.txt` |
| `2026-09-13-recent-year-sessions.toml`, in sample | 31 | `recent-year-sessions-in-sample-{repaired,before-instrumented}.txt` |
| `2026-09-13-recent-year-screen.toml`, in sample | 32 | `recent-year-screen-in-sample-{repaired,before-instrumented}.txt` |

**Re-run without a baseline, driven from each receipt's own recorded command,
and subject to the caution above:** `2026-09-13-{btc-m15-check,
btc-us-hours, btc-us-open, doji-2018, doji-btc, doji/out-of-sample,
gold-m15-check, london-range/in-sample-0trades, recent-year-gap,
recent-year-hours, trend-pullback-btc, volume-thrust, volume-thrust-gold}`
and `2026-09-14-volman-box-vantage`, as
`<dir>__<name>.txt` in the same directory.

**`recent-year-hours` deserves a sentence of its own**, because it holds the
one row of about 170 that passed the 2026-09-13 year screen and is running as
paper candidate #5, `xau-evening`. **It still passes: `hold/18-20-long`, 164
trades, moves 95th → 96th.** Every row in that batch has a count match of
exactly 1.00 before and after, and the reason is worth recording: a
`session-hold` is a self-managed-exit method, so its matched null is
`RandomHold`, which enters once per session window and therefore takes the
method's trade count by construction. **The hours batch was never affected by
this defect**, and its twelve distinct nulls, one per two-hour window, were
twelve because there were twelve filter sets and not because anything was
being matched. Nineteen of its 24 percentiles move by a point or two, which
is Monte-Carlo noise on 200 seeds and not this repair.

**Not re-run, and named rather than left to be discovered:**

- **Thirty-seven walk-forward hypotheses receipts.** The bulk re-run was
  ordered cheapest-first and stopped when the remaining ones were all long
  Dukascopy windows at 300 seeds costing tens of minutes each. What is left
  uncorrected: `close-reopen-drift`, `friday-weekend-hold`, `gap-fade`,
  `london-fix`, `london-range` (the real in-sample and out-of-sample),
  `orb-ny` and `orb-ny-matched-null`, `pdhl`, `tsmom`, `tsmom-2`,
  `tsmom-silver`, `trend-pullback` (gold), `volcond-breakout`, `vwap-fade`,
  `close-reopen-guarded`, all three `fx-local-hours` variants,
  `intraday-momentum`, `tsmom-eurusd`, `volman-box` (Dukascopy), the
  out-of-sample halves of the four recent-year batches, and both 5m
  recent-year batches. Three of those back paper candidates — `xau-close`
  (#4), `xau-ict-5m` (#8), `xau-box-5m` (#9) and `eur-hours` (#10) — and
  none backs a funded position.
- **The rebate rescore's context window** (`xauduka` 2022-06-16 →
  2025-04-10, the second table of `2026-09-23-rebate-rescore.md`). Context,
  never a gate.
- **Ten receipts that cannot be reproduced at all.** Four carry no command
  line (`2026-09-13-doji/in-sample-2022-2025-matched-null.txt`,
  `2026-09-13-ict-sweep-mss-fvg/{in-sample,out-of-sample}.txt`,
  `2026-09-13-orb-ny/out-of-sample-disjoint.txt`). Six are diagnostics kept
  under a sub-directory from code states that no longer exist
  (`before-fold-close`, `one-trade`, `stop-enforced`, `null-late-exit`);
  re-running them would produce a different thing wearing the same name.
- **`2026-09-12-gold-intraday-batch-1.md`**, which predates the `/research`
  pipeline and has no receipt directory.
- **The `--fixed` and direction receipts** — because they are unaffected, not
  because they were skipped.

## What would reopen this

- **The hold null**, under its own registration. It would restate
  `intraday-momentum` and every other self-managed-exit row, and the rebate
  rescore's "nearest thing to a candidate" with them.
- **The whole-window versus walk-forward calibration gap.** Calibrating the
  control to the method's *walk-forward* trade count instead would close the
  rows still outside the band. It changes `matched_rate`'s arithmetic, which
  this registration froze, so it is a separate decision.
- **Any further construct promoted to a funded account on a matched-null
  percentile.** Two are running on closed constructs already, and this
  document is the third time the record has had to be corrected underneath
  them.
