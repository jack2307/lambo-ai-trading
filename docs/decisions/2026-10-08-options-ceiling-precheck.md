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
