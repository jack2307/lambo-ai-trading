# RESULT 2026-10-07 — the record's highest percentile is an uncalibrated control, and the number is 62 not 98

Registration: `docs/decisions/2026-10-07-top-percentile.md`.
Binary: `/e/rust/fd-instr-repair/target/release/search.exe` (nothing built on
this branch). Data: `/e/rust/flowdesk/data`, read-only. News calendar:
`E:/rust/flowdesk/data/news/events.parquet`, 747 events — the `--data=` root,
not the default, and the receipts say so.

## 1. The published row reproduces byte for byte

`cell1-primary-noguards.txt` against
`docs/research/runs/2026-09-23-rebate-rescore/primary-xauusd-15m.txt`:

    197 trades | PF 1.088 gross / 1.121 net | rebate $0.26 = 0.13% of R
    matched null 98% gross / 99% net | direction null 66%
    matched null p50 0.952 / p95 1.067 gross, p50 0.973 / p95 1.090 net

Identical on every figure. The one-row batch is the twenty-two-row batch for
this row, as the design file claimed and now checks.

**What the patched binary adds, and what the 2026-09-23 engine did not print:**

    matched null 2914 trades median vs the method's 197 — count match 14.79
      ** outside the band: this percentile is unmatched **

The band is `1 +/- 0.25` (`COUNT_MATCH_BAND`). 14.79 is **59 band-widths**
outside it. Under the desk's own rule (brief section 4) the 98th/99th
percentile **is not read**, and this receipt does not publish a corrected one
for the coin-flip control.

## 2. Why the count match is 14.79 — it is the method's duty cycle, inverted

`matched_rate` is not involved. `intraday_momentum` declares
`Exits::Strategy` with an empty grid, so `hypotheses.rs` takes the **hold**
branch: `rate = None`, `stop = None`, control = `RandomHold`. The hold
branch's own calibrator, `matched_hold_rate`, is gated behind
`drift && null_sides.matches_hold_count()` — and `--null-sides=` is **inert in
`rescore`** (ADDENDUM-5 E(2)), so `hold_rate = None` and the control keeps
`RandomHold::default_params()`'s **`entryRate = 1.0`**, which `control_hold.rs`
itself documents as "not a neutral default": at 1.0 the control re-enters on
the first bar its filters allow after every exit.

The method's confinement to 15:15-16:30 New York, one entry per day, is a rule
of the **strategy**, not a `Filter`. The control inherits only
`hypothesis.filters` — `weekdays` and `flat:1630-1815` — so it holds all day,
every weekday. Measured, primary window (`cell5`):

    time in market: method 14,775 min   null median 230,265 min   ratio 15.58
    spread paid:    method $0.55        null median $8.16         cost match 14.79
    trades:         method 197          null median 2,914         count match 14.79
    long share:     method 0.487        null median 0.500         (this one matches)

23,531 bars x 15 min = 352,965 bar-minutes. The method occupies **4.19%** of
them; the control occupies **65.24%**. 65.24/4.19 = **15.58**, which is the
printed exposure ratio to three digits. The 14.8x is not a skewed probe. It is
`1 / duty cycle`, and it is **larger** than the 3.1-3.7x `control_hold.rs`
recorded for the long-hold methods for the obvious reason: a method holding
days at a time has a duty cycle near 1 and little room above it, while this one
holds 75 minutes a day.

**Why a fatter control raises the percentile rather than lowering it.** The
control's profit factor is a ratio of sums over its own trades, so its
dispersion falls as `1/sqrt(n)`. Measured:

    uncalibrated (2,914 null trades)  p50 0.952  p95 1.067   spread 0.115
    calibrated     (190 null trades)  p50 0.993  p95 1.343   spread 0.350

`0.115 x sqrt(2914/197) = 0.442`, predicting a calibrated p95 of 1.394 against
the **1.343 measured** — the tightening accounts for it. The method's PF of
1.088 sits above a 1.067 bar and below a 1.343 one. Nothing about the method
changed; the bar moved.

`matched_rate` was not touched, and neither was the hold branch's arithmetic.

## 3. The reading, on a control that calibrates — a DIFFERENT measurement

`--mode=hypotheses --null-sides=exposure`. This is not the published mode and
is not offered as a correction of it:

| cell | window | arm | trades | PF | expect | count match | null p50 / p95 | percentile |
|---|---|---|---|---|---|---|---|---|
| 6 | xauusd 15m, 2025-09-13 to 2026-09-12 | no guards | 197 | 1.088 | +0.004 R | **0.96** | 0.993 / 1.343 | **62nd** |
| 8 | the same | `--guards` | 197 | 1.089 | +0.004 R | **0.96** | 0.958 / 1.345 | **66th** |
| 7 | xauduka 15m, 2022-06-16 to 2025-04-10 | no guards | 559 | 0.840 | -0.005 R | **0.94** | 0.825 / 1.003 | **58th** |

Count match, cost match, exposure ratio and long share are **all four inside
the band** on all three. These percentiles are read.

**The record's highest percentile, read, is the 62nd — and the 58th on the
second window.** Both are ordinary. Not one of them is near the 95th.

ADDENDUM-5 B asked for two windows before anyone speaks: the two agree, and
they agree on "nothing here".

## 4. The coin-flip control on four cells, for the record it was published in

All four are `** outside the band **` and therefore **not read**. They are
listed because they are what the record contains:

| cell | window | arm | trades | PF gross / net | expect net | count match | percentile g / n |
|---|---|---|---|---|---|---|---|
| 1 | xauusd recent year | no guards | 197 | 1.088 / 1.121 | +0.005 R | 14.79 | 98% / 99% |
| 2 | xauusd recent year | `--guards` | 197 | 1.089 / 1.122 | +0.005 R | 14.26 | 98% / 99% |
| 3 | xauduka 2022-06 to 2025-04 | no guards | 559 | 0.840 / **0.987** | -0.000 R | 14.66 | 74% / **100%** |
| 4 | xauduka 2022-06 to 2025-04 | `--guards` | 559 | 0.838 / **0.985** | -0.001 R | 14.14 | 71% / **100%** |

**Cell 3 is the number worth keeping.** A profit factor of **0.987** — the book
loses money, with the rebate already credited — reads the **100th percentile**
of its own matched null, because that null's net p50 is 0.902 and its p95 is
0.944 on 8,195 trades. It is the `struct-80` precedent in `hypotheses.rs`
(PF 0.973 at the 98th) again, three times worse, on the row the record quotes
as its best.

## 5. The gate: one condition, and the binding leg is EXPECTANCY on this row

Primary window, 197 trades — past the desk's 40-trade floor by hand (the
tool's verdict string says `need 30`, which is not the gate):

    PF         1.121 net (1.088 gross)     against >= 1.200    — fails by 7.0%
    expectancy +0.005 R net (+0.004 gross) against >= +0.050 R — fails by 10x

ADDENDUM-5 A measured the PF leg to be the binding one at
`reward_risk = 1.8` with pure STOP/TARGET exits. **This row is the
counter-example, and the measurement wins.** It has no target, and its stop is
a `riskDailyRanges = 1.0` sizing stop the engine never enforces — every exit is
the clock. So R is one mean New York daily range while the hold is 75 minutes:
mean `|r|` per trade is tiny, the profit factor can sit near 1.1 on near-zero
R, and the expectancy leg is **ten times** further from its line than the PF
leg. Reporting "fails the gate" without naming the leg would hide that.

Second window: PF 0.840 gross / 0.987 net, expectancy -0.005 R. Both legs fail
and the gross book loses money.

## 6. Both arms, and `--exit-mix`

**Guards change nothing material.** PF 1.088 to 1.089, percentile 62nd to
66th, count match 0.96 either way. `WEEKEND_FLAT` closed **38 of 197**, which
is one Friday a week: the method is flat by 16:30 New York and the guard
flattens at 16:40, so the guard beats the 16:45 fill on Fridays and the mean
hold goes 75.0 to 72.1 min. **This row is not an ADDENDUM-5 D artefact** — it
never holds over a weekend by construction, so unlike every long-horizon
result in the record it exists in the arm the owner permits. It simply does not
pass in either arm.

**`--exit-mix`, and it is NOT the `tsmom/120d` trap.** `intraday_momentum`
declares `Exits::Strategy`. The registered row prints

    exits: flat window 197; mean hold 75.0 min

and `flat window` is `filter.rs:356` — the FILTER's close, which the comment
there says "overrides the strategy". The method's own label,
`window closed` (`intraday_momentum.rs:75`), appears **0 times**. But the two
fire at the same minute: the hold ends at 16:30 New York and `flat:1630-1815`
starts flattening at 16:30. `diag-exitlabel-noflat.txt` drops the flat filter
and the same 197 trades come back at the same PF 1.088 and the same
+0.004 R expectancy, now labelled

    exits: window closed 197; mean hold 75.0 min

So the method's rule **does** fire on every trade; `flat window` is a label
shadowing it, not an exit substituting for it. `max_hold_ms` is 4 h and a
75-minute hold never reaches it. (The diagnostic's own exposure ratio is 1.42
and out of band — without the flat filter the control spills into 16:30-18:15 —
which is why its percentile is not quoted as this construct's.)

## 7. Not measurable here

- **`cost/R`.** The row manages its own exits and has no enforced stop, so
  `spread / stop` has no denominator. The receipt prints
  `cost/R: NOT MEASURABLE on this row`, which is not 0%. Its cost in USD is
  measured instead: **$0.55** over 197 trades on the primary window, against
  **$587.97** over 559 on xauduka — 0.13% and 0.48% of R as the rebate column
  reports it.
- **Drawdown.** `max_drawdown_*` is in `engine::Metrics` and neither mode
  prints it (ADDENDUM-5 G). No risk figure sits beside these profit figures,
  and this axis promised none.
- **The coin-flip percentile at a matched count.** `--null-sides=` is inert in
  `rescore`, so there is no run that is the published measurement with the
  count repaired. Cells 6-8 are a different control, said so twice.
- **`wrong_side_stop`.** There is no such counter in this tree (`grep` over
  `crates/`: 0 hits), so brief section 7's first defect could not be counted on
  this row from the binary's output. The row takes no pending orders and its
  stop is never enforced, so section 7's second defect (`check_exit` pricing a
  gapped stop at `bar.open`) has no path here either.

## 8. Verdict

The falsifier's first branch fired, with one correction to it: the row is **not
unreadable**. It is unreadable **in the form the record published it** — count
match 14.79, no warning line, a percentile that is a property of
`entryRate = 1.0` — and it is **perfectly readable** under a control that
calibrates, where it reads the **62nd percentile on one window and the 58th on
the other**, and fails the gate on both with **expectancy** as the binding leg.

So the desk's most impressive published number is not a blank. It is an
ordinary number that was printed as an extraordinary one. The record should say
that the 98th/99th was **never evidence**, and that the figure in its place is
**62nd / 58th, count match 0.96 / 0.94**.

Eight cells declared, eight spent, plus two declared diagnostics. Zero through
the gate.
