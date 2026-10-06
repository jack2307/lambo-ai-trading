# Registration — re-reading the gold-intraday record against a null that carries the drift

**Date:** 2026-10-06
**Branch:** `agent/m10`, cut at `f684b5e`.
**Status:** REGISTRATION ONLY. Committed alone, before the first `search.exe`
call of this axis. No number below has been run.
**Brief:** `/e/rust/AGENT-BRIEF-2026-10-06.md`.
**This axis has no gate to pass.** It is a diagnostic on the measuring
instrument, not a search for a method. Nothing here can produce a candidate.
The gate (`PF >= 1.200 AND expectancy >= +0.050R`, `>= 30` trades) is quoted on
every row only so a reader cannot mistake a percentile for a pass.

## Hypothesis, one sentence

Every percentile this desk has published was read against `--null-sides=coin`,
a 50/50 control that carries no drift; on the `gold-intraday` batch the methods
are **not** 50/50 (one run today printed `rsi-reversion` at a long share of
0.188 and a signed share of time of −0.498 against a control's −0.025), so I
predict that some of those percentiles are the instrument reading gold's
direction rather than the method's mechanism, and will fall when the control is
given the method's own side ratio (`ratio`) and its own signed share of time
(`exposure`).

## What is already measured, and what is therefore new here

`docs/decisions/2026-10-02-drift-null.md` re-read three batches
(`recent-year-hours` 48 rows, `recent-year-sessions` 32, `recent-year-screen`
34) on the window 2025-09-13 → 2026-09-12, plus a 3-row `xauduka` reproduction.
It found the percentile removed on 48 of 48 **fixed-window holds** and
essentially unmoved on the **entry-branch** rows, because those rows were
two-sided (26 of 32 inside 40–60% long).

Not covered there, and the whole of this axis:

- the `gold-intraday` built-in batch has **never** been read at `ratio` or
  `exposure` on any window;
- neither of this brief's two 3-month windows has been read at any setting
  other than `coin`;
- `ratio` has never been run across a batch on the **entry** branch at all —
  the only three `ratio` rows on file are hold-branch reproductions.

The published `gold-intraday` table
(`docs/decisions/2026-09-12-gold-intraday-batch-1.md`, addendum) is on a
**different, four-year window** (2022-06-16 → 2026-09-11, 100,249 bars), not on
windows A or B. So on A and B there is no prior published percentile to
contradict; my `coin` column **is** the record's own instrument applied to
those windows, and the `coin` → `exposure` difference is the measurement of how
much of a percentile the instrument contributes. The four-year published window
is run as a third, clearly-labelled confirmation precisely so the central
question can be put against numbers that *were* published.

## What will be run

Binary: `/e/rust/fd-wt-crt/target/release/search.exe` (built 2026-10-04 15:13
from `agent/crt`). **No `cargo build`.** `--mode=hypotheses
--batch=gold-intraday --market=xauusd --data=/e/rust/flowdesk/data
--config=/e/rust/fd-a10/config`, walk-forward 4 folds, `select_by =
expectancy`, `min_trades_per_cell = 5` — all from `config/default.toml`,
untouched.

| run | window | `--null-sides=` |
|---|---|---|
| A-coin | 2025-07-01 → 2025-10-01 | coin |
| A-ratio | 2025-07-01 → 2025-10-01 | ratio |
| A-exposure | 2025-07-01 → 2025-10-01 | exposure |
| B-coin | 2025-04-01 → 2025-07-01 | coin |
| B-ratio | 2025-04-01 → 2025-07-01 | ratio |
| B-exposure | 2025-04-01 → 2025-07-01 | exposure |
| P-coin / P-ratio / P-exposure (confirmation) | 2022-06-16 → 2026-09-11, the published window | all three |

## Null draw count — declared in advance, with its ladder

The brief names `--samples=` and `--direction-samples=`. Read from
`crates/fd-backtest/src/bin/search.rs`, **neither flag is read in
`--mode=hypotheses`**: `--samples=` is read only by `--mode=null-dir` (line
207) and `--direction-samples=` only by `--mode=rescore` (line 282). The null
draw count in this mode is **`--seeds=`**, default 200 (line 223). I will pass
all three anyway so the receipt records them, and the number that matters is
`--seeds=`.

Declared target: **`--seeds=2000`** on every one of the runs above, so that all
three settings of a window are compared at the same resolution. 200 seeds
resolve 0.5 percentile points and were named as a limitation in
`2026-10-02-drift-null.md` ("what I could NOT fix", item 5); 2000 resolve 0.05.

**Fallback ladder, fixed now so it is not a choice made after seeing results:**
a wall-clock probe (one window, one setting, small seeds, reading *elapsed time
only* — no PF, no percentile, and it is not a look) estimates the matrix cost.
If the six core runs would exceed six hours of wall clock, step down
2000 → 1000 → 500 → 200 and take the largest rung that fits. Whatever rung is
taken is printed beside every table and in the final report. The confirmation
runs on the four-year window may take a lower rung than the core runs if they
are slower; if so, the rung is stated on their table and their numbers are
never compared across rungs with the core ones.

## Multiplicity — the cell count, declared before the first run

A "cell" is one (run, row): one hypothesis read under one null on one window.

- core: 13 hypotheses × 2 windows × 3 nulls = **78 cells**
- confirmation on the published window: 13 × 1 × 3 = **39 cells**
- **declared total: 117 cells.**

Plus the wall-clock probe, which reads no cell. Any cell I look at beyond 117
is reported as an overrun with its count, in the ledger, in the format
`docs/decisions/` asks for (declared vs incurred). A row that takes zero trades
still counts as a cell looked at.

## Falsifiers — what makes me declare this axis dead

1. **The collapse falsifier.** If **no** cell at or above the 95th percentile
   under `coin` falls below the 95th under `exposure`, on either window, then
   the axis found nothing to overturn and I say so: the record's percentiles on
   this batch survive a drift-carrying null. Fires if the count of such cells
   is 0.
2. **The inertness falsifier.** If the median absolute `coin` → `exposure`
   percentile move across all 26 core rows is **below 3 points**, the drift
   control is inert on this batch the way it was inert on `recent-year-sessions`,
   and the axis is dead on this batch regardless of any single row. 2000 seeds
   resolve 0.05 points, so 3 points cannot be seed noise.
3. **The side-match falsifier.** If the control fails to achieve the method's
   side ratio (printed `** the control's side ratio is not the method's **`) on
   a majority of rows under `exposure`, then I did not measure a drift-matched
   null, I measured a broken one, and no row of mine is quotable. This is the
   instrument-failure outcome and it is reported as such, not as a result.
4. **The one-sidedness falsifier.** If every row's signed share of time is
   inside ±0.10 under `exposure`, there is no drift exposure on this batch for
   a control to take away, and the prediction above is simply wrong about the
   batch. (The `rsi-reversion` figure in the brief says this will not fire, but
   that figure is one row of one window and I have not reproduced it yet.)

Falsifier 1 and falsifier 2 can both fire; either one closes the axis.

## How the result is to be read

- **Every percentile is printed with its null's `p50` beside it, always.** The
  desk's median null has `PF 0.867` — below 1. A percentile here means "lost
  less than a random book at the same cost, count and side ratio", never
  "profitable". No table of mine may be read as "this cell is good".
- The gate is the decision leg. `PF >= 1.200 AND expectancy >= +0.050R`, and
  `>= 30` trades. Under 40 trades nothing is concluded (brief §4: the same rule
  and window gave `PF 1.753 / 96th / 14 trades` and `PF 0.682 / 8th / 178
  trades`; the only difference was sample size).
- A cell is only called "collapsed" when it is `>= 95th` under `coin`, below
  the 95th under `exposure`, **and** has `>= 40` trades **and** its side match
  and count match are inside their bands. A thin or unmatched row is reported
  `null`, not as a collapse.
- Both windows. A statement about the record needs the same direction on A and
  B; a single-window move is reported as a single-window move.
- `null` is not `0` and not `[]`. A row whose null has no spread prints `null`
  in the percentile column and is never printed as 0 or 100.

## What this axis cannot answer, stated before it is run

- It cannot tell whether a mechanism works. It can only tell how much of a
  published percentile was the instrument.
- The direction null is **not** repaired (`2026-10-02-drift-null.md`, "what I
  could NOT fix", item 3) and `--null-sides=` does not touch it, so any
  direction percentile on these rows stays unreadable for a one-sided row. I am
  not opening that.
- `data-sealed/` is not opened, not read, not counted.

---

*Amendments go below this line as dated notes. No line above is ever rewritten.*
