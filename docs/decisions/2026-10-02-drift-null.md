# The drift-controlled null: it exists, it is count-, cost- and exposure-matched, and it takes away every percentile the record had for a fixed-window hold — 48 of 48 rows, including the one row of about 170 the recent-year screen said passed

**Date:** 2026-10-02
**Registration:** `docs/hypotheses/2026-10-02-drift-null.md`, committed at
`29cb740` before a line of this branch's code existed, amended at `6705393`
before any measurement on it was read. Its six falsifiers and seven
pre-commitments are answered one by one below.
**Branch:** `agent/drift-null`, cut from `main` (`90e8d30`). Finishes Task A of
`docs/hypotheses/2026-09-24-what-the-record-cannot-see.md`, registered on
`agent/repair-a` at `fc070cf` and cut off mid-fix at `d5e08d5`.
**Status:** the control is in, the receipts are published beside their
originals, and two verdicts change.
**Scope of the change:** `control.rs` and `control_hold.rs` (one parameter,
`longShare`, default 0.5), `hypotheses.rs` (`NullSides`, `side_distribution`,
`signed_exposure_minutes`, `gross_exposure_minutes`, `gross_cost_usd`,
`long_share_for_signed_share`, `matched_hold_rate`, six report fields and their
accessors, two new public entry points), `bin/search.rs`
(`--null-sides=coin|ratio|exposure` and four receipt lines), and
`tests/drift_null.rs` (new). **No threshold moved.** The gate stays
`min_trades 30 / min_profit_factor 1.2 / min_expectancy_r 0.05` and both nulls
stay at the 95th.

## Which branch I built on, and why

**I adopted `agent/repair-a`'s choice of control and re-implemented it against
`main` rather than merging or rebasing that branch.** Three reasons, stated
before any number was run:

- `main` has since gained the count-match pin (`2026-09-23-matched-null-repair`)
  and the cost match (`2026-09-24-cost-matched-null`), both of which rewrote
  `control_for`'s signature and body. A rebase would mean resolving the same
  functions by hand anyway.
- `d5e08d5`'s own commit message says nothing in it was reviewed and no number
  in it may be quoted.
- Its design was right and its diagnosis of its own collapse was wrong, and the
  only way to find that out was to rebuild it and measure.

What was taken from it unchanged: the control (a side-matched random entry with
the share measured the way `hold_distribution` measures a hold), the two
alternatives it rejected and its reasons for rejecting them (a long-only
control is this one at `longShare = 1.0`; a block bootstrap destroys the
wall-clock calendar every gated row depends on and cannot reproduce an
unaffected row unchanged), and its insistence that the collapse be surfaced
rather than hidden.

## What the drift control does

`longShare` on both controls, default **0.5**, which is the coin the whole
record was read against and is bit-for-bit the old side decision. Three
settings:

| `--null-sides=` | the control's side | the hold branch's entry rate |
|---|---|---|
| `coin` (default) | 0.5 | 1.0 — fills the gate |
| `ratio` | the method's long share by trade **count** | 1.0 — fills the gate |
| `exposure` | the share that reproduces the method's signed share of **time** | calibrated to the method's trade count |

`ratio` is `agent/repair-a`'s control exactly, kept so its collapse stays
reproducible. `exposure` is the instrument this record should be read against.

**Nothing about any drift is estimated anywhere.** The control is given, per
row, the method's own measured long share, its own realised hold distribution,
its own trade count, its own stop on the entry branch and the same spread, and
it runs inside the method's own filters over the method's own bars. That is why
the two reports that arrived mid-implementation — one predicting the standing
+0.3946 ATR20 / t = 6.74 is sqrt(5)-inflated by overlapping windows, one
measuring the drift as non-stationary by a factor of ten between eras — change
nothing here, and the amendment to the registration says so at length. **A
control that neutralises a quantity without estimating it cannot inherit an
error in the estimate.** It also cannot be wrong about the horizon or about
gross-versus-net, because it holds for the method's own time and pays the
method's own spread.

## The three properties, each with the test that proves it

| property | test | what it asserts |
|---|---|---|
| count-matched | `the_drift_null_takes_the_methods_own_trade_count` | the defect visible at `coin` (count match above the band) and inside the 0.25 band at `exposure`, on the real engine, with the gate figures bit-identical |
| cost-matched | `the_drift_null_pays_the_methods_own_cost_share` | the spread bill inside the band on the hold branch, and the control's `spread / stop` bit-identical across settings on the entry branch |
| drift-controlled | `the_drift_null_carries_the_methods_own_signed_exposure` | the coin's control near 0.500 long against a 100%-long method; the ratio match fixing the ratio and NOT the time in the market; the exposure match fixing both |

Two more hold the instrument rather than a property of it:

- `an_even_method_is_read_against_the_same_null_it_always_was` — falsifier 1.
- `a_fully_invested_one_sided_method_has_no_null_with_a_spread` — falsifier 5,
  and the reproduction of `agent/repair-a`'s collapse.

Plus `a_half_long_share_is_the_old_coin_flip_bit_for_bit`,
`a_null_whose_seeds_all_agree_is_not_a_distribution`,
`the_control_is_drawn_at_the_share_that_reproduces_the_signed_time` and five
more unit tests. The whole `fd-backtest` suite passes: 101 unit tests and
every integration file, `matched_null.rs` and `cost_matched_null.rs` included,
so neither earlier repair was disturbed.

## Reproducing the collapse first, as the WIP commit asked

`agent/repair-a`'s last words were "the ratio-matched null collapsed to a
single point — a defect in my own control". Reproduced before anything was
changed, on its own window and batch (`xauduka` 15m, 2018-06-16 → 2025-04-10,
300 seeds, fixed parameters), receipts
`docs/research/runs/2026-10-02-drift-null/repro-{coin,ratio,exposure}.txt`:

| row | PF | `coin` pct | `ratio` pct | `exposure` pct |
|---|---:|---:|---:|---:|
| `long-day` (long 00:00–23:45) | 1.138 | **100%** | `null` (p50 = p95 = 1.071) | **47%** |
| `short-day` (the same rule short) | 0.847 | 44% | `null` (p50 = p95 = 0.861) | **46%** |
| `long-ny` (long 09:30–16:00) | 0.866 | 76% | `null` (p50 = p95 = 0.896) | 15% |

The three `ratio` figures 1.071 / 0.861 / 0.896 are **identical to the ones on
`d5e08d5`**, so the collapse is reproduced exactly and is not an artefact of
this re-implementation.

**Its diagnosis of the collapse was wrong, and that is the finding.** It read
the collapse as a property of the row — "a method in the market on every bar,
on one side, has made no timing decision". It is a property of a control that
matches the side ratio and leaves `entryRate` at 1.0: at that rate the hold
control re-enters on the first bar its filters allow after every exit, so its
entry draw decides nothing, and once the share is 0 or 1 its side draw decides
nothing either. **There is no random input left.** Calibrating the rate to the
method's count puts the timing lottery back: `long-day` goes from one point to
a distribution with p50 1.146 and p95 1.276.

## Falsifier 6, the sign check, which is what decides whether this is a drift control

The same rule long and short must land at comparable distances from the middle
of their own nulls. Against a coin they cannot:

| | `coin` | `exposure` |
|---|---|---|
| `long-day` | **100th** | 47th |
| `short-day` | 44th | 46th |
| the gap | **56 points** | **1 point** |

The 56-point gap between a rule and its own mirror image was the instrument's
reading of gold's direction, not of either rule. It is gone. **Falsifier 6
passes.**

And the figure that says what the row actually is: `long-day`'s drift-matched
null has a profit factor of **1.146** against the method's **1.138**. Buy gold
every day and sell it before the close, for seven years, and you finish
marginally behind a coin-free control that is long the same amount of time for
the same cost.

## What the record's own "edges" do under it

### The two verdicts that change: `recent-year-hours`, the primary criterion window

`xauusd` 15m, 2025-09-13 → 2026-09-12, walk-forward 4 folds, 200 seeds — the
owner's primary criterion. Receipts
`hours-in-sample-{coin,exposure}.txt`. **All 24 rows are one-sided two-hour
holds. All 24 lose their percentile.** On every one of them the control is
matched on all four axes it can be: count match **1.00**, cost match **1.00**,
time in market ratio **1.00**, signed share of time **+1.000 against +1.000**
(or −1.000 against −1.000).

| row | trades | PF | published pct (2026-09-13) | `coin`, same binary | `exposure` | verdict |
|---|---:|---:|---:|---:|---|---|
| `hold/18-20-long` | 164 | 1.362 | **95th → 96th, SURVIVES** | 96% | **`null`** (p50 = p95 = 1.362) | **SURVIVES → no percentile** |
| `hold/04-06-short` | 205 | 1.507 | **100th matched / 89th direction, SURVIVES** | 99% | **`null`** (p50 = p95 = 1.507) | **SURVIVES → no percentile** |
| `hold/16-18-long` | 197 | 1.374 | 94th | 92% | `null` (p50 = p95 = 1.374) | unchanged (already closed) |
| the other 21 rows | 164–206 | 0.60–1.19 | 0th–92nd | 0%–86% | `null` | unchanged |

`docs/decisions/2026-09-13-recent-year-screen.md` opens with "**Under the
program's own falsifier, one row of about 170 passes (`hold/18-20-long`)**".
That row does not pass. Its drift-matched null's profit factor **is its own**:
1.362 against 1.362. The rule "be long from 18:00 to 20:00 every weekday" and
the control "be long from 18:00 to 20:00 every weekday at a matched count and
cost" are the same book, so the comparison has no content, and the 96th
percentile it was carrying was its position in the coin's side lottery.

The signature was visible in the published receipt and nobody read it that way:
on all twelve windows the long and short percentiles sum to about 100 — 60/48,
58/46, 1/99, 86/12, 61/24, 46/70, 30/72, 33/69, 92/8, 96/3, 33/78, 14/90. **A
percentile that is one minus its mirror's is a statement about direction, not
about the rule.**

### The long window, which that decision named as its next registration

The same decision records, as the one structure the year did not invent: "**long
across the New York close and the evening — 16:00 to 22:00 — beats random holds
and its own flipped sides at the 100th percentile on 460–560 sessions**. That
is not a candidate from this screen; it is the next registration."

Re-run on that window (`xauduka` 15m, 2022-06-16 → 2025-04-10, 200 seeds),
receipts `hours-long-window-{coin,exposure}.txt`:

| row | trades | PF | `coin` | `exposure` | the control's own PF |
|---|---:|---:|---:|---|---:|
| `hold/16-18-long` | 560 | 1.508 | **100%** | **`null`** | 1.508 |
| `hold/18-20-long` | 464 | 1.140 | **100%** | **`null`** | 1.131 |
| `hold/20-22-long` | 463 | 1.238 | **100%** | **`null`** | 1.238 |
| `hold/04-06-long` | 578 | 1.139 | **100%** | **`null`** | 1.139 |

Count match 1.00, cost match 1.00, time-in-market ratio 1.00 and signed share
+1.000 against +1.000 on all 24 rows. `hold/18-20-long` earns a profit factor
of 1.140 where being long that window every day for the same cost earns
**1.131** — it adds **0.009 of profit factor**, which is **99.2% of the
control's result reproduced and nothing beyond it**. The other three add
nothing at all to four decimal places.

**That registration should not be opened.** The structure it was going to test
is the instrument's own drift exposure, measured on 460–578 sessions, and this
null is the thing that says so.

### The rows with a stop and a target, including both funded books

`recent-year-sessions` (32 rows) and `recent-year-screen` (34 rows), same
window and settings, receipts `sessions-in-sample-*.txt` and
`screen-in-sample-*.txt`. **No verdict changes on either batch**, and the three
survivors stay the same three:

| row | trades | PF | `coin` pct | `exposure` pct | long share | signed share, method vs null | count / cost / time match |
|---|---:|---:|---:|---:|---:|---|---|
| `macd-cross/asia` — the funded `xau-macd-asia` | 300 | 1.558 | 100% | **100%** | 0.480 | −0.049 vs −0.036 | 0.88 / 0.83 / 0.87 — all in band |
| `keltner-break/asia` | 262 | 1.286 | 98% | **98%** | 0.500 | −0.004 vs +0.020 | 0.86 / 0.83 / 0.86 — all in band |
| `ema-cross/asia` | 37 | 1.622 | 99% | 100% | 0.459 | −0.174 vs −0.148 | **2.82** / 2.71 / 2.91 — unmatched both ways |
| `stoch-reversal/asia` — nearest to the funded `xau-stoch` | 420 | 0.918 | 26% | **26%** | 0.464 | −0.045 vs −0.028 | 0.71 / 0.72 / 0.70 |
| `stoch-reversal/all` | 1297 | 0.908 | 30% | 24% | 0.465 | −0.091 vs −0.086 | 0.63 / 0.64 / 0.90 |

**Pre-commitment 4, the funded books, answered unprompted: neither moves.**
`xau-macd-asia` is 100th against a coin and 100th against the drift control, on
a control matched on count, cost, side and time. `xau-stoch`'s nearest
published row moves 30 → 24 and 26 → 26, both far below the 95th either way.
The owner needs no action on this axis.

`ema-cross/asia` is the one survivor whose figure moves (99 → 100), and it is
**not quotable either way**: its count match is 2.82, outside the band before
this change and after it, on 37 trades. That was already recorded on
2026-09-23.

Why these rows barely move: **they have almost no signed time exposure to
start with.** 26 of the 32 session rows are inside 40–60% long, 5 are outside and 1 took no trades; the three
survivors are 0.459, 0.500 and 0.480. The defect is real and on these rows it
is inert, which is the other half of what the registration said would be
published either way.

### The direction the registration predicted, and it is not supported

Pre-commitment 3 said a row long-leaning on a positively-drifting instrument
was read against too weak a control and its percentile was too high, so it
should **fall**; short-leaning should **rise**. Of the session rows with at
least 30 trades, a signed share of time outside ±0.10, and all three matches
inside their bands, there are exactly **three**: `ema-cross/london` (+0.121,
30% → 28%), `orb/london` (−0.154, 22% → 18%) and `pdhl/london` (−0.138, 15% →
13%). **All three fell, whichever way they leaned.** One agrees with the
prediction and two contradict it.

The honest reading is that none of the three moved at all: 200 seeds resolve
0.5 points of percentile, so 2 to 4 points is 4 to 8 seeds, and a row whose
signed share of time is 0.12 has little exposure for a drift control to take
away. **The prediction is recorded as unsupported rather than as confirmed on
one row of three.**

## What I corrected in my own instrument after seeing numbers, and why each is not a threshold moved

Both are recorded here rather than absorbed, in the way
`2026-09-24-cost-matched-null.md` recorded its implementer's correction to its
own case 1.

### 1. The signed-minutes RATIO was a bad statistic and is replaced by two figures

The first version reported the exposure match as the control's median signed
minutes over the method's. On a two-sided row that is a ratio of two small
differences of large numbers, and it printed **−4.80 on `keltner-break/asia`**
— a row whose side match was 0.502 against 0.500 and whose count match was
0.86 — and 0.02, 4.54, −2.96 on others. Noise reported as failure.

Replaced by the two factors the exposure actually has, because either alone is
an instrument that lies in one direction: the **signed share of time** (within
0.05, the side-match band) and the **gross time in the market** (within 0.25,
the count-match band). A control with the method's side ratio exactly and 1.35x
its time in the market has a signed-share difference of zero and collects 35%
more drift; a control with the right time and the wrong sides has the right
gross and the wrong answer. Both are now asked.

### 2. The control is drawn at the share that reproduces the signed share of TIME, not at the method's long share by trade count

This is the substantive change and it came from the instrument's own reading.
**A method can be 50/50 by trade count and materially one-sided in time, and
the drift is paid on the time.** Measured on the 32 session rows: the median
gap between a row's signed share of time and the `2·long_share − 1` its trade
count implies is **0.055**, and **10 of 31 measurable rows exceed 0.10** — two
side-match bands. The worst is `doji-reversal/asia`, **52.6% of trades long and
a signed share of time of +0.478** against the +0.052 its count implies,
because its longs are held about two and a half times as long as its shorts
(10 longs and 9 shorts, and 74% of the minutes long). A control
drawn at 0.526 would have been given a twentieth of that row's exposure while
printing a side match of 0.526 against 0.531 and passing every check.

So under `exposure` the control is drawn at `(1 + signed_share) / 2`, measured
on the method's own trades, and the achieved signed share is printed on the
row. After the change `doji-reversal/asia` reads +0.478 against +0.519 and
`trend-pullback/asia` +0.252 against +0.292, both inside the band; before it
they read +0.478 against +0.078.

Neither correction moves a threshold, neither can make a method pass, and both
were found by the instrument failing its own published checks rather than by a
method failing a gate.

## Falsifiers 1 to 5, answered

1. **An even row must not move.** Held. 26 of 32 session rows are inside
   40–60% long; their null medians move by 0.000 to 0.028 of profit factor and
   no verdict changes. `an_even_method_is_read_against_the_same_null_it_always_was`
   asserts it on the real engine.
2. **No gate figure may move.** Held, checked column by column on all 90 rows
   of the three `xauusd` batches and all 48 rows of the two hours batches:
   **zero** differences in trades, profit factor or expectancy between `coin`
   and `exposure`. The control is built after the method's run and cannot reach
   it; the test asserts it by `to_bits`.
3. **The side match must be achieved, not claimed.** Printed on every row.
   Achieved within 0.05 on every row of the hours batches and on all but the
   thin ones elsewhere; a row outside is printed `** the control's side ratio is
   not the method's **` and is not quoted.
4. **The count match must survive the drift match.** Held. On the hold branch
   it is 3.39 before and 0.99 after on `long-ny`, and 1.00 on all 48 hours
   rows both ways; on the entry branch it is unchanged to two decimals on 21 of
   32 session rows and moves by at most 0.05 on the other 11, none of them
   across the band.
5. **A null with no spread reports `null`.** Implemented as
   `percentile_or_null`, which every printer and `survives()` goes through. It
   fired on 48 of 48 hours rows and on 3 of 3 `ratio` reproduction rows. Those
   rows report `null` in the `pct` column — not 0, not 100.

## What I could NOT fix, and why

1. **The spread bill is confounded by equity growth and is a weaker cost
   instrument than the stop.** `long-day` paid 36,124.98 USD of spread against
   its coin control's 5,400.83 — a ratio of 0.15 — on trade counts of 1,751
   against 1,760. The control did not pay a seventh of the cost per trade: the
   engine sizes from equity, the method compounded at 1.138 over 1,751 trades
   and the control at 1.071, so the method's later trades were about seven
   times larger. Under the exposure-matched control, whose profit factor comes
   out at 1.146 against the method's 1.138, the same pair reads 0.98. **The
   equity-free cost statement is `spread / stop`,** which exists only on the
   entry branch and is Task B's. I added a cost figure to the hold branch
   because it had none; it is confounded, it is documented as confounded on
   `HypothesisReport::cost_matched`, and a proper per-trade cost share for a
   hold null needs a sizing stop the hold null does not have.
2. **A window-hold control is not confined to the method's window, so on a row
   like `long-ny` it enters at hours the method never enters.** Its holds then
   straddle the daily halt and come out 1.35x longer than the method's even at
   a matched trade count, and it pays 2.22x the spread. Both are flagged and
   the row is printed unmatched. It is **not fixable inside this
   registration**: confining the control to the method's own window would make
   the control the method again and collapse the null, which is the trap the
   whole of this work is about.
3. **The direction null still removes the drift and I did not repair it.**
   `DirectionFlipped::on_bar` and `permuted_sides_pnls` flip each entry on
   `hash >> 63`, so both carry an expected long share of 0.5 whatever the
   method did — the same defect, in the other instrument. The ratio-preserving
   version is a random permutation of the method's own side labels, and **for a
   one-sided method that permutation is the identity: there is nothing to
   permute and no test exists.** Every direction percentile published for a
   long-only or short-only row is therefore reporting the coin's drift removal
   exactly as the matched null was, and should be read as `null`. The 24
   recent-year hours rows each carry one, in
   `docs/research/runs/2026-09-13-recent-year-hours/direction-hold-*.txt`, and
   `hold/04-06-short`'s is the 89th that
   `docs/decisions/2026-09-13-recent-year-screen.md` tabulates. Deciding what a direction null means for a
   one-sided method is a separate registration and I did not open it.
4. **The figure this work was motivated by cannot be reproduced from its own
   receipt.** The +0.3946 ATR20 / t = +6.74 has no script, no test and no
   receipt anywhere in this repository; it appears in three prose documents and
   nowhere else. Its magnitude is corroborated twice independently and its `t`
   is disputed at sqrt(5). Nothing on this branch depends on it, because the
   control estimates no drift — but the record should not keep quoting a number
   it cannot recompute, and repairing that is somebody's task and was not mine.
5. **The seed count floors the resolvable percentile and I did not raise it.**
   200 seeds resolve 0.5 points; `agent/new-method-2` measures 247 incurred
   looks against 6 declared, at which no row in that programme could survive a
   multiplicity correction however real its effect. The re-runs here use the
   seed counts of the receipts they sit beside, because a corrected figure must
   differ from its original in one thing only.
6. **The rebate rescore keeps its coin-flip null.** `rescore_hypothesis` is
   left at `coin` deliberately, so it reproduces `2026-09-23-rebate-rescore.md`
   exactly; its 22 rows are all two-sided engine-exit rows of the kind that do
   not move here. Named rather than omitted.

## What this means for the record, in one paragraph

The drift control changes nothing on a method with a stop and a target, because
those methods are two-sided and have almost no signed time exposure — the
defect is real and inert on 66 of the 90 rows re-run. It removes the percentile
entirely from every fixed-window hold, 48 of 48 across two windows, because for
a rule whose whole content is "be on this side during this window" the
drift-matched, count-matched, cost-matched control **is that rule**, and a
comparison of a thing with itself is not a quantile. That takes away the one
row of about 170 that `2026-09-13-recent-year-screen.md` reports as passing, and
it closes the evening-drift registration that decision said was coming next
before it is opened. Two programmes closed with zero survivors; this says a
third of the reason the first one had anything to look at was the instrument.
