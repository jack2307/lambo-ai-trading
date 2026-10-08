# Registration: what is the LARGEST gross-of-spread R per exposed session any rule in this registry reaches on this data, and which mechanism reaches it?

Registered **2026-10-08**, branch `agent/gross-ceiling` (cut from
`agent/drawdown`). Committed **alone**, before the design file and before the
first `xauduka` cell of this branch exists.

Briefs: `/e/rust/AGENT-BRIEF-2026-10-07-AUDIT.md`,
`/e/rust/AGENT-BRIEF-ADDENDUM-5.md`, `/e/rust/AGENT-BRIEF-ADDENDUM-6.md`. All
three apply in full.

## 0. This is NOT a gate programme, and the question is deliberately different

Eight programmes have asked *"does mechanism X clear the gate?"* and been
answered **0**. Addendum 6 section II then closed that whole shape by
arithmetic, not by a failed measurement: `+0.050R` per trade times 19.9
round trips is `+0.994R` per horizon against a measured gross of `+0.3775R`, so
**a slow signal expressed as many small trades cannot pass the expectancy leg
at any cost, including zero.**

`agent/rollover-flat` built a working frame - flat across every 17:00 New York
rollover, carry down to **3.4%**, `WEEKEND_FLAT` **0-1 in 1,157-1,944 trades**
so it lives in the only arm the owner permits - and the frame is **not** what
is missing. What is missing is a **big enough signal**: it measured
`+0.0036` to `+0.0224 R per session` gross, the spread ate **44-89%** of that,
and the frame needs about **0.1 R per session** gross. A ~10x gap.

So this job **ranks by a different quantity**: **gross-of-spread R per exposed
session**. A mechanism that misses the gate can still have a high gross per
session, and that is the quantity the frame consumes. **No gate verdict here is
a finding**; gate legs are printed because the tool prints them and are read
only as context.

## 1. Hypothesis, one sentence

On this feed there exists at least one registry mechanism whose
gross-of-spread R per exposed session, taken as the **minimum of the two
time-split windows**, reaches **>= 0.05 R** - half of what the
`rollover-flat` frame needs - and the mechanism that reaches the maximum is
identifiable and nameable.

## 2. THE RULER, defined exactly in columns the tool really prints

Printed per row by `--mode=hypotheses` on the patched binary: `trades`,
`expect` (3 dp), `max drawdown X USD = Y%`, `net Z USD`, `avg_mae`,
`expectancy_net`/`total_r_net` vs `expectancy`/`total_r` (printed only when
swap or commission is non-zero), `spread paid: method S USD`,
`long share: method`, `exposure: signed share of time method`,
`time in market method T min`, `exits: ... mean hold H min`.

**Step 1 - the R unit in dollars.** From the **swap arm** of the same row,
where the engine prints both columns:

    risk_usd = |swap$| / |total_r_net - total_r|

**Step 2 - the spread, in R, per round trip.** From the **swap-free arm**'s
`spread paid` (same row, same window, same guards):

    spreadR_per_trip = (spread_usd_A / trades) / risk_usd

**Step 3 - gross of spread, per round trip.**

    gross_per_trip = expectancy_A + spreadR_per_trip

`expectancy` is the mean of `r = points / risk`; `search.rs:1180` states the
spread is **inside** it (both fills moved by `apply_costs`) and that swap and
commission are **not**. So `expectancy_A` at swap 0 is **net of spread, gross
of financing**, and adding the spread back gives gross of both. This is
`agent/rollover-flat`'s own "flat gross / trip".

**Step 4 - sessions, and this is the one choice the calibration cannot pin.**

    sessions_exposed = time_in_market_min / S
    S = the mean hold of px-1s in the SAME run
        (session-hold, from 1800, to 1600, side 1, riskDailyRanges 1.5,
         rangeDays 20 - a one-session long book on the window
         agent/pure-drift proved crosses 0 rollovers)
    measured by agent/rollover-flat: S = 1371.2 min (IS), 1398.5 min (OOS)

    gross_per_session = (gross_per_trip x trades) / sessions_exposed
    trips_per_session = trades / sessions_exposed

**Declared now, before any new number:** on a row that holds exactly one
session per trade, `gross_per_session == gross_per_trip` to within 0.1%, so the
calibration of section 3 pins the **numerator exactly and the denominator only
there**. For a row holding many sessions per trade the denominator divides; for
an intraday row it **multiplies**, by up to ~23x at a 1 h hold. That is not a
hidden thumb on the scale, it is the definition the brief asks for ("gross per
**exposed** session"), and the two columns that expose what it costs -
`trips_per_session` and `spread/gross` - are printed **beside every row,
always**. A row whose high gross per session is bought with many round trips per
session is declared **frame-incompatible** in the report, because the
`rollover-flat` frame pays one round trip per session, not eight.

**`gross_per_trip` is therefore reported beside `gross_per_session` on every
row**, and the ranking table is published **twice**: once on all rows, once
restricted to rows with `trips_per_session <= 1.5` (the frame-compatible
subset). Both orderings are declared now; neither is chosen after the fact.

## 3. CALIBRATION - already executed, on agent/rollover-flat's own receipts, costing 0 cells

The brief requires the ruler to reproduce `tsf-l60` and `qsf-h15` before the
ruler is trusted. Steps 1-3 applied to
`/e/rust/fd-rollover-flat/receipts/rollover-flat/{A,B}-duka-{is,oos}.txt`:

| row | leg | my steps 1-3 | rollover-flat printed | |
|---|---|---|---|---|
| `qsf-h15` | IS | 0.011 + 1807.22/110.553/1435 = **+0.02239** | **+0.0224** | match |
| `qsf-h15` | OOS | -0.005 + 1028.82/91.089/1319 = **+0.00356** | **+0.0036** | match |
| `tsf-l60` | IS | 0.001 + 1553.52/103.015/1823 = **+0.00927** | **+0.0093** | match |
| `tsf-l60` | OOS | 0.007 + 972.34/98.824/1790 = **+0.01250** | **+0.0125** | match |
| `qsf-h5` | IS | 0.009 + 1597.30/106.011/1305 = **+0.02055** | **+0.0205** | match |
| `tsf-l20` | IS | 0.003 + 1622.68/101.508/1944 = **+0.01122** | **+0.0112** | match |

6 of 6 to the printed digit. **The ruler is calibrated.** Two readings fall
out of the same arithmetic and are recorded here before any new run:

* **`risk_usd` lands at 91.1-110.6 USD** across these rows against the
  configured 1% of 10,000 USD, which is the compounding equity, not a defect.
* **`px-1s`'s own `gross_per_trip` reproduces addendum 5 section B's drift
  independently**: IS `-0.015 + 1852.49/80.000/2014 = -0.00350` against the
  published **-0.0040**; OOS `+0.012 + 1571.72/96.040/1982 = +0.02026`
  against the published **+0.0205**. And its `spreadR_per_trip` reads
  **1.150% (IS) / 0.826% (OOS)** against `rollover-flat`'s published
  **1.15% / 0.83%**. So **`px-1s` measured in my own runs is the drift anchor**,
  not a number carried in from a brief.

## 4. DRIFT, subtracted, and the R-unit conversion stated

Addendum 5 section B: gold's one-session drift is **-0.0040 R (2010-2018)** and
**+0.0205 R (2018-2026)**, quoted in `px-1s`'s R unit (1.5 mean New York-day
ranges over 20 days). A row with a different stop has a different R, so the
drift in **its** units is different. The spread is a fixed **0.28 price points**
on this feed at every hour, so `spreadR_per_trip` **is** a receipt-internal
probe of the stop in price:

    R_row / R_px1s = spreadR_px1s / spreadR_row

    drift_attributed_per_session(row)
        = signed_share_of_exposure(row)
          x gross_per_session(px-1s, same run)
          x (spreadR_per_trip(row) / spreadR_per_trip(px-1s))

    residual_per_session = gross_per_session - drift_attributed_per_session

`signed_share_of_exposure` is the printed `exposure: signed share of time
method`, which is `(long time - short time) / time in market` - `px-1s` prints
`+1.000` on it, which is what makes it the right coefficient.

**If a row's `gross_per_session` is approximately its
`drift_attributed_per_session`, the row carries nothing beyond exposure and is
reported as exposure, not as signal.** Addendum 5 section B measured that
**75-100% of the sample rows' edge is NOT drift**, so a non-trivial residual is
an expected, measurable thing, not a hoped-for one.

**Declared limit:** `risk_usd` is recovered from a swap total that prints as a
whole dollar (`{:>8.0}`) and an R total that prints to 2 dp, and it is an
equity-path average over trades that are not uniformly spread across 8 years. On
a 93-trade row that average is a different epoch's average than on a
2,014-trade row. So `spreadR_per_trip`, and therefore the R-unit conversion and
the drift residual, carry a few percent of error that I cannot bound from the
receipt. **The residual is reported as an approximation with that stated, and
no conclusion rests on a residual alone.**

## 5. Rows, windows, arms - frozen

**Rows: every registry mechanism that can run on this feed, at its DEFAULT
parameters, no grid, `--fixed`, no selection.** `Registry::with_builtins`
registers 32. Excluded, with the reason:

* `level-reversion`, `maxpain-magnet`, `flow-momentum`, `flow-at-level` -
  `needs_options()`, and the header prints `timeline: none - options strategies
  will be skipped`. **4 rows.**
* `external` - reads a signal file that does not exist for this question.
  **1 row.**
* `companion-unconfirmed` - takes no trade without `--companion=`, which this
  plan does not pass. **1 row.**

**26 default rows.** Plus **4 labelled anchors**, which exist so this run is
shown to reproduce the record before anything new is read from it:

| label | base | overrides | role |
|---|---|---|---|
| `px-1s` | `session-hold` | from 1800, to 1600, side 1, riskDailyRanges 1.5, rangeDays 20 | the session-length constant S, AND the drift anchor of section 4 |
| `qs-h15` | `quiet-swing` | holdSessions 15 | parity anchor vs rollover-flat / agent/n5 |
| `ts-l20` | `tsmom` | lookbackDays 20 | parity anchor |
| `ts-l60` | `tsmom` | lookbackDays 60 | parity anchor, the 0.641R-of-carry row |

(`tsmom` at defaults is `lookbackDays 60`, so `ts-l60` duplicates it by
construction and the two must print identically - a free internal check.
`quiet-swing` at defaults is `holdSessions 5`.)

**30 rows.** `tsmom-flat` / `quiet-swing-flat` are **not** available: they live
on `agent/rollover-flat` and this binary cannot know an id that did not exist
when it was built (brief section 1 - an old binary swallows an unknown id in
silence). Their numbers are reproduced **arithmetically from their own
receipts** in section 3 instead, which is a stronger calibration than a
re-implementation would be, because it uses the identical printed columns.

**Windows, split by time, the record's own two halves:**

    IS   --from=2010-06-01 --to=2018-06-01    (190,889 bars)
    OOS  --from=2018-06-01 --to=2026-06-01    (187,860 bars)

**Arms:**

| arm | guards | swap | what it is for |
|---|---|---|---|
| **A** | off | 0.00 | the gross and net numbers, the drawdown, the spread in USD |
| **A-swap** | off | -0.83/-0.83 | `risk_usd` only (step 1) |
| **C** | **on** | 0.00 | the only tradeable arm (addendum 5 section D) |
| **C-swap** | **on** | -0.83/-0.83 | `risk_usd` only, guards on |

Guards on and guards off are **both** ranked and both published, because
addendum 5 section D settles that a result living only in the no-guards arm is
**not tradeable** and must be said so, and brief section 0 item 4 measured that
**23/398 paired rows (5.8%) cross PF 1.200 on the guards flag alone**.

## 6. Multiplicity ledger - declared NOW, counted BEFORE the first run

| | declared |
|---|---|
| rows | **30** |
| windows | 2 |
| guards arms | 2 |
| **ranking cells** (row x window x guards arm) | **120** |
| swap-arm reads of the same cells, for `risk_usd` only | **120** |
| **printed rows in total** | **240**, in **8 runs** |
| pre-check cells already spent (timing/parity probe, 2 rows, IS, A-swap, seeds 20) | **2** |

**No row is added, no window changed, no arm added, no parameter tuned and no
threshold moved after a number is seen.** Amendments are dated notes appended
to section 12; no line above is rewritten.

**The selection bias of a maximum, stated now.** The headline is a **max over
120 cells**. A max is biased **upward**, so:

* if the max comes out **below** the falsifier line, the negative conclusion is
  **safe** - bias could only have pushed it up, and it still did not reach;
* if the max comes out **above**, it is reported as *"the largest of 120 cells"*
  with the 30-row, 2-arm search stated beside it, and **not** as a candidate.

## 7. Falsifiers - specific and firable

**F1 - the one this job exists to fire. THE CEILING.** If
`max over all 120 cells of min(gross_per_session_IS, gross_per_session_OOS)`
is **< +0.050 R**, then this data's ceiling is **below** half of what the
`rollover-flat` frame needs, and the desk is not short of a better mechanism -
it is short of **a more mobile market or a lower cost**. *Fires on:* that max
being under +0.050 R. **This is the most valuable reading available, including
when it is negative**, because it tells the desk where to stop looking.

**F2 - the ruler. A PRE-CHECK, and it has already been run.** If my steps 1-3
cannot reproduce `rollover-flat`'s `tsf-l60` and `qsf-h15` to the printed digit,
**stop and report** - an uncalibrated ruler makes every ranking meaningless.
*Did not fire:* 6 of 6 reproduced, section 3. Cost **0 gate cells** (arithmetic
on receipts already in the repo).

**F3 - parity of this binary. ALSO A PRE-CHECK.** If `ts-l60` / `px-1s` in my
arm-A-swap IS run do not reproduce `rollover-flat`'s `B-duka-is.txt` to the
printed digit, the binary or the settings differ and **no new number is read**.
*Already checked on the 2-row probe:* `ts-l60` printed `trades 93 / PF 0.109 /
expect +0.151 / total_r +14.04 / total_r_net -68.00 / swap -6091 / spread 54.50
/ mean hold 39082.9 min`, every figure identical to `B-duka-is.txt` lines 54-59;
`px-1s` printed `2014 / 0.905 / -0.015 / -30.06 / -32.71 / -212 / 1832.80 /
1371.2 min`, identical to lines 27-32. **Did not fire.**

**F4 - artefact #10, the SIGNAL FAMILY.** Addendum 6 section V: `quiet-swing` is
strongest in the first half and **negative** in the second; `tsmom` is the
reverse. So a high gross per session in **one** window is worthless. *Fires on:*
the row with the highest **single-window** gross per session having a
**negative** gross per session in the other window. If it fires, that row is
written up as **the eleventh instance of artefact #10, not as a mechanism**, and
the ranking stands on `min(IS, OOS)` exactly as declared in section 2 - which is
why `min` was declared before the numbers and not after.

**F5 - exposure, not signal.** If the row with the maximum `min(IS,OOS)`
gross per session has `residual_per_session` within **20%** of zero after the
section 4 drift subtraction in **either** window, that row is reported as
**drift exposure re-expressed**, not as a mechanism.

**F6 - a burned row cannot be read.** Addendum 6 section IV, defect 14: a row
printing `max_drawdown_pct > 100%` has **blown the account** and kept trading on
`min_lot`, so its PF is flattered. *Fires per row.* Any such row is **excluded
from the ranking** and listed separately with its percentage. Declared now so
the exclusion is not a choice made after seeing which rows it removes.

**F1, F4, F5 and F6 can all fire. Every reading is written whatever it says.**

## 8. What will be reported, declared now

Per row, per window, per guards arm: `trades`, `gross_per_session`,
`gross_per_trip`, `trips_per_session`, `net_per_session` (= `expectancy_A` x
`trades` / `sessions_exposed`, i.e. spread in, financing out - the frame's own
net, since the frame has solved carry), **`spread/gross` as a percent**,
`drift_attributed_per_session`, `residual_per_session`,
**`max drawdown USD` and `_pct`**, `avg_mae`, `long share`, `signed share of
exposure`, `mean hold`, the **exits mix**, and the gate verdict as context only.

**Every quoted row carries its drawdown in USD beside its return** (addendum 6
section IV: `hold/20-22-long` at PF 1.238 fell **3,144.68 USD** against
`hold/16-18-long` at PF 1.508 falling **1,375.73 USD** - the higher PF with 44%
more of a fall). **`_pct` is quoted as a floor**, because it divides by the
curve's highest equity.

**Exits mix is read on every quoted row** (brief section 6a): `tsmom/120d`
printed PF 2.236 and `SURVIVES` with its own rule firing **0 times**. A row
whose exits are all `WEEKEND_FLAT` / `NEWS_FLAT` / `END_OF_DATA` /
`OPEN_LOSS_CAP` is reported as **void**, whatever its gross per session.

**If any mechanism reaches >= 0.1 R per session on both windows: it is reported,
with drawdown and with the post-drift residual, and it is NOT proposed as a
candidate.** The frame is built and tested against `swap_nights`; whether it
goes into `rollover-flat` is the owner's call, not this job's.

## 9. NOT measured, declared now so it cannot be claimed later

1. **Percentiles.** `rollover-flat` measured the `RandomHold` control's
   `cost match` at **0.57-0.84** on its flat rows - outside the band - and
   `px-1s`'s at **0.69-0.73** in the swap arm. Per brief section 4 a percentile
   read against an unmatched null is **not this row's percentile**, and
   addendum 6 section VI measures that the null's width moves with
   `1/sqrt(n)` so the bar itself moves. **No percentile is published and none is
   read.** `null p50` is reproduced in the receipts because the tool prints it,
   and `0.000` is read as "did not calibrate", never as "the median does not
   profit".
2. **Intrabar excursion.** `max_drawdown_*` walks the **closed-trade** curve;
   an open position's excursion is not in it. `avg_mae` is the only field that
   sees it and is printed, unconverted.
3. **Anything but `xauduka` 15m.** One instrument, one bar size. Brief section 0
   item 3 measured that the **sign** of an edge can be a property of the 15m
   ruler (`+0.114R -> -0.065R -> -0.043R` on the same entries at three bar
   sizes). So every number here is **a 15m number**, and the ranking is a
   ranking **on this ruler**. Not re-measured at another bar size; saying so is
   the honest form.
4. **Parameters.** Defaults only. A mechanism that would reach the ceiling at
   some other parameter is **not measured** by this plan, and a sweep over
   parameters is exactly the ~6,000-cell shape that returned 0.
5. **Drawdown as a criterion.** It is **reported** beside every quoted row and
   **ranked on by nothing**. No drawdown threshold is offered.
6. The two counted-not-repaired defects of brief section 7 (`wrong_side_stop`,
   `check_exit` pricing a gapped stop at `bar.open`). Several default rows DO
   enforce engine stops, so unlike `rollover-flat` these may be exercised here.
   **`wrong_side_stop` non-zero on a row means that row is unreadable and is
   reported as such**, not repaired.
7. **`data-sealed/`.** Not opened, read, pointed at or counted.
8. **Live spread at the session break.** This feed is a flat 0.28 at every hour;
   a real venue widens it. Every `spread/gross` here is a **lower bound on the
   real share**, and that asymmetry is against the hypothesis.

## 10. Settings, frozen

    --mode=hypotheses --fixed --exit-mix --null-sides=exposure --seeds=200
    --interval=15m --data=/e/rust/flowdesk/data --market=xauduka
    --batch-file=/e/rust/fd-gross-ceiling/docs/research/designs/2026-10-08-gross-ceiling.toml
    --config=/e/rust/fd-rollover-flat/config        (arms A, C)
    --config=/e/rust/fd-rollover-flat/config-swap   (arms A-swap, C-swap)
    --guards                                        (arms C, C-swap only)

Binary: **`/e/rust/fd-drawdown/target/release/search.exe`**, built 2026-10-07
18:08 from `agent/drawdown` at `eac9812`. `git diff --stat eac9812 fd0e718 --
crates/ src/ Cargo.toml Cargo.lock` is **empty**, so this worktree's code and
that binary are the same program and **nothing is built** (brief section 1: if
the job only needs to run, do not build). It is the only shared binary carrying
**both** `expectancy_net` and the drawdown line, which this plan needs both of.

`--samples=`, `--direction-samples=`, `--rebate-share=`, `--trail=`,
`--spread=`, `--params=`, `--filters=`, `--strategy=`, `--batch=`,
`--companion=`, `--null-registered-stop` are **not passed**: none is read by
`--mode=hypotheses` for this plan. Every receipt must reproduce the
`flags: N passed, every one of them read by --mode=hypotheses` line, and the
`news:` source line is reproduced in the report because the engine reads
`data/news/events.parquet` from the default `data/` whatever `--data=` says.

Not touched: `config/accounts.toml` outside this worktree, `config/local.toml`,
the VPS 103.19.29.194, `main`, `data/gold/`, `data/btc/`,
`/e/rust/flowdesk/target/`, and the two `collect.exe` processes (pids 5044 and
38720), which are not signalled. `df -h /e` before the first run: **27 GB
available**.

## 11. How to read the result

One number is the result: **`max over 120 cells of min(IS, OOS)
gross_per_session`**, with the mechanism that holds it named, its
`trips_per_session`, its `spread/gross`, its drawdown in USD, its exits mix and
its post-drift residual beside it.

* **under +0.050 R** - F1 fires. The ceiling of this data is below half of what
  the frame needs. The desk stops looking for a better mechanism on this
  instrument at this bar size and looks at **cost** or at **a different
  market**. This is a usable answer.
* **+0.050 to +0.100 R** - the frame's requirement is within one factor of 2.
  Reported as a measured distance, with the max's selection bias stated.
* **>= +0.100 R on both windows** - reported with drawdown and residual, and
  **handed to the owner**, not proposed.

## 12. Amendments
