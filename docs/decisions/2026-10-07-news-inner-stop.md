# A stop INSIDE the release range

**Date:** 2026-10-07 (pre-registration; written and committed before any other
file in this branch changed)
**Branch:** `agent/news-inner-stop`, cut from `agent/instr-repair`
**Parents:**
- `docs/decisions/2026-10-06-news-entry.md` (`agent/n1`) — spelled the family,
  published one survivor.
- `docs/decisions/2026-10-07-news-instrument-timeframe.md` (`agent/news-tf`) —
  refuted that survivor on a second feed and a second ruler, and **corrected a
  falsifier the brief had written wrongly**. This file exists because of that
  correction and nothing else.

**Outcome:** open. This file is the registration, not the result.

## 0. Why there is anything left to measure here

The calendar-entry family has been closed twice over and a third time sideways:

- `agent/n1`'s `np-brk-p4-m15` passed the gate on A' and B' at 15m.
- `agent/news-geom` fired F2 on it: the direction was inseparable from a coin
  flip once the 4-hour `max_hold_ms` ceiling — which closed 77-93% of its
  trades — was read as the mechanism.
- `agent/news-tf` fired F1, F2 and F3 on it: it dies on the second half of the
  broker's own feed (PF 2.226 to 1.125), and **the sign of its edge is a
  property of the 15m ruler** (B': +0.114R at 15m, -0.065R at 5m, -0.043R at
  1m).
- `agent/news-pre` closed the pre-release branch at 0 cells.

**This registration does not re-run any of that.** It exists because
`agent/news-tf`'s closing section corrected its own F3 in a way that left one
door open, in its words:

> F3's premise needs one correction ... The brief asked whether the
> post-release move runs "several times its own stop". It **cannot**, by
> construction: this rule's stop *is* the move's own range, so at entry the
> move and the stop are the same size and the rule needs a *further* 1.8x move.
> The hypothesis "a 6x range event is a tradable sequence of 1m bars" is
> therefore not refuted in general — what is refuted is that *this* rule, whose
> invalidation distance is the release range itself, can take it. **A rule whose
> stop sits *inside* the release range is a different registration and is not
> measured here.**

That is the whole of this job: the same anchor, the same minute, the same null,
and the stop moved **inside** the release range.

## 1. Hypothesis — one sentence

If the scheduled release expansion carries tradable structure at all, it is
reachable by an entry whose invalidation distance is a **fraction** of the
release range rather than the whole of it, because such a rule needs a further
move of `1.8 x f x range` instead of `1.8 x range` to reach its target.

## 2. The geometry, and the trade-off declared before it is measured

`news-pulse` already carries the parameter: `stopImpulse = f`, and the stop is
`f x` the impulse window's own range beyond the signal close. **Every published
row of this family — all 12 of `agent/n1`'s and all 32 of `agent/news-tf`'s —
set `f = 1.0` and none varied it.** So the mechanism `news-tf` named as
unmeasured needs **no new strategy code**: it is one already-registered
parameter that nobody has moved. That is stated here rather than discovered
later, because it is also the reason this job can be run without touching an
engine file.

The arithmetic of moving it, with `[trading] reward_risk = 1.8`:

| f | stop | further move to target | retracement that stops it out |
|---|---|---|---|
| 1.00 (published) | 1.00 x range | **1.80 x range** | a full range back |
| 0.75 | 0.75 x range | 1.35 x range | three quarters back |
| 0.50 | 0.50 x range | 0.90 x range | half back |
| 0.25 | 0.25 x range | **0.45 x range** | a quarter back |

So the target comes **4x closer** and the stop gets **4x easier to touch**.
Both directions of that trade are real and measuring which one wins is the
content of the job. Two consequences are declared now so that neither can be
presented later as a finding:

- **Cost/R rises in exact proportion.** `cost/R = spread / stop`, so a quarter
  stop costs four times as much per R. Section 4(f) puts numbers on it, measured.
- **Stop share rises and timeout share falls.** Section 4(e) puts numbers on
  that too.

## 3. Rows — fixed, declared, no grid

`news-pulse` declares no `grid()`; every run uses `--fixed`.

    stopImpulse  f in {0.25, 0.50, 0.75}   the inner-stop family
    stopImpulse  f = 1.00                  the published control, re-run here
    probeBars    p in {1, 2}
    mode         m in {0 with the impulse, 1 against it}
    held fixed:  minMoveAtr 0.5, minImpact 3, atrPeriod 14, maxLagMin 15

= **16 rows per interval**, of which 12 are the new family and 4 are the
control. `minMoveAtr` is pinned at 0.5 and not swept: `agent/news-tf` measured
that 0.5 against 1.5 moves no sign on this family, and at a one-bar gate 0.5 is
the value that keeps the trade count highest (section 4(c): 112-122 of about
123 admitted events pass it). Pinning it is what keeps 16 rows from being 32.

`mode` 0 **and** 1 are both run on every row. `agent/news-tf`'s verdict was
that the parent "dies on direction", so a job that measured one direction and
reported it would be choosing the sign after the fact.

Each row carries the `newsonly:` gate that admits **exactly** the one bar its
own `probeBars` fires on at that interval:

| interval | p = 1 | p = 2 |
|---|---|---|
| 1m | `newsonly:0/1:3:USD` | `newsonly:1/2:3:USD` |
| 5m | `newsonly:0/5:3:USD` | `newsonly:5/10:3:USD` |

That gate is what makes the control a **timing** match: `hypotheses.rs` wraps
the row's own filter around `RandomEntry`, so the null draws from the same
single bar after the same release. This is brief section 4(v) and it is the only
reason a percentile is quotable at all here.

## 4. Measured BEFORE this file was committed, and disclosed in full

Read this session, on the live store, read-only. No profit factor and no
expectancy from `search.exe` is among them — but 4(e) **does** carry an exit mix
from an independent Python walk of the tape, and the R arithmetic implied by
that mix is visible to anyone who multiplies it out. It is disclosed here,
before the registration was committed, rather than presented later as a
discovery. Scripts: `scripts/nis_precheck.py`, `scripts/nis_resolution.py`,
`scripts/nis_wrongside.py`.

**(a) Stores.** `XAUDUKA-1m` 5,635,777 rows 2010-06-01 00:00 to 2026-05-31
23:59; `XAUDUKA-5m` 1,135,389 rows to 23:55; `XAUDUKA-15m` 378,749 rows to
23:45. Reproduces `agent/news-tf` section 2 exactly. All three `XAUUSD` stores
are truncated near 100,000 rows (that agent's finding), so **no broker-feed
store below 15m reaches these windows** and both arms here are `XAUDUKA`. That
is a store limit, not a choice.

**(b) Calendar.** `data/news/events.parquet`, 747 rows, 562 with
`currency == USD and impact >= 3`, 2010-01-08 to 2027-12-08. A'
(2022-01-01 to 2026-01-01) **127** events, B' (2018-01-01 to 2022-01-01)
**134**.

**(c) Anchoring.** With `maxLagMin = 15`, **123 of 127** A' events and **130 of
134** B' events anchor on a contiguous bar at both 1m and 5m; the 4 missing in
each window are releases that fell in a gap. Of those, the 0.5-ATR gate admits
**120 / 119** (1m, p = 1 / p = 2) on A', **122 / 120** on B', **112 / 112** on
A' at 5m and **113 / 115** on B' at 5m. So every row is expected to take about
110-122 trades, well clear of the 40-trade floor.

**(d) The stop in ATR — brief section 2's mandatory question, answered in
advance.** The release window's range, in units of the **pre-release ATR(14) of
the backtest's own bars**, median over anchored events:

| interval | window | p | range (points) | range (ATR) | ATR (points) |
|---|---|---|---|---|---|
| 1m | A' | 1 | 10.21 | **11.13** | 0.845 |
| 1m | A' | 2 | 10.92 | 13.44 | 0.845 |
| 1m | B' | 1 | 4.50 | **7.79** | 0.549 |
| 1m | B' | 2 | 5.26 | 9.29 | 0.549 |
| 5m | A' | 1 | 12.80 | 7.99 | 1.526 |
| 5m | A' | 2 | 14.89 | 9.36 | 1.526 |
| 5m | B' | 1 | 6.77 | **6.47** | 1.129 |
| 5m | B' | 2 | 7.49 | 7.69 | 1.129 |

So the stop this job actually runs, `f x range`, in ATR of its own bars:

| f | 1m A' p1 | 1m B' p1 | 5m A' p1 | 5m B' p1 | resolution band |
|---|---|---|---|---|---|
| 0.25 | 2.78 ATR | 1.95 ATR | 2.00 ATR | **1.62 ATR** | readable |
| 0.50 | 5.57 | 3.90 | 4.00 | 3.23 | readable |
| 0.75 | 8.35 | 5.84 | 6.00 | 4.85 | readable |

**The smallest stop anywhere in the declared grid is 1.616 ATR.** Against
`docs/decisions/2026-10-07-intrabar-resolution.md`'s table that sits between its
1.500 row (0.14% disagreement) and its 2.000 row (0.08%) — **readable**, and
nowhere near the 0.600 ATR line, let alone the 0.300 ATR "NOT EVIDENCE" line.
The reason is structural and is the same fact the parents measured: the release
bar's range is 6-13x its own pre-release ATR, so a **quarter** of it is still a
wide stop in ATR. **Brief section 2's declared main risk for this job does not
bind, and 4(e) measures the thing the table stands in for directly.**

**(e) Resolution and the two defects, measured directly rather than looked up.**
An independent Python walk of the tape, applying the engine's own rules (fill at
the next bar's open plus or minus half the 0.28 spread, `reward_risk` 1.8,
4-hour cap, stop-before-target when a bar covers both), over all 16 rows times
2 windows times 2 intervals:

- **`wrong_side_stop`: 0 everywhere.** The feared consequence of a tight stop —
  the fill bar opening past it, so the position opens with its stop on the
  profitable side — happens **0 times in 192 (window x probe x f x mode)
  pre-check cells**, including every f = 0.25 cell at 1m. On this feed the
  1m-to-1m open gap after a release never reaches a quarter of the release
  range. Brief section 7's first defect is therefore expected to print 0; if a
  `search.exe` row prints it non-zero, that row is reported unreadable (F4).
- **Both-touched on the backtest's own bars: 0.0% to 2.7%**, worst at f = 0.25.
- **5m verdict against 1m verdict: 0.0% to 0.9% disagreement**, every cell
  inside the resolution record's "readable" band of 1.0% or less. The 1m arm
  cannot be audited finer than 1m on this disk, so for it the figure reported is
  the both-touched share (0.0% to 1.7%) labelled *unresolved at 1m*, with the
  one-sided direction stated: the engine books a TARGET only on a bar that never
  touched the stop, so the error can only **understate** a positive result
  (0 fake wins in 4,277,568 adjudications, same record).
- **The 4-hour ceiling, which was F5 on every parent row, is dissolved by the
  geometry.** TIMEOUT share, 1m A' p1 breakout, from the same walk:

| f | STOP | TARGET | TIMEOUT | own-rule share |
|---|---|---|---|---|
| 1.00 (published) | 68 | 23 | 29 | 91 of 120 = 76% |
| 0.50 | 71 | 43 | 6 | 114 of 120 = **95%** |
| 0.25 | 73 | 47 | **0** | 120 of 120 = **100%** |

At f = 0.25 the registered geometry decides **every** trade. This family is the
first configuration in this axis where F6 cannot fire, and that is a consequence
of the design rather than a result.

**(f) Cost, where the brief's number does not carry — brief section 8's rule
applies.** The brief states this family's cost range is 0.85% to 5.62% of R and
concludes "cost is not what blocks; the old rule died on DIRECTION". Both halves
are `agent/news-tf`'s measurements at **f = 1.0** and neither survives the move
inside the range. `cost/R = spread / stop` and the stop is being cut by up to
4x, so predicted from the 4(d) medians at the configured 0.28 spread:

| f | 1m A' p1 | 1m B' p1 | 5m A' p1 | 5m B' p1 |
|---|---|---|---|---|
| 1.00 | 2.74% | 6.22% | 2.19% | 4.13% |
| 0.50 | 5.48% | 12.43% | 4.38% | 8.27% |
| 0.25 | **10.97%** | **24.86%** | 8.75% | 16.54% |

**So for the inner-stop family cost IS a candidate blocker, and on B' at 1m with
f = 0.25 the spread eats a quarter of R.** That is the price of the geometry and
it is declared before the run, not discovered after it. Every figure published
will quote the row's own printed
`cost-matched null: control stop <x> ATR = <y> points, cost <z>% of R` line with
the stop in points beside it; no number in this table is used in place of a
printed one.

Because cost is now a live suspect rather than a settled non-issue, a
**zero-spread diagnostic arm** is declared in section 8 — the only way to tell
"this geometry has no direction" from "this geometry has direction and pays all
of it to the spread", which is exactly what the falsifier needs to be clean.

## 5. Windows — the parents', unchanged

**A' 2022-01-01 to 2026-01-01** (127 events) and **B' 2018-01-01 to 2022-01-01**
(134 events), non-overlapping, exactly as `agent/n1` and `agent/news-tf` used
them. Nothing is re-split and no window is introduced, so `stopImpulse` and the
bar size are the only things that differ from published rows.

Brief section 2 requires 1m or 5m and forbids 15m, because at 15m the first bar
covers both an inner stop and its target and the engine takes the STOP. Both 1m
and 5m are run. **No 15m cell is declared or will be run.**

## 6. Gate — not adjustable

    profit factor >= 1.200  AND  expectancy >= +0.050R  AND  >= 40 trades
    on BOTH A' and B', in the SAME guard arm

Counted by hand off the receipt, because the tool's own verdict checks a
30-trade floor while the desk's is 40 (brief section 4). A percentile is not a
gate. `null p50` is printed beside every percentile, and no percentile is
published for a row whose count match falls outside **0.25** of 1.00 (brief
section 4). `--exit-mix` on every run.

## 7. The guard arm, declared to produce zeros

`--guards` prints `news flat 60/30 (impact>=3, USD)`: no entry from 60 minutes
before to 30 minutes after a high-impact USD release. **Every entry minute in
this job (+0 to +10) is inside that blackout**, so the guarded arm is expected to
take **0 trades on all 64 guarded cells**, exactly as `agent/news-tf` measured on
its 48 guarded short-probe cells (`refused NEWS_FLAT 89` to `123` per row). It is
run and reported anyway (brief section 4(iii)); `[trading.guards]` is not touched
and nothing is worked around. **Zero trades there is a fact about the desk's own
configuration, not an absence of signal**, and it will be written that way.

## 8. Multiple testing — declared BEFORE the first run

| block | cells (run x row) | gate? |
|---|---|---|
| plumbing: 1m, A', 16 rows, `--seeds=20`, no guards — flag and filter check, numbers NOT read at the gate | 16 | no |
| 1m: 16 rows x {A', B'} x {no guards, guards} | 64 | **yes** |
| 5m: 16 rows x {A', B'} x {no guards, guards} | 64 | **yes** |
| zero-spread diagnostic: 1m, 16 rows x {A', B'}, no guards, `--spread=0.0` | 32 | no |
| **declared total** | **176** | |

The plumbing block is declared because the binary question is live: `agent/n1`'s
`Filter::NewsOnly` and `news-pulse` are **not** on `agent/instr-repair`, and
brief section 1 records that an older binary **swallows an unknown flag
silently** (runs, exits 0, prints nothing). So this job builds its own binary in
its own target directory and the plumbing run is where the filter is proved to
bite before any gate number is read.

No `--mode=rescore`, no `--null-sides=exposure`, no `--trail`, no spread fan
beyond the single 0.0 diagnostic, no third window, no parameter outside the
declared 16 rows. The real `(run, row)` count is published at the end against the
176. Anything looked at beyond it goes in a dated note **appended below**, never
by editing a line above.

## 9. Falsifiers — each fires on a printed line

1. **F1 — the brief's own, and the one this job exists to fire.** If **no** cell
   of the inner-stop family (f in {0.25, 0.50, 0.75}) passes the gate on both A'
   and B' at either interval, then the scheduled expansion contains no tradable
   structure **at any stop geometry**: the whole-range stop is refuted
   (`news-tf`), the pre-release branch is refuted (`news-pre`), the direction is
   a coin flip at the ceiling (`news-geom`), and the inner stop is refuted here.
   **The calendar mechanism family closes completely.**
2. **F2 — cost, not direction.** If the zero-spread arm (section 8) puts an
   inner-stop row through both gate legs while the 0.28-spread arm of the same
   row fails, the geometry is refuted **on cost** and not on direction, and that
   is a different sentence from F1. It is a diagnostic, not a pass: no cell of
   the zero-spread arm is a gate cell.
3. **F3 — the control.** If f = 1.00 re-run in this binary does not reproduce
   `agent/news-tf`'s published figures for the same rows and windows within
   reading tolerance, the comparison in this file is invalid and every number
   here is withdrawn until the difference is explained.
4. **F4 — void, not fail.** A row is reported **void** rather than failed when it
   takes fewer than 40 trades, when its count match falls outside 0.25, when
   `wrong_side_stop` prints non-zero, or when its own exits never fire.
5. **F5 — resolution.** If a row's printed realised stop falls below **0.714
   ATR** (the resolution record's threshold for a mechanism entering on expansion
   bars), that row's verdict mix is an **indicator, not evidence**, and is
   written that way. Section 4(d) predicts this will not fire; it is declared so
   that the prediction is falsifiable. Because the error is one-sided (0 fake
   wins), a **positive** result below the threshold is still believable in that
   direction and a negative one is not weakened by it.
6. **F6 — the ceiling.** A row whose own STOP+TARGET share is under 20% is
   measuring `max_hold_ms = 4 h` rather than the rule (brief section 6a).
   Section 4(e) predicts this cannot fire at f of 0.50 or less; declared so the
   prediction can fail.

## 10. What this will NOT say

- **Nothing here is a fill model.** A market order into the minute after a
  release is the one thing this family most needs and cannot have: the engine
  fills at the next bar's open at one constant spread. The zero-spread arm is a
  bound, not a model.
- Nothing here measures a per-second or per-minute spread at a release. The
  parent measured p50 0.250 / max 0.260 at +45 to +60 minutes off the live
  account; **the first minutes after a release are not covered by that log**, and
  the real spread there is almost certainly wider than 0.28, which makes 4(f)'s
  table a **floor** on the cost of this geometry.
- Nothing here measures a broker-feed sub-15m store, because 4(a) shows none
  exists at a readable length.
- Nothing here re-opens the whole-range rule, the pre-release branch, the 4-hour
  ceiling as an axis, or the second instrument.
- `events-extended.parquet` is not used; the `news:` line of every receipt is
  quoted to prove which calendar was read.
- `data-sealed/` is not opened, read or counted. `config/accounts.toml`,
  `config/local.toml`, the VPS and `main` are not touched.
  `--data=/e/rust/flowdesk/data` is read-only.

## 11. Signed

Written by the `news-inner-stop` agent (Claude Opus 5, 1M context) on
`agent/news-inner-stop`, 2026-10-07, **before** `agent/n1`'s filter and strategy
were brought into this worktree and before any binary was built here.
`df -h /e` at the start of the job: **23 GB free** of 301 G.

---

## Note added 2026-10-07, results. Append-only; no line above is altered.

Receipts: `receipts/nis_{1m,5m}_{A,B}_{noguards,guards}.txt` (the gate),
`receipts/nis_1m_{A,B}_spread0.txt` (the diagnostic),
`receipts/nis_plumb_1m.txt` (the declared plumbing cell),
`receipts/nis_table.txt` (all 176 cells, one line each),
`receipts/nis_runlog.txt`, `receipts/nis-{1m,5m}.toml` (the 32 declared rows).
Code: `scripts/nis_runs.sh`, `scripts/nis_table.py`, and the four pre-check
scripts. No engine file was touched: the only Rust in this branch is
`agent/n1`'s commit `ca9125c`, cherry-picked byte for byte.

### 0. The gate: 0 of 128 cells pass. F1 FIRED.

**No row of the inner-stop family — and no row of the f = 1.00 control — reaches
`PF >= 1.200` on both A' and B' in the same guard arm, at either interval.**
32 rows, 0 survivors. Of the 64 unguarded gate cells, 3 pass both legs on one
window and every one of the 3 inverts on the other:

| row | A' | B' |
|---|---|---|
| `nis5m-rev-p2-f075` | PF **1.467** / **+0.216R** / 112 | PF **0.652** / **-0.204R** / 115 |
| `nis5m-rev-p2-f050` | PF 1.291 / +0.167R / 112 | PF 0.900 / -0.056R / 115 |
| `nis5m-rev-p2-f100` | PF 1.298 / +0.124R / 112 | PF 0.601 / -0.207R / 115 |
| `nis1m-brk-p2-f025` (best 1m on A') | PF 1.170 / +0.117R / 119 | PF **0.610** / **-0.297R** / 120 |
| `nis1m-brk-p1-f075` (best 1m on B') | PF 0.786 / -0.128R / 120 | PF 1.106 / +0.073R / 122 |

Every window carries 112-122 trades, so none of this is a small-sample void:
the minimum trade count in the whole unguarded set is **112**, nearly three
times the 40-trade floor. Counted by hand against 40, not the tool's 30.

### 1. The number worth keeping: the stop fraction is a property of the MEASUREMENT

This is the finding, and it is an eighth entry for the brief's own section 0
list. **`corr(PF_A', PF_B')` is negative at both intervals** — `-0.228` over the
16 rows at 1m and `-0.438` over the 16 at 5m — which falls inside the
`-0.672 ... -0.140` range the brief reports for its six existing groups. And the
**sign of expectancy flips between the two windows on 16 of 32 rows: exactly
half.**

The profile of profit factor against f has a *different shape* on each window,
and the best f is not the same one:

| branch | A' (f = 0.25 / 0.50 / 0.75 / 1.00) | B' (same) |
|---|---|---|
| 1m breakout, probe 2 | **1.170** / 0.876 / 0.923 / 1.025 | 0.610 / 0.790 / **0.818** / 0.786 |
| 1m breakout, probe 1 | 1.045 / **1.075** / 0.786 / 0.766 | 0.788 / 0.948 / **1.106** / 0.849 |
| 5m reversion, probe 2 | 0.788 / 1.291 / **1.467** / 1.298 | 0.749 / **0.900** / 0.652 / 0.601 |
| 1m reversion, probe 1 | 0.715 / 0.895 / 0.963 / **1.005** | 0.680 / **1.034** / 0.884 / 0.876 |

So `stopImpulse` joins window, clock, ruler, guard arm, bar resolution,
swap-blindness and the null declaration: **a parameter whose optimum, and whose
sign, is a property of which four years were measured.** Picking f on A' and
reading it on B' loses money on three of these four branches.

### 2. F2 did NOT fire, and the reason is the sharper half of the answer

The zero-spread arm was declared precisely so that "no direction" could not be
confused with "direction paid to the spread". **At zero spread, still 0 of 16
rows pass the gate on both windows.** Five rows pass on one window and none on
two; the closest is `nis1m-brk-p1-f025` at PF 1.228 / +0.143R on A' and PF
**1.150** / +0.102R on B' — positive on both windows, and short of the profit
factor leg on B' by 0.050. **So the inner-stop geometry is not refuted on cost.
It is refuted on direction, with cost making it worse.**

But cost decides the *sign*, and that is measured rather than argued. The same
row, the same 120 and 122 entries, only the spread changing:

| `nis1m-brk-p1-f025` | printed cost/R | spread 0.28 | spread 0.00 | difference |
|---|---|---|---|---|
| A' | 10.49% of R | **+0.038R** | +0.143R | 0.105R |
| B' | **20.74% of R** | **-0.145R** | **+0.102R** | 0.247R |

The one row that is positive on **both** windows at zero cost is **negative on
B' at the desk's own 0.28 spread**, and the amount it loses is about its own
printed cost. The difference between the arms (0.105R and 0.247R) is a little
larger than the printed `spread / stop` (10.49% and 20.74%) because removing the
spread also moves the stop/target mix, so the two are not purely additive; both
are quoted rather than reconciled.

**Brief section 3 item 3 is therefore half right for this family, and the
correction is mine to report (brief section 8).** "Cost is not what blocks"
holds at f = 1.00, where the printed figures are 1.82% to 3.46% of R. It does
**not** hold at f = 0.25, where the printed figures are **9.46% to 21.71% of R**
— because cutting the stop to a quarter of the release range multiplies
`spread / stop` by four, and because B'-era gold at a 1,200 to 1,800 dollar
price level has a release range less than half of A'-era gold's (4.50 points
against 10.21 at 1m, section 4(d)). The brief's quoted band of 0.85% to 5.62% is
`agent/news-tf`'s band at f = 1.00 and does not survive the move inside the
range.

### 3. What the inner stop DID deliver: the four-hour ceiling is gone, and the rule still fails

This is the part that closes the family rather than merely adding to it.
`agent/news-geom` fired F2 on the published survivor because 77% to 93% of its
trades were closed by `max_hold_ms = 4 h`, so what was measured was "hold four
hours in a direction" and not the registered stop geometry. **The inner stop
removes that confound by construction**, and the receipts show it:

| f | own-rule share (STOP+TARGET) | mean hold |
|---|---|---|
| 1.00 (the published geometry) | 51% to 76% | hours |
| 0.75 | 70% to 93% | — |
| 0.50 | 91% to 99% | — |
| 0.25 | **99% to 100%** | **8.2 min** (1m A' probe 2) |

At f = 0.25 the registered rule decides **every trade in every cell**: 0 to 1
TIMEOUT out of 112 to 122. **F6 could not fire anywhere** — the minimum own-rule
share across all 176 cells is 51%, against the 20% line. So the calendar family
has now been read in the one configuration where neither the hold ceiling nor the
bar ruler can be blamed, and it fails there too: at f = 0.25, where the geometry
is 100% of the result, the 1m breakout branch reads PF 1.045 and 1.170 on A' and
PF **0.610 and 0.788** on B'.

### 4. The measuring instrument worked, on every count it was asked about

- **F5, resolution, did NOT fire.** The smallest printed realised stop across all
  176 cells is **1.261 ATR**; the largest is 6.907 ATR. The threshold for a
  mechanism entering on expansion bars is 0.714 ATR, and the "not evidence" line
  is 0.300 ATR. **Every cell of this job sits in the readable band**, and section
  4(d) said so before the first run. Independently measured on the tape:
  both-touched on the backtest's own bars 0.0% to 2.7%, and the 5m verdict
  against the 1m verdict **0.0% to 0.9%**, inside the record's 1.0% readable
  line. **So these results are evidence, not an indicator** — which is the one
  thing brief section 2 warned this job would most likely have to concede.
- **The null calibrates everywhere.** Count match across all 112 non-guarded
  cells runs **0.82 to 1.15**, every one inside `COUNT_MATCH_BAND = 0.25`. So
  unlike `agent/news-tf`'s `xauusd` arm (0.03 to 0.04) and the parent's B'
  (0.38), **every percentile in this job is publishable** — and they are reported
  beside `null p50`, which is below 1.000 on 104 of 112 cells, meaning a high
  percentile here says "loses less than random entry at the same minute, count,
  cost and side ratio", not "makes money".
- **`null p50` printed `0.000` on 0 of 176 cells.** Brief section 4's defect did
  not occur here; the guarded cells print `nan`, not `0`, which is the correct
  behaviour.
- **`wrong_side_stop`: 0.** No such counter exists in this branch's engine, so it
  was measured directly instead: 0 occurrences in 192 pre-check cells, including
  every f = 0.25 cell. On this feed the 1m-to-1m open gap after a release never
  reaches a quarter of the release range. F4 did not fire on that ground.
- **F3, the control, reproduced exactly.** The f = 1.00 rows in this binary read
  120 / PF 0.766 / -0.132R, 120 / 1.005 / +0.010R and 119 / 1.025 / +0.019R —
  identical to three decimals to `agent/news-tf`'s `np1m-brk-p1-m05`,
  `np1m-rev-p1-m05` and `np1m-brk-p2-m05` on the same window. So the only thing
  that differs between this job and the published record is `stopImpulse`.
- **The flag audit answered the brief's binary warning.** Every receipt prints
  `flags: 9 passed` (or 10 with `--guards`), `every one of them read by
  --mode=hypotheses`. Nothing was swallowed silently, including `--spread=0.0`,
  whose arm prints `spread: 0 per round trip`.
- **An independent reimplementation agrees with the engine.** The Python walk in
  `scripts/nis_resolution.py` applies the engine's documented fill and exit rules
  and reproduces the printed exit mix **exactly** on 7 of 8 cross-checked cells
  (`73/47/0`, `71/43/6`, `68/23/29`, `69/49/1`, `79/43/0`, `72/39/1`, `51/15/46`)
  and is off by **one trade** on the eighth (predicted `73/28/11`, printed
  `74/28/10`). The engine's exit accounting is therefore reproducible from its
  own description.
- **One pre-check number of mine was wrong and the printed one wins.** Section
  4(d) predicted the f = 0.25 stop at 1m A' probe 1 as 2.78 ATR; the receipt
  prints **1.823 ATR**. The stop in *points* agrees (2.55 predicted, 2.67
  printed), so the whole gap is the ATR denominator: my pre-check normalised by
  the **pre-release** ATR(14) and the engine normalises by an ATR that already
  contains the release bar, which is roughly twice as large. The printed figure
  is the conservative one and both clear 0.714 ATR, so the verdict of section
  4(d) stands — but the arithmetic behind it was mine and it was off by a factor
  of about two.

### 5. The guard arm: 0 trades on all 64 cells, as declared

`--guards` prints `news flat 60/30 (impact>=3, USD)` and refused every entry:
`refused NEWS_FLAT 112` to `122` per row, `closed none`, `sized down 0`, on all
16 rows of all 4 guarded runs. **This is a fact about the desk's own
configuration, not an absence of signal**: the desk is configured so that it
cannot take any entry this family describes, at any stop geometry, at either
interval. Declared in section 7 before it ran, run anyway, and
`[trading.guards]` was not touched.

### 6. One tool observation, free and exact

At `reward_risk = 1.8` with exits that are purely STOP or TARGET — which is what
f of 0.50 or less produces here — the desk's two gate legs are not independent,
and the profit factor leg is strictly the harder one. With win rate `w`,
`PF = 1.8w / (1 - w)` and `expectancy = 2.8w - 1`, so

    PF >= 1.200            requires  w >= 40.0%
    expectancy >= +0.050R  requires  w >= 37.5%

The expectancy leg is therefore **redundant** for any such mechanism, and the
gate reduces to "win rate at or above 40%". That is visible on the rows: four
cells in this job clear expectancy and fail profit factor, and none does the
reverse. Reported as an observation about the gate's arithmetic; the gate is not
adjusted.

### 7. Falsifiers

| # | what it tested | fired? | where |
|---|---|---|---|
| F1 | no tradable structure at any stop geometry | **FIRED** | 0 of 128 gate cells pass; 32 rows, 0 survivors on two windows |
| F2 | refuted on cost rather than direction | did **not** fire | 0 of 16 rows pass at zero spread either; best is 1.228 then **1.150** |
| F3 | the control does not reproduce | did not fire | f = 1.00 matches `agent/news-tf` to 3 decimals |
| F4 | void rather than fail | did not fire | min 112 trades, count match 0.82 to 1.15, `wrong_side_stop` 0 |
| F5 | the stop is below the resolution threshold | did not fire | min printed stop **1.261 ATR** against the 0.714 line |
| F6 | the row measures the four-hour ceiling | did not fire | min own-rule share 51%; **99% to 100% at f = 0.25** |

### 8. Multiple-testing ledger

| block | declared | looked at |
|---|---|---|
| plumbing: 1m, A', 16 rows, `--seeds=20`, no guards | 16 | 16 |
| 1m: 16 rows x {A', B'} x {no guards, guards} | 64 | 64 |
| 5m: 16 rows x {A', B'} x {no guards, guards} | 64 | 64 |
| zero-spread diagnostic, no gate | 32 | 32 |
| **total** | **176** | **176** |

Declared and looked at agree exactly; the count is taken off the receipts by
`scripts/nis_table.py`, which prints one line per (run, row) and totals 176. No
`--mode=rescore`, no `--null-sides=exposure`, no `--trail`, no third window, no
spread value other than the configured 0.28 and the declared 0.0, and no
parameter outside the 32 declared rows. The plumbing cell was declared in
section 8 before it ran and no gate figure is taken from it; the only thing read
off it is F3's control reproduction, which is what it was declared to check.

### 9. Outcome — the calendar mechanism family is closed

Four independent branches, four negatives, and this one removes the last defence
the record had:

| branch | verdict | by |
|---|---|---|
| enter BEFORE the release | 0 cells, both windows | `agent/news-pre` |
| enter AFTER it, stop = the whole release range, 15m | passed, then refuted on a second feed and a second ruler | `agent/n1`, `agent/news-tf` |
| the same, read as geometry rather than as a 4-hour hold | direction inseparable from a coin flip | `agent/news-geom` |
| enter AFTER it, **stop INSIDE the release range**, 1m and 5m | **0 of 128 cells, both windows, both intervals, both guard arms** | this file |

**The scheduled expansion is real and it has no direction a stop geometry can
take.** The expansion itself was never in doubt — 5.96x the day's median range,
44% of the first hour's range inside the first minute — and this job settles the
one thing that was still arguable: at the resolution where the registered rule
decides 100% of its own trades, where the matched null calibrates on every cell,
where the stop is 1.3 to 6.9 ATR and therefore readable, and where the four-hour
ceiling cannot contribute, the rule's profit factor on the second window is
**0.610 to 1.106** and its correlation with the first window is **negative**.
There is no stop fraction that survives two windows, and the fraction that looks
best is a property of which four years were used.

The desk's own guards already forbid the whole family. Nothing here asks for that
to change.

### 10. What would reopen this, and what this still does not measure

- **A limit or stop fill model.** The engine fills at the next bar's open at one
  constant spread. A market order into the first minute after a release is the
  one thing this family most needs and cannot have here.
- **A per-second spread series at a release.** Section 10 above said the 0.28
  charged is a floor, and section 2 of this note now shows why that matters: at
  f = 0.25 the result moves 0.105R to 0.247R per trade on the spread term alone,
  so a realistic release-minute spread would move it further in the losing
  direction, never the other way.
- **A second instrument below 15m.** Section 4(a): no broker-feed store exists at
  a readable length. Both arms here are one venue's bid feed.
- **A stop anchored to the range's own levels** (a retracement level, the release
  bar's midpoint) rather than to the signal close. That is a third geometry and
  is not measured here; this file measures a *fraction of the range from the
  signal close*, which is what the brief asked for. It is named so that nobody
  reads this file as having closed geometries it did not run.
- `data-sealed/` was not opened, read or counted. `config/accounts.toml`,
  `config/local.toml`, the VPS and `main` were not touched. No `taskkill` was
  run, `/e/rust/flowdesk/target/release/` was never written or cleaned, and
  `data/gold/` and `data/btc/` were not touched.
  `--data=/e/rust/flowdesk/data` was read-only.

### 11. Disk

`df -h /e`: **23 GB free** before the first run and **23 GB free** after the
last, unchanged. This job built its own binary into its own worktree `target/`
(about 1.5 GB) and wrote about 0.5 MB of text under `receipts/`. The shared
`/e/rust/fd-instr-repair/target/release/search.exe` was not used, because it does
not know `newsonly:` or `news-pulse`, and brief section 1 records that an older
binary swallows an unknown flag in silence. Nothing was deleted.
