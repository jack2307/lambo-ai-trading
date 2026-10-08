# 2026-10-08-options-ceiling-precheck: how many entries can the options-level rule even have

**Written:** 2026-10-08, before the first line of code and before any count was run.
**Agent:** `options-precheck`, branch `agent/options-precheck`, worktree
`/e/rust/fd-options-precheck` (cut from `agent/drawdown`).
**Axis:** gold options flow — the thesis the project was built on.
**Parent registration:** `docs/decisions/2026-10-06-options-thesis-scope.md` §6
(branch `agent/n4`). This document executes that §6 pre-check and nothing else.

## Hypothesis, one sentence

On the only gold option tape that exists, the number of bars at which
`level-reversion`'s entry condition is satisfiable is large enough that the
desk's 40-trade floor is reachable in principle — i.e. the axis is blocked by
calendar, not by arithmetic.

## What is being counted, exactly

`level-reversion`'s entry condition, read off
`crates/fd-strategy/src/builtin.rs:435-462`, with `ctx.position.is_some()`
deliberately ignored:

    exists cluster with cluster.score >= minClusterScore
      and  cluster_distance(bar.close, cluster.low, cluster.high) <= atr * entryAtr

Registered cell: `minClusterScore = 3.0`, `entryAtr = 0.35`, `atrPeriod = 14`
— the strategy's own `default_params()`. Clusters come from the ported engine
(`fd_backtest::timeline::build_timeline` over `data/gold/tape`, `step_ms`
300,000 from `config/default.toml:79`), never re-implemented, so the count is
the engine's own clusters and not a copy of them.

Two numbers per arm, because one bound is not enough:

* **CEILING** — every bar that satisfies the condition. A strict upper bound on
  the trade count, since a position can only remove entries.
* **4h-SPACED** — the same bars, taken greedily with a `max_hold_ms` (4 h,
  `config/default.toml [trading]`) skip after each. The count if every position
  were held to the horizon cap, i.e. the pessimistic end of occupancy.
  The realised trade count lies between the two.

## Falsifier, specific and firable

**F1.** CEILING < 40 on a **tradable** bar series ⇒ the gate's 40-trade leg is
unreachable from this tape and **this axis closes on arithmetic** — no
backtest, no null, no percentile, and nobody re-asks until the collector has
banked months. This is the outcome the parent registration states in advance
as expected.

**F2.** CEILING >= 40 but only on `GC-1m` ⇒ the number is about a series that
cannot be traded (see the pre-check below), so F1 is **not** cleared and the
axis stays shut until a basis-adjusted market exists.

**F3.** `GC-1m` is NOT degenerate (some bar has `high > low`) ⇒ the parent
survey's central blocker is wrong and this whole document is rebuilt on the
corrected fact. Checked first, before anything is counted.

## Pre-check inside the pre-check, run before the ceiling

Count, on `data/bars/GC-1m.parquet`, the bars with
`open == high == low == close`, and the bars with null or zero `volume`. The
parent survey reports 11,813 / 11,813 and "volume 0 on every bar". If the first
number holds, a `GC-1m` bar has no range, cannot cover a stop
(`tests/golden/gold-manifest.json`: "stop wins when a bar covers both stop and
target"), charges no adverse intrabar excursion, and every fill and exit lands
on a close. **A ceiling measured on `GC-1m` is then a count about a series that
is not tradable, and must be reported as such.**

## Arms, and why the basis ones are in

`XAUUSD` is the tradable series and is blocked by `options_source = "none"`
(config, not data). The parent survey measured the blocker: GC − XAUUSD over
6,718 overlapping minutes, mean **+43.70**, sd **1.91**, p10 **41.26**, p90
**45.78**, monotone drift 45.66 → 41.35. A constant offset is a translation of
the price axis and is therefore exactly expressible here; the rolling-vs-constant
question is bounded by running the constant at **p10 / mean / p90** and reading
how far the ceiling moves. No basis-adjusted market is built, no level is
shifted in the engine, no config is edited.

| arm | bars | offset applied to bar prices |
|---|---|---|
| A | `GC-1m` | 0 (levels and bars already on the GC axis) |
| B | `XAUUSD-1m` | +41.26 / +43.70 / +45.78 |
| C | `XAUUSD-5m` | +41.26 / +43.70 / +45.78 |
| D | `XAUUSD-15m` | +41.26 / +43.70 / +45.78 |

## Multiple-testing budget, declared before the first run

**Gate cells: 0.** This axis has no gate (brief §10 item 2) and this job reads
none: no PF, no expectancy, no percentile, no null is produced. Nothing is
spent from the parent registration's 9 declared cells; its 8 compare cells stay
unspent.

Counted rows, declared: 4 arms × offsets (1 + 3 + 3 + 3 = 10 bar/offset
combinations) × the strategy's own declared grid
`minClusterScore ∈ {2, 3, 5}` × `entryAtr ∈ {0.2, 0.35, 0.6}` (9) = **90
counted rows.** The registered cell is `(3, 0.35)`; the other 80 are
**descriptive** and carry no verdict — they exist so that "the ceiling" cannot
be read as a single lucky number. Any widening beyond these is a new
registration.

## How to read it

* **No PF, no expectancy, no percentile, no null, no verdict word.** Brief §V:
  the tape is ~16 days across two disjoint blocks; a distribution statistic off
  it would describe September and October 2026 gold, not a mechanism.
* The gate's 40-trade floor is the desk's (brief §4), **not** the tool's
  `need 30`. Counted by hand against 40.
* A ceiling on `GC-1m` is reported with the words "not tradable" attached every
  time it appears.
* `null` is never printed as `0` (brief §8); a column that is null is reported
  null.
* If F1 fires, the product is the closure. If it does not, the product is a
  pre-registration for the measurement to run when the tape is long enough —
  and the registration says which mechanism goes first, which gate, how many
  cells.

## Effort declared

The parent survey estimates 2–4 hours for this item. If it runs past that,
stop and report rather than overrun.

---

## Note added 2026-10-08, after the run — RESULT: F1 did not fire, the arithmetic does not close the axis

Tool: `crates/fd-backtest/src/bin/options_ceiling.rs`, built from this branch
(`cargo +stable-x86_64-pc-windows-gnu build --release -p fd-backtest --bin
options_ceiling`, target dir inside this worktree). Clusters are the engine's
own, through `fd_backtest::timeline::build_timeline` over
`/e/rust/flowdesk/data/gold/tape`, bound to bars by the same `view_from` cursor
`engine.rs:615` uses. **No strategy ran, no position opened, no exit was
priced. No PF, no expectancy, no percentile, no null is reported below** — the
tape is ~16 days in two disjoint blocks and a distribution statistic off it
would describe September and October 2026 gold.

### F3 first: the degeneracy pre-check holds, and one of its two numbers was wrong

`data/bars/GC-1m.parquet`, read twice (pyarrow, and the tool itself):

    16,903 of 16,903 bars have open == high == low == close   100.00%
    volume: 0 bars zero, 16,903 bars NULL

The parent survey's 11,813 / 11,813 is now **16,903 / 16,903** — the series has
grown with the restarted collector and the share is unchanged at 100.00%. So
F3 does not fire and the blocker stands: a `GC-1m` bar has no range, cannot
cover a stop, charges no adverse intrabar excursion, and every fill and exit
lands on a close.

**One correction to the parent survey, and it is the brief's own rule.** The
survey says "`volume` is 0 on every bar". It is not 0, it is **null**:
`fd_core::types::Bar::volume` is `Option<f64>` and holds `None` on all 16,903
rows. `null` is not `0` — a reader told "volume 0" concludes the feed published
a zero; the truth is it published nothing.

### Tape, as it stood during these runs

    46,897 prints, 9,117 frames (step 300,000 ms), 2026-09-06 22:09 -> 2026-10-08 13:33
    migrated 11,770 / reference-feed 35,039     premium $511.4M
    implied_volatility 100.0% null   bid 100.0% null   ask 100.0% null
    flag_block / flag_sweep / flag_multi_leg / flag_spread: 0 true of 46,897
    underlying == '' on 46,897 of 46,897

The count moved between runs (46,846 -> 46,936) because the collector is
writing while this was measured. Every closure the parent survey put in
writing survives the larger tape unchanged: the gamma wall is still impossible
and venue flags still carry nothing. The calendar has a hole:
**2026-09-17 -> 2026-10-04, 16 days, lost for good.**

### The registered cell, `minClusterScore = 3.0` / `entryAtr = 0.35`

CEILING = every bar where the entry condition is satisfiable. 4h-SPACED = the
same bars taken greedily with a 4 h `max_hold_ms` skip, i.e. the count if every
position were held to the horizon cap. The realised trade count lies between
them. `hours` is `framed` bars x bar length; `cap` is `hours / 4 h`, the number
of 4 h slots the overlap physically contains.

| arm | offset | framed | hours | cap | **CEILING** | 4h-SPACED | % of cap |
|---|---|---|---|---|---|---|---|
| `GC-1m` **NOT TRADABLE** | +0.00 | 16,861 | 281.0 | 71 | **3,764** | 58 | 82% |
| `XAUUSD-1m` | +41.26 | 6,731 | 112.2 | 29 | **725** | 22 | 76% |
| `XAUUSD-1m` | +43.70 | 6,731 | 112.2 | 29 | **691** | 23 | 79% |
| `XAUUSD-1m` | +45.78 | 6,731 | 112.2 | 29 | **687** | 23 | 79% |
| `XAUUSD-5m` | +41.26 | 1,841 | 153.4 | 39 | **362** | 29 | 74% |
| `XAUUSD-5m` | +43.70 | 1,841 | 153.4 | 39 | **327** | 31 | 79% |
| `XAUUSD-5m` | +45.78 | 1,841 | 153.4 | 39 | **335** | 29 | 74% |
| `XAUUSD-15m` | +41.26 | 786 | 196.5 | 50 | **248** | 38 | 76% |
| `XAUUSD-15m` | +43.70 | 786 | 196.5 | 50 | **235** | 39 | 78% |
| `XAUUSD-15m` | +45.78 | 786 | 196.5 | 50 | **229** | 40 | 80% |

**F1 does not fire.** The ceiling is **40 or more on every arm, tradable ones
included**, by a factor of **5.7x (`XAUUSD-15m` at +45.78) to 94x (`GC-1m`)**.
The expected outcome the parent registration stated in advance did not happen:
**the entry opportunity is not what is scarce, and this axis does not close on
arithmetic.**

**F2 does not fire either**, which is the part that matters: the ceiling clears
40 on `XAUUSD-5m` and `XAUUSD-15m` — series with real ranges, 1 degenerate bar
in 101,242 and 1 in 100,586 — so the result is not an artefact of the
rangeless `GC-1m` series.

Full grid (90 counted rows, 9 parameter cells x 10 arms) in
`docs/research/runs/2026-10-08-options-ceiling/`. The ceiling never falls below
40 anywhere in it: the lowest row of all 90 is **137** (`XAUUSD-15m`, +43.70,
`minClusterScore` 5.0, `entryAtr` 0.20), still **3.4x** the floor.

### The number worth keeping: 4h-SPACED is 74-82% of the calendar cap in all ten arms

That band is tight across four different bar lengths and three offsets, and it
says the scarce resource plainly. The occupancy-pessimum trade count is

    4h-SPACED  ~=  0.78 x (market-hours of tape-and-bar overlap) / 4 h

because a qualifying bar is almost always available the moment the book is
free. **The rule is not selective enough to be the constraint; the overlap in
hours is.** Inverting it, at the pessimum:

    40 trades in ONE window   needs ~205 market-hours of overlap
    40 trades in BOTH windows needs ~410 market-hours = ~3.6 weeks of gold session

`XAUUSD-15m` already holds **196.5 h** and already measures **38-40** trades at
the pessimum — one window is at the floor **today**. This is the one number in
the parent survey this run overturns: its §7 item 4 estimates **"3-6 months of
tape"**, derived from the four trade counts (9 / 12 / 3 / 2) in
`docs/research/runs/2026-09-15-trail-exploratory/trail-off.txt`. That file
**has no header** — no `market:`, no `interval:`, no `data:` line — so those
four counts cannot be attributed to a bar series at all, and an estimate built
on them inherits that. Measured against the engine's own clusters, the
requirement is **weeks, not months.**

### What the basis offsets bound

Across +41.26 / +43.70 / +45.78 — p10, mean and p90 of a basis re-measured
here as **mean +43.70, sd 1.91, p10 41.26, p50 43.97, p90 45.78, quartile means
45.66 -> 44.19 -> 43.59 -> 41.35 on 6,718 overlapping minutes**, reproducing the
parent survey to the hundredth — the ceiling moves by **-5.2% to +10.7%** and
the 4h-SPACED count by at most **2 trades**. A constant offset wrong by the
full p10-p90 width therefore changes no verdict here. It would still be wrong
for a measurement: at `entryAtr` 0.35 the tolerance is 0.33 USD on `GC-1m` and
1.58 USD on `XAUUSD-5m`, against a residual sd of 1.91 USD, so a basis error is
larger than the entry tolerance itself on both. **The rolling correction is
needed for the measurement and is not needed for the ceiling** — which is
exactly the boundary the parent survey asked to have drawn.

### The blocker this run found, which neither the brief nor the parent survey has

**The collector is banking tape that has no bars to be measured against, and
restarting it did not fix that.**

    data/gold/tape        2026-09-06 -> 2026-10-08   (collector running, pid 5044)
    data/bars/GC-1m       2026-09-06 -> 2026-10-08   (written with the tape; 100% rangeless)
    data/bars/XAUUSD-1m   2026-06-02 -> 2026-09-11   <- frozen
    data/bars/XAUUSD-5m   2025-04-11 -> 2026-09-15   <- frozen
    data/bars/XAUUSD-15m  2022-06-16 -> 2026-09-17   <- frozen

The October block of the tape — **13,024 prints, 28% of it, 2026-10-04 to
10-08** — has **0 bars** on every tradable series. `collect.exe` writes the
tape and the rangeless `GC-1m` beside it; the tradable series comes from
`py/ingest/mt5_export.py`, which needs the MT5 terminal and is not running.
**Nothing is accumulating tradable bars.** So the 410 market-hours the gate
needs are not being accumulated either, however long the collector runs:
every hour of tape it banks from here is an hour that cannot be measured until
someone exports the matching XAUUSD bars. The export is cheap and reaches back,
so the hours are **recoverable** — unlike the tape — but they are not being
collected now, and the table above is what an agent in six weeks will find if
nobody does it.

### One defect, counted not fixed (brief §7)

`search.exe`'s flag audit validates the flags but **not the mode**. Run with
`--mode=header-only`, a mode no branch has, it printed

    flags:    5 passed, every one of them read by --mode=header-only

and ran nothing. The line is the one the brief added so a receipt says for
itself which flags were read; on an unknown mode it issues a clean bill for a
mode that read nothing at all. Counted here, not fixed: a fix touches the
header of every receipt.

### Multiple-testing ledger: declared 90 counted rows and 0 gate cells, examined exactly that

Declared: 90 counted rows (10 arms x 9 parameter cells), 0 gate cells.
Examined: **90 counted rows, 0 gate cells.** No PF, no expectancy, no
percentile, no null, no verdict word was produced by any of them. The parent
registration's 8 compare cells remain **unspent**. Nothing was loosened after
seeing a number; the 40-trade threshold is the brief's own §4 floor, written
down before the build.

### Carried forward — the measurement this unblocks, pre-registered now

**Not to be run until the overlap reaches ~410 market-hours on a tradable
series.** Registered here so it cannot be written after seeing a number.

* **Which mechanism goes first: `level-reversion` on `XAUUSD-15m`, nothing
  else.** It is the only one of the four whose entry the ceiling was measured
  for; it is the cheapest horizon measured here - **cost/R 2.43% at 1.5 x
  in-window ATR14(15m) = 8.227 USD, spread 0.3**, against 10.82% at 1.5 x
  ATR14(1m) on `XAUUSD-1m` and **21.19% at 1.5 x ATR14(1m) on `GC-1m`**, the
  last of which is worse than silver's 17.32% at the same stop; and its ceiling
  of 235 leaves the most room above 40. The
  other three are a later registration.
* **Which gate: the desk's, unchanged.** PF >= 1.200 and expectancy >= +0.050R
  and >= 40 trades, on **both** windows, counted by hand against 40 and not
  against the tool's `need 30`. Per addendum I the expectancy leg binds only
  when `Lbar < 0.250R`, so the receipt must print `Lbar` and say which leg
  bound. Drawdown in USD beside every profit number (addendum IV); a row
  printing `max_drawdown_pct > 100%` has blown up and its PF is not to be read.
* **How many cells: 8**, which are the parent registration's own unspent 8 —
  1 market-config x 4 strategies x 2 guard arms — and **not one more** until
  the first eight are read. `--exit-mix` on every one, and the method's own
  rule verified to have fired (the `tsmom/120d` trap).
* **The null:** clusters of the same count and width at random strikes on the
  same grid, matched on distance-to-ATR, because the rule enters on a price
  location. **No null is to be built until `count match` can be shown inside
  band**; on this tape it cannot (the brief's own measurement: 69 and 70 events
  giving 1.13-1.17 and 0.03-0.04). Until then the gate is reported alone and
  **no percentile is published.**
* **Prerequisite, and it is the whole thing:** `py/ingest/mt5_export.py` for
  XAUUSD, running or re-run, so the tradable bars keep pace with the tape.
  Without it the overlap stops growing at 196.5 h and this registration can
  never be executed no matter how long the collector runs.
* **Still closed, do not re-ask:** gamma wall (IV and bid and ask 100% null on
  all 46,897 prints), sweep and block by venue flag (0 true on all 46,897),
  and `--market=xauusd` reaching the tape (config, not data).
