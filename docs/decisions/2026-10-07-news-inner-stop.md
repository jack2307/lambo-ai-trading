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
